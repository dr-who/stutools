//! REPORT SUPPORTED OPERATION CODES (SPC-6 6.32) as the disk cases send and read it: the all_commands
//! form with or without command timeouts descriptors, the one-command forms (REPORTING OPTIONS 001b and
//! 010b), and the all_commands list parsed into one entry per operation code and service action.

use super::port::Port;
use super::scsi_fields::{be16, be32};
use super::session::Outcome;
use crate::Result;

/// One command REPORT SUPPORTED OPERATION CODES lists (SPC-6 6.32.2, table 260).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Supported {
    pub opcode: u8,
    pub service_action: Option<u16>,
    pub cdb_len: u16,
}

/// The all_commands list of REPORT SUPPORTED OPERATION CODES (SPC-5 tables 259, 260). A descriptor
/// with CTDP set carries a 12-byte command timeouts descriptor.
pub(super) fn parse_rsoc(d: &[u8]) -> Vec<Supported> {
    let end = (4 + be32(d, 0) as usize).min(d.len());
    let mut out = Vec::new();
    let mut i = 4;
    while i + 8 <= end {
        let flags = d[i + 5];
        out.push(Supported {
            opcode: d[i],
            service_action: (flags & 0x01 != 0).then(|| be16(d, i + 2)),
            cdb_len: be16(d, i + 6),
        });
        i += if flags & 0x02 != 0 { 20 } else { 8 };
    }
    out
}

pub(super) fn rsoc_all(s: &mut Port, rctd: bool) -> Result<Outcome> {
    let a = 65536u32.to_be_bytes();
    let cdb = [
        0xa3,
        0x0c,
        if rctd { 0x80 } else { 0 },
        0,
        0,
        0,
        a[0],
        a[1],
        a[2],
        a[3],
        0,
        0,
    ];
    s.command(&cdb, 65536, None)
}

/// REPORT SUPPORTED OPERATION CODES for one command (REPORTING OPTIONS 001b, or 010b with a service
/// action; SPC-6 table 257).
pub(super) fn rsoc_one(s: &mut Port, opcode: u8, service_action: Option<u16>) -> Result<Outcome> {
    let sa = service_action.unwrap_or(0).to_be_bytes();
    let options = if service_action.is_some() { 0x02 } else { 0x01 };
    let cdb = [
        0xa3, 0x0c, options, opcode, sa[0], sa[1], 0, 0, 0x01, 0, 0, 0,
    ];
    s.command(&cdb, 256, None)
}

/// The SUPPORT field of REPORT SUPPORTED OPERATION CODES for one command, one-command format (SPC-3 6.23.1,
/// table 159): 0 no data yet, 1 not supported, 3 supported in conformance with a standard, 5 vendor specific.
/// None when the device gives no answer.
pub(super) fn support(s: &mut Port, opcode: u8, service_action: Option<u16>) -> Option<u8> {
    let o = rsoc_one(s, opcode, service_action).ok()?;
    o.data.get(1).map(|b| b & 0x07)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rsoc_descriptors_with_and_without_timeouts() {
        let mut d = vec![0, 0, 0, 28];
        d.extend_from_slice(&[0x28, 0, 0, 0, 0, 0x00, 0, 10]);
        d.extend_from_slice(&[0x9e, 0, 0, 0x10, 0, 0x03, 0, 16]);
        d.extend_from_slice(&[0; 12]);
        let list = parse_rsoc(&d);
        assert_eq!(list.len(), 2);
        assert_eq!(
            list[0],
            Supported {
                opcode: 0x28,
                service_action: None,
                cdb_len: 10
            }
        );
        assert_eq!(
            list[1],
            Supported {
                opcode: 0x9e,
                service_action: Some(0x10),
                cdb_len: 16
            }
        );
    }
}
