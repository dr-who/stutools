//! REPORT TARGET PORT GROUPS as the `mpio` cases send and read it (SPC-6 6.34, table 272): the length
//! only and the extended header formats, the target port group descriptors in either (ID, state,
//! supported-state bits, member relative target ports), and the supported-state bit for each
//! asymmetric access state (table 273).

use super::scsi_fields::{be16, be32};
use super::session::{Outcome, Session};
use crate::Result;

/// One target port group descriptor (SPC-6 table 272).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Group {
    pub id: u16,
    pub state: u8,
    pub supported: u8,
    pub ports: Vec<u16>,
}

/// REPORT TARGET PORT GROUPS, in the length only format or, with `extended`, asking for the extended
/// header (PARAMETER DATA FORMAT 001b, SPC-6 table 268).
pub(super) fn rtpg(s: &mut Session, extended: bool) -> Result<Outcome> {
    let a = 4096u32.to_be_bytes();
    let cdb = [
        0xa3,
        if extended { 0x2a } else { 0x0a },
        0,
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
    s.command(&cdb, 4096, None)
}

/// The groups in RTPG data. The extended header (RTPG_FMT 001b in byte 4) is 8 bytes, the length only
/// header 4 (SPC-5 tables 271, 272).
pub(super) fn parse_groups(d: &[u8]) -> (bool, Vec<Group>) {
    let extended = d.get(4).is_some_and(|b| (b >> 4) & 0x7 == 1);
    let end = (4 + be32(d, 0) as usize).min(d.len());
    let mut i = if extended { 8 } else { 4 };
    let mut out = Vec::new();
    while i + 8 <= end {
        let count = d[i + 7] as usize;
        out.push(Group {
            id: be16(d, i + 2),
            state: d[i] & 0x0f,
            supported: d[i + 1],
            ports: (0..count).map(|p| be16(d, i + 8 + 4 * p + 2)).collect(),
        });
        i += 8 + 4 * count;
    }
    (extended, out)
}

/// The supported-state bit for a state (SPC-6 table 272): AO_SUP, AN_SUP, S_SUP, U_SUP, LBD_SUP, O_SUP,
/// T_SUP.
pub(super) fn state_bit(state: u8) -> u8 {
    match state {
        0x0 => 0x01,
        0x1 => 0x02,
        0x2 => 0x04,
        0x3 => 0x08,
        0x4 => 0x10,
        0xe => 0x40,
        0xf => 0x80,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(id: u16, state: u8, supported: u8, ports: &[u16]) -> Vec<u8> {
        let mut d = vec![
            state,
            supported,
            (id >> 8) as u8,
            id as u8,
            0,
            0,
            0,
            ports.len() as u8,
        ];
        for p in ports {
            d.extend_from_slice(&[0, 0, (p >> 8) as u8, *p as u8]);
        }
        d
    }

    #[test]
    fn groups_parse_in_both_header_formats() {
        let mut body = descriptor(0, 0x0, 0x0f, &[1]);
        body.extend(descriptor(1, 0x2, 0x0f, &[2, 3]));
        let mut short = ((body.len()) as u32).to_be_bytes().to_vec();
        short.extend_from_slice(&body);
        let (ext, groups) = parse_groups(&short);
        assert!(!ext);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[1].ports, vec![2, 3]);
        let mut long = ((body.len() + 4) as u32).to_be_bytes().to_vec();
        long.extend_from_slice(&[0x10, 5, 0, 0]);
        long.extend_from_slice(&body);
        let (ext, again) = parse_groups(&long);
        assert!(ext);
        assert_eq!(again, groups);
    }

    #[test]
    fn supported_state_bits_follow_table_273() {
        assert_eq!(state_bit(0x0), 0x01);
        assert_eq!(state_bit(0x2), 0x04);
        assert_eq!(state_bit(0xe), 0x40, "offline is O_SUP");
        assert_eq!(state_bit(0xf), 0x80, "transitioning is T_SUP");
        assert_eq!(state_bit(0x5), 0);
    }
}
