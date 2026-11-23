//! The shared multi-path scheduler. A `Pool` holds one open session per path and runs a queue's requests
//! across them: the policy picks the path, the pool bounds the commands in flight, and a path whose
//! connection drops is taken out of rotation, its commands are re-sent elsewhere, and it reconnects
//! later. A transport plugs in through two traits: `Fabric` opens a path, and `Transport` runs one session.

use std::{
    io,
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

use super::{
    lanes::{Lanes, Policy},
    stats::{PathStats, record_totals, take_dump},
};
use crate::Result;
use crate::io::{AsyncIo, Failure, Next, Request, StreamCtx};

/// How long a path stays down after its connection drops, before the pool connects it again.
const RECONNECT_AFTER: Duration = Duration::from_secs(3);
/// How often a waiting pool checks for a reconnect.
const RECONNECT_POLL: Duration = Duration::from_millis(20);
/// How long a command may wait on a path before the pool takes that path down and re-sends its commands
/// on another path. Applies only while another usable path exists, so a single path is never dropped for
/// being slow. A silently dropped path otherwise holds its commands for the whole session timeout while
/// the shared queue depth fills up behind them.
const FAILOVER_AFTER: Duration = Duration::from_secs(3);
/// Sense bytes kept for a finished command; the same size as the iSCSI session's.
pub const SENSE_CAPACITY: usize = 64;

/// The block geometry shared by every path of one LUN or namespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Geometry {
    pub block_len: u32,
    pub capacity_bytes: u64,
}

/// One request on a path, in blocks of the namespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Io {
    pub write: bool,
    pub lba: u64,
    pub blocks: u32,
}

/// A command that finished on a path. `status` is zero on success. A path that reports it cannot serve
/// I/O sets `unavailable`; the sense bytes, when there are any, say why.
#[derive(Debug, Clone, Copy)]
pub struct Done {
    pub tag: usize,
    pub status: u8,
    /// The transport's full status code, for messages: SCSI leaves it zero, NVMe sets the status field.
    pub code: u16,
    pub unavailable: bool,
    pub sense: [u8; SENSE_CAPACITY],
    pub sense_len: usize,
}

/// One open path: a session that sends reads and writes and reports when they finish.
pub trait Transport {
    /// Whether another command can be sent now.
    fn can_submit(&self) -> bool;
    /// Commands in flight on this path.
    fn inflight(&self) -> usize;
    /// The path's tier from its ALUA state: optimized, non-optimized, or standby (cannot take I/O). None
    /// when the target does not report ALUA.
    fn tier(&self) -> Option<u8> {
        None
    }
    /// Rejects a request this transport cannot express, before anything is sent.
    fn check(&self, io: Io) -> Result<()>;
    /// Sends one request for `tag`, moving its `len` bytes to or from `buf`.
    ///
    /// # Safety
    /// `buf` must stay valid and untouched until `poll` returns `tag`.
    unsafe fn submit(&mut self, tag: usize, io: Io, buf: *mut u8, len: usize) -> Result<()>;
    /// Waits for one command on this path to finish.
    fn poll(&mut self) -> Result<Done>;
    /// `poll`, giving up with an error once `wait` has passed. An error takes the path down.
    fn poll_within(&mut self, wait: Duration) -> Result<Done> {
        let _ = wait;
        self.poll()
    }
    /// Called on every pass of the scheduler, so a path the policy has stopped choosing still does its
    /// housekeeping (keep-alive). An error takes the path down.
    fn tick(&mut self) -> Result<()> {
        Ok(())
    }
    /// Describes a command that finished with an error.
    fn failure(&self, io: Io, done: &Done) -> String;
    /// Describes why a path reported that it cannot serve I/O.
    fn unavailable(&self, done: &Done) -> String;
}

/// How a transport opens a path. `Path` names one target path; `Session` is what it opens.
pub trait Fabric: Send + Sync + 'static {
    type Path: Clone + Send + 'static;
    type Session: Transport + 'static;
    /// The label the path reports under in the totals.
    fn label(&self, path: &Self::Path) -> String;
    /// Connects one path and returns its session and the namespace's geometry.
    fn open(&self, path: &Self::Path) -> Result<(Self::Session, Geometry)>;
}

/// A reconnect's result, moved from its thread. A session is built on that thread and moved back whole with
/// nothing in flight, so no buffer pointer it holds is ever used from two threads at once.
struct SendOpen<S>(Result<(S, Geometry)>);
// SAFETY: see the type's comment: the session's raw buffer pointers are unused until it is owned by the pool.
unsafe impl<S> Send for SendOpen<S> {}

/// The queue's buffers: the base of the arena, the bytes per slot, and the progress counter.
#[derive(Clone, Copy)]
struct Arena<'a> {
    base: *mut u8,
    stride: usize,
    observer: Option<&'a crate::progress::Counter>,
}

/// One session per path, owned by the queue that runs over it.
pub struct Pool<F: Fabric> {
    fabric: Arc<F>,
    /// A reconnect running on its own thread, per path, so a path that does not answer does not hold up
    /// the others. None while the path is up or waiting for its next attempt.
    reconnecting: Vec<Option<mpsc::Receiver<SendOpen<F::Session>>>>,
    paths: Vec<F::Path>,
    /// One session per path. None while the path is down, until it reconnects.
    sessions: Vec<Option<F::Session>>,
    /// When each down path dropped. None while the path is up.
    down_at: Vec<Option<Instant>>,
    /// Per slot: whether a command is in flight on it, so a drop can re-queue it.
    slot_inflight: Vec<bool>,
    /// Commands in flight over time, for the progress line and the end-of-run report.
    stream_stats: crate::io::StreamStats,
    /// Set for write-verify: a read-back may be sent while its write is still in flight.
    race: bool,
    /// Per slot: the request, when it was sent, and the path it went to. Reserved with the queue.
    slot_req: Vec<Request>,
    slot_started: Vec<Instant>,
    slot_path: Vec<usize>,
    slot_dumped: Vec<bool>,
    free: Vec<usize>,
    retry: Vec<usize>,
    in_flight: usize,
    /// The path the last write used. A verify read avoids it.
    last_write_path: Option<usize>,
    labels: Vec<String>,
    stats: Vec<PathStats>,
    /// Paths taken out of rotation, with the reason. Reported when the pool closes.
    failed: Vec<(usize, String)>,
    lanes: Lanes,
    geometry: Geometry,
}

impl<F: Fabric> Pool<F> {
    /// Opens every path and checks that they all name the same namespace.
    pub fn open(fabric: F, paths: &[F::Path], policy: Policy, seed: u64) -> Result<Self> {
        let mut sessions: Vec<Option<F::Session>> = Vec::with_capacity(paths.len());
        let mut geometry: Option<Geometry> = None;
        let mut tiers: Vec<Option<u8>> = Vec::with_capacity(paths.len());
        for path in paths {
            let (session, found) = fabric.open(path)?;
            tiers.push(session.tier());
            match geometry {
                None => geometry = Some(found),
                Some(first) if first != found => {
                    return Err(format!(
                        "paths report block size and capacity {first:?} and {found:?}; they are not one LUN"
                    ));
                }
                Some(_) => {}
            }
            sessions.push(Some(session));
        }
        let geometry = geometry.ok_or("no paths")?;
        let lanes = Lanes::new(sessions.len(), policy, seed)?;
        let stats = tiers
            .iter()
            .map(|&alua| PathStats {
                alua,
                ..PathStats::default()
            })
            .collect();
        for (path, tier) in tiers.into_iter().enumerate() {
            lanes.set_tier(path, tier.unwrap_or(super::lanes::TIER_OPTIMIZED));
        }
        Ok(Self {
            labels: paths.iter().map(|p| fabric.label(p)).collect(),
            reconnecting: (0..paths.len()).map(|_| None).collect(),
            lanes,
            stream_stats: crate::io::StreamStats::new(),
            race: false,
            slot_req: Vec::new(),
            slot_started: Vec::new(),
            slot_path: Vec::new(),
            slot_dumped: Vec::new(),
            free: Vec::new(),
            retry: Vec::new(),
            in_flight: 0,
            paths: paths.to_vec(),
            down_at: vec![None; paths.len()],
            slot_inflight: Vec::new(),
            sessions,
            last_write_path: None,
            stats,
            failed: Vec::with_capacity(paths.len()),
            geometry,
            fabric: Arc::new(fabric),
        })
    }

    pub fn capacity_bytes(&self) -> u64 {
        self.geometry.capacity_bytes
    }

    pub fn block_len(&self) -> u32 {
        self.geometry.block_len
    }

    /// Reserves the per-slot state for a queue of `depth` slots. Called once, before the timed loop.
    pub fn reserve(&mut self, depth: usize) {
        self.slot_req = vec![Request::default(); depth];
        self.slot_started = vec![Instant::now(); depth];
        self.slot_path = vec![0; depth];
        self.slot_dumped = vec![false; depth];
        self.slot_inflight = vec![false; depth];
        self.free = (0..depth).rev().collect();
        self.retry = Vec::with_capacity(depth);
    }

    /// The path for the next request: the policy's choice if it has room, otherwise the next one that
    /// does. A verify read avoids the path that last wrote.
    fn path_with_room(&self, verify_read: bool) -> Option<usize> {
        let first = if verify_read {
            self.lanes
                .pick_available_avoiding(self.last_write_path.unwrap_or(usize::MAX))
        } else {
            self.lanes.pick_available()
        }?;
        (0..self.sessions.len())
            .map(|k| (first + k) % self.sessions.len())
            .find(|&p| {
                self.lanes.is_usable(p) && self.sessions[p].as_ref().is_some_and(|s| s.can_submit())
            })
    }

    /// Takes a path down after its connection dropped. Its commands in flight go back on the retry list,
    /// and the path reconnects after `RECONNECT_AFTER`.
    fn disconnect(&mut self, path: usize) {
        // Taking a path down is failover bookkeeping, not I/O: its allocations are not the hot path.
        crate::allocation::untracked(|| self.take_down(path));
    }

    fn take_down(&mut self, path: usize) {
        self.sessions[path] = None;
        self.down_at[path] = Some(Instant::now());
        self.stats[path].disconnects += 1;
        self.lanes.mark_unavailable(path);
        for slot in 0..self.slot_inflight.len() {
            if self.slot_inflight[slot] && self.slot_path[slot] == path {
                self.slot_inflight[slot] = false;
                self.in_flight -= 1;
                self.stream_stats.change(Instant::now(), false);
                self.retry.push(slot);
            }
        }
    }

    /// How long the pool may wait on path `p` before it must look at the others: the time left before the
    /// oldest command on `p` has waited `FAILOVER_AFTER`. With no other usable path to fail over to, the
    /// transport's own command limit applies.
    fn failover_wait(&self, p: usize) -> Duration {
        if self.lanes.usable_count() < 2 {
            return Duration::MAX;
        }
        let oldest = (0..self.slot_inflight.len())
            .filter(|&s| self.slot_inflight[s] && self.slot_path[s] == p)
            .map(|s| self.slot_started[s])
            .min();
        match oldest {
            Some(started) => FAILOVER_AFTER
                .saturating_sub(started.elapsed())
                .max(Duration::from_millis(1)),
            None => FAILOVER_AFTER,
        }
    }

    /// Whether `req` overlaps a request still in flight, where at least one of the two writes.
    fn overlaps_in_flight(&self, req: Request) -> bool {
        self.slot_inflight
            .iter()
            .enumerate()
            .any(|(slot, &inflight)| {
                if !inflight {
                    return false;
                }
                let other = self.slot_req[slot];
                // Write-verify sends a read-back while its own write is in flight: that race is what it must
                // catch. Only such a read-back is exempt, since a write waits for the read-back of its extent.
                let own_read_back = self.race && req.verify && !req.write && other.write;
                !own_read_back
                    && (other.write || req.write)
                    && other.offset < req.offset + req.len as u64
                    && req.offset < other.offset + other.len as u64
            })
    }

    /// The number of paths requests can be sent to: standby paths do not count.
    pub fn endpoints(&self) -> usize {
        self.lanes.usable_count()
    }

    /// Lets a verify read-back run while its write is in flight (write-verify), or holds them apart.
    pub fn set_race(&mut self, on: bool) {
        self.race = on;
    }

    fn any_down(&self) -> bool {
        self.sessions.iter().any(|s| s.is_none())
    }

    /// Connects again every down path whose wait is over. Each attempt runs on its own thread: a path that
    /// does not answer holds up only its own reconnect, not the commands on the others. A failed attempt
    /// waits another round.
    fn try_reconnect(&mut self) {
        // Starting a reconnect thread allocates; that is failover bookkeeping, not I/O.
        crate::allocation::untracked(|| self.reconnect_due_paths());
    }

    fn reconnect_due_paths(&mut self) {
        for path in 0..self.sessions.len() {
            let got = match &self.reconnecting[path] {
                Some(rx) => rx.try_recv(),
                None => {
                    let Some(since) = self.down_at[path] else {
                        continue;
                    };
                    if since.elapsed() < RECONNECT_AFTER {
                        continue;
                    }
                    let (tx, rx) = mpsc::channel();
                    let fabric = Arc::clone(&self.fabric);
                    let spec = self.paths[path].clone();
                    std::thread::spawn(move || {
                        let _ = tx.send(SendOpen(fabric.open(&spec)));
                    });
                    self.reconnecting[path] = Some(rx);
                    continue;
                }
            };
            match got {
                Ok(SendOpen(Ok((session, _)))) => {
                    self.reconnecting[path] = None;
                    self.lanes
                        .set_tier(path, session.tier().unwrap_or(super::lanes::TIER_OPTIMIZED));
                    self.sessions[path] = Some(session);
                    self.down_at[path] = None;
                    self.stats[path].reconnects += 1;
                    self.lanes.mark_available(path);
                }
                Ok(SendOpen(Err(_))) | Err(mpsc::TryRecvError::Disconnected) => {
                    self.reconnecting[path] = None;
                    self.down_at[path] = Some(Instant::now());
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
    }

    /// Runs requests with up to `depth` in flight across the paths. With `explicit`, each request names its
    /// slot, which is also its buffer (execute). Otherwise slots are handed out from a free list and
    /// `prepare` fills write buffers (stream).
    fn drive<N, P, C>(
        &mut self,
        arena: Arena<'_>,
        explicit: bool,
        mut next: N,
        mut prepare: P,
        mut complete: C,
    ) -> std::result::Result<(), Failure>
    where
        N: FnMut() -> Offer,
        P: FnMut(Request, &mut [u8]),
        C: FnMut(usize, Request),
    {
        let Arena {
            base,
            stride,
            observer,
        } = arena;
        let block = self.geometry.block_len as u64;
        let mut held: Option<(usize, Request)> = None;
        let mut exhausted = false;
        loop {
            // Bring back any path whose wait is over, even while the others are busy.
            self.try_reconnect();
            for path in 0..self.sessions.len() {
                let failed = match self.sessions[path].as_mut() {
                    Some(session) => session.tick().is_err(),
                    None => false,
                };
                if failed {
                    self.disconnect(path);
                }
            }
            // Send as much as the paths will take.
            loop {
                if held.is_none() {
                    if let Some(slot) = self.retry.pop() {
                        held = Some((slot, self.slot_req[slot]));
                    } else if exhausted {
                        break;
                    } else if explicit {
                        match next() {
                            Offer::Request(slot, req) => {
                                self.slot_req[slot] = req;
                                held = Some((slot, req));
                            }
                            Offer::Wait => break,
                            Offer::Done => exhausted = true,
                        }
                    } else {
                        if self.free.is_empty() {
                            break;
                        }
                        match next() {
                            Offer::Request(_, req) => {
                                let slot = self.free.pop().unwrap_or(0);
                                self.slot_req[slot] = req;
                                if req.write {
                                    // SAFETY: slot `slot` is `stride` bytes into the arena and req.len <= stride.
                                    let buf = unsafe {
                                        std::slice::from_raw_parts_mut(
                                            base.add(slot * stride),
                                            req.len,
                                        )
                                    };
                                    prepare(req, buf);
                                }
                                held = Some((slot, req));
                            }
                            Offer::Wait => break,
                            Offer::Done => exhausted = true,
                        }
                    }
                    if held.is_none() {
                        continue;
                    }
                }
                let Some((slot, req)) = held else {
                    continue;
                };
                if req.offset % block != 0 || !(req.len as u64).is_multiple_of(block) {
                    return Err(Failure::from(io::Error::other(format!(
                        "request at {} of {} bytes is not aligned to the {block}-byte block size",
                        req.offset, req.len
                    ))));
                }
                let io = io_of(req, block).map_err(|e| Failure::from(io::Error::other(e)))?;
                // Commands on different queues have no mutual order, so a request that overlaps one still in
                // flight (with either side a write) waits for it to complete.
                if self.overlaps_in_flight(req) {
                    break;
                }
                let verify_read = req.verify && !req.write;
                let Some(path) = self.path_with_room(verify_read) else {
                    if self.any_down() {
                        // Nothing is usable yet: wait for a reconnect, then try again.
                        std::thread::sleep(RECONNECT_POLL);
                        self.try_reconnect();
                        continue;
                    }
                    if self.lanes.pick_available().is_none() {
                        return Err(Failure::from(io::Error::other(
                            "no path can take I/O: each is standby, unavailable or transitioning",
                        )));
                    }
                    break;
                };
                if let Some(Some(session)) = self.sessions.get(path).map(|s| s.as_ref()) {
                    session
                        .check(io)
                        .map_err(|e| Failure::from(io::Error::other(e)))?;
                }
                // SAFETY: the queue's arena holds `depth` slots of `stride` bytes; `slot` is one of them.
                let buf = unsafe { base.add(slot * stride) };
                // SAFETY: the buffer stays valid and untouched until `poll` returns this slot's tag.
                let sent = match self.sessions[path].as_mut() {
                    Some(session) => unsafe { session.submit(slot, io, buf, req.len) },
                    None => Err("path has no session".into()),
                };
                if sent.is_err() {
                    // The connection dropped while sending: take the path down and send elsewhere.
                    self.disconnect(path);
                    continue;
                }
                self.slot_inflight[slot] = true;
                let depth = self.sessions[path].as_ref().map_or(0, |s| s.inflight()) as u64;
                self.stats[path].inflight_max = self.stats[path].inflight_max.max(depth);
                self.slot_dumped[slot] = take_dump();
                if self.slot_dumped[slot] {
                    eprintln!(
                        "dump: send path {path} {} lba {} blocks {} offset {} verify {}",
                        if req.write { "write" } else { "read" },
                        io.lba,
                        io.blocks,
                        req.offset,
                        req.verify
                    );
                }
                let now = Instant::now();
                self.slot_started[slot] = now;
                self.slot_path[slot] = path;
                self.in_flight += 1;
                self.stream_stats.change(now, true);
                if let Some(counter) = observer {
                    counter.submitted(1);
                }
                held = None;
            }
            if self.in_flight == 0 {
                if exhausted && self.retry.is_empty() {
                    return Ok(());
                }
                if held.is_some() {
                    return Err(Failure::from(io::Error::other(
                        "no path has room for a request",
                    )));
                }
                continue;
            }
            // Wait for the first finished command on any session with work in flight.
            let Some(p) = (0..self.sessions.len())
                .find(|&i| self.sessions[i].as_ref().is_some_and(|s| s.inflight() > 0))
            else {
                return Err(Failure::from(io::Error::other(
                    "requests in flight on no path",
                )));
            };
            let wait = self.failover_wait(p);
            let mark = crate::allocation::count_now();
            let polled = match self.sessions[p].as_mut() {
                Some(session) => session.poll_within(wait),
                None => Err("path has no session".into()),
            };
            if polled.is_err() {
                // The error text of a path that failed is failover bookkeeping, not I/O.
                crate::allocation::forgive_since(mark);
            }
            let done = match polled {
                Ok(done) => done,
                Err(_) => {
                    // The connection dropped while waiting: take the path down; its commands are re-sent.
                    self.disconnect(p);
                    continue;
                }
            };
            self.slot_inflight[done.tag] = false;
            self.in_flight -= 1;
            self.stream_stats.change(Instant::now(), false);
            if let Some(counter) = observer {
                counter.completed(1);
            }
            let slot = done.tag;
            let req = self.slot_req[slot];
            if self.slot_dumped[slot] {
                eprintln!(
                    "dump: done path {p} {} status {:#04x}",
                    if req.write { "write" } else { "read" },
                    done.status
                );
            }
            if done.status == 0 {
                let nanos = self.slot_started[slot].elapsed().as_nanos() as u64;
                self.lanes.record(p, nanos);
                self.stats[p].record(req.write, req.len as u64, nanos);
                if req.write {
                    self.last_write_path = Some(p);
                }
                complete(slot, req);
                if !explicit {
                    self.free.push(slot);
                }
            } else if done.unavailable {
                let reason = crate::allocation::untracked(|| {
                    self.sessions[p]
                        .as_ref()
                        .map_or_else(String::new, |s| s.unavailable(&done))
                });
                self.failed.push((p, reason));
                self.stats[p].failovers += 1;
                self.lanes.mark_unavailable(p);
                self.retry.push(slot);
            } else {
                let io = io_of(req, block).unwrap_or(Io {
                    write: req.write,
                    lba: 0,
                    blocks: 0,
                });
                let message = self.sessions[p].as_ref().map_or_else(
                    || format!("status {:#04x}", done.status),
                    |s| s.failure(io, &done),
                );
                return Err(Failure::from(io::Error::other(message)));
            }
        }
    }
}

/// What the drive loop's source offers: a request for a slot, a wait for a completion, or the end.
enum Offer {
    Request(usize, Request),
    Wait,
    Done,
}

/// The request in blocks of the namespace.
fn io_of(req: Request, block: u64) -> Result<Io> {
    let blocks = u32::try_from(req.len as u64 / block)
        .map_err(|_| "transfer beyond the 32-bit block count of this transport".to_string())?;
    Ok(Io {
        write: req.write,
        lba: req.offset / block,
        blocks,
    })
}

impl<F: Fabric> Drop for Pool<F> {
    fn drop(&mut self) {
        for (path, reason) in &self.failed {
            eprintln!(
                "*info* path {path} cannot serve I/O ({reason}); requests moved to the other paths"
            );
        }
        for (label, stats) in self.labels.iter().zip(&self.stats) {
            // A path that carried nothing is still reported when its ALUA state keeps it from I/O.
            if !stats.is_idle()
                || stats
                    .alua
                    .is_some_and(|t| t != super::lanes::TIER_OPTIMIZED)
            {
                record_totals(label, stats);
            }
        }
    }
}

impl<F: Fabric> AsyncIo for Pool<F> {
    fn take_stats(&mut self) -> Option<crate::io::StreamStats> {
        Some(self.stream_stats.take())
    }

    fn stream<N, P, C>(
        &mut self,
        ctx: StreamCtx<'_>,
        mut next: N,
        prepare: P,
        complete: C,
    ) -> std::result::Result<(), Failure>
    where
        N: FnMut() -> Next,
        P: FnMut(Request, &mut [u8]),
        C: FnMut(usize, Request),
    {
        self.drive(
            Arena {
                base: ctx.ptr,
                stride: ctx.stride,
                observer: ctx.observer,
            },
            false,
            || match next() {
                Next::Request(r) => Offer::Request(0, r),
                Next::Wait => Offer::Wait,
                Next::Done => Offer::Done,
            },
            prepare,
            complete,
        )
    }

    fn execute<C>(&mut self, ctx: StreamCtx<'_>, complete: C) -> std::result::Result<(), Failure>
    where
        C: FnMut(usize, Request),
    {
        let count = ctx.requests.len();
        let mut i = 0usize;
        let requests = ctx.requests;
        self.drive(
            Arena {
                base: ctx.ptr,
                stride: ctx.stride,
                observer: ctx.observer,
            },
            true,
            || {
                if i < count {
                    i += 1;
                    Offer::Request(i - 1, requests[i - 1])
                } else {
                    Offer::Done
                }
            },
            |_, _| {},
            complete,
        )
    }
}
