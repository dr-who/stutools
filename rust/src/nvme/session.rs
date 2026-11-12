//! NVMe-oF as a path transport for the shared pool. A path is one controller: an admin queue for Connect,
//! Identify and Keep Alive, and one I/O queue, each on its own TCP connection. Reads and writes pipeline
//! on the I/O queue: the pool's slot number is the command ID, and completions come back in any order.

use std::{
    net::SocketAddr,
    time::{Duration, Instant},
};

use super::{
    controller::{connect_io_queue, Association, Controller, CAP, KATO_MS},
    identify::{
        parse_active_namespaces, parse_controller, parse_namespace, CNS_ACTIVE_NAMESPACES,
        CNS_CONTROLLER, CNS_NAMESPACE,
    },
    link::Link,
    pdu::{self, Pdu},
    sqe::{self, Command, Completion},
    Result,
};
use crate::paths::{Done, Fabric, Geometry, Io, Transport};

/// The I/O queue's size, 0-based, when the controller allows more: 128 entries.
const IO_SQSIZE: u16 = 127;
/// Command slots reserved up front; a workload deeper than this grows the table once.
const PENDING_SLOTS: usize = 1024;
/// NVMe's largest transfer in one read or write: the NLB field is 16 bits, 0-based.
const MAX_BLOCKS: u32 = 65536;
/// Status code type 3: path-related status. The controller reports ANA states this way.
const SCT_PATH: u16 = 0x3;

/// One NVMe-oF path: a controller reached at `addr`, for subsystem `subnqn`. `nsid` names the namespace;
/// None takes the subsystem's first active namespace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NvmePath {
    pub addr: SocketAddr,
    pub subnqn: String,
    pub nsid: Option<u32>,
}

/// How the pool opens an NVMe-oF path: the host NQN, and the host identifier sent with each Connect.
pub struct NvmeFabric {
    hostnqn: String,
    hostid: [u8; 16],
}

impl NvmeFabric {
    pub fn new(hostnqn: &str, hostid: [u8; 16]) -> Self {
        Self {
            hostnqn: hostnqn.to_string(),
            hostid,
        }
    }
}

impl Fabric for NvmeFabric {
    type Path = NvmePath;
    type Session = NvmeSession;

    fn label(&self, path: &NvmePath) -> String {
        match path.nsid {
            Some(nsid) => format!("{} {} ns{nsid}", path.addr, path.subnqn),
            None => format!("{} {}", path.addr, path.subnqn),
        }
    }

    fn open(&self, path: &NvmePath) -> Result<(NvmeSession, Geometry)> {
        let assoc = Association {
            addr: path.addr,
            subnqn: &path.subnqn,
            hostnqn: &self.hostnqn,
            hostid: self.hostid,
        };
        let mut admin = Controller::connect(&assoc)?;
        admin.enable()?;
        let cap = admin.property_get(CAP, true)?;
        // MQES is 0-based in CAP bits 0-15; the I/O queue is no larger than the controller allows.
        let sqsize = IO_SQSIZE.min((cap & 0xffff) as u16);
        let nsid = match path.nsid {
            Some(nsid) => nsid,
            None => *parse_active_namespaces(&admin.identify(CNS_ACTIVE_NAMESPACES, 0)?)
                .first()
                .ok_or("NVMe/TCP: the subsystem has no active namespace")?,
        };
        let ns = parse_namespace(&admin.identify(CNS_NAMESPACE, nsid)?);
        let inline_limit = parse_controller(&admin.identify(CNS_CONTROLLER, 0)?).inline_bytes;
        let io = connect_io_queue(&assoc, admin.cntlid(), 1, sqsize)?;
        let geometry = Geometry {
            block_len: ns.block_len,
            capacity_bytes: ns.blocks * ns.block_len as u64,
        };
        Ok((
            NvmeSession {
                admin,
                io,
                nsid,
                rx: Pdu::with_capacity(pdu::DATA_HEADER_LEN, super::link::FRAME_CAPACITY),
                pending: vec![None; PENDING_SLOTS],
                inflight: 0,
                limit: sqsize as usize + 1,
                inline_limit,
                last_keep_alive: Instant::now(),
            },
            geometry,
        ))
    }
}

/// Where one command's data is: the slot buffer, how long it is, and whether it is a write.
#[derive(Clone, Copy)]
struct Pending {
    buf: *mut u8,
    len: usize,
    write: bool,
}

/// One path's session: the admin queue and the I/O queue, and the commands in flight on the I/O queue.
pub struct NvmeSession {
    admin: Controller,
    io: Link,
    nsid: u32,
    /// The PDU the poll reads into; kept here so its buffers are reused.
    rx: Pdu,
    /// Per command ID (the pool's slot), the buffer and direction while the command is in flight.
    pending: Vec<Option<Pending>>,
    inflight: usize,
    /// The most commands the I/O queue takes at once.
    limit: usize,
    /// Writes up to this many bytes travel in the capsule, as the controller requires.
    inline_limit: usize,
    last_keep_alive: Instant,
}

impl NvmeSession {
    /// Sends Keep Alive on the admin queue when half the keep-alive timeout has passed.
    fn keep_alive_if_due(&mut self) -> Result<()> {
        if self.last_keep_alive.elapsed() >= Duration::from_millis(KATO_MS as u64 / 2) {
            self.admin.keep_alive()?;
            self.last_keep_alive = Instant::now();
        }
        Ok(())
    }

    /// Records a finished command: frees its slot and builds the pool's completion.
    fn finish(&mut self, cid: u16, cqe: Completion) -> Done {
        if let Some(slot) = self.pending.get_mut(cid as usize) {
            *slot = None;
        }
        self.inflight -= 1;
        let code = cqe.status >> 1;
        Done {
            tag: cid as usize,
            status: if cqe.ok() { 0 } else { 1 },
            code,
            unavailable: (code >> 8) & 0x7 == SCT_PATH,
            sense: [0; crate::paths::SENSE_CAPACITY],
            sense_len: 0,
        }
    }
}

impl NvmeSession {
    /// Handles one PDU from the I/O queue: returns the finished command when this PDU finishes one.
    fn handle_pdu(&mut self, p: &Pdu) -> Result<Option<Done>> {
        match p.kind {
            pdu::C2H_DATA => {
                let c2h = pdu::parse_c2h(p)?;
                let cid = c2h.cccid;
                let pending = self
                    .pending
                    .get(cid as usize)
                    .copied()
                    .flatten()
                    .ok_or("NVMe/TCP: data for a command that is not in flight")?;
                let start = c2h.datao as usize;
                let end = start + c2h.datal as usize;
                if pending.write || end > pending.len {
                    return Err("NVMe/TCP: data outside a read's buffer".into());
                }
                // SAFETY: the pool keeps the slot's buffer valid until this command completes, and
                // `end` is within the buffer checked above.
                let dst =
                    unsafe { std::slice::from_raw_parts_mut(pending.buf.add(start), end - start) };
                dst.copy_from_slice(&p.data);
                if c2h.success {
                    return Ok(Some(self.finish(
                        cid,
                        Completion {
                            dw0: 0,
                            wide: 0,
                            sq_head: 0,
                            cid,
                            status: 0,
                        },
                    )));
                }
                Ok(None)
            }
            pdu::R2T => {
                let r2t = pdu::parse_r2t(p)?;
                let pending = self
                    .pending
                    .get(r2t.cccid as usize)
                    .copied()
                    .flatten()
                    .ok_or("NVMe/TCP: R2T for a command that is not in flight")?;
                if !pending.write {
                    return Err("NVMe/TCP: R2T for a read".into());
                }
                // SAFETY: as above; the slice covers the command's `len` bytes of its slot.
                let data = unsafe { std::slice::from_raw_parts(pending.buf, pending.len) };
                self.io.send_h2c(r2t.cccid, r2t, data)?;
                Ok(None)
            }
            pdu::CAPSULE_RESP => {
                let cqe = Completion::parse(&pdu::parse_capsule_resp(p)?);
                Ok(Some(self.finish(cqe.cid, cqe)))
            }
            pdu::TERM_C2H | pdu::TERM_H2C => Err(pdu::describe_term(p)),
            other => Err(format!(
                "NVMe/TCP: unexpected PDU type {other:#x} on the I/O queue"
            )),
        }
    }
}

impl Transport for NvmeSession {
    fn can_submit(&self) -> bool {
        self.inflight < self.limit
    }

    fn inflight(&self) -> usize {
        self.inflight
    }

    fn check(&self, io: Io) -> Result<()> {
        if io.blocks == 0 || io.blocks > MAX_BLOCKS {
            return Err(format!(
                "NVMe transfers are 1 to {MAX_BLOCKS} blocks; this one is {}",
                io.blocks
            ));
        }
        Ok(())
    }

    unsafe fn submit(&mut self, tag: usize, io: Io, buf: *mut u8, len: usize) -> Result<()> {
        self.keep_alive_if_due()?;
        let cid = u16::try_from(tag).map_err(|_| "NVMe command ID beyond 16 bits".to_string())?;
        let mut cmd = Command::new(
            if io.write {
                sqe::OP_WRITE
            } else {
                sqe::OP_READ
            },
            cid,
        );
        cmd.set_nsid(self.nsid);
        cmd.set_u64(40, io.lba);
        cmd.set_u16(48, (io.blocks - 1) as u16); // NLB, 0-based
        if io.write && len <= self.inline_limit {
            // Small writes travel in the capsule itself, so the controller needs no R2T.
            cmd.set_data(len, true);
            // SAFETY: the pool keeps `buf` valid until this command completes.
            let data = unsafe { std::slice::from_raw_parts(buf, len) };
            self.io.send_capsule(&cmd.0, data)?;
        } else {
            cmd.set_data(len, false);
            self.io.send_capsule(&cmd.0, &[])?;
        }
        if self.pending.len() <= tag {
            self.pending.resize(tag + 1, None);
        }
        self.pending[tag] = Some(Pending {
            buf,
            len,
            write: io.write,
        });
        self.inflight += 1;
        Ok(())
    }

    fn poll(&mut self) -> Result<Done> {
        loop {
            self.keep_alive_if_due()?;
            let mut p = std::mem::replace(&mut self.rx, Pdu::empty());
            self.io.read_into(&mut p)?;
            let result = self.handle_pdu(&p);
            self.rx = p;
            if let Some(done) = result? {
                return Ok(done);
            }
        }
    }

    fn tick(&mut self) -> Result<()> {
        self.keep_alive_if_due()
    }

    fn failure(&self, io: Io, done: &Done) -> String {
        let op = if io.write { "write" } else { "read" };
        format!(
            "NVMe {op} failed: status code type {:#x}, status code {:#x}",
            (done.code >> 8) & 0x7,
            done.code & 0xff
        )
    }

    fn unavailable(&self, done: &Done) -> String {
        format!("NVMe path status {:#x}", done.code & 0xff)
    }
}
