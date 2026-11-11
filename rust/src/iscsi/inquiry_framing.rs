//! The standard INQUIRY framing cases of the `disk` section that the Microsoft SCSI compliance tests
//! check (SPC-6 6.7.1, 6.7.2, 4.2.5.6): a PAGE CODE with EVPD 0 refused as INVALID FIELD IN CDB;
//! ADDITIONAL LENGTH counting the bytes returned; every allocation length from 16 to 240 returning
//! exactly that many bytes of the same data; the VERSION DESCRIPTOR fields holding table 155 codes;
//! and MULTIP set when the logical unit is seen through more than one target port. The target ports
//! are counted from what the paths spit was given report (`target_ports`): the relative target port
//! identifiers of VPD 83h through every iSCSI path to the LUN, or the SAS ports of a local disk's
//! paths. Fewer ports seen than the device has only informs.

use super::cases::{describe, expect, record, record_info, residual_not_reported, Want};
use super::disk::{label, Disk, SECTION};
use super::lun_access::{open, vpd, Creds};
use super::path::Path;
use super::port::Port;
use super::version_descriptors::{descriptors, known};
use super::vpd_pages::parse_designations;
use crate::compliance::Case;
use std::collections::BTreeSet;

/// The distinct target ports `paths` (one LUN through several portals) reach: the relative target port
/// identifiers of VPD 83h (association 1, type 4h), or for a local disk the SAS ports of its paths.
pub(super) fn target_ports(paths: &[&Path], initiator: &str, creds: Creds) -> usize {
    let mut ports = BTreeSet::new();
    for path in paths {
        if path.is_local() {
            return super::local_nexus::target_ports(&path.target).unwrap_or(1);
        }
        let Ok(mut s) = open(path, initiator, creds, super::login::random_isid()) else {
            continue;
        };
        let _ = super::lun_access::unit_attentions(&mut s);
        if let Some(o) = vpd(&mut s, 0x83, 1024).ok().filter(|o| o.status == 0) {
            let (list, _) = parse_designations(&o.data);
            ports.extend(
                list.into_iter()
                    .filter(|d| d.association == 1 && d.kind == 4)
                    .map(|d| d.value),
            );
        }
        let _ = s.logout();
    }
    ports.len().max(1)
}

/// The allocation lengths of the truncation case, from which answer bytes the failures were.
pub(super) fn truncation_misses(full: &[u8], answers: &[(usize, Vec<u8>)]) -> Vec<usize> {
    answers
        .iter()
        .filter(|(n, got)| {
            got.len() != (*n).min(full.len()) || full.get(..got.len()) != Some(&got[..])
        })
        .map(|(n, _)| *n)
        .collect()
}

pub(super) fn run(s: &mut Port, name: &str, disk: &Disk, ports: usize, cases: &mut Vec<Case>) {
    let at = |clause: &str, what: &str| label(name, &format!("{clause} {what}"));
    // SPC-6 6.7.1: a PAGE CODE other than zero with EVPD zero is an invalid field in the CDB.
    for page in [0x80u8, 0xff] {
        expect(
            cases,
            SECTION,
            at(
                "SPC-6 6.7.1",
                &format!("INQUIRY with EVPD 0 and PAGE CODE {page:02x}h refused"),
            ),
            &[Want::Ascq(0x5, 0x24, 0x00)],
            s.command(&[0x12, 0, page, 0, 255, 0], 255, None),
        );
    }
    // SPC-6 6.7.2, 4.2.5.6: ADDITIONAL LENGTH is the bytes after byte 4, whatever the allocation length.
    let (full, full_received) = match s.command(&[0x12, 0, 0, 0, 255, 0], 255, None) {
        Ok(o) if o.status == 0 => (o.data[..o.received.min(o.data.len())].to_vec(), o.received),
        Ok(o) => {
            record(
                cases,
                SECTION,
                at("SPC-6 6.7.2", "standard INQUIRY at allocation 255"),
                "GOOD",
                describe(&o),
                false,
            );
            return;
        }
        Err(e) => {
            record(
                cases,
                SECTION,
                at("SPC-6 6.7.2", "standard INQUIRY at allocation 255"),
                "GOOD",
                e,
                false,
            );
            return;
        }
    };
    let additional = full.get(4).copied().unwrap_or(0) as usize;
    let want = (additional + 5).min(255);
    if residual_not_reported(full_received, full.len(), additional + 5) {
        record(
            cases,
            SECTION,
            at(
                "SPC-6 6.7.2",
                "standard INQUIRY ADDITIONAL LENGTH counts the bytes returned",
            ),
            format!("ADDITIONAL LENGTH + 5 = {want} bytes at allocation 255"),
            "not run: the transport reports the whole allocation as returned (no residual)",
            false,
        );
    } else {
        record(
            cases,
            SECTION,
            at(
                "SPC-6 6.7.2",
                "standard INQUIRY ADDITIONAL LENGTH counts the bytes returned",
            ),
            format!("ADDITIONAL LENGTH + 5 = {want} bytes at allocation 255"),
            format!("ADDITIONAL LENGTH {additional}, {} bytes", full.len()),
            full.len() == want,
        );
    }
    let mut answers = Vec::new();
    for n in (16..=240).step_by(16) {
        if let Ok(o) = s.command(&[0x12, 0, 0, 0, n as u8, 0], n as u32, None) {
            if o.status == 0 {
                answers.push((n, o.data[..o.received.min(o.data.len())].to_vec()));
                continue;
            }
        }
        answers.push((n, Vec::new()));
    }
    let misses = truncation_misses(&full, &answers);
    record(
        cases,
        SECTION,
        at(
            "SPC-6 4.2.5.6",
            "standard INQUIRY at allocation 16 to 240: the bytes asked for, the start of the whole",
        ),
        "min(allocation, ADDITIONAL LENGTH + 5) bytes, the same data",
        if misses.is_empty() {
            "all 15 exact".to_string()
        } else {
            format!("wrong at allocation {misses:?}")
        },
        misses.is_empty(),
    );
    // SPC-6 6.7.2: each VERSION DESCRIPTOR is a code from table 155 (0000h: unused).
    let codes = descriptors(&disk.inquiry);
    if additional + 5 >= 74 {
        let shown: Vec<String> = codes
            .iter()
            .filter(|&&c| c != 0)
            .map(|c| format!("{c:04x}h"))
            .collect();
        let unknown: Vec<String> = codes
            .iter()
            .filter(|&&c| c != 0 && !known(c))
            .map(|c| format!("{c:04x}h"))
            .collect();
        record(
            cases,
            SECTION,
            at(
                "SPC-6 6.7.2",
                "standard INQUIRY version descriptors are table 155 codes",
            ),
            "each non-zero descriptor a table 155 code",
            if unknown.is_empty() {
                if shown.is_empty() {
                    "none".to_string()
                } else {
                    shown.join(" ")
                }
            } else {
                format!("not in table 155: {}", unknown.join(" "))
            },
            unknown.is_empty(),
        );
    } else {
        record_info(
            cases,
            SECTION,
            at("SPC-6 6.7.2", "standard INQUIRY version descriptors"),
            format!("not returned (ADDITIONAL LENGTH {additional}: the data ends before byte 74)"),
        );
    }
    // SPC-6 6.7.2: MULTIP says whether the device has more than one target port.
    let multip = disk.inquiry.get(6).is_some_and(|b| b & 0x10 != 0);
    let what = at(
        "SPC-6 6.7.2",
        "standard INQUIRY MULTIP agrees with the target ports seen",
    );
    let actual = format!(
        "MULTIP {}, {ports} target port{} seen",
        u8::from(multip),
        if ports == 1 { "" } else { "s" }
    );
    if ports > 1 {
        record(
            cases,
            SECTION,
            what,
            "1 (more than one target port)",
            actual,
            multip,
        );
    } else {
        record_info(cases, SECTION, what, actual);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_truncated_answer_is_the_start_of_the_whole() {
        let full: Vec<u8> = (0..40).collect();
        let good = vec![(16, full[..16].to_vec()), (48, full.clone())];
        assert!(truncation_misses(&full, &good).is_empty());
        let bad = vec![
            (16, full[..15].to_vec()),
            (32, vec![9; 32]),
            (48, full[..36].to_vec()),
        ];
        assert_eq!(truncation_misses(&full, &bad), vec![16, 32, 48]);
    }
}
