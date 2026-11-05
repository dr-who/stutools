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
pub const CNS_NS_DESCRIPTORS: u32 = 0x03;

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

/// The fields of Identify Controller that spit reports or tests.
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
    /// CMIC: bit 1 more than one controller, bit 3 ANA reporting.
    pub cmic: u8,
    pub cntlid: u16,
    /// VER: the version in the same form as the VS property.
    pub ver: u32,
    /// AERL: the asynchronous event requests outstanding at once, 0-based.
    pub aerl: u8,
    /// KAS: the keep-alive timer granularity in 100 ms units.
    pub kas: u16,
    /// ANACAP, ANAGRPMAX and NANAGRPID: the ANA states and groups the controller can report.
    pub anacap: u8,
    pub anagrpmax: u32,
    pub nanagrpid: u32,
    /// SQES and CQES: required (low nibble) and largest (high nibble) entry sizes as powers of two.
    pub sqes: u8,
    pub cqes: u8,
    /// ONCS: optional NVM commands (bit 0 Compare, 2 Dataset Management, 3 Write Zeroes,
    /// 5 Reservations).
    pub oncs: u16,
    /// FUSES: bit 0 fused Compare and Write.
    pub fuses: u16,
    /// VWC: bit 0 a volatile write cache is present.
    pub vwc: u8,
    /// SGLS: SGL support; bits 1:0 are non-zero when SGLs are supported, which fabrics need.
    pub sgls: u32,
    /// IOCCSZ and IORCSZ: the I/O command and response capsule sizes in 16-byte units.
    pub ioccsz: u32,
    pub iorcsz: u32,
}

pub fn parse_controller(d: &[u8]) -> ControllerInfo {
    let u16_at = |at: usize| u16::from_le_bytes([d[at], d[at + 1]]);
    let u32_at = |at: usize| u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]]);
    let ioccsz = u32_at(1792);
    ControllerInfo {
        inline_bytes: (ioccsz as usize * 16).saturating_sub(64),
        serial: ascii(&d[4..24]),
        model: ascii(&d[24..64]),
        firmware: ascii(&d[64..72]),
        namespaces: u32_at(516),
        mdts: d[77],
        nqn: ascii(&d[768..1024]),
        cmic: d[76],
        cntlid: u16_at(78),
        ver: u32_at(80),
        aerl: d[259],
        kas: u16_at(320),
        anacap: d[343],
        anagrpmax: u32_at(344),
        nanagrpid: u32_at(348),
        sqes: d[512],
        cqes: d[513],
        oncs: u16_at(520),
        fuses: u16_at(522),
        vwc: d[525],
        sgls: u32_at(536),
        ioccsz,
        iorcsz: u32_at(1796),
    }
}

/// The fields of Identify Namespace that spit reports or tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NamespaceInfo {
    /// Namespace size in logical blocks (NSZE).
    pub blocks: u64,
    pub block_len: u32,
    /// NCAP and NUSE: the capacity and the blocks in use.
    pub capacity: u64,
    pub used: u64,
    /// NLBAF (0-based) and the format index FLBAS selects.
    pub formats: u8,
    pub format: u8,
    /// NMIC bit 0: the namespace may be attached to more than one controller.
    pub nmic: u8,
    /// RESCAP: bit 1 to 6 the reservation types 1 to 6, bit 7 Ignore Existing Key.
    pub rescap: u8,
    /// DLFEAT bits 2:0: what a deallocated block reads as (1 zeros, 2 ones, 0 not reported).
    pub dlfeat: u8,
    pub anagrpid: u32,
    pub nguid: [u8; 16],
    pub eui64: [u8; 8],
}

pub fn parse_namespace(d: &[u8]) -> NamespaceInfo {
    let u64_at = |at: usize| u64::from_le_bytes(d[at..at + 8].try_into().unwrap_or([0; 8]));
    let blocks = u64_at(0);
    // NVM 1.1 Identify Namespace FLBAS: the format index is bits 3:0, with bits 6:5 above them when
    // there are more than 16 formats (NLBAF is 0-based).
    let flbas = if d[25] >= 16 {
        (d[26] & 0x0f) as usize | ((d[26] as usize >> 5) & 0x3) << 4
    } else {
        (d[26] & 0x0f) as usize
    };
    let lbaf = u32::from_le_bytes(
        d[128 + 4 * flbas..132 + 4 * flbas]
            .try_into()
            .unwrap_or([0; 4]),
    );
    let lbads = (lbaf >> 16) & 0xff;
    NamespaceInfo {
        blocks,
        block_len: 1u32 << lbads.min(31),
        capacity: u64_at(8),
        used: u64_at(16),
        formats: d[25],
        format: flbas as u8,
        nmic: d[30],
        rescap: d[31],
        dlfeat: d[33],
        anagrpid: u32::from_le_bytes([d[92], d[93], d[94], d[95]]),
        nguid: d[104..120].try_into().unwrap_or([0; 16]),
        eui64: d[120..128].try_into().unwrap_or([0; 8]),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identify_controller_fields_come_from_their_offsets() {
        let mut d = vec![0u8; 4096];
        d[76] = 0x0b;
        d[78..80].copy_from_slice(&7u16.to_le_bytes());
        d[80..84].copy_from_slice(&0x0002_0000u32.to_le_bytes());
        d[320..322].copy_from_slice(&5u16.to_le_bytes());
        d[512] = 0x66;
        d[520..522].copy_from_slice(&0x2cu16.to_le_bytes());
        d[525] = 1;
        d[536..540].copy_from_slice(&1u32.to_le_bytes());
        d[1792..1796].copy_from_slice(&260u32.to_le_bytes());
        let c = parse_controller(&d);
        assert_eq!((c.cmic, c.cntlid, c.ver, c.kas), (0x0b, 7, 0x0002_0000, 5));
        assert_eq!((c.sqes, c.oncs, c.vwc, c.sgls), (0x66, 0x2c, 1, 1));
        assert_eq!(c.inline_bytes, 260 * 16 - 64);
    }

    #[test]
    fn the_format_index_uses_its_upper_bits_past_16_formats() {
        let mut d = vec![0u8; 4096];
        d[25] = 17; // 18 formats
        d[26] = 0x21; // index 0b01_0001 = 17
        d[128 + 4 * 17 + 2] = 12; // LBADS 12: 4096-byte blocks
        let ns = parse_namespace(&d);
        assert_eq!((ns.format, ns.block_len), (17, 4096));
        d[25] = 3; // 4 formats: bits 6:5 are ignored
        d[128 + 4 + 2] = 9;
        assert_eq!(parse_namespace(&d).block_len, 512);
    }
}
