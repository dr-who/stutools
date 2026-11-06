//! The INQUIRY cases of the `disk` section (SPC-6 6.7, 7.7; SBC-5 6.6): standard INQUIRY data at the
//! allocation lengths hosts use, and every VPD page page 00h lists that a disk driver reads (Unit
//! Serial Number, Device Identification, Extended INQUIRY Data, Block Limits, Block Device
//! Characteristics, Logical Block Provisioning, SCSI Feature Sets and the others), each checked for
//! its page code, length and fields. What it reads goes into `Disk`.

use super::cases::{expect, record, Want};
use super::disk::{hex_list, label, Disk, SECTION};
use super::lun_access::vpd;
use super::port::Port;
use super::scsi_fields::be16;
use super::seen::Seen;
use super::vpd_pages::{names_lun, parse_designations};
use crate::compliance::{Case, ScsiLevel};

/// VPD pages read when page 00h lists them (SPC-6 7.7, SBC-5 6.6), Power Condition (8Ah) and Power
/// Consumption (8Dh) among them for the feature sets.
pub(super) const PAGES: [u8; 11] = [
    0x80, 0x83, 0x86, 0x87, 0x8a, 0x8d, 0x8f, 0x92, 0xb0, 0xb1, 0xb2,
];

/// The product serial number rule at `level`, and whether `serial` meets it. From SPC-4 on the field
/// is right-aligned ASCII, 20h to 7Eh with no NUL (SPC-6 7.7.19, 4.3.1); SPC-3 calls it ASCII data
/// that may end in one or more NULs (SPC-3 7.6.10, 4.4.1).
fn serial_rule(serial: &[u8], level: ScsiLevel) -> (&'static str, bool) {
    let printable = |b: &[u8]| !b.is_empty() && b.iter().all(|c| (0x20..=0x7e).contains(c));
    if level.spc() == 3 {
        let text = serial
            .iter()
            .rposition(|&c| c != 0)
            .map_or(&[][..], |end| &serial[..=end]);
        ("20h to 7Eh, NUL padding allowed", printable(text))
    } else {
        ("20h to 7Eh", printable(serial))
    }
}

/// SPC-6 6.7 INQUIRY, 7.7 VPD pages.
pub(super) fn inquiry(
    s: &mut Port,
    name: &str,
    levels: &[ScsiLevel],
    cases: &mut Vec<Case>,
    seen: &mut Seen,
    disk: &mut Disk,
) {
    let full = s.command(&[0x12, 0, 0, 0, 96, 0], 96, None);
    let answer = expect(
        cases,
        SECTION,
        label(name, "standard INQUIRY"),
        &[Want::Good],
        full,
    );
    seen.answer("INQUIRY", SECTION, &answer);
    let d = answer.1.map(|o| o.data).unwrap_or_default();
    // SPC-6 6.7.2: qualifier 000b, type 00h (direct access), RESPONSE DATA FORMAT 2h, at least 36 bytes.
    let at = |i: usize| d.get(i).copied().unwrap_or(0);
    let format_ok = at(0) == 0 && at(3) & 0x0f == 2 && at(4) >= 31;
    record(
        cases,
        SECTION,
        label(
            name,
            "standard INQUIRY: direct access, RESPONSE DATA FORMAT 2h, 36+ bytes",
        ),
        "qualifier 0, type 00h, format 2h, ADDITIONAL LENGTH >= 31",
        format!(
            "byte 0 {:02x}h, format {:x}h, ADDITIONAL LENGTH {}",
            at(0),
            at(3) & 0x0f,
            at(4)
        ),
        format_ok,
    );
    // SPC-6 table 152: 05h SPC-3, 06h SPC-4, 07h SPC-5, 0Dh SPC-6. Windows MPIO and clustering need
    // SPC-3 or later.
    record(
        cases,
        SECTION,
        label(name, "standard INQUIRY VERSION"),
        "05h, 06h, 07h or 0Dh (SPC-3 or later)",
        format!("{:02x}h", at(2)),
        matches!(at(2), 5..=7 | 0x0d),
    );
    // SPC-6 6.7.2: the CMDQUE bit shall be set to one.
    record(
        cases,
        SECTION,
        label(name, "standard INQUIRY CMDQUE"),
        "1",
        format!("{}", (at(7) >> 1) & 1),
        at(7) & 0x02 != 0,
    );
    // SPC-6 4.3.1: ASCII data fields hold 20h to 7Eh.
    let ascii = d
        .get(8..36)
        .is_some_and(|f| f.iter().all(|b| (0x20..=0x7e).contains(b)));
    record(
        cases,
        SECTION,
        label(name, "vendor, product and revision are printable ASCII"),
        "20h to 7Eh",
        super::scsi_fields::printable(d.get(8..36).unwrap_or(&[])),
        ascii,
    );
    // SPC-6 4.2.5.6: a shorter allocation length returns the start of the same data.
    let short = s.command(&[0x12, 0, 0, 0, 36, 0], 36, None);
    let same = matches!(&short, Ok(o) if o.status == 0 && d.len() >= 36 && o.data[..36] == d[..36]);
    record(
        cases,
        SECTION,
        label(name, "36-byte INQUIRY matches the start of the full one"),
        "GOOD, same 36 bytes",
        match &short {
            Ok(o) if o.status == 0 => {
                if same {
                    "same".into()
                } else {
                    "different".into()
                }
            }
            Ok(o) => super::cases::describe(o),
            Err(e) => e.clone(),
        },
        same,
    );
    disk.inquiry = d;

    // SPC-6 7.7.17: page 00h lists the supported pages in ascending order, 00h, 80h and 83h among them.
    let list = vpd(s, 0x00, 255);
    let (_, list) = expect(
        cases,
        SECTION,
        label(name, "VPD 00h supported pages"),
        &[Want::Good],
        list,
    );
    let raw = list.map(|o| o.data).unwrap_or_default();
    let n = raw.get(3).copied().unwrap_or(0) as usize;
    disk.pages = raw.get(4..4 + n).unwrap_or(&[]).to_vec();
    let sorted = disk.pages.windows(2).all(|w| w[0] < w[1]);
    record(
        cases,
        SECTION,
        label(name, "VPD 00h ascending, with 00h, 80h and 83h"),
        "ascending; 00h 80h 83h",
        hex_list(&disk.pages),
        sorted && [0x00, 0x80, 0x83].iter().all(|p| disk.pages.contains(p)),
    );
    for page in PAGES {
        if !disk.pages.contains(&page) {
            continue;
        }
        let o = vpd(s, page, 4096);
        let (passed, o) = expect(
            cases,
            SECTION,
            label(name, &format!("VPD {page:02x}h")),
            &[Want::Good],
            o,
        );
        if let Some(o) = o.filter(|o| o.status == 0) {
            let len = 4 + be16(&o.data, 2) as usize;
            let ok = o.data.get(1) == Some(&page);
            // SBC-5 6.6.2 and 6.6.4: Block Device Characteristics and Block Limits are 003Ch long;
            // SPC-6 7.7.7: Extended INQUIRY Data is 003Ch long.
            let fixed = match page {
                0xb0 | 0xb1 | 0x86 => len == 0x40,
                0xb2 => len >= 8,
                _ => true,
            };
            if !(ok && fixed) {
                record(
                    cases,
                    SECTION,
                    label(name, &format!("VPD {page:02x}h page code and length")),
                    "as the standard sets",
                    format!(
                        "page {:02x}h, length {}",
                        o.data.get(1).copied().unwrap_or(0),
                        len - 4
                    ),
                    false,
                );
            }
            let data = o.data[..len.min(o.data.len())].to_vec();
            disk.vpd.push((page, data));
        }
        let _ = passed;
    }
    // SPC-6 6.7.1: a VPD page the device server does not support is an invalid field in the CDB.
    let unlisted = [0xb1u8, 0xb2, 0xb0, 0x86, 0x87, 0x92, 0x8f]
        .into_iter()
        .find(|p| !disk.pages.contains(p))
        .unwrap_or(0xfe);
    if !disk.pages.contains(&unlisted) {
        expect(
            cases,
            SECTION,
            label(name, &format!("unlisted VPD {unlisted:02x}h refused")),
            &[Want::Ascq(0x5, 0x24, 0x00)],
            vpd(s, unlisted, 255),
        );
    }
    if let Some(d) = disk.page(0x80) {
        let serial = d.get(4..).unwrap_or(&[]);
        for &level in levels {
            let (expected, passed) = serial_rule(serial, level);
            cases.push(
                crate::compliance::case(
                    SECTION,
                    label(
                        name,
                        "SPC-6 7.7.19 VPD 80h serial number is printable ASCII",
                    ),
                    expected.into(),
                    super::scsi_fields::printable(serial),
                    passed,
                )
                .at(level),
            );
        }
    }
    // SPC-6 7.7.6: the descriptors fill the page; one names the LUN uniquely.
    if let Some(d) = disk.page(0x83) {
        let (list, fills) = parse_designations(d);
        record(
            cases,
            SECTION,
            label(name, "VPD 83h descriptors fill the page"),
            "exactly",
            if fills { "exactly" } else { "not" },
            fills,
        );
        disk.unique_id = list.iter().any(names_lun);
        record(
            cases,
            SECTION,
            label(
                name,
                "VPD 83h names the LUN uniquely (NAA, EUI-64 or SCSI name)",
            ),
            "an association 0 designator",
            list.iter()
                .filter(|d| d.association == 0)
                .map(|d| format!("type {:x}h", d.kind))
                .collect::<Vec<_>>()
                .join(", "),
            disk.unique_id,
        );
    }
    // SBC-5 6.6.4: OPTIMAL TRANSFER LENGTH is at most MAXIMUM TRANSFER LENGTH when both are reported.
    if let Some(l) = disk.limits() {
        record(
            cases,
            SECTION,
            label(name, "VPD B0h optimal transfer within the maximum"),
            "optimal <= maximum",
            format!("optimal {}, maximum {}", l.optimal_transfer, l.max_transfer),
            l.max_transfer == 0 || l.optimal_transfer <= l.max_transfer,
        );
    }
    // SBC-5 6.6.2: MEDIUM ROTATION RATE 0000h (not reported), 0001h (non-rotating), or 0401h to FFFEh.
    if let Some(d) = disk.page(0xb1) {
        let rate = be16(d, 4);
        record(
            cases,
            SECTION,
            label(name, "VPD B1h medium rotation rate"),
            "0000h, 0001h or 0401h to FFFEh",
            format!("{rate:04x}h"),
            matches!(rate, 0 | 1 | 0x401..=0xfffe),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spc_3_lets_a_serial_number_end_in_nuls_and_later_editions_do_not() {
        let level = |n| ScsiLevel::new(n).unwrap();
        let padded = b"b516f489\0";
        assert!(serial_rule(padded, level(3)).1);
        assert!(!serial_rule(padded, level(4)).1);
        assert!(serial_rule(b"b516f489", level(6)).1);
        assert!(!serial_rule(b"\0\0", level(3)).1);
        assert!(!serial_rule(b"a\0b", level(3)).1);
    }
}
