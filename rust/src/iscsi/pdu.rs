//! iSCSI PDU framing for the login phase (RFC 7143). Every PDU starts with a 48-byte basic header
//! segment (BHS), then an optional data segment of text key=value pairs, padded to 4 bytes.

use crate::Result;

const LOGIN_REQUEST: u8 = 0x03;
const LOGIN_RESPONSE: u8 = 0x23;
const LOGOUT_REQUEST: u8 = 0x06;
const LOGOUT_RESPONSE: u8 = 0x26;
pub const NOP_OUT: u8 = 0x00;
pub const NOP_IN: u8 = 0x20;
pub const SCSI_COMMAND: u8 = 0x01;
pub const SCSI_RESPONSE: u8 = 0x21;
pub const DATA_OUT: u8 = 0x05;
pub const DATA_IN: u8 = 0x25;
pub const R2T: u8 = 0x31;
/// The task tag a target uses when it does not expect a reply (RFC 7143 11.10.4).
pub const NO_TTT: u32 = 0xffff_ffff;
const IMMEDIATE: u8 = 0x40;
const TRANSIT: u8 = 0x80;
pub const BHS_LEN: usize = 48;

/// Login stages: the security negotiation, the operational negotiation, and full feature phase.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stage {
    Security = 0,
    Operational = 1,
    FullFeature = 3,
}

/// One login request PDU.
pub struct LoginRequest {
    pub isid: [u8; 6],
    pub tsih: u16,
    pub cid: u16,
    pub itt: u32,
    pub cmd_sn: u32,
    pub exp_stat_sn: u32,
    pub current: Stage,
    pub next: Stage,
    pub transit: bool,
    pub text: Vec<(String, String)>,
}

impl LoginRequest {
    /// Encodes the PDU: the BHS, then the text pairs, each NUL-terminated and padded to 4 bytes.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut data = Vec::new();
        for (key, value) in &self.text {
            data.extend_from_slice(format!("{key}={value}").as_bytes());
            data.push(0);
        }
        // DataSegmentLength is the real length; the padding after it is not counted (RFC 7143 11.2.1.6).
        let length = data.len();
        if length >= 1 << 24 {
            return Err("login text is too large for one PDU".into());
        }
        while data.len() % 4 != 0 {
            data.push(0);
        }
        let mut bhs = [0u8; BHS_LEN];
        bhs[0] = LOGIN_REQUEST | IMMEDIATE;
        bhs[1] = (if self.transit { TRANSIT } else { 0 })
            | ((self.current as u8) << 2)
            | self.next as u8;
        bhs[5..8].copy_from_slice(&(length as u32).to_be_bytes()[1..]);
        bhs[8..14].copy_from_slice(&self.isid);
        bhs[14..16].copy_from_slice(&self.tsih.to_be_bytes());
        bhs[16..20].copy_from_slice(&self.itt.to_be_bytes());
        bhs[20..22].copy_from_slice(&self.cid.to_be_bytes());
        bhs[24..28].copy_from_slice(&self.cmd_sn.to_be_bytes());
        bhs[28..32].copy_from_slice(&self.exp_stat_sn.to_be_bytes());
        let mut pdu = bhs.to_vec();
        pdu.extend_from_slice(&data);
        Ok(pdu)
    }
}

/// One login response PDU, as the target sent it.
#[derive(Debug, PartialEq)]
pub struct LoginResponse {
    pub transit: bool,
    pub current: u8,
    pub next: u8,
    pub tsih: u16,
    pub itt: u32,
    pub stat_sn: u32,
    /// Status class and detail; class 0 is success.
    pub status: (u8, u8),
    pub text: Vec<(String, String)>,
}

impl LoginResponse {
    /// Decodes a login response from its BHS and data segment.
    pub fn decode(pdu: &[u8]) -> Result<Self> {
        if pdu.len() < BHS_LEN {
            return Err("login response is shorter than its header".into());
        }
        if pdu[0] & 0x3f != LOGIN_RESPONSE {
            return Err(format!(
                "expected a login response, got opcode {:#x}",
                pdu[0] & 0x3f
            ));
        }
        let len = u32::from_be_bytes([0, pdu[5], pdu[6], pdu[7]]) as usize;
        let data = pdu
            .get(BHS_LEN..BHS_LEN + len)
            .ok_or("login response data is truncated")?;
        Ok(Self {
            transit: pdu[1] & TRANSIT != 0,
            current: (pdu[1] >> 2) & 0x3,
            next: pdu[1] & 0x3,
            tsih: u16::from_be_bytes([pdu[14], pdu[15]]),
            itt: u32::from_be_bytes([pdu[16], pdu[17], pdu[18], pdu[19]]),
            stat_sn: u32::from_be_bytes([pdu[24], pdu[25], pdu[26], pdu[27]]),
            status: (pdu[36], pdu[37]),
            text: parse_text(data)?,
        })
    }
}

/// Splits a text data segment into key=value pairs. Empty pairs (padding) are skipped.
pub fn parse_text(data: &[u8]) -> Result<Vec<(String, String)>> {
    let mut pairs = Vec::new();
    for item in data.split(|&b| b == 0).filter(|item| !item.is_empty()) {
        let text = std::str::from_utf8(item).map_err(|_| "login text is not UTF-8")?;
        let (key, value) = text
            .split_once('=')
            .ok_or_else(|| format!("login text item has no '=': {text}"))?;
        pairs.push((key.to_string(), value.to_string()));
    }
    Ok(pairs)
}

/// Logout request PDU header. Reason 0 closes the session.
pub fn logout_request(itt: u32, cid: u16, cmd_sn: u32, exp_stat_sn: u32) -> [u8; BHS_LEN] {
    let mut bhs = [0u8; BHS_LEN];
    bhs[0] = LOGOUT_REQUEST | IMMEDIATE;
    bhs[1] = TRANSIT;
    bhs[16..20].copy_from_slice(&itt.to_be_bytes());
    bhs[20..22].copy_from_slice(&cid.to_be_bytes());
    bhs[24..28].copy_from_slice(&cmd_sn.to_be_bytes());
    bhs[28..32].copy_from_slice(&exp_stat_sn.to_be_bytes());
    bhs
}

/// Checks a logout response's opcode and response code (0 is "connection or session closed").
pub fn check_logout_response(pdu: &[u8]) -> Result<()> {
    if pdu.len() < BHS_LEN || pdu[0] & 0x3f != LOGOUT_RESPONSE {
        return Err("expected a logout response".into());
    }
    match pdu[2] {
        0 => Ok(()),
        code => Err(format!("logout refused with response {code}")),
    }
}

pub fn be32(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// Pads a data segment to a 4-byte boundary, as every iSCSI data segment is padded.
pub fn padded(len: usize) -> usize {
    len.next_multiple_of(4)
}

/// The data segment length a PDU's header declares (before padding).
pub fn data_len(bhs: &[u8]) -> usize {
    u32::from_be_bytes([0, bhs[5], bhs[6], bhs[7]]) as usize
}

/// SCSI Command PDU (RFC 7143 11.3). Carries the CDB and the expected transfer length; the write data
/// follows in Data-Out PDUs after the target asks for it with R2T.
pub struct ScsiCommand<'a> {
    pub lun: u64,
    pub itt: u32,
    pub cmd_sn: u32,
    pub exp_stat_sn: u32,
    pub cdb: &'a [u8],
    pub read: bool,
    pub write: bool,
    pub expected: u32,
}

impl ScsiCommand<'_> {
    pub fn encode(&self) -> Result<[u8; BHS_LEN]> {
        if self.cdb.is_empty() || self.cdb.len() > 16 {
            return Err(format!("CDB must be 1 to 16 bytes, not {}", self.cdb.len()));
        }
        let mut bhs = [0u8; BHS_LEN];
        bhs[0] = SCSI_COMMAND;
        bhs[1] = TRANSIT | (if self.read { 0x40 } else { 0 }) | (if self.write { 0x20 } else { 0 });
        bhs[8..16].copy_from_slice(&self.lun.to_be_bytes());
        bhs[16..20].copy_from_slice(&self.itt.to_be_bytes());
        bhs[20..24].copy_from_slice(&self.expected.to_be_bytes());
        bhs[24..28].copy_from_slice(&self.cmd_sn.to_be_bytes());
        bhs[28..32].copy_from_slice(&self.exp_stat_sn.to_be_bytes());
        bhs[32..32 + self.cdb.len()].copy_from_slice(self.cdb);
        Ok(bhs)
    }
}

/// Data-Out PDU header (RFC 7143 11.7) for `data_len` bytes of write data the target asked for with R2T.
/// The data itself follows the header on the wire.
pub fn data_out_header(
    itt: u32,
    ttt: u32,
    exp_stat_sn: u32,
    data_sn: u32,
    offset: u32,
    last: bool,
    data_len: usize,
) -> [u8; BHS_LEN] {
    let mut bhs = [0u8; BHS_LEN];
    bhs[0] = DATA_OUT;
    bhs[1] = if last { TRANSIT } else { 0 };
    bhs[5..8].copy_from_slice(&(data_len as u32).to_be_bytes()[1..]);
    bhs[16..20].copy_from_slice(&itt.to_be_bytes());
    bhs[20..24].copy_from_slice(&ttt.to_be_bytes());
    bhs[28..32].copy_from_slice(&exp_stat_sn.to_be_bytes());
    bhs[36..40].copy_from_slice(&data_sn.to_be_bytes());
    bhs[40..44].copy_from_slice(&offset.to_be_bytes());
    bhs
}

/// NOP-Out header, used to answer a NOP-In that carries a target transfer tag.
pub fn nop_out(lun: u64, itt: u32, ttt: u32, cmd_sn: u32, exp_stat_sn: u32) -> [u8; BHS_LEN] {
    let mut bhs = [0u8; BHS_LEN];
    bhs[0] = NOP_OUT | 0x40;
    bhs[1] = TRANSIT;
    bhs[8..16].copy_from_slice(&lun.to_be_bytes());
    bhs[16..20].copy_from_slice(&itt.to_be_bytes());
    bhs[20..24].copy_from_slice(&ttt.to_be_bytes());
    bhs[24..28].copy_from_slice(&cmd_sn.to_be_bytes());
    bhs[28..32].copy_from_slice(&exp_stat_sn.to_be_bytes());
    bhs
}

/// The sequence numbers every target-to-initiator PDU carries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sequence {
    pub stat_sn: u32,
    pub exp_cmd_sn: u32,
    pub max_cmd_sn: u32,
}

pub fn sequence(bhs: &[u8]) -> Sequence {
    Sequence {
        stat_sn: be32(bhs, 24),
        exp_cmd_sn: be32(bhs, 28),
        max_cmd_sn: be32(bhs, 32),
    }
}

/// Ready To Transfer (RFC 7143 11.8): the target asks for `desired` bytes of write data at `offset`.
#[derive(Debug, PartialEq)]
pub struct R2t {
    pub itt: u32,
    pub ttt: u32,
    pub seq: Sequence,
    pub r2t_sn: u32,
    pub offset: u32,
    pub desired: u32,
}

pub fn decode_r2t(pdu: &[u8]) -> Result<R2t> {
    if pdu.len() < BHS_LEN || pdu[0] & 0x3f != R2T {
        return Err("expected R2T".into());
    }
    Ok(R2t {
        itt: be32(pdu, 16),
        ttt: be32(pdu, 20),
        seq: sequence(pdu),
        r2t_sn: be32(pdu, 36),
        offset: be32(pdu, 40),
        desired: be32(pdu, 44),
    })
}

/// NOP-In (RFC 7143 11.18). Its target transfer tag is set when the target wants a NOP-Out reply.
#[derive(Debug, PartialEq)]
pub struct NopIn {
    pub itt: u32,
    pub ttt: u32,
    pub seq: Sequence,
}

pub fn decode_nop_in(pdu: &[u8]) -> Result<NopIn> {
    if pdu.len() < BHS_LEN || pdu[0] & 0x3f != NOP_IN {
        return Err("expected NOP-In".into());
    }
    Ok(NopIn {
        itt: be32(pdu, 16),
        ttt: be32(pdu, 20),
        seq: sequence(pdu),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> LoginRequest {
        LoginRequest {
            isid: [0x40, 0, 0, 0x12, 0x34, 0x56],
            tsih: 0,
            cid: 1,
            itt: 7,
            cmd_sn: 1,
            exp_stat_sn: 0,
            current: Stage::Security,
            next: Stage::Operational,
            transit: true,
            text: vec![("InitiatorName".into(), "iqn.test".into())],
        }
    }

    #[test]
    fn request_header_has_the_login_fields_in_place() {
        let pdu = request().encode().unwrap();
        assert_eq!(pdu[0], 0x43, "login request with the immediate bit");
        assert_eq!(pdu[1], 0x80 | 0x01, "transit, CSG 0, NSG 1");
        assert_eq!(&pdu[8..14], &[0x40, 0, 0, 0x12, 0x34, 0x56]);
        assert_eq!(&pdu[16..20], &7u32.to_be_bytes());
        assert_eq!(&pdu[24..28], &1u32.to_be_bytes());
    }

    #[test]
    fn request_text_is_nul_terminated_and_padded_to_four_bytes() {
        let pdu = request().encode().unwrap();
        let text = b"InitiatorName=iqn.test\0";
        let padded = text.len().next_multiple_of(4);
        assert_eq!(pdu.len(), BHS_LEN + padded);
        assert_eq!(&pdu[5..8], &(text.len() as u32).to_be_bytes()[1..]);
        assert_eq!(&pdu[BHS_LEN..BHS_LEN + text.len()], text);
    }

    #[test]
    fn response_decodes_status_stage_and_keys() {
        let mut pdu = vec![0u8; BHS_LEN];
        pdu[0] = 0x23;
        pdu[1] = 0x80 | 1;
        pdu[14..16].copy_from_slice(&5u16.to_be_bytes());
        pdu[24..28].copy_from_slice(&9u32.to_be_bytes());
        let data = b"AuthMethod=CHAP\0";
        pdu[5..8].copy_from_slice(&(data.len() as u32).to_be_bytes()[1..]);
        pdu.extend_from_slice(data);
        let r = LoginResponse::decode(&pdu).unwrap();
        assert!(r.transit);
        assert_eq!((r.current, r.next, r.tsih, r.stat_sn), (0, 1, 5, 9));
        assert_eq!(r.status, (0, 0));
        assert_eq!(r.text, vec![("AuthMethod".to_string(), "CHAP".to_string())]);
    }

    #[test]
    fn response_reports_a_refusal_status() {
        let mut pdu = vec![0u8; BHS_LEN];
        pdu[0] = 0x23;
        pdu[36] = 0x02;
        pdu[37] = 0x01;
        assert_eq!(LoginResponse::decode(&pdu).unwrap().status, (2, 1));
    }

    #[test]
    fn rejects_other_opcodes_and_truncated_data() {
        let mut pdu = vec![0u8; BHS_LEN];
        pdu[0] = 0x05;
        assert!(LoginResponse::decode(&pdu).is_err());
        pdu[0] = 0x23;
        pdu[7] = 4;
        assert!(LoginResponse::decode(&pdu).is_err());
    }
}
