//! A connection for a workload over iSCSI: one session per target path, and the `AsyncIo` trait that
//! the queue runs. Each request becomes one READ(10) or WRITE(10) on the path the policy picks, and
//! the data moves between the session and the queue's buffer.

use super::lanes::{Lanes, Policy};
use super::report::{login_with_fallback, Path};
use super::scsi;
use super::session::Session;
use crate::io::{AsyncIo, Failure, Request, StreamCtx};
use crate::Result;
use std::{io, time::Instant};

/// One session per path, shared by the queue that owns this connection.
/// Counts for one path: operations, bytes, latency and failovers. Plain integers, updated per request.
#[derive(Debug, Default, Clone, Copy)]
pub struct PathStats {
    pub reads: u64,
    pub writes: u64,
    pub bytes: u64,
    pub nanos_total: u64,
    pub nanos_max: u64,
    pub failovers: u64,
    pub disconnects: u64,
    pub reconnects: u64,
}

impl PathStats {
    fn record(&mut self, write: bool, bytes: u64, nanos: u64) {
        if write {
            self.writes += 1;
        } else {
            self.reads += 1;
        }
        self.bytes += bytes;
        self.nanos_total += nanos;
        self.nanos_max = self.nanos_max.max(nanos);
    }

    fn merge(&mut self, other: &PathStats) {
        self.reads += other.reads;
        self.writes += other.writes;
        self.bytes += other.bytes;
        self.nanos_total += other.nanos_total;
        self.nanos_max = self.nanos_max.max(other.nanos_max);
        self.failovers += other.failovers;
        self.disconnects += other.disconnects;
        self.reconnects += other.reconnects;
    }
}

/// How long a path stays down after its connection drops, before the connection logs in again.
const RECONNECT_AFTER: std::time::Duration = std::time::Duration::from_secs(3);
/// How often a waiting connection checks for a reconnect.
const RECONNECT_POLL: std::time::Duration = std::time::Duration::from_millis(20);

/// Commands still to dump (`-d`). Shared by every connection in the run.
static DUMP_LEFT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Sets how many commands to dump (`-d N`).
pub fn set_dump(count: u64) {
    DUMP_LEFT.store(count, std::sync::atomic::Ordering::Relaxed);
}

/// Takes one dump slot if any remain.
fn take_dump() -> bool {
    DUMP_LEFT
        .fetch_update(
            std::sync::atomic::Ordering::Relaxed,
            std::sync::atomic::Ordering::Relaxed,
            |n| n.checked_sub(1),
        )
        .is_ok()
}

/// The queue's buffers: the base of the arena, the bytes per slot, and the progress counter.
#[derive(Clone, Copy)]
struct Arena<'a> {
    base: *mut u8,
    stride: usize,
    observer: Option<&'a crate::progress::Counter>,
}

/// Totals for every connection in the run, keyed by path label. Each connection adds its counts when it
/// closes, which is outside the timed loop.
static TOTALS: std::sync::Mutex<Vec<(String, PathStats)>> = std::sync::Mutex::new(Vec::new());

pub struct Connection {
    /// One session per path. None while the path is down, until it reconnects.
    sessions: Vec<Option<Session>>,
    /// What each path needs to log in again.
    paths_info: Vec<Path>,
    initiator: String,
    chap: Option<(String, Vec<u8>)>,
    /// When each down path dropped. None while the path is up.
    down_at: Vec<Option<Instant>>,
    /// Per slot: whether a command is in flight on it, so a drop can re-queue it.
    slot_inflight: Vec<bool>,
    /// Commands in flight over time, for the progress line and the end-of-run report.
    stream_stats: crate::io::StreamStats,
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
    /// Paths taken out of rotation, with their ASC and ASCQ. Reported when the connection closes, so the
    /// message is built outside the timed loop.
    failed: Vec<(usize, u8, u8)>,
    lanes: Lanes,
    block_len: u32,
    capacity_bytes: u64,
}

impl Connection {
    /// Logs in to every path and reads the LUN's size, which must agree across paths. `chap` is used
    /// only where a target asks for it.
    pub fn open(
        paths: &[Path],
        initiator: &str,
        chap: Option<(&str, &[u8])>,
        policy: Policy,
        seed: u64,
    ) -> Result<Self> {
        let mut sessions: Vec<Option<Session>> = Vec::with_capacity(paths.len());
        let mut layout: Option<(u32, u64)> = None;
        for path in paths {
            let mut session = login_with_fallback(path, initiator, chap, "CRC32C,None")?;
            let (last, len) = scsi::capacity(&mut session)?;
            let found = (len, (last + 1) * len as u64);
            match layout {
                None => layout = Some(found),
                Some(first) if first != found => {
                    return Err(format!(
                        "paths report block size and capacity {first:?} and {found:?}; they are not one LUN"
                    ));
                }
                Some(_) => {}
            }
            sessions.push(Some(session));
        }
        let (block_len, capacity_bytes) = layout.ok_or("no iSCSI paths")?;
        Ok(Self {
            stream_stats: crate::io::StreamStats::new(),
            slot_req: Vec::new(),
            slot_started: Vec::new(),
            slot_path: Vec::new(),
            slot_dumped: Vec::new(),
            free: Vec::new(),
            retry: Vec::new(),
            in_flight: 0,
            lanes: Lanes::new(sessions.len(), policy, seed)?,
            paths_info: paths.to_vec(),
            initiator: initiator.to_string(),
            chap: chap.map(|(u, s)| (u.to_string(), s.to_vec())),
            down_at: vec![None; paths.len()],
            slot_inflight: Vec::new(),
            sessions,
            last_write_path: None,
            labels: paths
                .iter()
                .map(|p| format!("{} {}", p.portal, p.target))
                .collect(),
            stats: vec![PathStats::default(); paths.len()],
            failed: Vec::with_capacity(paths.len()),
            block_len,
            capacity_bytes,
        })
    }

    pub fn capacity_bytes(&self) -> u64 {
        self.capacity_bytes
    }

    pub fn block_len(&self) -> u32 {
        self.block_len
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
                !self.lanes.is_unavailable(p)
                    && self.sessions[p].as_ref().is_some_and(|s| s.can_submit())
            })
    }

    /// Takes a path down after its connection dropped. Its commands in flight go back on the retry list,
    /// and the path reconnects after `RECONNECT_AFTER`.
    fn disconnect(&mut self, path: usize) {
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

    fn any_down(&self) -> bool {
        self.sessions.iter().any(|s| s.is_none())
    }

    /// Logs in again to every down path whose wait is over. A failed attempt waits another round.
    fn try_reconnect(&mut self) {
        for path in 0..self.sessions.len() {
            let Some(since) = self.down_at[path] else {
                continue;
            };
            if since.elapsed() < RECONNECT_AFTER {
                continue;
            }
            let chap = self.chap.as_ref().map(|(u, s)| (u.as_str(), s.as_slice()));
            match login_with_fallback(&self.paths_info[path], &self.initiator, chap, "CRC32C,None")
            {
                Ok(session) => {
                    self.sessions[path] = Some(session);
                    self.down_at[path] = None;
                    self.stats[path].reconnects += 1;
                    self.lanes.mark_available(path);
                }
                Err(_) => self.down_at[path] = Some(Instant::now()),
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
        N: FnMut() -> Option<(usize, Request)>,
        P: FnMut(Request, &mut [u8]),
        C: FnMut(usize, Request),
    {
        let Arena {
            base,
            stride,
            observer,
        } = arena;
        let block = self.block_len as u64;
        let mut held: Option<(usize, Request)> = None;
        let mut exhausted = false;
        loop {
            // Bring back any path whose wait is over, even while the others are busy.
            self.try_reconnect();
            // Send as much as the paths will take.
            loop {
                if held.is_none() {
                    if let Some(slot) = self.retry.pop() {
                        held = Some((slot, self.slot_req[slot]));
                    } else if exhausted {
                        break;
                    } else if explicit {
                        match next() {
                            Some((slot, req)) => {
                                self.slot_req[slot] = req;
                                held = Some((slot, req));
                            }
                            None => exhausted = true,
                        }
                    } else {
                        if self.free.is_empty() {
                            break;
                        }
                        match next() {
                            Some((_, req)) => {
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
                            None => exhausted = true,
                        }
                    }
                    if held.is_none() {
                        continue;
                    }
                }
                let Some((slot, req)) = held else {
                    continue;
                };
                if req.offset % block != 0 || req.len as u64 % block != 0 {
                    return Err(Failure::from(io::Error::other(format!(
                        "request at {} of {} bytes is not aligned to the {block}-byte block size",
                        req.offset, req.len
                    ))));
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
                            "every iSCSI path is unavailable",
                        )));
                    }
                    break;
                };
                let lba = u32::try_from(req.offset / block)
                    .map_err(|_| Failure::from(io::Error::other("LBA beyond READ(10) range")))?;
                let blocks = u16::try_from(req.len as u64 / block).map_err(|_| {
                    Failure::from(io::Error::other("transfer beyond READ(10) range"))
                })?;
                let cdb = scsi::block_cdb(if req.write { 0x2a } else { 0x28 }, lba, blocks);
                // SAFETY: the queue's arena holds `depth` slots of `stride` bytes; `slot` is one of them.
                let buf = unsafe { base.add(slot * stride) };
                // SAFETY: the buffer stays valid and untouched until `poll` returns this slot's tag.
                let sent = match self.sessions[path].as_mut() {
                    // SAFETY: as above; the buffer stays valid until this slot's tag comes back.
                    Some(session) => unsafe { session.submit(&cdb, slot, buf, req.len, req.write) },
                    None => Err("path has no session".into()),
                };
                if sent.is_err() {
                    // The connection dropped while sending: take the path down and send elsewhere.
                    self.disconnect(path);
                    continue;
                }
                self.slot_inflight[slot] = true;
                self.slot_dumped[slot] = take_dump();
                if self.slot_dumped[slot] {
                    eprintln!(
                        "dump: send path {path} {} lba {lba} blocks {blocks} offset {} verify {}",
                        if req.write { "WRITE(10)" } else { "READ(10)" },
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
                        "no iSCSI path has room for a request",
                    )));
                }
                continue;
            }
            // Wait for the first finished command on any session with work in flight.
            let Some(p) = (0..self.sessions.len())
                .find(|&i| self.sessions[i].as_ref().is_some_and(|s| s.inflight() > 0))
            else {
                return Err(Failure::from(io::Error::other(
                    "requests in flight on no session",
                )));
            };
            let polled = match self.sessions[p].as_mut() {
                Some(session) => session.poll(),
                None => Err("path has no session".into()),
            };
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
                    if req.write { "WRITE(10)" } else { "READ(10)" },
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
                self.failed.push((p, done.sense[12], done.sense[13]));
                self.stats[p].failovers += 1;
                self.lanes.mark_unavailable(p);
                self.retry.push(slot);
            } else {
                let op = if req.write { "WRITE(10)" } else { "READ(10)" };
                return Err(Failure::from(io::Error::other(
                    scsi::check_status(op, done.status, &done.sense[..done.sense_len]).unwrap_err(),
                )));
            }
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        for (path, asc, ascq) in &self.failed {
            eprintln!("*info* path {path} reported the LUN unavailable (ASC {asc:02x}/{ascq:02x}); requests moved to the other paths");
        }
        let mut totals = TOTALS.lock().unwrap_or_else(|e| e.into_inner());
        for (label, stats) in self.labels.iter().zip(&self.stats) {
            if stats.reads + stats.writes + stats.failovers + stats.disconnects == 0 {
                continue;
            }
            match totals.iter_mut().find(|(l, _)| l == label) {
                Some((_, total)) => total.merge(stats),
                None => totals.push((label.clone(), *stats)),
            }
        }
    }
}

/// Prints the per-path totals for the run: operations, bytes, latency and failovers. Prints nothing when
/// no iSCSI path carried I/O.
pub fn print_totals() {
    let totals = TOTALS.lock().unwrap_or_else(|e| e.into_inner());
    if totals.is_empty() {
        return;
    }
    println!("per-path totals:");
    println!(
        "  {:<44}{:>10}{:>10}{:>10}{:>12}{:>12}{:>12}{:>11}{:>8}{:>12}",
        "path",
        "ops",
        "reads",
        "writes",
        "MiB",
        "avg ms",
        "max ms",
        "failovers",
        "drops",
        "reconnects"
    );
    for (label, t) in totals.iter() {
        let ops = t.reads + t.writes;
        let avg = if ops == 0 {
            0.0
        } else {
            t.nanos_total as f64 / ops as f64 / 1e6
        };
        println!(
            "  {:<44}{:>10}{:>10}{:>10}{:>12.1}{:>12.3}{:>12.3}{:>11}{:>8}{:>12}",
            label,
            ops,
            t.reads,
            t.writes,
            t.bytes as f64 / 1_048_576.0,
            avg,
            t.nanos_max as f64 / 1e6,
            t.failovers,
            t.disconnects,
            t.reconnects
        );
    }
}

impl AsyncIo for Connection {
    fn take_stats(&mut self) -> Option<crate::io::StreamStats> {
        Some(self.stream_stats.take())
    }

    fn stream<N, P, C>(
        &mut self,
        ctx: StreamCtx<'_>,
        mut next: N,
        prepare: P,
        mut complete: C,
    ) -> std::result::Result<(), Failure>
    where
        N: FnMut() -> Option<Request>,
        P: FnMut(Request, &mut [u8]),
        C: FnMut(Request),
    {
        self.drive(
            Arena {
                base: ctx.ptr,
                stride: ctx.stride,
                observer: ctx.observer,
            },
            false,
            || next().map(|r| (0, r)),
            prepare,
            |_, r| complete(r),
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
                    Some((i - 1, requests[i - 1]))
                } else {
                    None
                }
            },
            |_, _| {},
            complete,
        )
    }
}
