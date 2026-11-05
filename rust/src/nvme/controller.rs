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

/// Identify CNS values.
pub const CNS_NAMESPACE: u32 = 0x00;
pub const CNS_CONTROLLER: u32 = 0x01;
pub const CNS_ACTIVE_NAMESPACES: u32 = 0x02;

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

/// The fields of Identify Controller that spit reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControllerInfo {
    /// Bytes of write data the controller takes in the capsule (IOCCSZ less the 64-byte command).
    /// The controller expects in-capsule data for writes up to this size, so the host must send it.
    pub inline_bytes: usize,
    pub serial: String,
    pub model: String,
    pub firmware: String,
    pub namespaces: u32,
    /// Maximum data transfer size as a power of two in units of the minimum page size, 0 for none.
    pub mdts: u8,
    pub nqn: String,
}

pub fn parse_controller(d: &[u8]) -> ControllerInfo {
    let ioccsz = u32::from_le_bytes([d[1792], d[1793], d[1794], d[1795]]) as usize;
    ControllerInfo {
        inline_bytes: (ioccsz * 16).saturating_sub(64),
        serial: ascii(&d[4..24]),
        model: ascii(&d[24..64]),
        firmware: ascii(&d[64..72]),
        namespaces: u32::from_le_bytes([d[516], d[517], d[518], d[519]]),
        mdts: d[77],
        nqn: ascii(&d[768..1024]),
    }
}

/// The fields of Identify Namespace that spit reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NamespaceInfo {
    /// Namespace size in logical blocks.
    pub blocks: u64,
    pub block_len: u32,
}

pub fn parse_namespace(d: &[u8]) -> NamespaceInfo {
    let blocks = u64::from_le_bytes(d[0..8].try_into().unwrap_or([0; 8]));
    let flbas = (d[26] & 0x0f) as usize;
    let lbaf = u32::from_le_bytes(
        d[128 + 4 * flbas..132 + 4 * flbas]
            .try_into()
            .unwrap_or([0; 4]),
    );
    let lbads = (lbaf >> 16) & 0xff;
    NamespaceInfo {
        blocks,
        block_len: 1u32 << lbads.min(31),
    }
}

/// The active namespace IDs listed by CNS 2, in order, until the zero terminator.
pub fn parse_active_namespaces(d: &[u8]) -> Vec<u32> {
    d.chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .take_while(|&nsid| nsid != 0)
        .collect()
}

fn ascii(b: &[u8]) -> String {
    String::from_utf8_lossy(b)
        .trim_end_matches(|c: char| c == ' ' || c == '\0')
        .trim()
        .to_string()
}
