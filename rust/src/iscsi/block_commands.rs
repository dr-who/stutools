//! The `sbc` cases of `--compliance`: SBC table 34, row by row, each command probed once and judged at
//! every `--scsilevel` (see `command_set`); those cases are recorded per level. A command another section already checks in depth is not sent again: its
//! row reports that section's verdict. The rest are sent here in a form that changes nothing (writes
//! carry the block's own data, ORWRITE carries zeros, BACKGROUND CONTROL asks for no change), and pass
//! when they behave as their clause requires or are refused cleanly (ILLEGAL REQUEST with INVALID COMMAND
//! OPERATION CODE, or INVALID FIELD IN CDB for an unimplemented service action, SPC-6 4.2.5.2). Commands
//! that destroy data or change device state are never sent; REPORT SUPPORTED OPERATION CODES is asked
//! about them instead. When it lists commands, its list must agree with what each command did.

use super::cases::record;
use super::command_probes::{probe, spc2, type_2_protection, Block, DROPPED, TYPE_2_ONLY};
use super::command_set::{Row, Rule, Sending, ROWS};
use super::disk::Disk;
use super::login;
use super::lun_access::{open, open_with, read_block, Lun};
use super::seen::{Probe, Seen};
use super::session::Session;
use crate::compliance::{Case, ScsiLevel};

pub(super) const SECTION: &str = "sbc";

/// The operation code of the variable-length (32-byte) CDBs.
const VARIABLE_LENGTH: u8 = 0x7f;

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

/// The case name: the clause (SBC-5's, which a report at an older level cites in its edition), the
/// command, the hosts that send it, and a note when the level's editions do not define it.
fn title(row: &Row, level: ScsiLevel) -> String {
    let clause = match row.clause.strip_prefix("SBC ") {
        Some(c) => format!("SBC-5 {c}"),
        None => row.clause.to_string(),
    };
    let hosts = if row.hosts.is_empty() {
        String::new()
    } else {
        format!(" [{}]", row.hosts)
    };
    let note = row
        .edition_note(level)
        .map(|n| format!(" ({n})"))
        .unwrap_or_default();
    format!("{clause} {}{hosts}{note}", row.name)
}

/// Runs the table 34 checklist for one LUN.
pub(super) fn run(
    s: &mut Session,
    lun: Lun,
    name: &str,
    disk: &Disk,
    levels: &[ScsiLevel],
    seen: &mut Seen,
    cases: &mut Vec<Case>,
) {
    let Lun {
        path,
        initiator,
        creds,
    } = lun;
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
    let mut plain = None;
    for row in ROWS {
        if let Sending::Never(why) = row.send {
            for &level in levels {
                let mut one = Vec::new();
                not_sent_case(row, level, name, disk, why, &mut one);
                cases.extend(one.into_iter().map(|c| c.at(level)));
            }
            continue;
        }
        let found = match (seen.probe(row.name).cloned(), row.name) {
            (Some(p), _) => Some(p),
            (None, "RESERVE (6)" | "RELEASE (6)") => {
                if spc2_probes.is_none() {
                    spc2_probes =
                        open(path, initiator, creds, login::random_isid())
                            .ok()
                            .map(|mut other| {
                                let _ = super::lun_access::unit_attentions(&mut other);
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
            // A 32-byte CDB travels in an Extended CDB AHS; it goes on a session without header
            // digests so a target that digests the AHS wrongly (`header_digest`) is judged here on the
            // command alone.
            (None, _) if row.opcode == VARIABLE_LENGTH => {
                if plain.is_none() {
                    plain = open_with(path, initiator, creds, login::random_isid(), "None")
                        .ok()
                        .map(|mut p| {
                            let _ = super::lun_access::unit_attentions(&mut p);
                            p
                        });
                }
                plain.as_mut().and_then(|p| probe(p, row, &block, disk))
            }
            (None, _) => probe(s, row, &block, disk),
        };
        // A probe that cost the connection is that row's failure only: the rest run on a new session.
        if found
            .as_ref()
            .is_some_and(|p| p.actual.starts_with(DROPPED))
        {
            if row.opcode == VARIABLE_LENGTH {
                plain = None;
            } else if let Ok(mut again) = open(path, initiator, creds, login::random_isid()) {
                let _ = super::lun_access::unit_attentions(&mut again);
                *s = again;
            }
        }
        for &level in levels {
            let mut one = Vec::new();
            judge(row, level, name, disk, found.as_ref(), &mut one);
            cases.extend(one.into_iter().map(|c| c.at(level)));
        }
    }
    if let Some(p) = plain {
        let _ = p.logout();
    }
}

/// One row's verdict at `level`, from what its probe found.
fn judge(
    row: &Row,
    level: ScsiLevel,
    name: &str,
    disk: &Disk,
    found: Option<&Probe>,
    cases: &mut Vec<Case>,
) {
    let rule = row.rule(level);
    let label = format!("{name} {}", title(row, level));
    // A command no section sent (SET TARGET PORT GROUPS on a device without ALUA) is no verdict.
    let Some(p) = found else {
        super::cases::record_info(
            cases,
            SECTION,
            label,
            "not sent: nothing on this device calls for it",
        );
        return;
    };
    let strict = matches!(rule, Rule::Mandatory | Rule::Host);
    let expected = if TYPE_2_ONLY.contains(&row.name) && !type_2_protection(disk) {
        "a clean refusal: no type 2 protection (SBC-5 4.21.2.4)".to_string()
    } else {
        expected(&rule)
    };
    // SPC-6 6.32: the all_commands list names exactly the commands the device server supports.
    let rsoc = disk
        .lists(row.opcode, row.service_action)
        .filter(|&listed| listed != p.supported && p.actual != "no answer")
        .map(|listed| {
            if listed {
                "RSOC lists it"
            } else {
                "RSOC does not list it"
            }
        });
    // A command another section judged is that section's verdict: here it is only noted, unless
    // REPORT SUPPORTED OPERATION CODES contradicts what it found.
    if p.section != SECTION {
        let seen = format!(
            "see {}: {}",
            p.section,
            if p.passed { "passed" } else { "failed" }
        );
        match rsoc {
            Some(why) => {
                record(
                    cases,
                    SECTION,
                    label,
                    expected,
                    format!("{seen}; {why}"),
                    false,
                );
            }
            None => super::cases::record_info(cases, SECTION, label, seen),
        }
        return;
    }
    let mut actual = if p.supported || strict {
        p.actual.clone()
    } else {
        format!("refused ({})", p.actual)
    };
    if let Some(why) = rsoc {
        actual.push_str("; ");
        actual.push_str(why);
    }
    let passed = p.passed && (p.supported || !strict) && rsoc.is_none();
    record(cases, SECTION, label, expected, actual, passed);
}
/// A row that is never sent, at `level`: a mandatory one must not be denied by REPORT SUPPORTED
/// OPERATION CODES; for the others its answer is recorded.
fn not_sent_case(
    row: &Row,
    level: ScsiLevel,
    name: &str,
    disk: &Disk,
    why: &str,
    cases: &mut Vec<Case>,
) {
    let label = format!("{name} {}", title(row, level));
    let (actual, passed) = not_sent(row, disk, why);
    if row.in_sbc(level.sbc()) && row.sbc.and_then(|s| s.mandatory_from).is_some() {
        let expected = format!(
            "mandatory, so REPORT SUPPORTED OPERATION CODES says supported (not sent: {why})"
        );
        record(cases, SECTION, label, expected, actual, passed);
    } else {
        super::cases::record_info(cases, SECTION, label, actual);
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
    use super::*;

    #[test]
    fn titles_cite_the_edition_and_hosts() {
        let read16 = super::super::command_set::row("READ (16)").unwrap();
        let level = |n| ScsiLevel::new(n).unwrap();
        assert_eq!(
            title(read16, level(5)),
            "SBC-5 5.18 READ (16) [Windows, macOS, Linux]"
        );
        assert_eq!(
            crate::compliance::cite(&title(read16, level(3)), level(3)).text,
            "SBC-3 5.13 READ (16) [Windows, macOS, Linux]"
        );
        let inquiry = super::super::command_set::row("INQUIRY").unwrap();
        assert!(title(inquiry, level(4)).starts_with("SPC-6 6.7 INQUIRY"));
    }
}
