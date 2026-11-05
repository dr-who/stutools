//! The `features` cases of `--compliance`: the SCSI feature sets, SPC-6 annex A (Discovery 2016) and
//! SBC-5 annex A, the same in SBC-4 annex H apart from wording (SBC Base 2010, SBC Base 2016, Basic
//! Provisioning 2016, Drive Maintenance 2016). Each requirement is judged from what the other sections
//! already found; nothing is sent here. A LUN claims a set by listing its code in the SCSI Feature Sets
//! VPD page (92h, SPC-6 7.7.14). One case per set: claimed and met passes, claimed and missed fails
//! with what is missing, and an unclaimed set is reported (informational) with what it would miss. SBC-3
//! defines no feature sets, so under `--sbc 3` the SBC sets are informational.

use super::cases::{record, record_info, Seen};
use super::disk::Disk;
use crate::compliance::Case;

const SECTION: &str = "features";

/// A feature set: its code (SPC-6 table A.1, SBC-5 table A.1), name, clause, and the first SBC edition
/// that defines it (None for the SPC set, which applies under every edition).
struct Set {
    code: u16,
    name: &'static str,
    clause: &'static str,
    sbc_min: Option<u8>,
}

const SETS: [Set; 5] = [
    Set {
        code: 0x0001,
        name: "Discovery 2016",
        clause: "SPC-6 A.2",
        sbc_min: None,
    },
    Set {
        code: 0x0102,
        name: "SBC Base 2010",
        clause: "SBC-5 A.2",
        sbc_min: Some(4),
    },
    Set {
        code: 0x0101,
        name: "SBC Base 2016",
        clause: "SBC-5 A.3",
        sbc_min: Some(4),
    },
    Set {
        code: 0x0103,
        name: "Basic Provisioning 2016",
        clause: "SBC-5 A.4",
        sbc_min: Some(4),
    },
    Set {
        code: 0x0104,
        name: "Drive Maintenance 2016",
        clause: "SBC-5 A.5",
        sbc_min: Some(4),
    },
];

/// One requirement and whether it is met.
type Requirement = (&'static str, bool);

fn sent_ok(seen: &Seen, command: &str) -> bool {
    seen.probe(command).is_some_and(|p| p.supported && p.passed)
}

fn listed(disk: &Disk, opcode: u8, sa: Option<u16>) -> bool {
    disk.lists(opcode, sa).unwrap_or(false)
}

/// SPC-6 A.2: Discovery 2016.
fn discovery(disk: &Disk, seen: &Seen) -> Vec<Requirement> {
    let ext = disk.extended();
    // A.2.4.1: a T10 vendor ID designator's vendor specific part is the product ID then the serial.
    let t10_ok = disk.page(0x83).is_none_or(|d| {
        let product = disk.inquiry.get(16..32).unwrap_or(&[]);
        let serial = disk.page(0x80).and_then(|p| p.get(4..)).unwrap_or(&[]);
        let mut want = product.to_vec();
        want.extend_from_slice(serial);
        super::disk::parse_designations(d)
            .0
            .iter()
            .filter(|x| x.kind == 1)
            .all(|x| x.value.get(8..) == Some(&want[..]))
    });
    vec![
        ("INQUIRY", sent_ok(seen, "INQUIRY")),
        ("MODE SENSE (10)", sent_ok(seen, "MODE SENSE (10)")),
        ("REPORT LUNS", sent_ok(seen, "REPORT LUNS")),
        (
            "REPORT SUPPORTED OPERATION CODES",
            sent_ok(seen, "REPORT SUPPORTED OPERATION CODES"),
        ),
        ("TEST UNIT READY", sent_ok(seen, "TEST UNIT READY")),
        ("Control mode page", disk.mode_page(0x0a, 0).is_some()),
        (
            "Control Extension mode page",
            disk.mode_page(0x0a, 0x01).is_some(),
        ),
        (
            "VPD 00h, 83h, 86h, 87h, 92h",
            [0x00, 0x83, 0x86, 0x87, 0x92]
                .iter()
                .all(|p| disk.pages.contains(p)),
        ),
        (
            "RESPONSE DATA FORMAT 2h",
            disk.inquiry.get(3).is_some_and(|b| b & 0x0f == 2),
        ),
        ("T10 vendor ID designator is product ID and serial", t10_ok),
        (
            "SIMPSUP and LUICLR",
            ext.is_some_and(|e| e.simpsup && e.luiclr),
        ),
        ("ABORT TASK", seen.has("TMF ABORT TASK") == Some(true)),
        (
            "ABORT TASK SET",
            seen.has("TMF ABORT TASK SET") == Some(true),
        ),
        (
            "CLEAR TASK SET",
            seen.has("TMF CLEAR TASK SET") == Some(true),
        ),
        (
            "LOGICAL UNIT RESET",
            seen.has("TMF LOGICAL UNIT RESET") == Some(true),
        ),
    ]
}

/// What SBC Base 2010 and 2016 share (SBC-5 A.2 tables A.3, A.4 and A.3.4, A.3.5): mode pages and VPD.
fn base_pages(disk: &Disk, seen: &Seen) -> Vec<Requirement> {
    let large = disk.last_lba > 0xffff_ffff;
    let ext = disk.extended();
    let v_sup = ext.is_some_and(|e| e.v_sup);
    vec![
        (
            "FORMAT UNIT (as REPORT SUPPORTED OPERATION CODES reports it)",
            matches!(disk.format_unit_support, Some(0b011 | 0b101)),
        ),
        ("INQUIRY", sent_ok(seen, "INQUIRY")),
        ("MODE SELECT (10)", sent_ok(seen, "MODE SELECT (10)")),
        ("MODE SENSE (10)", sent_ok(seen, "MODE SENSE (10)")),
        ("REPORT LUNS", sent_ok(seen, "REPORT LUNS")),
        ("REQUEST SENSE", sent_ok(seen, "REQUEST SENSE")),
        ("START STOP UNIT", sent_ok(seen, "START STOP UNIT")),
        ("TEST UNIT READY", sent_ok(seen, "TEST UNIT READY")),
        // A.3.4.1: a long LBA block descriptor when LBAs exceed FFFF_FFFFh.
        (
            "mode parameter block descriptor",
            disk.block_descriptor_len
                .is_some_and(|l| if large { l == 16 } else { l == 8 || l == 16 }),
        ),
        // A.3.4.2: WCE both ways when there is a volatile cache.
        (
            "Caching mode page (WCE changeable when V_SUP)",
            disk.mode_page(0x08, 0).is_some()
                && (!v_sup
                    || disk
                        .changeable
                        .iter()
                        .any(|(k, p)| *k == (0x08, 0) && p.get(2).is_some_and(|b| b & 0x04 != 0))),
        ),
        // A.3.4.3: QUEUE ALGORITHM MODIFIER 1h and QERR 00b; D_SENSE above FFFF_FFFFh.
        (
            "Control mode page QAM 1h, QERR 00b",
            disk.mode_page(0x0a, 0).is_some_and(|p| {
                p.get(3).is_some_and(|b| b >> 4 == 1 && (b >> 1) & 0x3 == 0)
                    || disk.mode_bit(0x0a, 3, 0x10)
            }),
        ),
        (
            "Control mode page D_SENSE when LBAs exceed FFFF_FFFFh",
            !large || disk.mode_bit(0x0a, 2, 0x04),
        ),
        (
            "Control Extension mode page",
            disk.mode_page(0x0a, 0x01).is_some(),
        ),
        // A.3.4.5: AWRE.
        (
            "Read-Write Error Recovery AWRE",
            disk.mode_bit(0x01, 2, 0x80),
        ),
        // A.3.5.1: a non-zero MEDIUM ROTATION RATE.
        (
            "Block Device Characteristics rotation rate non-zero",
            disk.page(0xb1)
                .is_some_and(|d| super::cases::be16(d, 4) != 0),
        ),
        (
            "VPD 00h, 83h, 86h, 87h, 8Ah, 92h",
            [0x00, 0x83, 0x86, 0x87, 0x8a, 0x92]
                .iter()
                .all(|p| disk.pages.contains(p)),
        ),
        // A.3.3.7: NDOB when the UNMAP bit is supported.
        (
            "WRITE SAME NDOB when UNMAP is supported",
            seen.has("WRITE SAME (16) UNMAP") != Some(true)
                || seen.has("WRITE SAME (16) NDOB") == Some(true),
        ),
    ]
}

/// SBC-5 A.2: SBC Base 2010 adds its 10-byte commands.
fn base_2010(disk: &Disk, seen: &Seen) -> Vec<Requirement> {
    let mut r = base_pages(disk, seen);
    r.extend([
        ("READ (10)", sent_ok(seen, "READ (10)")),
        ("READ CAPACITY (10)", sent_ok(seen, "READ CAPACITY (10)")),
        (
            "SYNCHRONIZE CACHE (10)",
            sent_ok(seen, "SYNCHRONIZE CACHE (10)"),
        ),
        ("WRITE (10)", sent_ok(seen, "WRITE (10)")),
        ("WRITE SAME (10)", sent_ok(seen, "WRITE SAME (10)")),
    ]);
    r
}

/// SBC-5 A.3: SBC Base 2016.
fn base_2016(disk: &Disk, seen: &Seen) -> Vec<Requirement> {
    let large = disk.last_lba > 0xffff_ffff;
    let mut r = base_pages(disk, seen);
    let limits = disk.limits().unwrap_or_default();
    let mib = (1u64 << 20) / disk.block_len.max(1) as u64;
    let v_sup = disk.extended().is_some_and(|e| e.v_sup);
    r.extend([
        (
            "Discovery 2016",
            discovery(disk, seen).iter().all(|(_, ok)| *ok),
        ),
        ("READ (16)", sent_ok(seen, "READ (16)")),
        ("READ CAPACITY (16)", sent_ok(seen, "READ CAPACITY (16)")),
        (
            "READ CAPACITY (16) LOWEST ALIGNED LBA 0",
            disk.rc16
                .get(14..16)
                .is_some_and(|b| b[0] & 0x3f == 0 && b[1] == 0),
        ),
        (
            "REPORT SUPPORTED OPERATION CODES with RCTD",
            seen.has("RSOC RCTD") == Some(true),
        ),
        (
            "REPORT SUPPORTED OPERATION CODES options 000b, 001b, 010b",
            ["RSOC 000b", "RSOC 001b", "RSOC 010b"]
                .iter()
                .all(|f| seen.has(f) == Some(true)),
        ),
        (
            "REPORT SUPPORTED TASK MANAGEMENT FUNCTIONS with REPD",
            seen.has("RSTMF REPD") == Some(true),
        ),
        (
            "REQUEST SENSE DESC 0",
            seen.has("REQUEST SENSE DESC 0") == Some(true),
        ),
        (
            "REQUEST SENSE DESC 1 when LBAs exceed FFFF_FFFFh",
            !large || seen.has("REQUEST SENSE DESC 1") == Some(true),
        ),
        (
            "SYNCHRONIZE CACHE (16) with IMMED",
            seen.has("SYNCHRONIZE CACHE (16) IMMED") == Some(true),
        ),
        ("WRITE (16)", sent_ok(seen, "WRITE (16)")),
        ("WRITE SAME (16)", sent_ok(seen, "WRITE SAME (16)")),
        // A.3.2: a volatile write cache needs V_SUP and FUA.
        (
            "FUA when V_SUP",
            !v_sup || seen.has("FUA write") == Some(true),
        ),
        (
            "Informational Exceptions Control mode page",
            disk.mode_page(0x1c, 0).is_some(),
        ),
        (
            "Block Limits MAXIMUM TRANSFER LENGTH >= 1 MiB",
            limits.max_transfer as u64 >= mib,
        ),
        (
            "Block Limits MAXIMUM WRITE SAME LENGTH >= 1 MiB",
            limits.max_write_same >= mib,
        ),
        (
            "VPD B0h, B1h",
            disk.pages.contains(&0xb0) && disk.pages.contains(&0xb1),
        ),
    ]);
    r
}

/// SBC-5 A.4: Basic Provisioning 2016.
fn provisioning(disk: &Disk, seen: &Seen) -> Vec<Requirement> {
    let limits = disk.limits().unwrap_or_default();
    let p = disk.provisioning().unwrap_or_default();
    let thin = p.provisioning_type == 0b010;
    vec![
        ("GET LBA STATUS (16)", sent_ok(seen, "GET LBA STATUS (16)")),
        ("READ CAPACITY (16) LBPME 1", disk.lbpme()),
        ("UNMAP", sent_ok(seen, "UNMAP")),
        (
            "WRITE SAME (16) UNMAP and NDOB",
            seen.has("WRITE SAME (16) UNMAP") == Some(true)
                && seen.has("WRITE SAME (16) NDOB") == Some(true),
        ),
        (
            "MAXIMUM UNMAP LBA COUNT >= MAXIMUM WRITE SAME LENGTH",
            limits.max_unmap_lba as u64 >= limits.max_write_same,
        ),
        (
            "MAXIMUM UNMAP BLOCK DESCRIPTOR COUNT >= 64",
            limits.max_unmap_descriptors >= 64,
        ),
        ("LBPU and LBPWS", p.lbpu && p.lbpws),
        (
            "PROVISIONING TYPE 001b or 010b",
            matches!(p.provisioning_type, 0b001 | 0b010),
        ),
        ("LBPRZ not 000b", p.lbprz != 0),
        // A.4.2: a thin provisioned LUN has the Logical Block Provisioning log page (0Ch).
        (
            "Logical Block Provisioning log page when thin",
            !thin || disk.log_pages.as_ref().is_some_and(|l| l.contains(&0x0c)),
        ),
    ]
}

/// SBC-5 A.5: Drive Maintenance 2016, judged from REPORT SUPPORTED OPERATION CODES, the VPD pages and
/// LOG SENSE only: its destructive commands are never sent.
fn maintenance(disk: &Disk, seen: &Seen) -> Vec<Requirement> {
    let log = |page: u8| disk.log_pages.as_ref().is_some_and(|l| l.contains(&page));
    let rbwz = disk
        .page(0xb1)
        .and_then(|d| d.get(8))
        .is_some_and(|b| b & 0x08 != 0);
    vec![
        ("FORMAT UNIT, LOG SELECT, REASSIGN BLOCKS, SANITIZE, SEND DIAGNOSTIC, WRITE BUFFER, WRITE LONG (16) listed",
            [(0x04, None), (0x4c, None), (0x07, None), (0x48, None), (0x1d, None), (0x3b, None), (0x9f, Some(0x11))].iter().all(|&(op, sa)| listed(disk, op, sa))),
        ("LOG SENSE", sent_ok(seen, "LOG SENSE")),
        ("READ BUFFER (10)", sent_ok(seen, "READ BUFFER (10)")),
        ("READ DEFECT DATA (12)", sent_ok(seen, "READ DEFECT DATA (12)")),
        ("RECEIVE DIAGNOSTIC RESULTS", sent_ok(seen, "RECEIVE DIAGNOSTIC RESULTS")),
        ("VPD 86h, B1h, 8Dh", [0x86, 0xb1, 0x8d].iter().all(|p| disk.pages.contains(p))),
        ("Block Device Characteristics RBWZ", rbwz),
        ("log pages 00h, 03h, 06h, 0Dh, 0Eh, 10h, 11h, 15h, 17h, 2Fh", [0x00, 0x03, 0x06, 0x0d, 0x0e, 0x10, 0x11, 0x15, 0x17, 0x2f].iter().all(|&p| log(p))),
        ("log page 00h/FFh (pages and subpages)", disk.log_subpages.is_some()),
    ]
}

pub(super) fn run(name: &str, disk: &Disk, seen: &Seen, edition: u8, cases: &mut Vec<Case>) {
    let claimed = disk.feature_sets();
    record_info(
        cases,
        SECTION,
        format!("{name} SPC-6 7.7.14 SCSI Feature Sets VPD page"),
        match &claimed {
            Some(list) => list
                .iter()
                .map(|c| format!("{c:04x}h"))
                .collect::<Vec<_>>()
                .join(" "),
            None => "not listed".into(),
        },
    );
    let claimed = claimed.unwrap_or_default();
    for set in SETS {
        let label = format!("{name} {} {} ({:04x}h)", set.clause, set.name, set.code);
        if set.sbc_min.is_some_and(|m| edition < m) {
            record_info(
                cases,
                SECTION,
                label,
                format!("SBC-{edition} defines no feature sets"),
            );
            continue;
        }
        let reqs = match set.code {
            0x0001 => discovery(disk, seen),
            0x0102 => base_2010(disk, seen),
            0x0101 => base_2016(disk, seen),
            0x0103 => provisioning(disk, seen),
            _ => maintenance(disk, seen),
        };
        let missing: Vec<&str> = reqs.iter().filter(|(_, ok)| !ok).map(|(r, _)| *r).collect();
        match (claimed.contains(&set.code), missing.is_empty()) {
            (true, met) => {
                let actual = if met {
                    "claimed and met".to_string()
                } else {
                    format!("claimed but misses {}", missing.join(", "))
                };
                record(cases, SECTION, label, "met", actual, met);
            }
            (false, true) => record_info(cases, SECTION, label, "not claimed; meets it"),
            (false, false) => record_info(
                cases,
                SECTION,
                label,
                format!("not claimed; would miss {}", missing.join(", ")),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_codes_are_the_annex_codes() {
        let codes: Vec<u16> = SETS.iter().map(|s| s.code).collect();
        assert_eq!(codes, vec![0x0001, 0x0102, 0x0101, 0x0103, 0x0104]);
        assert!(SETS.iter().skip(1).all(|s| s.sbc_min == Some(4)));
    }

    #[test]
    fn an_empty_lun_misses_discovery() {
        let disk = Disk::default();
        let seen = Seen::default();
        let reqs = discovery(&disk, &seen);
        assert!(reqs.iter().any(|(r, ok)| *r == "INQUIRY" && !ok));
        assert!(
            reqs.iter()
                .any(|(r, ok)| r.starts_with("T10 vendor ID") && *ok),
            "no 83h page has no bad designator"
        );
    }
}
