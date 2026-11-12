//! NVMe/TCP protocol data units (NVMe/TCP transport, TP 8000). Every header field is little-endian.
//! A PDU is an 8-byte common header, a type-specific header whose length is HLEN, the data, and optional
//! digests. spit does not negotiate digests yet, so each PDU ends with its data.

use std::io::{Read, Write};

use super::Result;

pub const ICREQ: u8 = 0x00;
pub const ICRESP: u8 = 0x01;
pub const TERM_H2C: u8 = 0x02;
pub const TERM_C2H: u8 = 0x03;
pub const CAPSULE_CMD: u8 = 0x04;
pub const CAPSULE_RESP: u8 = 0x05;
pub const H2C_DATA: u8 = 0x06;
pub const C2H_DATA: u8 = 0x07;
pub const R2T: u8 = 0x09;

/// The last data PDU of a transfer.
pub const FLAG_LAST: u8 = 0x04;
/// On a C2H data PDU that is also the last one: the command succeeded, so no CapsuleResp follows.
pub const FLAG_SUCCESS: u8 = 0x08;
/// The header and data digest bits. spit does not negotiate digests, so they must be clear.
const DIGEST_FLAGS: u8 = 0x03;

const COMMON_LEN: usize = 8;
pub const ICREQ_LEN: usize = 128;
pub const CAPSULE_CMD_LEN: usize = 72;
pub const CAPSULE_RESP_LEN: usize = 24;
pub const DATA_HEADER_LEN: usize = 24;

/// MAXR2T in ICReq, 0-based: the most R2T PDUs the controller may have outstanding on the connection.
pub const MAX_R2T: u32 = 63;

/// One received PDU: its type, flags, the full type-specific header, and the data after it.
#[derive(Debug, Clone)]
pub struct Pdu {
    pub kind: u8,
    pub flags: u8,
    pub header: Vec<u8>,
    pub data: Vec<u8>,
}

/// Puts the common header fields into `header` and appends `data`. `header` must be the full
/// type-specific header, of HLEN bytes, with its type-specific fields already filled in.
pub fn encode(kind: u8, flags: u8, mut header: Vec<u8>, data: &[u8]) -> Vec<u8> {
    let hlen = header.len();
    header[0] = kind;
    header[1] = flags;
    header[2] = hlen as u8;
    header[3] = if data.is_empty() { 0 } else { hlen as u8 };
    header[4..8].copy_from_slice(&((hlen + data.len()) as u32).to_le_bytes());
    header.extend_from_slice(data);
    header
}

/// Reads one PDU from the stream into `p`, reusing its buffers, so the hot path does not allocate.
pub fn read_into(stream: &mut impl Read, p: &mut Pdu) -> Result<()> {
    let mut common = [0u8; COMMON_LEN];
    stream
        .read_exact(&mut common)
        .map_err(|e| format!("NVMe/TCP: reading a PDU header: {e}"))?;
    let kind = common[0];
    let flags = common[1];
    let hlen = common[2] as usize;
    let plen = u32::from_le_bytes([common[4], common[5], common[6], common[7]]) as usize;
    if hlen < COMMON_LEN || plen < hlen {
        return Err(format!(
            "NVMe/TCP: bad PDU type {kind:#x}: header length {hlen}, PDU length {plen}"
        ));
    }
    if kind != ICRESP && flags & DIGEST_FLAGS != 0 {
        return Err(format!(
            "NVMe/TCP: PDU type {kind:#x} sets digest flags {flags:#x}, but spit does not negotiate digests"
        ));
    }
    p.kind = kind;
    p.flags = flags;
    p.header.clear();
    p.header.resize(hlen, 0);
    p.header[..COMMON_LEN].copy_from_slice(&common);
    stream
        .read_exact(&mut p.header[COMMON_LEN..])
        .map_err(|e| format!("NVMe/TCP: reading PDU type {kind:#x} header: {e}"))?;
    p.data.clear();
    p.data.resize(plen - hlen, 0);
    stream
        .read_exact(&mut p.data)
        .map_err(|e| format!("NVMe/TCP: reading PDU type {kind:#x} data: {e}"))?;
    Ok(())
}

impl Pdu {
    /// An empty PDU whose buffers are filled by `read_into`.
    pub fn empty() -> Self {
        Self::with_capacity(0, 0)
    }

    /// An empty PDU with room for `header` and `data` bytes, so reading does not allocate.
    pub fn with_capacity(header: usize, data: usize) -> Self {
        Self {
            kind: 0,
            flags: 0,
            header: Vec::with_capacity(header),
            data: Vec::with_capacity(data),
        }
    }
}

/// Reads one PDU from the stream.
pub fn read(stream: &mut impl Read) -> Result<Pdu> {
    let mut p = Pdu::empty();
    read_into(stream, &mut p)?;
    Ok(p)
}

/// Frames a PDU into `out`, which is cleared first and reused across calls: the header has `hlen` bytes,
/// `fill` writes its type-specific fields, and `data` follows it.
pub fn frame_into(
    out: &mut Vec<u8>,
    kind: u8,
    flags: u8,
    hlen: usize,
    fill: impl FnOnce(&mut [u8]),
    data: &[u8],
) {
    out.clear();
    out.resize(hlen, 0);
    fill(&mut out[..]);
    out[0] = kind;
    out[1] = flags;
    out[2] = hlen as u8;
    out[3] = if data.is_empty() { 0 } else { hlen as u8 };
    out[4..8].copy_from_slice(&((hlen + data.len()) as u32).to_le_bytes());
    out.extend_from_slice(data);
}

pub fn write(stream: &mut impl Write, bytes: &[u8]) -> Result<()> {
    stream
        .write_all(bytes)
        .map_err(|e| format!("NVMe/TCP: writing a PDU: {e}"))
}

/// The ICReq PDU that opens the connection. spit asks for no digests.
pub fn icreq() -> Vec<u8> {
    let mut h = vec![0u8; ICREQ_LEN];
    h[8..10].copy_from_slice(&0u16.to_le_bytes()); // PFV, version 0
    h[16..20].copy_from_slice(&MAX_R2T.to_le_bytes());
    encode(ICREQ, 0, h, &[])
}

/// What the controller agreed to in ICResp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IcResp {
    pub digest: u8,
    /// The largest H2CData PDU the controller accepts, in bytes.
    pub max_h2c_data: u32,
}

pub fn parse_icresp(p: &Pdu) -> Result<IcResp> {
    if p.kind != ICRESP || p.header.len() != ICREQ_LEN {
        return Err(format!(
            "NVMe/TCP: expected ICResp, got type {:#x} with header length {}",
            p.kind,
            p.header.len()
        ));
    }
    let digest = p.header[11];
    if digest != 0 {
        return Err(format!(
            "NVMe/TCP: controller asked for digests ({digest:#x}); spit does not support them"
        ));
    }
    Ok(IcResp {
        digest,
        max_h2c_data: le32(&p.header, 16),
    })
}

/// The 16-byte completion in a CapsuleResp.
pub fn parse_capsule_resp(p: &Pdu) -> Result<[u8; 16]> {
    if p.kind != CAPSULE_RESP || p.header.len() != CAPSULE_RESP_LEN {
        return Err(format!(
            "NVMe/TCP: expected CapsuleResp, got type {:#x} with header length {}",
            p.kind,
            p.header.len()
        ));
    }
    let mut cqe = [0u8; 16];
    cqe.copy_from_slice(&p.header[8..24]);
    Ok(cqe)
}

/// A C2HData PDU: controller-to-host data for command `cccid` at byte `datao`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct C2hData {
    pub cccid: u16,
    pub datao: u32,
    pub datal: u32,
    pub last: bool,
    pub success: bool,
}

pub fn parse_c2h(p: &Pdu) -> Result<C2hData> {
    if p.kind != C2H_DATA || p.header.len() != DATA_HEADER_LEN {
        return Err(format!(
            "NVMe/TCP: expected C2HData, got type {:#x}",
            p.kind
        ));
    }
    Ok(C2hData {
        cccid: le16(&p.header, 8),
        datao: le32(&p.header, 12),
        datal: le32(&p.header, 16),
        last: p.flags & FLAG_LAST != 0,
        success: p.flags & FLAG_SUCCESS != 0,
    })
}

/// An R2T PDU: the controller asks for `r2tl` bytes of host data for command `cccid`, at `r2to`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct R2t {
    pub cccid: u16,
    pub ttag: u16,
    pub r2to: u32,
    pub r2tl: u32,
}

pub fn parse_r2t(p: &Pdu) -> Result<R2t> {
    if p.kind != R2T || p.header.len() != DATA_HEADER_LEN {
        return Err(format!("NVMe/TCP: expected R2T, got type {:#x}", p.kind));
    }
    Ok(R2t {
        cccid: le16(&p.header, 8),
        ttag: le16(&p.header, 10),
        r2to: le32(&p.header, 12),
        r2tl: le32(&p.header, 16),
    })
}

/// A termination request from the controller: the error the connection is closed for.
pub fn describe_term(p: &Pdu) -> String {
    let fes = le16(&p.header, 8);
    let fei = le32(&p.header, 12);
    format!(
        "NVMe/TCP: controller terminated the connection (type {:#x}, fatal error status {fes:#x}, info {fei:#x})",
        p.kind
    )
}

fn le16(d: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([d[at], d[at + 1]])
}

fn le32(d: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}

/// An H2CData PDU answering an R2T: `chunk` is the host data at `datao` for command `cccid`.
#[cfg(test)]
pub fn h2c_data(cccid: u16, ttag: u16, datao: u32, chunk: &[u8], last: bool) -> Vec<u8> {
    let mut h = vec![0u8; DATA_HEADER_LEN];
    h[8..10].copy_from_slice(&cccid.to_le_bytes());
    h[10..12].copy_from_slice(&ttag.to_le_bytes());
    h[12..16].copy_from_slice(&datao.to_le_bytes());
    h[16..20].copy_from_slice(&(chunk.len() as u32).to_le_bytes());
    encode(H2C_DATA, if last { FLAG_LAST } else { 0 }, h, chunk)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icreq_has_the_fixed_length_and_version() {
        let p = icreq();
        assert_eq!(p.len(), ICREQ_LEN);
        assert_eq!(p[0], ICREQ);
        assert_eq!(p[2] as usize, ICREQ_LEN);
        assert_eq!(le32(&p, 4) as usize, ICREQ_LEN);
        assert_eq!(le32(&p, 16), MAX_R2T);
    }

    #[test]
    fn encode_then_read_round_trips_a_data_pdu() {
        let chunk = [7u8; 512];
        let bytes = h2c_data(3, 9, 4096, &chunk, true);
        let p = read(&mut bytes.as_slice()).unwrap();
        assert_eq!(p.kind, H2C_DATA);
        assert_eq!(p.flags, FLAG_LAST);
        assert_eq!(p.data, chunk);
        assert_eq!(le16(&p.header, 8), 3);
        assert_eq!(le16(&p.header, 10), 9);
        assert_eq!(le32(&p.header, 12), 4096);
        assert_eq!(le32(&p.header, 16), 512);
    }

    #[test]
    fn data_pdus_put_the_data_offset_after_the_header() {
        let bytes = h2c_data(1, 1, 0, &[1, 2, 3], false);
        assert_eq!(bytes[2] as usize, DATA_HEADER_LEN);
        assert_eq!(bytes[3] as usize, DATA_HEADER_LEN);
    }

    #[test]
    fn digest_flags_are_rejected_when_not_negotiated() {
        let mut bytes = h2c_data(1, 1, 0, &[0u8; 4], true);
        bytes[1] |= 0x01;
        assert!(read(&mut bytes.as_slice()).is_err());
    }
}
