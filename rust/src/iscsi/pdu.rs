//! iSCSI PDU framing for the login phase (RFC 7143). Every PDU starts with a 48-byte basic header
//! segment (BHS), then an optional data segment of text key=value pairs, padded to 4 bytes.

use crate::Result;

const LOGIN_REQUEST: u8 = 0x03;
const LOGIN_RESPONSE: u8 = 0x23;
const LOGOUT_REQUEST: u8 = 0x06;
const LOGOUT_RESPONSE: u8 = 0x26;
pub const NOP_OUT: u8 = 0x00;
/// Text Request and Text Response: the key=value exchange of discovery (SendTargets).
pub const TEXT_REQUEST: u8 = 0x04;
pub const TEXT_RESPONSE: u8 = 0x24;
pub const NOP_IN: u8 = 0x20;
pub const SCSI_COMMAND: u8 = 0x01;
pub const SCSI_RESPONSE: u8 = 0x21;
pub const DATA_OUT: u8 = 0x05;
pub const DATA_IN: u8 = 0x25;
pub const R2T: u8 = 0x31;
/// Task management (RFC 7143 11.5, 11.6) and the target's Reject of a PDU it cannot take (11.17).
pub const TASK_MANAGEMENT_REQUEST: u8 = 0x02;
pub const TASK_MANAGEMENT_RESPONSE: u8 = 0x22;
pub const REJECT: u8 = 0x3f;
/// Task management functions (RFC 7143 11.5.1).
pub const TMF_ABORT_TASK: u8 = 1;
pub const TMF_ABORT_TASK_SET: u8 = 2;
pub const TMF_CLEAR_TASK_SET: u8 = 3;
pub const TMF_LOGICAL_UNIT_RESET: u8 = 5;
/// The functions RFC 7144 6.3 adds.
pub const TMF_QUERY_TASK: u8 = 9;
pub const TMF_QUERY_TASK_SET: u8 = 10;
pub const TMF_I_T_NEXUS_RESET: u8 = 11;
pub const TMF_QUERY_ASYNC_EVENT: u8 = 12;
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
    /// The BHS. A CDB longer than 16 bytes puts its first 16 here and the rest in the Extended CDB AHS
    /// (`extended_cdb`), whose length goes in TotalAHSLength (RFC 7143 11.2.1.5, 11.3.5).
    pub fn encode(&self) -> Result<[u8; BHS_LEN]> {
        if self.cdb.is_empty() || self.cdb.len() > MAX_CDB {
            return Err(format!(
                "CDB must be 1 to {MAX_CDB} bytes, not {}",
                self.cdb.len()
            ));
        }
        let mut bhs = [0u8; BHS_LEN];
        bhs[0] = SCSI_COMMAND;
        bhs[4] = (extended_cdb(self.cdb).len() / 4) as u8;
        bhs[1] = TRANSIT | (if self.read { 0x40 } else { 0 }) | (if self.write { 0x20 } else { 0 });
        bhs[8..16].copy_from_slice(&self.lun.to_be_bytes());
        bhs[16..20].copy_from_slice(&self.itt.to_be_bytes());
        bhs[20..24].copy_from_slice(&self.expected.to_be_bytes());
        bhs[24..28].copy_from_slice(&self.cmd_sn.to_be_bytes());
        bhs[28..32].copy_from_slice(&self.exp_stat_sn.to_be_bytes());
        let n = self.cdb.len().min(16);
        bhs[32..32 + n].copy_from_slice(&self.cdb[..n]);
        Ok(bhs)
    }
}

/// The largest CDB spit sends: the 32-byte variable-length CDBs (operation code 7Fh).
const MAX_CDB: usize = 32;

/// The Extended CDB AHS for the bytes of a CDB past the 16th (RFC 7143 11.2.2.3): AHSLength (the CDB
/// length less 15, counting the reserved byte), AHSType 1, a reserved byte, then the spill, padded to
/// 4 bytes. Empty for a CDB of 16 bytes or fewer.
pub fn extended_cdb(cdb: &[u8]) -> Vec<u8> {
    if cdb.len() <= 16 {
        return Vec::new();
    }
    let spill = &cdb[16..];
    let mut ahs = Vec::with_capacity(4 + padded(spill.len()));
    ahs.extend_from_slice(&((cdb.len() - 15) as u16).to_be_bytes());
    ahs.push(0x01);
    ahs.push(0);
    ahs.extend_from_slice(spill);
    ahs.resize(4 + padded(spill.len()), 0);
    ahs
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

/// Sets the data segment length of a header, for a PDU that carries data (a NOP-Out ping).
pub fn set_data_len(bhs: &mut [u8; BHS_LEN], len: usize) {
    bhs[5..8].copy_from_slice(&(len as u32).to_be_bytes()[1..]);
}

/// Task Management Function Request (RFC 7143 11.5), sent immediate. `referenced` is the task tag
/// ABORT TASK names; other functions carry 0xffffffff there. `ref_cmd_sn` is the CmdSN of that task.
pub fn task_management_request(
    function: u8,
    itt: u32,
    referenced: u32,
    cmd_sn: u32,
    exp_stat_sn: u32,
    ref_cmd_sn: u32,
) -> [u8; BHS_LEN] {
    let mut bhs = [0u8; BHS_LEN];
    bhs[0] = TASK_MANAGEMENT_REQUEST | IMMEDIATE;
    bhs[1] = TRANSIT | (function & 0x7f);
    bhs[16..20].copy_from_slice(&itt.to_be_bytes());
    bhs[20..24].copy_from_slice(&referenced.to_be_bytes());
    bhs[24..28].copy_from_slice(&cmd_sn.to_be_bytes());
    bhs[28..32].copy_from_slice(&exp_stat_sn.to_be_bytes());
    bhs[32..36].copy_from_slice(&ref_cmd_sn.to_be_bytes());
    bhs
}

/// Text Request header for `data_len` bytes of text, which is one final PDU. `ttt` names the target
/// transfer tag of a continuation, or `NO_TTT` to start a new exchange.
pub fn text_request(
    itt: u32,
    ttt: u32,
    cmd_sn: u32,
    exp_stat_sn: u32,
    data_len: usize,
) -> [u8; BHS_LEN] {
    let mut bhs = [0u8; BHS_LEN];
    bhs[0] = TEXT_REQUEST;
    bhs[1] = 0x80; // Final
    bhs[5..8].copy_from_slice(&(data_len as u32).to_be_bytes()[1..]);
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
    fn a_32_byte_cdb_spills_into_the_extended_cdb_ahs() {
        let cdb: Vec<u8> = (0..32).collect();
        let ahs = extended_cdb(&cdb);
        assert_eq!(ahs.len(), 20);
        assert_eq!(&ahs[..4], &[0, 17, 0x01, 0]);
        assert_eq!(&ahs[4..], &cdb[16..]);
        let command = ScsiCommand {
            lun: 0,
            itt: 1,
            cmd_sn: 1,
            exp_stat_sn: 1,
            cdb: &cdb,
            read: true,
            write: false,
            expected: 512,
        };
        let bhs = command.encode().unwrap();
        assert_eq!(bhs[4], 5, "TotalAHSLength in 4-byte words");
        assert_eq!(&bhs[32..48], &cdb[..16]);
        assert!(extended_cdb(&cdb[..16]).is_empty());
    }

    #[test]
    fn task_management_request_is_immediate_with_its_function_and_tags() {
        let bhs = task_management_request(TMF_ABORT_TASK, 9, 0x1234, 5, 6, 4);
        assert_eq!(bhs[0], 0x42, "task management with the immediate bit");
        assert_eq!(bhs[1], 0x80 | 1);
        assert_eq!(&bhs[16..20], &9u32.to_be_bytes());
        assert_eq!(&bhs[20..24], &0x1234u32.to_be_bytes());
        assert_eq!(&bhs[24..28], &5u32.to_be_bytes());
        assert_eq!(&bhs[28..32], &6u32.to_be_bytes());
        assert_eq!(&bhs[32..36], &4u32.to_be_bytes());
    }

    #[test]
    fn nop_ping_carries_its_data_length() {
        let mut bhs = nop_out(0, 3, NO_TTT, 1, 1);
        set_data_len(&mut bhs, 0x0102);
        assert_eq!(&bhs[4..8], &[0, 0, 1, 2]);
        assert_eq!(bhs[0], 0x40);
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
