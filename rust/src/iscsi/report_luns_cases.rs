//! The REPORT LUNS cases of the `spc` section that Windows' scan and the Microsoft SCSI compliance tests
//! make (SPC-6 6.30, 4.2.5.6): each SELECT REPORT value, 00h (the logical units by the usual addressing
//! methods), 01h (well known logical units only, the W-LUN form), 02h (every logical unit, at least
//! those of 00h), and the administrative selections 10h, 11h and 12h SPC-4 added (an empty list unless
//! the command reached LUN 0, the REPORT LUNS well known logical unit or an administrative logical unit;
//! a device server without them may refuse them as INVALID FIELD IN CDB); the allocation lengths
//! Windows uses (16) and the bare header (8, which SPC-3 let a device server refuse); and LUN LIST
//! LENGTH a multiple of 8 never cut by the allocation length. Returns the 00h list for the cases that
//! address each logical unit.

use super::cases::{describe, expect, introduced_in, record, residual_not_reported, Want};
use super::port::Port;
use super::scsi_fields::be32;
use super::session::Outcome;
use crate::compliance::{Case, Edition};
use crate::Result;

const SECTION: &str = "spc";

/// REPORT LUNS with `select` and `alloc` bytes of allocation.
pub(super) fn report_luns(s: &mut Port, select: u8, alloc: u32) -> Result<Outcome> {
    let a = alloc.to_be_bytes();
    s.command(
        &[0xa0, 0, select, 0, 0, 0, a[0], a[1], a[2], a[3], 0, 0],
        alloc,
        None,
    )
}

/// The LUN LIST LENGTH and the 8-byte LUNs of a REPORT LUNS answer, as far as it reaches.
pub(super) fn parse_luns(d: &[u8]) -> (usize, Vec<[u8; 8]>) {
    let len = be32(d, 0) as usize;
    let luns = d
        .get(8..(8 + len).min(d.len()))
        .unwrap_or(&[])
        .chunks_exact(8)
        .map(|c| c.try_into().unwrap())
        .collect();
    (len, luns)
}

/// The single-level LUN number of an 8-byte LUN (SAM-6 4.7.7): peripheral device (00b) or flat space
/// (01b) addressing with nothing past the first level; None otherwise.
pub(super) fn single_level(lun: &[u8; 8]) -> Option<u16> {
    if lun[2..].iter().any(|&b| b != 0) {
        return None;
    }
    match lun[0] >> 6 {
        0b00 if lun[0] == 0 => Some(u16::from(lun[1])),
        0b01 => Some(u16::from_be_bytes([lun[0] & 0x3f, lun[1]])),
        _ => None,
    }
}

/// A well known logical unit's LUN (SAM-6 4.7.7.6): extended addressing, C1h in byte 0.
pub(super) fn well_known(lun: &[u8; 8]) -> bool {
    lun[0] == 0xc1
}

fn show(luns: &[[u8; 8]]) -> String {
    if luns.is_empty() {
        return "none".into();
    }
    luns.iter()
        .map(|l| match single_level(l) {
            Some(n) => n.to_string(),
            None => l.iter().map(|b| format!("{b:02x}")).collect::<String>(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Runs the REPORT LUNS cases and returns the SELECT REPORT 00h list.
pub(super) fn run(s: &mut Port, name: &str, at_lun_0: bool, cases: &mut Vec<Case>) -> Vec<[u8; 8]> {
    let at = |what: &str| format!("{name} SPC-6 6.30 REPORT LUNS {what}");
    let answer = |o: &Option<Outcome>| {
        o.as_ref()
            .filter(|o| o.status == 0)
            .map(|o| o.data[..o.received.min(o.data.len())].to_vec())
    };
    let (_, o) = expect(
        cases,
        SECTION,
        at("SELECT REPORT 00h"),
        &[Want::Good],
        report_luns(s, 0, 4096),
    );
    let Some(d) = answer(&o) else {
        return Vec::new();
    };
    let (len, all) = parse_luns(&d);
    record(
        cases,
        SECTION,
        at("SELECT REPORT 00h: LUN LIST LENGTH 8 per LUN, every LUN returned"),
        "a multiple of 8; 8 + LUN LIST LENGTH bytes",
        if residual_not_reported(d.len(), (8 + len).min(4096)) {
            format!(
                "not run: the transport reports the whole allocation as returned ({} bytes)",
                d.len()
            )
        } else {
            format!("LUN LIST LENGTH {len}, {} bytes", d.len())
        },
        len % 8 == 0 && d.len() == (8 + len).min(4096),
    );
    // SPC-6 table 250: 01h lists only well known logical units.
    let (_, o) = expect(
        cases,
        SECTION,
        at("SELECT REPORT 01h"),
        &[Want::Good],
        report_luns(s, 1, 4096),
    );
    if let Some(d) = answer(&o) {
        let (_, luns) = parse_luns(&d);
        record(
            cases,
            SECTION,
            at("SELECT REPORT 01h: well known logical units only"),
            "none, or W-LUNs (C1h)",
            show(&luns),
            luns.iter().all(well_known),
        );
    }
    // 02h lists every logical unit the nexus reaches: at least those of 00h.
    let (_, o) = expect(
        cases,
        SECTION,
        at("SELECT REPORT 02h"),
        &[Want::Good],
        report_luns(s, 2, 4096),
    );
    if let Some(d) = answer(&o) {
        let (_, luns) = parse_luns(&d);
        let missing: Vec<[u8; 8]> = all.iter().filter(|l| !luns.contains(l)).copied().collect();
        record(
            cases,
            SECTION,
            at("SELECT REPORT 02h: every LUN of 00h"),
            "every LUN of SELECT REPORT 00h",
            if missing.is_empty() {
                show(&luns)
            } else {
                format!("missing {}", show(&missing))
            },
            missing.is_empty(),
        );
    }
    // SPC-4 added 10h, 11h and 12h; outside LUN 0 and administrative logical units the list is empty.
    for select in [0x10u8, 0x11, 0x12] {
        let (_, o) = expect(
            cases,
            SECTION,
            at(&format!("SELECT REPORT {select:02x}h")),
            &[Want::Good, Want::Ascq(0x5, 0x24, 0x00)],
            report_luns(s, select, 4096),
        );
        introduced_in(cases, Edition::Spc(4));
        let Some(d) = answer(&o) else {
            continue;
        };
        let (len, luns) = parse_luns(&d);
        // 11h from LUN 0 lists the single-level LUNs; 10h and 12h list administrative units, which a
        // disk is not. Elsewhere every one of them is empty.
        let ok = match select {
            0x11 if at_lun_0 => luns
                .iter()
                .all(|l| single_level(l).is_some() || well_known(l)),
            0x10 if at_lun_0 => true,
            _ => len == 0,
        };
        record(
            cases,
            SECTION,
            at(&format!(
                "SELECT REPORT {select:02x}h: the units it selects"
            )),
            if select == 0x11 && at_lun_0 {
                "single-level LUNs only".to_string()
            } else if select == 0x10 && at_lun_0 {
                "administrative logical units only".to_string()
            } else {
                "LUN LIST LENGTH 0 (not sent to LUN 0 or an administrative logical unit)"
                    .to_string()
            },
            show(&luns),
            ok,
        );
        introduced_in(cases, Edition::Spc(4));
    }
    // SPC-6 4.2.5.6: a short allocation length cuts the list, not LUN LIST LENGTH. Windows asks for 16.
    for alloc in [16u32, 8] {
        let mut wants = vec![Want::Good];
        if alloc < 16 {
            // SPC-3 6.21 note 30: an older device server refuses less than 16 bytes.
            wants.push(Want::Ascq(0x5, 0x24, 0x00));
        }
        let o = report_luns(s, 0, alloc);
        let (actual, passed) = match &o {
            Ok(o) if o.status == 0 => {
                let got = &o.data[..o.received.min(o.data.len())];
                let (n, _) = parse_luns(got);
                (
                    format!("{} bytes, LUN LIST LENGTH {n}", got.len()),
                    got.len() == (8 + len).min(alloc as usize)
                        && n == len
                        && d.get(..got.len()) == Some(got),
                )
            }
            Ok(o) => (describe(o), wants.iter().any(|w| w.matches(o))),
            Err(e) => (e.clone(), false),
        };
        record(
            cases,
            SECTION,
            format!("{name} SPC-6 4.2.5.6 REPORT LUNS at allocation {alloc}"),
            if alloc < 16 {
                format!("GOOD, the first 8 bytes, LUN LIST LENGTH {len}; or CHECK CONDITION 5/24/00 (SPC-3)")
            } else {
                format!(
                    "GOOD, the first {} bytes, LUN LIST LENGTH {len}",
                    (8 + len).min(16)
                )
            },
            actual,
            passed,
        );
    }
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn luns_parse_and_name_their_level() {
        let mut d = vec![0, 0, 0, 16, 0, 0, 0, 0];
        d.extend_from_slice(&[0, 1, 0, 0, 0, 0, 0, 0]);
        d.extend_from_slice(&[0x41, 0x2c, 0, 0, 0, 0, 0, 0]);
        let (len, luns) = parse_luns(&d);
        assert_eq!(len, 16);
        assert_eq!(single_level(&luns[0]), Some(1));
        assert_eq!(single_level(&luns[1]), Some(300));
        assert_eq!(single_level(&[0, 1, 0, 1, 0, 0, 0, 0]), None, "two levels");
        assert!(well_known(&[0xc1, 0x01, 0, 0, 0, 0, 0, 0]));
        assert_eq!(show(&luns), "1, 300");
        assert_eq!(parse_luns(&d[..12]).1.len(), 0, "a LUN cut by the data");
    }
}
