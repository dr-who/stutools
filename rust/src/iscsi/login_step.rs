//! One Login Request at a time, built field by field (RFC 7143 11.12), and the answer as the target
//! gave it (11.13): for the login cases that judge what `login` hides. A request carries any T, C,
//! CSG, NSG, version range, ISID, TSIH, ITT, CID, CmdSN and ExpStatSN, and any data segment bytes, so a
//! case can send repeated keys, bad syntax or a stage the RFC forbids. The answer is a Login Response
//! with every field and its raw text, another PDU (its opcode), or the connection closed: a target that
//! drops the connection has answered too. A response is waited for `ANSWER_WAIT` at most; silence
//! is `Silent`. After a refusal the target must close the connection (11.13.5), which
//! `closed_within` checks.

use super::login::{connect_stream, LOGIN_CMD_SN, LOGIN_ITT};
use super::pdu::{self, BHS_LEN};
use crate::Result;
use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

/// How long a login answer is waited for.
pub(super) const ANSWER_WAIT: Duration = Duration::from_secs(5);

/// The largest login data segment read back: RFC 7143 6.1 requires 8192 bytes of key=value data in a
/// negotiation, and a target may send more in one PDU only if it ignores the default
/// MaxRecvDataSegmentLength (6.3), which `LoginReply::data_len` shows.
const MAX_LOGIN_DATA: usize = 1 << 20;

/// One Login Request PDU, every field as sent.
#[derive(Debug, Clone)]
pub(super) struct RawLogin {
    pub transit: bool,
    pub cont: bool,
    pub csg: u8,
    pub nsg: u8,
    pub version_max: u8,
    pub version_min: u8,
    pub isid: [u8; 6],
    pub tsih: u16,
    pub itt: u32,
    pub cid: u16,
    pub cmd_sn: u32,
    pub exp_stat_sn: u32,
    /// The data segment as sent, before padding.
    pub data: Vec<u8>,
}

impl RawLogin {
    /// The PDU bytes: the BHS and the data segment padded to 4 bytes.
    pub(super) fn encode(&self) -> Vec<u8> {
        let mut out = vec![0u8; BHS_LEN];
        out[0] = 0x03 | 0x40;
        out[1] = (if self.transit { 0x80 } else { 0 })
            | (if self.cont { 0x40 } else { 0 })
            | ((self.csg & 3) << 2)
            | (self.nsg & 3);
        out[2] = self.version_max;
        out[3] = self.version_min;
        out[5..8].copy_from_slice(&(self.data.len() as u32).to_be_bytes()[1..]);
        out[8..14].copy_from_slice(&self.isid);
        out[14..16].copy_from_slice(&self.tsih.to_be_bytes());
        out[16..20].copy_from_slice(&self.itt.to_be_bytes());
        out[20..22].copy_from_slice(&self.cid.to_be_bytes());
        out[24..28].copy_from_slice(&self.cmd_sn.to_be_bytes());
        out[28..32].copy_from_slice(&self.exp_stat_sn.to_be_bytes());
        out.extend_from_slice(&self.data);
        out.resize(BHS_LEN + pdu::padded(self.data.len()), 0);
        out
    }
}

/// Text pairs as a data segment: each `key=value` followed by one NUL (RFC 7143 6.1).
pub(super) fn text(pairs: &[(&str, &str)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (k, v) in pairs {
        out.extend_from_slice(k.as_bytes());
        out.push(b'=');
        out.extend_from_slice(v.as_bytes());
        out.push(0);
    }
    out
}

/// A Login Response, every field as received.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct LoginReply {
    pub transit: bool,
    pub cont: bool,
    pub csg: u8,
    pub nsg: u8,
    pub version_max: u8,
    pub version_active: u8,
    pub isid: [u8; 6],
    pub tsih: u16,
    pub itt: u32,
    pub stat_sn: u32,
    pub exp_cmd_sn: u32,
    pub max_cmd_sn: u32,
    pub status: (u8, u8),
    /// The DataSegmentLength the response declared.
    pub data_len: usize,
    /// The data segment without padding.
    pub data: Vec<u8>,
}

impl LoginReply {
    fn decode(bhs: &[u8; BHS_LEN], data: Vec<u8>) -> LoginReply {
        LoginReply {
            transit: bhs[1] & 0x80 != 0,
            cont: bhs[1] & 0x40 != 0,
            csg: (bhs[1] >> 2) & 3,
            nsg: bhs[1] & 3,
            version_max: bhs[2],
            version_active: bhs[3],
            isid: bhs[8..14].try_into().unwrap(),
            tsih: u16::from_be_bytes([bhs[14], bhs[15]]),
            itt: pdu::be32(bhs, 16),
            stat_sn: pdu::be32(bhs, 24),
            exp_cmd_sn: pdu::be32(bhs, 28),
            max_cmd_sn: pdu::be32(bhs, 32),
            status: (bhs[36], bhs[37]),
            data_len: pdu::data_len(bhs),
            data,
        }
    }

    /// The text items in order, split at each NUL; empty items (a double NUL) are kept as "".
    pub(super) fn items(&self) -> Vec<String> {
        let body = self.data.strip_suffix(&[0]).unwrap_or(&self.data);
        if body.is_empty() {
            return Vec::new();
        }
        body.split(|&b| b == 0)
            .map(|i| String::from_utf8_lossy(i).into_owned())
            .collect()
    }

    /// The key=value pairs; an item without "=" has an empty value.
    pub(super) fn pairs(&self) -> Vec<(String, String)> {
        self.items()
            .into_iter()
            .filter(|i| !i.is_empty())
            .map(|i| match i.split_once('=') {
                Some((k, v)) => (k.to_string(), v.to_string()),
                None => (i, String::new()),
            })
            .collect()
    }

    /// The value of the first `key`, when the response has it.
    pub(super) fn value(&self, key: &str) -> Option<String> {
        self.pairs()
            .into_iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }

    /// Whether this is the Login Final-Response that opens full feature phase (RFC 7143 6.3).
    pub(super) fn full_feature(&self) -> bool {
        self.status == (0, 0) && self.transit && self.nsg == 3
    }
}

/// What the target did with one Login Request.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum LoginAnswer {
    Response(LoginReply),
    /// A PDU other than a Login Response: its opcode.
    Other(u8),
    /// The target closed or reset the connection.
    Closed,
    /// Nothing within `ANSWER_WAIT`.
    Silent,
}

impl LoginAnswer {
    /// The answer in a few words, for a case's actual column.
    pub(super) fn show(&self) -> String {
        match self {
            LoginAnswer::Response(r) if r.status == (0, 0) => format!(
                "status 0000, T={} CSG={} NSG={}",
                r.transit as u8, r.csg, r.nsg
            ),
            LoginAnswer::Response(r) => format!("status {:02x}{:02x}", r.status.0, r.status.1),
            LoginAnswer::Other(op) => format!("opcode {op:#04x}"),
            LoginAnswer::Closed => "connection closed".into(),
            LoginAnswer::Silent => format!("no answer in {} s", ANSWER_WAIT.as_secs()),
        }
    }

    /// The status of a Login Response.
    pub(super) fn status(&self) -> Option<(u8, u8)> {
        match self {
            LoginAnswer::Response(r) => Some(r.status),
            _ => None,
        }
    }

    pub(super) fn reply(&self) -> Option<&LoginReply> {
        match self {
            LoginAnswer::Response(r) => Some(r),
            _ => None,
        }
    }
}

/// Whether an answer refuses the login with Status-Class 2 and one of `details`, or closes the
/// connection when `close_passes`.
pub(super) fn refused_with(answer: &LoginAnswer, details: &[u8], close_passes: bool) -> bool {
    match answer {
        LoginAnswer::Response(r) => r.status.0 == 2 && details.contains(&r.status.1),
        LoginAnswer::Closed => close_passes,
        _ => false,
    }
}

/// One TCP connection in the login phase, with the numbers the next request carries.
pub(super) struct LoginConnection {
    stream: TcpStream,
    pub isid: [u8; 6],
    pub tsih: u16,
    pub cid: u16,
    pub itt: u32,
    pub cmd_sn: u32,
    pub exp_stat_sn: u32,
    /// Every answer so far, in order.
    pub answers: Vec<LoginAnswer>,
}

impl LoginConnection {
    /// A new connection to `portal` for a new session of initiator port `isid`.
    pub(super) fn open(portal: &str, isid: [u8; 6]) -> Result<LoginConnection> {
        let stream = connect_stream(portal)?;
        Ok(LoginConnection {
            stream,
            isid,
            tsih: 0,
            cid: 0,
            itt: LOGIN_ITT,
            cmd_sn: LOGIN_CMD_SN,
            exp_stat_sn: 0,
            answers: Vec::new(),
        })
    }

    /// A request with this connection's numbers, version 0, C=0, and `data`.
    pub(super) fn request(&self, transit: bool, csg: u8, nsg: u8, data: Vec<u8>) -> RawLogin {
        RawLogin {
            transit,
            cont: false,
            csg,
            nsg,
            version_max: 0,
            version_min: 0,
            isid: self.isid,
            tsih: self.tsih,
            itt: self.itt,
            cid: self.cid,
            cmd_sn: self.cmd_sn,
            exp_stat_sn: self.exp_stat_sn,
            data,
        }
    }

    /// Sends `request` and waits for its answer. A status-0 response moves ExpStatSN on.
    pub(super) fn send(&mut self, request: &RawLogin) -> LoginAnswer {
        crate::watchdog::beat(&[]);
        let answer = match self.stream.write_all(&request.encode()) {
            Ok(()) => self.read_answer(),
            Err(_) => LoginAnswer::Closed,
        };
        if let LoginAnswer::Response(r) = &answer {
            if r.status == (0, 0) {
                self.exp_stat_sn = r.stat_sn.wrapping_add(1);
            }
        }
        self.answers.push(answer.clone());
        answer
    }

    /// `request(transit, csg, nsg, text(pairs))`, sent.
    pub(super) fn step(
        &mut self,
        transit: bool,
        csg: u8,
        nsg: u8,
        pairs: &[(&str, &str)],
    ) -> LoginAnswer {
        let r = self.request(transit, csg, nsg, text(pairs));
        self.send(&r)
    }

    /// Sends bytes that are not a Login Request (a SCSI Command during login) and waits for an answer.
    pub(super) fn send_bytes(&mut self, bytes: &[u8]) -> LoginAnswer {
        crate::watchdog::beat(&[]);
        let answer = match self.stream.write_all(bytes) {
            Ok(()) => self.read_answer(),
            Err(_) => LoginAnswer::Closed,
        };
        self.answers.push(answer.clone());
        answer
    }

    /// Reads the next PDU.
    pub(super) fn read_answer(&mut self) -> LoginAnswer {
        crate::watchdog::beat(&[]);
        let _ = self.stream.set_read_timeout(Some(ANSWER_WAIT));
        let mut bhs = [0u8; BHS_LEN];
        if let Err(e) = self.stream.read_exact(&mut bhs) {
            return match e.kind() {
                ErrorKind::WouldBlock | ErrorKind::TimedOut => LoginAnswer::Silent,
                _ => LoginAnswer::Closed,
            };
        }
        if bhs[0] & 0x3f != 0x23 {
            return LoginAnswer::Other(bhs[0] & 0x3f);
        }
        let len = pdu::data_len(&bhs);
        if len > MAX_LOGIN_DATA {
            return LoginAnswer::Closed;
        }
        let mut data = vec![0u8; pdu::padded(len)];
        if self.stream.read_exact(&mut data).is_err() {
            return LoginAnswer::Closed;
        }
        data.truncate(len);
        LoginAnswer::Response(LoginReply::decode(&bhs, data))
    }

    /// Whether the target closes the connection within `wait`: a read returns end of file or an error
    /// other than a timeout. Bytes that arrive meanwhile are read and dropped.
    pub(super) fn closed_within(&mut self, wait: Duration) -> bool {
        let deadline = Instant::now() + wait;
        let mut buf = [0u8; 4096];
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            let _ = self.stream.set_read_timeout(Some(left));
            match self.stream.read(&mut buf) {
                Ok(0) => return true,
                Ok(_) => continue,
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                    return false
                }
                Err(_) => return true,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> RawLogin {
        RawLogin {
            transit: true,
            cont: false,
            csg: 1,
            nsg: 3,
            version_max: 1,
            version_min: 4,
            isid: [0x80, 1, 2, 3, 4, 5],
            tsih: 0x1234,
            itt: 9,
            cid: 7,
            cmd_sn: 123,
            exp_stat_sn: 0x1234_5678,
            data: text(&[("A", "1"), ("A", "2")]),
        }
    }

    #[test]
    fn raw_request_puts_every_field_in_place() {
        let pdu = request().encode();
        assert_eq!(pdu[0], 0x43);
        assert_eq!(pdu[1], 0x80 | (1 << 2) | 3);
        assert_eq!((pdu[2], pdu[3]), (1, 4));
        assert_eq!(&pdu[5..8], &[0, 0, 8]);
        assert_eq!(&pdu[8..14], &[0x80, 1, 2, 3, 4, 5]);
        assert_eq!(&pdu[14..16], &[0x12, 0x34]);
        assert_eq!(pdu::be32(&pdu, 16), 9);
        assert_eq!(&pdu[20..22], &[0, 7]);
        assert_eq!(pdu::be32(&pdu, 24), 123);
        assert_eq!(pdu::be32(&pdu, 28), 0x1234_5678);
        assert_eq!(&pdu[48..56], b"A=1\0A=2\0");
        let mut c = request();
        c.transit = false;
        c.cont = true;
        c.data = b"Max".to_vec();
        let pdu = c.encode();
        assert_eq!(pdu[1] & 0xc0, 0x40, "C without T");
        assert_eq!(pdu.len(), 52, "padded to four bytes");
    }

    #[test]
    fn reply_keeps_empty_items_and_repeats() {
        let mut bhs = [0u8; BHS_LEN];
        bhs[0] = 0x23;
        bhs[1] = 0x87;
        bhs[36] = 2;
        bhs[37] = 7;
        let r = LoginReply::decode(&bhs, b"A=1\0\0A=2\0Bad\0".to_vec());
        assert_eq!(r.items(), vec!["A=1", "", "A=2", "Bad"]);
        assert_eq!(r.pairs().len(), 3);
        assert_eq!(r.value("A").as_deref(), Some("1"));
        assert_eq!((r.csg, r.nsg, r.status), (1, 3, (2, 7)));
        assert!(!r.full_feature());
        let answer = LoginAnswer::Response(r);
        assert!(refused_with(&answer, &[7], false));
        assert!(!refused_with(&answer, &[1], false));
        assert!(refused_with(&LoginAnswer::Closed, &[1], true));
        assert!(!refused_with(&LoginAnswer::Silent, &[1], true));
        assert_eq!(answer.show(), "status 0207");
    }
}
