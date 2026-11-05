//! The NVMe/TCP admin queue of one controller: Fabrics Connect, controller enable, Identify and Keep
//! Alive, and the Connect of each I/O queue. Commands here run one at a time.

use std::{
    io::Read,
    net::SocketAddr,
    time::{Duration, Instant},
};

use super::{
    link::{Data, Link},
    sqe::{self, Command, Completion},
    Result,
};

const TIMEOUT: Duration = Duration::from_secs(30);
/// Admin queue size, 0-based: 32 entries.
const ADMIN_SQSIZE: u16 = 31;
/// Keep-alive timeout in milliseconds that Connect asks for. The admin queue sends Keep Alive at half this.
pub const KATO_MS: u32 = 10_000;

/// Property offsets (NVMe base specification, controller registers).
pub const CAP: u32 = 0x00;
pub const CC: u32 = 0x14;
pub const CSTS: u32 = 0x1c;

/// The discovery subsystem's NQN, which a discovery controller serves.
pub const DISCOVERY_NQN: &str = "nqn.2014-08.org.nvmexpress.discovery";
/// Log page ID of the discovery log.
pub const LOG_DISCOVERY: u8 = 0x70;

/// The connection parameters for one association.
pub struct Association<'a> {
    pub addr: SocketAddr,
    pub subnqn: &'a str,
    pub hostnqn: &'a str,
    pub hostid: [u8; 16],
}

/// The admin queue of one association, after Connect.
pub struct Controller {
    link: Link,
    cntlid: u16,
    next_cid: u16,
}

impl Controller {
    /// Opens the admin queue and connects it: the controller ID comes back from Connect.
    pub fn connect(a: &Association) -> Result<Self> {
        let mut link = Link::open(a.addr)?;
        let cntlid = fabrics_connect(&mut link, a, 0xffff, 0, ADMIN_SQSIZE)?;
        Ok(Self {
            link,
            cntlid,
            next_cid: 1,
        })
    }

    /// The controller ID that Connect assigned.
    pub fn cntlid(&self) -> u16 {
        self.cntlid
    }

    /// Reads a controller property. `wide` reads the 8-byte form.
    pub fn property_get(&mut self, offset: u32, wide: bool) -> Result<u64> {
        let mut cmd = Command::new(sqe::OP_FABRICS, 0);
        cmd.set_u8(4, sqe::FCTYPE_PROPERTY_GET);
        cmd.set_u8(40, wide as u8);
        cmd.set_u32(44, offset);
        let done = self.submit(cmd, Data::None)?;
        check(&done, "Property Get")?;
        Ok(if wide { done.wide } else { done.dw0 as u64 })
    }

    /// Writes a 4-byte controller property.
    pub fn property_set(&mut self, offset: u32, value: u32) -> Result<()> {
        let mut cmd = Command::new(sqe::OP_FABRICS, 0);
        cmd.set_u8(4, sqe::FCTYPE_PROPERTY_SET);
        cmd.set_u8(40, 0);
        cmd.set_u32(44, offset);
        cmd.set_u64(48, value as u64);
        let done = self.submit(cmd, Data::None)?;
        check(&done, "Property Set")
    }

    /// Sets CC.EN with 64-byte submission entries and 16-byte completion entries, then waits for
    /// CSTS.RDY.
    pub fn enable(&mut self) -> Result<()> {
        const IOSQES: u32 = 6; // 2^6 = 64 bytes
        const IOCQES: u32 = 4; // 2^4 = 16 bytes
        self.property_set(CC, 1 | (IOSQES << 16) | (IOCQES << 20))?;
        let start = Instant::now();
        loop {
            let csts = self.property_get(CSTS, false)?;
            if csts & 0x2 != 0 {
                return Err("NVMe/TCP: controller reports a fatal status (CSTS.CFS)".into());
            }
            if csts & 0x1 != 0 {
                return Ok(());
            }
            if start.elapsed() > TIMEOUT {
                return Err("NVMe/TCP: controller did not become ready (CSTS.RDY)".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Runs Identify with the given CNS and NSID, returning its 4096-byte response.
    pub fn identify(&mut self, cns: u32, nsid: u32) -> Result<Vec<u8>> {
        let mut cmd = Command::new(sqe::OP_IDENTIFY, 0);
        cmd.set_nsid(nsid);
        cmd.set_u32(40, cns);
        let mut buf = vec![0u8; 4096];
        let done = self.submit(cmd, Data::In(&mut buf))?;
        check(&done, "Identify")?;
        Ok(buf)
    }

    /// Runs Identify with the given CNS and NSID into `buf` (4096 bytes), returning the completion
    /// rather than failing on an error status.
    pub fn identify_status(&mut self, cns: u32, nsid: u32, buf: &mut [u8]) -> Result<Completion> {
        let mut cmd = Command::new(sqe::OP_IDENTIFY, 0);
        cmd.set_nsid(nsid);
        cmd.set_u32(40, cns);
        self.submit(cmd, Data::In(buf))
    }

    /// Reads `len` bytes of a log page from `offset`. The length is in whole dwords.
    pub fn get_log_page(&mut self, lid: u8, offset: u64, len: usize) -> Result<Vec<u8>> {
        let mut cmd = Command::new(sqe::OP_GET_LOG_PAGE, 0);
        cmd.set_u32(40, ((len / 4 - 1) as u32) << 16 | lid as u32);
        cmd.set_u32(48, offset as u32);
        cmd.set_u32(52, (offset >> 32) as u32);
        let mut buf = vec![0u8; len];
        let done = self.submit(cmd, Data::In(&mut buf))?;
        check(&done, "Get Log Page")?;
        Ok(buf)
    }

    /// Sends Keep Alive so the controller does not drop an idle association.
    pub fn keep_alive(&mut self) -> Result<()> {
        let done = self.submit(Command::new(sqe::OP_KEEP_ALIVE, 0), Data::None)?;
        check(&done, "Keep Alive")
    }

    /// Get Features for `fid` with `cdw11`, returning the completion; `buf` receives any data.
    pub fn get_features(
        &mut self,
        fid: u8,
        cdw11: u32,
        buf: Option<&mut [u8]>,
    ) -> Result<Completion> {
        let mut cmd = Command::new(sqe::OP_GET_FEATURES, 0);
        cmd.set_u32(40, fid as u32);
        cmd.set_u32(44, cdw11);
        self.submit(cmd, buf.map_or(Data::None, Data::In))
    }

    /// Get Log Page `lid` for `nsid` with log specific field `lsp`, into `buf` (whole dwords), returning
    /// the completion rather than failing on an error status.
    pub fn log_page(&mut self, lid: u8, nsid: u32, lsp: u8, buf: &mut [u8]) -> Result<Completion> {
        let mut cmd = Command::new(sqe::OP_GET_LOG_PAGE, 0);
        cmd.set_nsid(nsid);
        cmd.set_u32(
            40,
            ((buf.len() / 4 - 1) as u32) << 16 | ((lsp as u32 & 0x7f) << 8) | lid as u32,
        );
        self.submit(cmd, Data::In(buf))
    }

    /// Sends one admin command without waiting for it, for a command that completes later (an
    /// Asynchronous Event Request). Returns its command ID.
    pub fn post(&mut self, mut cmd: Command) -> Result<u16> {
        let cid = self.next_cid;
        self.next_cid = self.next_cid.wrapping_add(1).max(1);
        cmd.0[2..4].copy_from_slice(&cid.to_le_bytes());
        cmd.set_data(0, false);
        self.link.send_capsule(&cmd.0, &[])?;
        Ok(cid)
    }

    /// Reads the next completion on the admin queue, whichever command it is for.
    pub fn next_completion(&mut self) -> Result<Completion> {
        let p = self.link.read()?;
        match p.kind {
            super::pdu::CAPSULE_RESP => Ok(Completion::parse(&super::pdu::parse_capsule_resp(&p)?)),
            super::pdu::TERM_C2H | super::pdu::TERM_H2C => Err(super::pdu::describe_term(&p)),
            other => Err(format!(
                "NVMe/TCP: unexpected PDU type {other:#x} on the admin queue"
            )),
        }
    }

    /// Runs one admin command to completion, moving data as `data` describes.
    pub fn submit(&mut self, cmd: Command, data: Data) -> Result<Completion> {
        let cid = self.next_cid;
        self.next_cid = self.next_cid.wrapping_add(1).max(1);
        self.link.submit(cid, cmd, data)
    }
}

/// Connects one I/O queue of an association that the admin queue has already connected. `qid` names the
/// queue; `sqsize` is its size, 0-based. The queue is its own TCP connection.
pub fn connect_io_queue(a: &Association, cntlid: u16, qid: u16, sqsize: u16) -> Result<Link> {
    let mut link = Link::open(a.addr)?;
    fabrics_connect(&mut link, a, cntlid, qid, sqsize)?;
    Ok(link)
}

/// Fabrics Connect for one queue. The controller ID comes back from the admin queue's Connect, and
/// `0xffff` asks for a new controller. Returns the controller ID the queue was connected to.
fn fabrics_connect(
    link: &mut Link,
    a: &Association,
    cntlid: u16,
    qid: u16,
    sqsize: u16,
) -> Result<u16> {
    let data = connect_data(a, cntlid)?;
    let mut cmd = Command::new(sqe::OP_FABRICS, 0);
    cmd.set_u8(4, sqe::FCTYPE_CONNECT);
    cmd.set_u16(40, 0); // RECFMT
    cmd.set_u16(42, qid);
    cmd.set_u16(44, sqsize);
    cmd.set_u32(48, if qid == 0 { KATO_MS } else { 0 });
    let done = link.submit(1, cmd, Data::Inline(&data))?;
    check(&done, "Fabrics Connect")?;
    Ok((done.dw0 & 0xffff) as u16)
}

/// The 1024-byte data for Fabrics Connect: the host ID, the controller ID, and both NQNs.
fn connect_data(a: &Association, cntlid: u16) -> Result<Vec<u8>> {
    if a.subnqn.len() > 223 || a.hostnqn.len() > 223 {
        return Err("NVMe/TCP: a subsystem or host NQN is longer than 223 bytes".into());
    }
    let mut d = vec![0u8; 1024];
    d[0..16].copy_from_slice(&a.hostid);
    d[16..18].copy_from_slice(&cntlid.to_le_bytes());
    d[256..256 + a.subnqn.len()].copy_from_slice(a.subnqn.as_bytes());
    d[512..512 + a.hostnqn.len()].copy_from_slice(a.hostnqn.as_bytes());
    Ok(d)
}

fn check(done: &Completion, what: &str) -> Result<()> {
    if done.ok() {
        return Ok(());
    }
    let (sct, sc) = done.code();
    Err(format!(
        "NVMe/TCP: {what} failed: status code type {sct:#x}, status code {sc:#x}"
    ))
}

/// A random 16-byte host identifier, read from the kernel's random source.
pub fn random_hostid() -> Result<[u8; 16]> {
    let mut id = [0u8; 16];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut id))
        .map_err(|e| format!("NVMe/TCP: reading a host identifier: {e}"))?;
    Ok(id)
}
