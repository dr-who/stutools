//! The `spc` cases for logical unit numbers REPORT LUNS does not list, which Linux and Windows scan and
//! VMware ESXi takes as permanently lost when they answer LOGICAL UNIT NOT SUPPORTED (SAM-6 5.11; SPC-6
//! 6.7.2, 6.30, 6.36). The first single-level LUN not listed, and LUN 255 when it is not listed either,
//! are addressed on a session of their own: INQUIRY returns data with the peripheral qualifier 011b and
//! device type 1Fh (or 001b, a unit the target could reach but not now); with 011b, TEST UNIT READY and
//! READ (10) are refused as 5/25/00, REQUEST SENSE returns sense data saying 5/25/00, and REPORT LUNS
//! returns the inventory or 5/25/00. Every listed LUN in turn answers INQUIRY with a qualifier other
//! than 011b, and never answers TEST UNIT READY with 25h. iSCSI only: a local path device reaches one
//! logical unit.

use super::block_io::cdb10;
use super::cases::{describe, expect, record, Want};
use super::lun_access::{open, test_unit_ready, unit_attentions, Lun};
use super::path::Path;
use super::port::Port;
use super::report_luns_cases::{report_luns, single_level};
use crate::compliance::Case;

const SECTION: &str = "spc";

/// LOGICAL UNIT NOT SUPPORTED (SPC-6 table 61).
const NOT_SUPPORTED: Want = Want::Ascq(0x5, 0x25, 0x00);

/// The first single-level LUN below 256 that `listed` lacks, and 255 when it lacks that too.
pub(super) fn unlisted(listed: &[u16]) -> Vec<u16> {
    let mut out: Vec<u16> = (0..256).find(|n| !listed.contains(n)).into_iter().collect();
    if !listed.contains(&255) && !out.contains(&255) {
        out.push(255);
    }
    out
}

fn session(lun: Lun, number: u16) -> Option<Port> {
    let path = Path {
        lun: number,
        ..lun.path.clone()
    };
    let mut s = open(&path, lun.initiator, lun.creds, super::login::random_isid()).ok()?;
    let _ = unit_attentions(&mut s);
    Some(s)
}

pub(super) fn run(lun: Lun, name: &str, luns: &[[u8; 8]], block_len: u32, cases: &mut Vec<Case>) {
    if lun.path.is_local() {
        return;
    }
    let listed: Vec<u16> = luns.iter().filter_map(single_level).collect();
    for number in unlisted(&listed) {
        let at =
            |clause: &str, what: &str| format!("{name} {clause} unlisted LUN {number}: {what}");
        let Some(mut s) = session(lun, number) else {
            record(
                cases,
                SECTION,
                at("SAM-6 5.11", "login"),
                "login ok",
                "refused",
                false,
            );
            continue;
        };
        let o = s.command(&[0x12, 0, 0, 0, 36, 0], 36, None);
        let byte0 = match &o {
            Ok(o) if o.status == 0 => o.data.first().copied(),
            _ => None,
        };
        // SPC-6 6.7.2, 6.7.1: INQUIRY returns data; 7Fh when the target cannot reach the unit.
        record(
            cases,
            SECTION,
            at(
                "SPC-6 6.7.2",
                "INQUIRY peripheral qualifier 011b, device type 1Fh",
            ),
            "GOOD, byte 0 7Fh (or qualifier 001b)",
            match (&o, byte0) {
                (_, Some(b)) => format!("byte 0 {b:02x}h"),
                (Ok(o), None) => describe(o),
                (Err(e), None) => e.clone(),
            },
            byte0.is_some_and(|b| b == 0x7f || b >> 5 == 0b001),
        );
        if byte0 != Some(0x7f) {
            let _ = s.logout();
            continue;
        }
        // SAM-6 5.11 a): any other command is LOGICAL UNIT NOT SUPPORTED.
        expect(
            cases,
            SECTION,
            at("SAM-6 5.11", "TEST UNIT READY"),
            &[NOT_SUPPORTED],
            test_unit_ready(&mut s),
        );
        expect(
            cases,
            SECTION,
            at("SAM-6 5.11", "READ (10)"),
            &[NOT_SUPPORTED],
            s.command(&cdb10(0x28, 0, 0, 1), block_len, None),
        );
        // SPC-6 6.36: REQUEST SENSE returns sense data saying LOGICAL UNIT NOT SUPPORTED.
        let o = s.command(&[0x03, 0, 0, 0, 252, 0], 252, None);
        let (actual, passed) = match &o {
            Ok(o) if o.status == 0 => {
                let k = super::cases::sense(&o.data);
                (
                    format!("GOOD, sense {:x}/{:02x}/{:02x}", k.0, k.1, k.2),
                    k == (0x5, 0x25, 0x00),
                )
            }
            Ok(o) => (describe(o), false),
            Err(e) => (e.clone(), false),
        };
        record(
            cases,
            SECTION,
            at("SPC-6 6.36", "REQUEST SENSE"),
            "GOOD, sense data 5/25/00",
            actual,
            passed,
        );
        // SPC-6 6.30: the inventory, or 5/25/00 when the device server cannot return it.
        expect(
            cases,
            SECTION,
            at("SPC-6 6.30", "REPORT LUNS"),
            &[Want::Good, NOT_SUPPORTED],
            report_luns(&mut s, 0, 4096),
        );
        let _ = s.logout();
    }
    // SPC-6 6.30: no listed LUN has the qualifier 011b; SAM-6 5.11: none answers as if it were missing.
    for &number in listed.iter().take(8) {
        let at = |what: &str| format!("{name} SPC-6 6.30 listed LUN {number}: {what}");
        let Some(mut s) = session(lun, number) else {
            continue;
        };
        let o = s.command(&[0x12, 0, 0, 0, 36, 0], 36, None);
        let (actual, passed) = match &o {
            Ok(o) if o.status == 0 => {
                let b = o.data.first().copied().unwrap_or(0);
                (format!("byte 0 {b:02x}h"), b >> 5 != 0b011)
            }
            Ok(o) => (describe(o), false),
            Err(e) => (e.clone(), false),
        };
        record(
            cases,
            SECTION,
            at("INQUIRY qualifier not 011b"),
            "GOOD, qualifier 000b or 001b",
            actual,
            passed,
        );
        let tur = test_unit_ready(&mut s);
        let (actual, passed) = match &tur {
            Ok(o) => (
                describe(o),
                !(o.status == 2 && super::cases::sense(&o.sense).1 == 0x25),
            ),
            Err(e) => (e.clone(), false),
        };
        record(
            cases,
            SECTION,
            at("TEST UNIT READY not LOGICAL UNIT NOT SUPPORTED"),
            "no ASC 25h",
            actual,
            passed,
        );
        let _ = s.logout();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_unlisted_luns_are_the_first_gap_and_255() {
        assert_eq!(unlisted(&[0, 1]), vec![2, 255]);
        assert_eq!(unlisted(&[1]), vec![0, 255]);
        assert_eq!(unlisted(&[0, 255]), vec![1]);
        let all: Vec<u16> = (0..255).collect();
        assert_eq!(unlisted(&all), vec![255]);
    }
}
