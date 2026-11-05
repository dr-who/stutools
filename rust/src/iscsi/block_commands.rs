//! The `sbc` cases of `--compliance`: SBC table 34, row by row, judged against the SBC edition `--sbc`
//! picks (see `command_set`). A command another section already checks in depth is not sent again: its
//! row reports that section's verdict. The rest are sent here in a form that changes nothing (writes
//! carry the block's own data, ORWRITE carries zeros, BACKGROUND CONTROL asks for no change), and pass
//! when they behave as their clause requires or are refused cleanly (ILLEGAL REQUEST with INVALID COMMAND
//! OPERATION CODE, or INVALID FIELD IN CDB for an unimplemented service action, SPC-6 4.2.5.2). Commands
//! that destroy data or change device state are never sent; REPORT SUPPORTED OPERATION CODES is asked
//! about them instead. When it lists commands, its list must agree with what each command did.

use super::cases::{be16, describe, open, read_block, record, refused, Creds, Probe, Seen, Want};
use super::command_set::{Row, Rule, ROWS};
use super::disk::Disk;
use super::report::Path;
use super::session::{self, Outcome, Session};
use crate::compliance::Case;
use crate::Result;

const SECTION: &str = "sbc";
/// The start of a probe's answer when the connection failed under it.
const DROPPED: &str = "connection failed";

/// Where the probes read and write: the second-to-last block and its original data.
struct Block {
    lba: u64,
    len: usize,
    original: Vec<u8>,
}

/// A 32-byte variable-length CDB (SBC-5 table 85): operation code 7Fh, ADDITIONAL CDB LENGTH 18h, the
/// service action at 8, the LBA at 12, the count at 28.
pub(super) fn cdb32(service_action: u16, lba: u64, count: u32) -> [u8; 32] {
    let mut c = [0u8; 32];
    c[0] = 0x7f;
    c[7] = 0x18;
    c[8..10].copy_from_slice(&service_action.to_be_bytes());
    c[12..20].copy_from_slice(&lba.to_be_bytes());
    c[28..32].copy_from_slice(&count.to_be_bytes());
    c
}

/// A SERVICE ACTION IN (16) or MAINTENANCE IN style CDB with the allocation length where these
/// commands put it.
fn sa_in(op: u8, sa: u8, len: usize, alloc_at: usize, alloc: u32) -> Vec<u8> {
    let mut c = vec![0u8; len];
    c[0] = op;
    c[1] = sa;
    c[alloc_at..alloc_at + 4].copy_from_slice(&alloc.to_be_bytes());
    c
}

fn cdb12(op: u8, lba: u32, count: u32) -> [u8; 12] {
    let mut c = [0u8; 12];
    c[0] = op;
    c[2..6].copy_from_slice(&lba.to_be_bytes());
    c[6..10].copy_from_slice(&count.to_be_bytes());
    c
}

/// The data for WRITE SCATTERED (16) of one range (SBC-5 tables 153, 154): a 32-byte header, one 32-byte
/// LBA range descriptor, padding to a block, then the block itself.
pub(super) fn scattered(lba: u64, block: &[u8]) -> Vec<u8> {
    let mut d = vec![0u8; block.len()];
    d[32..40].copy_from_slice(&lba.to_be_bytes());
    d[40..44].copy_from_slice(&1u32.to_be_bytes());
    d.extend_from_slice(block);
    d
}

/// One command's answer as a probe: what it found and whether that is right.
fn judge(o: Result<Outcome>, ok: impl Fn(&Outcome) -> bool) -> Probe {
    match o {
        Ok(o) if refused(&o, true) => Probe {
            section: SECTION,
            supported: false,
            passed: true,
            actual: describe(&o),
        },
        Ok(o) => Probe {
            section: SECTION,
            supported: true,
            passed: ok(&o),
            actual: describe(&o),
        },
        // RFC 7143 11.17: a target may Reject a PDU it cannot take (reason 05h, command not supported),
        // which refuses the command cleanly.
        Err(e) if e.starts_with(session::REJECTED) => Probe {
            section: SECTION,
            supported: false,
            passed: true,
            actual: e,
        },
        Err(e) => Probe {
            section: SECTION,
            supported: false,
            passed: false,
            actual: format!("{DROPPED}: {e}"),
        },
    }
}

fn good(o: &Outcome) -> bool {
    o.status == 0
}

/// Sends the row's command, in a form that changes nothing, and judges it.
fn probe(s: &mut Session, row: &Row, b: &Block, disk: &Disk) -> Option<Probe> {
    let lba32 = b.lba as u32;
    let len = b.len as u32;
    let zeros = vec![0u8; b.len];
    let reads_back = |s: &mut Session| read_block(s, lba32, b.len).is_ok_and(|d| d == b.original);
    // SBC-5 4.21.2: without type 2 protection the 32-byte medium access commands are INVALID COMMAND
    // OPERATION CODE.
    let type2 = disk
        .rc16
        .get(12)
        .is_some_and(|p| p & 0x01 != 0 && (p >> 1) & 0x7 == 1);
    let thirty_two = |o: &Outcome| {
        if type2 {
            o.status == 0
        } else {
            Want::Ascq(0x5, 0x20, 0x00).matches(o)
        }
    };
    let p = match row.name {
        "READ (12)" => judge(s.command(&cdb12(0xa8, lba32, 1), len, None), |o| {
            good(o) && o.data == b.original
        }),
        "WRITE (12)" => {
            let mut p = judge(
                s.command(&cdb12(0xaa, lba32, 1), 0, Some(&b.original)),
                good,
            );
            p.passed &= !p.supported || reads_back(s);
            p
        }
        "VERIFY (12)" => judge(s.command(&cdb12(0xaf, lba32, 1), 0, None), good),
        "WRITE AND VERIFY (10)" => judge(
            s.command(
                &super::block_io::cdb10(0x2e, 0, lba32, 1),
                0,
                Some(&b.original),
            ),
            good,
        ),
        "WRITE AND VERIFY (12)" => judge(
            s.command(&cdb12(0xae, lba32, 1), 0, Some(&b.original)),
            good,
        ),
        "WRITE AND VERIFY (16)" => judge(
            s.command(
                &super::block_io::cdb16(0x8e, 0, b.lba, 1),
                0,
                Some(&b.original),
            ),
            good,
        ),
        // SBC-5 5.13: CONDITION MET (04h) when the blocks are in the cache, else GOOD.
        "PRE-FETCH (10)" => judge(
            s.command(&super::block_io::cdb10(0x34, 0, lba32, 1), 0, None),
            |o| matches!(o.status, 0 | 4),
        ),
        "PRE-FETCH (16)" => judge(
            s.command(&super::block_io::cdb16(0x90, 0, b.lba, 1), 0, None),
            |o| matches!(o.status, 0 | 4),
        ),
        // SBC-5 5.10: OR with zeros leaves the block as it was.
        "ORWRITE (16)" => {
            let mut p = judge(
                s.command(&super::block_io::cdb16(0x8b, 0, b.lba, 1), 0, Some(&zeros)),
                good,
            );
            p.passed &= !p.supported || reads_back(s);
            p
        }
        "READ (32)" => judge(s.command(&cdb32(0x0009, b.lba, 1), len, None), thirty_two),
        "VERIFY (32)" => judge(s.command(&cdb32(0x000a, b.lba, 1), 0, None), thirty_two),
        "WRITE (32)" => judge(
            s.command(&cdb32(0x000b, b.lba, 1), 0, Some(&b.original)),
            thirty_two,
        ),
        "WRITE AND VERIFY (32)" => judge(
            s.command(&cdb32(0x000c, b.lba, 1), 0, Some(&b.original)),
            thirty_two,
        ),
        "WRITE SAME (32)" => judge(
            s.command(&cdb32(0x000d, b.lba, 1), 0, Some(&b.original)),
            thirty_two,
        ),
        "ORWRITE (32)" => judge(
            s.command(&cdb32(0x000e, b.lba, 1), 0, Some(&zeros)),
            thirty_two,
        ),
        "WRITE ATOMIC (32)" | "WRITE SCATTERED (32)" if type2 => Probe {
            section: SECTION,
            supported: true,
            passed: true,
            actual: "not sent on a type 2 protection LUN".into(),
        },
        "WRITE ATOMIC (32)" => judge(
            s.command(&cdb32(0x000f, b.lba, 1), 0, Some(&b.original)),
            thirty_two,
        ),
        "WRITE SCATTERED (32)" => judge(s.command(&cdb32(0x0011, 0, 0), 0, None), thirty_two),
        "GET LBA STATUS (32)" => {
            let mut c = cdb32(0x0012, b.lba, 1);
            c[28..32].copy_from_slice(&64u32.to_be_bytes());
            c[20..24].copy_from_slice(&1u32.to_be_bytes());
            judge(s.command(&c, 64, None), good)
        }
        // SBC-5 5.48: one block of the block's own data, when Block Limits reports atomic writes.
        "WRITE ATOMIC (16)" => {
            let mut c = super::block_io::cdb16(0x9c, 0, b.lba, 0);
            c[12..14].copy_from_slice(&1u16.to_be_bytes());
            let limited = disk.limits().is_some_and(|l| l.max_atomic_transfer > 0);
            let mut p = judge(s.command(&c, 0, Some(&b.original)), good);
            p.passed &= p.supported || !limited;
            p
        }
        // SBC-5 5.55: LOGICAL BLOCK DATA OFFSET 1 (the data starts at the second block), one range.
        "WRITE SCATTERED (16)" => {
            let mut c = [0u8; 16];
            c[0] = 0x9f;
            c[1] = 0x12;
            c[4..6].copy_from_slice(&1u16.to_be_bytes());
            c[8..10].copy_from_slice(&1u16.to_be_bytes());
            c[10..14].copy_from_slice(&2u32.to_be_bytes());
            let mut p = judge(s.command(&c, 0, Some(&scattered(b.lba, &b.original))), good);
            p.passed &= !p.supported || reads_back(s);
            p
        }
        // SBC-5 5.15: PREVENT 00b allows removal; harmless on any disk.
        "PREVENT ALLOW MEDIUM REMOVAL" => judge(s.command(&[0x1e, 0, 0, 0, 0, 0], 0, None), good),
        // SBC-5 5.22, 5.23: REQ_PLIST and REQ_GLIST clear return only the header, with no defects; a
        // format the device does not use is RECOVERED ERROR, DEFECT LIST NOT FOUND (1Ch/00h).
        "READ DEFECT DATA (10)" => judge(
            s.command(&[0x37, 0, 0, 0, 0, 0, 0, 0, 4, 0], 4, None),
            |o| good(o) || Want::Ascq(0x1, 0x1c, 0x00).matches(o),
        ),
        "READ DEFECT DATA (12)" => judge(s.command(&cdb12(0xb7, 0, 8), 8, None), |o| {
            good(o) || Want::Ascq(0x1, 0x1c, 0x00).matches(o)
        }),
        // SPC-6 6.18.4: descriptor mode (03h) returns four bytes.
        "READ BUFFER (10)" => judge(
            s.command(&[0x3c, 0x03, 0, 0, 0, 0, 0, 0, 4, 0], 4, None),
            good,
        ),
        "READ BUFFER (16)" => {
            let mut c = [0u8; 16];
            c[0] = 0x9b;
            c[1] = 0x03;
            c[10..14].copy_from_slice(&4u32.to_be_bytes());
            judge(s.command(&c, 4, None), good)
        }
        // SPC-6 6.24: PCV with page 00h returns the supported diagnostic pages, page code 00h.
        "RECEIVE DIAGNOSTIC RESULTS" => judge(
            s.command(&[0x1c, 0x01, 0x00, 0x00, 0xff, 0], 255, None),
            |o| good(o) && o.data.first() == Some(&0),
        ),
        // SPC-6 6.37: security protocol 00h lists the supported protocols, 00h among them.
        "SECURITY PROTOCOL IN" => {
            judge(s.command(&sa_in(0xa2, 0x00, 12, 6, 512), 512, None), |o| {
                let n = be16(&o.data, 6) as usize;
                good(o) && o.data.get(8..8 + n).is_some_and(|l| l.contains(&0))
            })
        }
        "REPORT IDENTIFYING INFORMATION" => {
            judge(s.command(&sa_in(0xa3, 0x05, 12, 6, 256), 256, None), good)
        }
        // SPC-6 6.35: TIMESTAMP PARAMETER DATA LENGTH is 000Ah.
        "REPORT TIMESTAMP" => judge(s.command(&sa_in(0xa3, 0x0f, 12, 6, 12), 12, None), |o| {
            good(o) && be16(&o.data, 0) == 0x0a
        }),
        "REPORT PROVISIONING INITIALIZATION PATTERN" => {
            judge(s.command(&sa_in(0xa3, 0x1d, 12, 6, 512), 512, None), good)
        }
        // SBC-5 5.28: required when Extended INQUIRY Data sets R_SUP.
        "REPORT REFERRALS" => {
            let mut p = judge(s.command(&sa_in(0x9e, 0x13, 16, 10, 512), 512, None), good);
            p.passed &= p.supported || !disk.extended().is_some_and(|e| e.r_sup);
            p
        }
        "GET PHYSICAL ELEMENT STATUS" => {
            judge(s.command(&sa_in(0x9e, 0x17, 16, 10, 64), 64, None), good)
        }
        "GET STREAM STATUS" => judge(s.command(&sa_in(0x9e, 0x16, 16, 10, 64), 64, None), good),
        // SBC-5 5.2: BO_CTL 00b asks for no change.
        "BACKGROUND CONTROL" => judge(s.command(&sa_in(0x9e, 0x15, 16, 10, 0), 0, None), good),
        "READ MEDIA SERIAL NUMBER" => {
            judge(s.command(&sa_in(0xab, 0x01, 12, 6, 64), 64, None), good)
        }
        "READ ATTRIBUTE" => judge(s.command(&sa_in(0x8c, 0x00, 16, 10, 64), 64, None), good),
        "REPORT ALIASES" => judge(s.command(&sa_in(0xa3, 0x0b, 12, 6, 64), 64, None), good),
        "REPORT PRIORITY" => judge(s.command(&sa_in(0xa3, 0x0e, 12, 6, 64), 64, None), good),
        "REPORT ALL ROD TOKENS" => judge(s.command(&sa_in(0x84, 0x08, 16, 10, 64), 64, None), good),
        _ => return None,
    };
    // INVALID FIELD IN CDB refuses only a service action (SPC-6 4.2.5.2); for a command without one it
    // is an answer to the command, so the command is implemented.
    let mut p = p;
    if row.service_action.is_none() && !p.supported && p.actual == "CHECK CONDITION 5/24/00" {
        p.supported = true;
        p.passed = false;
    }
    Some(p)
}

/// RESERVE (6) and RELEASE (6) (SPC-2, obsolete since SPC-4): with an SPC-2 reservation held by this
/// nexus, another nexus's READ conflicts; after RELEASE it reads again.
fn spc2(s: &mut Session, other: &mut Session, b: &Block) -> (Probe, Probe) {
    let lba = b.lba as u32;
    let reserve = s.command(&super::ops::RESERVE_6, 0, None);
    match reserve {
        Ok(o) if o.status == 0 => {
            let blocked =
                super::cases::read_cmd(other, lba, b.len).is_ok_and(|o| Want::Conflict.matches(&o));
            let release = s.command(&super::ops::RELEASE_6, 0, None);
            let freed = super::cases::read_cmd(other, lba, b.len).is_ok_and(|o| good(&o));
            (
                Probe {
                    section: SECTION,
                    supported: true,
                    passed: blocked,
                    actual: format!(
                        "GOOD; another nexus's READ {}",
                        if blocked {
                            "conflicts"
                        } else {
                            "is not blocked"
                        }
                    ),
                },
                judge(release, |o| good(o) && freed),
            )
        }
        other_answer => {
            let r = judge(other_answer, |_| false);
            let release = judge(s.command(&super::ops::RELEASE_6, 0, None), good);
            (r, release)
        }
    }
}

fn expected(rule: &Rule) -> String {
    match rule {
        Rule::Mandatory => "mandatory: works".into(),
        Rule::Host => "sent by hosts: works".into(),
        Rule::HostFallback(how) => format!("works, or a clean refusal ({how})"),
        Rule::Optional => "works, or a clean refusal".into(),
        Rule::Informational(why) => format!("informational ({why}): works, or a clean refusal"),
        Rule::NotSent(why) => format!("not sent ({why})"),
    }
}

/// The case name: the edition and clause, the command, and the hosts that send it.
fn title(row: &Row, edition: u8) -> String {
    let clause = match row.clause.strip_prefix("SBC ") {
        Some(c) if edition >= 4 => format!("SBC-{edition} {c}"),
        Some(c) => format!("SBC-3 (SBC-5 {c})"),
        None => row.clause.to_string(),
    };
    let hosts = if row.hosts.is_empty() {
        String::new()
    } else {
        format!(" [{}]", row.hosts)
    };
    let note = row
        .edition_note(edition)
        .map(|n| format!(" ({n})"))
        .unwrap_or_default();
    format!("{clause} {}{hosts}{note}", row.name)
}

/// Runs the table 34 checklist for one LUN.
pub(super) fn run(
    s: &mut Session,
    path: &Path,
    initiator: &str,
    creds: Creds,
    name: &str,
    disk: &Disk,
    edition: u8,
    seen: &mut Seen,
    cases: &mut Vec<Case>,
) {
    let Ok(lba) = u32::try_from(disk.last_lba.saturating_sub(1)) else {
        return;
    };
    let len = disk.block_len as usize;
    let original = match read_block(s, lba, len) {
        Ok(d) => d,
        Err(err) => {
            record(
                cases,
                SECTION,
                format!("{name} read the test block"),
                "GOOD",
                err,
                false,
            );
            return;
        }
    };
    let block = Block {
        lba: lba as u64,
        len,
        original,
    };
    let mut spc2_probes = None;
    for row in ROWS {
        let rule = row.rule(edition);
        let label = format!("{name} {}", title(row, edition));
        if let Rule::NotSent(why) = rule {
            let (actual, passed) = not_sent(row, disk, why);
            if row.in_sbc(edition) && row.sbc.and_then(|s| s.mandatory_from).is_some() {
                record(cases, SECTION, label, expected(&rule), actual, passed);
            } else {
                super::cases::record_info(cases, SECTION, label, actual);
            }
            continue;
        }
        let found = match (seen.probe(row.name).cloned(), row.name) {
            (Some(p), _) => Some(p),
            (None, "RESERVE (6)" | "RELEASE (6)") => {
                if spc2_probes.is_none() {
                    spc2_probes = open(path, initiator, creds, session::random_isid())
                        .ok()
                        .map(|mut other| {
                            let _ = super::cases::unit_attentions(&mut other);
                            let pair = spc2(s, &mut other, &block);
                            let _ = other.logout();
                            pair
                        });
                }
                spc2_probes.as_ref().map(|(r, l)| {
                    if row.name == "RESERVE (6)" {
                        r.clone()
                    } else {
                        l.clone()
                    }
                })
            }
            (None, _) => probe(s, row, &block, disk),
        };
        // A probe that cost the connection is that row's failure only: the rest run on a new session.
        if found
            .as_ref()
            .is_some_and(|p| p.actual.starts_with(DROPPED))
        {
            if let Ok(mut again) = open(path, initiator, creds, session::random_isid()) {
                let _ = super::cases::unit_attentions(&mut again);
                *s = again;
            }
        }
        let Some(p) = found else {
            record(
                cases,
                SECTION,
                label,
                expected(&rule),
                "not checked by spit",
                matches!(rule, Rule::Informational(_) | Rule::Optional),
            );
            continue;
        };
        let strict = matches!(rule, Rule::Mandatory | Rule::Host);
        let mut passed = p.passed && (p.supported || !strict);
        let mut actual = if p.section == SECTION {
            p.actual.clone()
        } else {
            format!(
                "see {}: {}",
                p.section,
                if p.passed { "passed" } else { "failed" }
            )
        };
        if !p.supported && !strict {
            actual = format!("refused ({})", p.actual);
        }
        // SPC-6 6.32: the all_commands list names exactly the commands the device server supports.
        if let Some(listed) = disk.lists(row.opcode, row.service_action) {
            if listed != p.supported && p.actual != "no answer" {
                passed = false;
                actual.push_str(if listed {
                    "; RSOC lists it"
                } else {
                    "; RSOC does not list it"
                });
            }
        }
        record(cases, SECTION, label, expected(&rule), actual, passed);
    }
}

/// The verdict for a row that is never sent: FORMAT UNIT is mandatory, so REPORT SUPPORTED OPERATION
/// CODES must not deny it; for the others the RSOC answer is recorded.
fn not_sent(row: &Row, disk: &Disk, why: &str) -> (String, bool) {
    if row.name == "FORMAT UNIT" {
        return match (disk.rsoc.is_some(), disk.format_unit_support) {
            (false, _) => (
                "not checkable: REPORT SUPPORTED OPERATION CODES not supported".into(),
                true,
            ),
            // SPC-6 6.32.3 table: SUPPORT 011b (standard) or 101b (vendor specific) means supported.
            (true, Some(s)) => (
                format!("RSOC one_command SUPPORT {s:03b}b"),
                matches!(s, 0b011 | 0b101),
            ),
            (true, None) => ("RSOC one_command refused".into(), false),
        };
    }
    let listed = match disk.lists(row.opcode, row.service_action) {
        Some(true) => "RSOC lists it",
        Some(false) => "RSOC does not list it",
        None => "RSOC not supported",
    };
    (format!("{listed}; {why}"), true)
}

#[cfg(test)]
mod tests {
    use super::super::cases::REFUSED_SA;
    use super::*;

    #[test]
    fn a_32_byte_cdb_has_its_fixed_fields() {
        let c = cdb32(0x0009, 0x0102_0304_0506_0708, 1);
        assert_eq!((c[0], c[7]), (0x7f, 0x18));
        assert_eq!(&c[8..10], &[0, 9]);
        assert_eq!(&c[12..20], &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(&c[28..32], &[0, 0, 0, 1]);
    }

    #[test]
    fn scattered_data_has_header_descriptor_pad_and_block() {
        let block = vec![0xab; 512];
        let d = scattered(9, &block);
        assert_eq!(d.len(), 1024);
        assert_eq!(&d[32..40], &9u64.to_be_bytes());
        assert_eq!(&d[40..44], &1u32.to_be_bytes());
        assert!(d[44..512].iter().all(|&b| b == 0));
        assert_eq!(&d[512..], &block[..]);
    }

    #[test]
    fn titles_cite_the_edition_and_hosts() {
        let read16 = super::super::command_set::row("READ (16)").unwrap();
        assert_eq!(
            title(read16, 5),
            "SBC-5 5.18 READ (16) [Windows, macOS, Linux]"
        );
        assert_eq!(
            title(read16, 3),
            "SBC-3 (SBC-5 5.18) READ (16) [Windows, macOS, Linux]"
        );
        let inquiry = super::super::command_set::row("INQUIRY").unwrap();
        assert!(title(inquiry, 4).starts_with("SPC-6 6.7 INQUIRY"));
    }

    #[test]
    fn refusals_pass_unless_the_rule_is_strict() {
        let refusal = Outcome {
            status: 2,
            sense: vec![0x72, 5, 0x20, 0],
            data: vec![],
            received: 0,
        };
        let p = judge(Ok(refusal), |_| false);
        assert!(!p.supported && p.passed);
        assert!(REFUSED_SA.iter().any(|w| w.matches(&Outcome {
            status: 2,
            sense: vec![0x72, 5, 0x24, 0],
            data: vec![],
            received: 0
        })));
    }
}
