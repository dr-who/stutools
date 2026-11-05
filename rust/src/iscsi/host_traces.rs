//! Host traces: the exact CDBs two initiators sent when bringing a LUN online, replayed in order on one
//! session. The Windows Server 2022 Microsoft initiator's come from a capture of it onlining a disk; the
//! Linux open-iscsi (sd) scan's from a capture of a login and scan. Each must end GOOD, return exactly
//! its allocation length when the answer is longer (SPC-6 4.2.5.6), and agree with the full-length
//! answers the `disk` section read. A VPD page the LUN does not list, and the Informational Exceptions
//! Control page, may be refused with INVALID FIELD IN CDB.

use super::cases::{be16, be32, describe, record, Want};
use super::disk::{mode_page_of, Disk};
use super::session::{Outcome, Session};
use crate::compliance::Case;

/// What an answer is compared with.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Check {
    /// GOOD, nothing to compare.
    Good,
    /// The standard INQUIRY data.
    Inquiry,
    /// A VPD page; refused when VPD 00h does not list it.
    Vpd(u8),
    ReadCapacity10,
    ReadCapacity16,
    ReportLuns,
    /// A MODE SENSE (6) page; `optional` pages may be refused.
    ModeSense6 {
        page: u8,
        optional: bool,
    },
    /// READ (16) of 16 blocks at LBA 0, or ending at the last LBA.
    Read16 {
        at_end: bool,
    },
}

struct Step {
    what: &'static str,
    cdb: &'static [u8],
    check: Check,
}

const fn step(what: &'static str, cdb: &'static [u8], check: Check) -> Step {
    Step { what, cdb, check }
}

/// Windows Server 2022 bringing a disk online, in order.
const WINDOWS_2022: &[Step] = &[
    step(
        "READ CAPACITY (10)",
        &[0x25, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        Check::ReadCapacity10,
    ),
    step(
        "READ CAPACITY (16), allocation 12",
        &[0x9e, 0x10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x0c, 0, 0],
        Check::ReadCapacity16,
    ),
    step(
        "REPORT LUNS, allocation 16",
        &[0xa0, 0, 0, 0, 0, 0, 0, 0, 0, 0x10, 0, 0],
        Check::ReportLuns,
    ),
    step(
        "INQUIRY, allocation 36",
        &[0x12, 0, 0, 0, 0x24, 0],
        Check::Inquiry,
    ),
    step(
        "INQUIRY, allocation 74",
        &[0x12, 0, 0, 0, 0x4a, 0],
        Check::Inquiry,
    ),
    step(
        "INQUIRY VPD 00h",
        &[0x12, 1, 0x00, 0, 0xff, 0],
        Check::Vpd(0x00),
    ),
    step(
        "INQUIRY VPD 83h",
        &[0x12, 1, 0x83, 0, 0xff, 0],
        Check::Vpd(0x83),
    ),
    step(
        "INQUIRY VPD 80h",
        &[0x12, 1, 0x80, 0, 0xff, 0],
        Check::Vpd(0x80),
    ),
    step(
        "INQUIRY VPD B0h",
        &[0x12, 1, 0xb0, 0, 0xff, 0],
        Check::Vpd(0xb0),
    ),
    step(
        "INQUIRY VPD B2h",
        &[0x12, 1, 0xb2, 0, 0xff, 0],
        Check::Vpd(0xb2),
    ),
    step(
        "INQUIRY VPD B1h, allocation 64",
        &[0x12, 1, 0xb1, 0, 0x40, 0],
        Check::Vpd(0xb1),
    ),
    step(
        "MODE SENSE (6) page 1Ch",
        &[0x1a, 0, 0x1c, 0, 0xc0, 0],
        Check::ModeSense6 {
            page: 0x1c,
            optional: true,
        },
    ),
    step(
        "MODE SENSE (6) page 0Ah",
        &[0x1a, 0, 0x0a, 0, 0xc0, 0],
        Check::ModeSense6 {
            page: 0x0a,
            optional: false,
        },
    ),
    step(
        "MODE SENSE (6) page 08h",
        &[0x1a, 0, 0x08, 0, 0xc0, 0],
        Check::ModeSense6 {
            page: 0x08,
            optional: false,
        },
    ),
    step(
        "SYNCHRONIZE CACHE (10)",
        &[0x35, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        Check::Good,
    ),
    step(
        "READ CAPACITY (16), allocation 32",
        &[0x9e, 0x10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x20, 0, 0],
        Check::ReadCapacity16,
    ),
    step("TEST UNIT READY", &[0, 0, 0, 0, 0, 0], Check::Good),
];

/// Linux open-iscsi and sd scanning a new LUN, in order.
const LINUX_OPEN_ISCSI: &[Step] = &[
    step(
        "INQUIRY VPD 00h, allocation 36",
        &[0x12, 1, 0x00, 0, 0x24, 0],
        Check::Vpd(0x00),
    ),
    step(
        "INQUIRY, allocation 254",
        &[0x12, 0, 0, 0, 0xfe, 0],
        Check::Inquiry,
    ),
    step(
        "INQUIRY VPD 00h, allocation 254",
        &[0x12, 1, 0x00, 0, 0xfe, 0],
        Check::Vpd(0x00),
    ),
    step(
        "INQUIRY VPD 80h",
        &[0x12, 1, 0x80, 0, 0xfe, 0],
        Check::Vpd(0x80),
    ),
    step(
        "INQUIRY VPD 83h",
        &[0x12, 1, 0x83, 0, 0xfe, 0],
        Check::Vpd(0x83),
    ),
    step(
        "READ CAPACITY (10)",
        &[0x25, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        Check::ReadCapacity10,
    ),
    step(
        "READ CAPACITY (16), allocation 12",
        &[0x9e, 0x10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x0c, 0, 0],
        Check::ReadCapacity16,
    ),
    step(
        "READ (16) of 16 blocks at LBA 0",
        &[],
        Check::Read16 { at_end: false },
    ),
    step(
        "READ (16) of the last 16 blocks",
        &[],
        Check::Read16 { at_end: true },
    ),
];

/// The allocation length a CDB names (SPC-6 4.2.5.6), for the commands in the traces.
fn allocation(cdb: &[u8]) -> u32 {
    match cdb[0] {
        0x12 => be16(cdb, 3) as u32,
        0x1a => cdb[4] as u32,
        0x25 => 8,
        0x9e => be32(cdb, 10),
        0xa0 => be32(cdb, 6),
        _ => 0,
    }
}

/// How long the whole answer is, from its own header.
fn available(cdb: &[u8], d: &[u8]) -> usize {
    match cdb[0] {
        0x12 if cdb[1] & 1 == 0 => d.get(4).map_or(0, |n| *n as usize + 5),
        0x12 => be16(d, 2) as usize + 4,
        0x1a => d.first().map_or(0, |n| *n as usize + 1),
        0x25 => 8,
        0x9e => 32,
        0xa0 => be32(d, 0) as usize + 8,
        _ => d.len(),
    }
}

/// The data the `disk` section read that an answer must start with.
fn reference(check: Check, disk: &Disk) -> Option<Vec<u8>> {
    match check {
        Check::Inquiry => Some(disk.inquiry.clone()),
        Check::Vpd(0x00) => {
            let mut d = vec![0, 0, 0, disk.pages.len() as u8];
            d[0] = disk.inquiry.first().copied().unwrap_or(0);
            d.extend_from_slice(&disk.pages);
            Some(d)
        }
        Check::Vpd(page) => disk.page(page).map(|d| d.to_vec()),
        Check::ReadCapacity16 => Some(disk.rc16.clone()),
        Check::ReadCapacity10 => {
            let mut d = (disk.last_lba.min(0xffff_ffff) as u32)
                .to_be_bytes()
                .to_vec();
            d.extend_from_slice(&disk.block_len.to_be_bytes());
            Some(d)
        }
        _ => None,
    }
}

fn judge(step: &Step, o: &Outcome, disk: &Disk) -> (String, bool) {
    let refused_field = Want::Ascq(0x5, 0x24, 0x00).matches(o);
    match step.check {
        Check::Vpd(page) if page != 0 && !disk.pages.contains(&page) => {
            return (describe(o), refused_field);
        }
        Check::ModeSense6 { optional: true, .. } if refused_field => return (describe(o), true),
        _ => {}
    }
    if o.status != 0 {
        return (describe(o), false);
    }
    let alloc = allocation(step.cdb) as usize;
    let want_len = available(step.cdb, &o.data).min(alloc);
    let length_ok = alloc == 0 || o.received == want_len;
    let data = &o.data[..o.received.min(o.data.len())];
    let agrees = match step.check {
        Check::ModeSense6 { page, .. } => mode_page_of(data, false).is_some_and(|p| {
            Some(p) == disk.mode_page(page, 0)
                || p.len() > 1 && p[0] & 0x3f == page && disk.mode_page(page, 0).is_none()
        }),
        Check::ReportLuns => data.get(8..16) == Some(&[0u8; 8]),
        other => match reference(other, disk) {
            Some(r) => {
                let n = data.len().min(r.len());
                data[..n] == r[..n]
            }
            None => true,
        },
    };
    (
        format!(
            "GOOD, {} of {alloc} bytes{}",
            o.received,
            if agrees {
                ""
            } else {
                ", differs from the full answer"
            }
        ),
        length_ok && agrees,
    )
}

/// Replays one host's trace.
fn replay(
    s: &mut Session,
    name: &str,
    host: &str,
    section: &'static str,
    steps: &[Step],
    disk: &Disk,
    cases: &mut Vec<Case>,
) {
    for step in steps {
        let label = format!("{name} {host}: {}", step.what);
        let result = match step.check {
            Check::Read16 { at_end } => {
                let lba = if at_end {
                    disk.last_lba.saturating_sub(15)
                } else {
                    0
                };
                let len = 16 * disk.block_len;
                s.command(&super::block_io::cdb16(0x88, 0, lba, 16), len, None)
                    .map(|o| {
                        let ok = o.status == 0 && o.received == len as usize;
                        (
                            if o.status == 0 {
                                format!("GOOD, {} of {len} bytes", o.received)
                            } else {
                                describe(&o)
                            },
                            ok,
                        )
                    })
            }
            _ => s
                .command(step.cdb, allocation(step.cdb), None)
                .map(|o| judge(step, &o, disk)),
        };
        let (actual, passed) = result.unwrap_or_else(|e| (e, false));
        record(
            cases,
            section,
            label,
            "GOOD, exactly the allocation length, agrees with the full answer",
            actual,
            passed,
        );
    }
}

pub(super) fn windows(s: &mut Session, name: &str, disk: &Disk, cases: &mut Vec<Case>) {
    replay(
        s,
        name,
        "Windows Server 2022 online",
        "windows",
        WINDOWS_2022,
        disk,
        cases,
    );
}

pub(super) fn linux(s: &mut Session, name: &str, disk: &Disk, cases: &mut Vec<Case>) {
    replay(
        s,
        name,
        "Linux open-iscsi scan",
        "disk",
        LINUX_OPEN_ISCSI,
        disk,
        cases,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocation_lengths_are_read_where_each_cdb_puts_them() {
        assert_eq!(allocation(&[0x12, 1, 0xb1, 0, 0x40, 0]), 0x40);
        assert_eq!(allocation(&[0x1a, 0, 0x08, 0, 0xc0, 0]), 0xc0);
        assert_eq!(allocation(WINDOWS_2022[1].cdb), 12);
        assert_eq!(allocation(WINDOWS_2022[2].cdb), 16);
    }

    #[test]
    fn available_length_comes_from_the_answer_header() {
        assert_eq!(available(&[0x12, 0, 0, 0, 0x24, 0], &[0, 0, 6, 2, 91]), 96);
        assert_eq!(
            available(&[0x12, 1, 0x83, 0, 0xff, 0], &[0, 0x83, 0, 20]),
            24
        );
        assert_eq!(available(&[0xa0], &[0, 0, 0, 8]), 16);
        assert_eq!(available(&[0x1a], &[35]), 36);
    }
}
