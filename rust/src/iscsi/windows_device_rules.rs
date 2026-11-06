//! What Microsoft asks of a disk beyond the SCSI standards, in the `windows` section (the HLK SCSI
//! compliance tests and the WHCP Device.Storage requirements): each a host requirement that only warns,
//! since no standard makes it a "shall". From what the other sections read: VPD B1h MEDIUM ROTATION
//! RATE reported (Microsoft rejects 0000h, which SBC-5 6.6.2 allows as "not reported"); the Caching
//! page saveable (PS 1); the Control (0Ah), Power Condition (1Ah) and Informational Exceptions Control
//! (1Ch) mode pages present; HISUP set in standard INQUIRY; on iSCSI, an iSCSI version descriptor
//! (0960h block); and every LUN of REPORT LUNS a single-level LUN of 255 or less (peripheral or flat
//! addressing), the only kind Windows addresses.

use super::cases::record_should;
use super::disk::Disk;
use super::report_luns_cases::single_level;
use super::scsi_fields::be16;
use super::version_descriptors::{descriptors, ISCSI};
use crate::compliance::Case;

const SECTION: &str = "windows";

/// The LUNs of `luns` Windows cannot address: not single level, or above 255.
pub(super) fn unaddressable(luns: &[[u8; 8]]) -> Vec<String> {
    luns.iter()
        .filter(|l| single_level(l).is_none_or(|n| n > 255))
        .map(|l| l.iter().map(|b| format!("{b:02x}")).collect())
        .collect()
}

pub(super) fn run(name: &str, disk: &Disk, luns: &[[u8; 8]], iscsi: bool, cases: &mut Vec<Case>) {
    let at = |what: &str| format!("{name} Microsoft: {what}");
    if let Some(d) = disk.page(0xb1) {
        let rate = be16(d, 4);
        record_should(
            cases,
            SECTION,
            at("VPD B1h MEDIUM ROTATION RATE reported (not 0000h)"),
            "0001h (non-rotating) or a rate; SBC-5 6.6.2 allows 0000h, Windows does not",
            format!("{rate:04x}h"),
            rate != 0,
        );
    }
    if let Some(p) = disk.mode_page(0x08, 0) {
        record_should(
            cases,
            SECTION,
            at("Caching mode page saveable (PS 1)"),
            "PS 1",
            format!("PS {}", p[0] >> 7),
            p[0] & 0x80 != 0,
        );
    }
    if !disk.mode_list.is_empty() {
        for (page, what) in [
            (0x0au8, "Control mode page (0Ah) present"),
            (0x1a, "Power Condition mode page (1Ah) present"),
            (
                0x1c,
                "Informational Exceptions Control mode page (1Ch) present",
            ),
        ] {
            let has = disk.mode_list.contains(&(page, 0));
            record_should(
                cases,
                SECTION,
                at(what),
                "listed by MODE SENSE page 3Fh",
                if has { "present" } else { "absent" },
                has,
            );
        }
    }
    let hisup = disk.inquiry.get(3).is_some_and(|b| b & 0x10 != 0);
    record_should(
        cases,
        SECTION,
        at("standard INQUIRY HISUP set"),
        "1",
        format!("{}", u8::from(hisup)),
        hisup,
    );
    if iscsi {
        let additional = disk.inquiry.get(4).copied().unwrap_or(0) as usize;
        let codes = if additional + 5 >= 74 {
            descriptors(&disk.inquiry)
        } else {
            Vec::new()
        };
        let has = codes.iter().any(|c| c & 0xffe0 == ISCSI);
        record_should(
            cases,
            SECTION,
            at("an iSCSI version descriptor (0960h block) in standard INQUIRY"),
            "a code from 0960h to 097Fh",
            codes
                .iter()
                .filter(|&&c| c != 0)
                .map(|c| format!("{c:04x}h"))
                .collect::<Vec<_>>()
                .join(" "),
            has,
        );
    }
    if !luns.is_empty() {
        let bad = unaddressable(luns);
        record_should(
            cases,
            SECTION,
            at("REPORT LUNS lists single-level LUNs of 255 or less"),
            "peripheral or flat addressing, one level, LUN <= 255",
            if bad.is_empty() {
                format!("all {} addressable", luns.len())
            } else {
                format!("not addressable: {}", bad.join(", "))
            },
            bad.is_empty(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_addresses_single_level_luns_to_255() {
        let ok = [[0u8, 1, 0, 0, 0, 0, 0, 0], [0x40, 0xff, 0, 0, 0, 0, 0, 0]];
        assert!(unaddressable(&ok).is_empty());
        let big = [[0x41u8, 0x00, 0, 0, 0, 0, 0, 0]];
        assert_eq!(unaddressable(&big).len(), 1);
        let two_level = [[0u8, 1, 0, 1, 0, 0, 0, 0]];
        assert_eq!(unaddressable(&two_level).len(), 1);
    }
}
