//! The `pr` cases for the whole block command set under persistent reservations (SBC-5 4.17 table 13;
//! SBC-3 table 12 holds the same entries for the commands it has). With A holding each of Write
//! Exclusive, Exclusive Access and their registrants-only forms, B (registered) and C (not registered)
//! send each command in a form that leaves the device as it was, and the answer must follow the table:
//! a command marked conflict shall get RESERVATION CONFLICT; one marked allowed should complete
//! normally, which for a command the device does not have is its usual refusal. Each command is sent
//! once before any reservation, so the device's own answer to it is known. Needs a second initiator
//! name (or a local disk's second host adapter): without one, B and C are A's own name, which some
//! targets do not tell apart from A.

use super::block_io::{cdb10, cdb16};
use super::cases::{record, record_should, refused, Want};
use super::lun_access::{last_block, open, read_block, unit_attentions, write_block, Creds};
use super::ops;
use super::path::Path;
use super::port::Port;
use super::pr_commands::out;
use super::pr_ports::{clean_slate, ISID_A, ISID_B, ISID_C, KEY_A, KEY_B};
use super::session::Outcome;
use crate::compliance::Case;
use crate::Result;

const SECTION: &str = "pr";

/// What table 13 says of a command from a nexus that does not hold the reservation.
#[derive(Clone, Copy, PartialEq)]
enum Access {
    /// Allowed under every type (READ CAPACITY, START STOP UNIT start, PREVENT 0).
    Always,
    /// Allowed under Write Exclusive, conflict under Exclusive Access (VERIFY, PRE-FETCH, GET LBA
    /// STATUS).
    Reads,
    /// Conflict under both (writes, SYNCHRONIZE CACHE, UNMAP).
    Writes,
}

/// One command: its name, its row in table 13, and whether its CDB has a service action.
struct Command {
    name: &'static str,
    access: Access,
    service_action: bool,
}

const COMMANDS: [Command; 13] = [
    Command {
        name: "SYNCHRONIZE CACHE (10)",
        access: Access::Writes,
        service_action: false,
    },
    Command {
        name: "SYNCHRONIZE CACHE (16)",
        access: Access::Writes,
        service_action: false,
    },
    Command {
        name: "WRITE SAME (16)",
        access: Access::Writes,
        service_action: false,
    },
    Command {
        name: "WRITE AND VERIFY (16)",
        access: Access::Writes,
        service_action: false,
    },
    Command {
        name: "COMPARE AND WRITE",
        access: Access::Writes,
        service_action: false,
    },
    Command {
        name: "ORWRITE (16)",
        access: Access::Writes,
        service_action: false,
    },
    Command {
        name: "UNMAP",
        access: Access::Writes,
        service_action: false,
    },
    Command {
        name: "VERIFY (16)",
        access: Access::Reads,
        service_action: false,
    },
    Command {
        name: "PRE-FETCH (16)",
        access: Access::Reads,
        service_action: false,
    },
    Command {
        name: "GET LBA STATUS (16)",
        access: Access::Reads,
        service_action: true,
    },
    Command {
        name: "READ CAPACITY (16)",
        access: Access::Always,
        service_action: true,
    },
    Command {
        name: "START STOP UNIT (START 1)",
        access: Access::Always,
        service_action: false,
    },
    Command {
        name: "PREVENT ALLOW MEDIUM REMOVAL (PREVENT 0)",
        access: Access::Always,
        service_action: false,
    },
];

/// The reservation types, whether they are registrants-only, and how the cases name them.
const TYPES: [(u8, bool, &str); 4] = [
    (1, false, "type 1 (write exclusive)"),
    (3, false, "type 3 (exclusive access)"),
    (5, true, "type 5 (write exclusive, registrants only)"),
    (6, true, "type 6 (exclusive access, registrants only)"),
];

/// Sends `c` to the test block in its form that changes nothing: writes carry the block's own data,
/// ORWRITE zeros, UNMAP one block (written back afterwards), START STOP UNIT starts.
fn send(s: &mut Port, c: &Command, lba: u64, block: &[u8]) -> Result<Outcome> {
    match c.name {
        "SYNCHRONIZE CACHE (10)" => s.command(&cdb10(0x35, 0, lba as u32, 1), 0, None),
        "SYNCHRONIZE CACHE (16)" => s.command(&cdb16(0x91, 0, lba, 1), 0, None),
        "WRITE SAME (16)" => s.command(&cdb16(0x93, 0, lba, 1), 0, Some(block)),
        "WRITE AND VERIFY (16)" => s.command(&cdb16(0x8e, 0, lba, 1), 0, Some(block)),
        "COMPARE AND WRITE" => {
            let mut both = block.to_vec();
            both.extend_from_slice(block);
            ops::compare_and_write(s, lba, 1, &both)
        }
        "ORWRITE (16)" => s.command(&cdb16(0x8b, 0, lba, 1), 0, Some(&vec![0u8; block.len()])),
        "UNMAP" => ops::unmap(s, lba, 1),
        "VERIFY (16)" => s.command(&cdb16(0x8f, 0, lba, 1), 0, None),
        "PRE-FETCH (16)" => s.command(&cdb16(0x90, 0, lba, 1), 0, None),
        "GET LBA STATUS (16)" => {
            let mut c = cdb16(0x9e, 0x12, lba, 64);
            c[14] = 0;
            s.command(&c, 64, None)
        }
        "READ CAPACITY (16)" => s.command(&cdb16(0x9e, 0x10, 0, 32), 32, None),
        // IMMED 1: LIO takes only that form, and it is no less a start.
        "START STOP UNIT (START 1)" => s.command(&[0x1b, 0x01, 0, 0, 0x01, 0], 0, None),
        _ => s.command(&[0x1e, 0, 0, 0, 0, 0], 0, None),
    }
}

pub(super) fn run(
    path: &Path,
    initiator: &str,
    second: Option<&str>,
    creds: Creds,
    name: &str,
    cases: &mut Vec<Case>,
) {
    let Some(other) = second else {
        return;
    };
    let opened = (|| -> Result<(Port, Port, Port)> {
        Ok((
            open(path, initiator, creds, ISID_A)?,
            open(path, other, creds, ISID_B)?,
            open(path, other, creds, ISID_C)?,
        ))
    })();
    let (mut a, mut b, mut c) = match opened {
        Ok(p) => p,
        Err(e) => {
            record(
                cases,
                SECTION,
                format!("{name} command set: open A, B and C"),
                "open",
                e,
                false,
            );
            return;
        }
    };
    for s in [&mut a, &mut b, &mut c] {
        let _ = unit_attentions(s);
    }
    let Ok((last, len)) = last_block(&mut a) else {
        return;
    };
    let lba = u64::from(last.saturating_sub(1));
    let Ok(original) = read_block(&mut a, lba as u32, len) else {
        return;
    };
    // The device's own answer to each command, with no reservation.
    let supported: Vec<bool> = COMMANDS
        .iter()
        .map(|cmd| {
            send(&mut a, cmd, lba, &original).is_ok_and(|o| !refused(&o, cmd.service_action))
        })
        .collect();
    let _ = write_block(&mut a, lba as u32, &original);
    for (kind, registrants, label) in TYPES {
        let ready = out(&mut a, ops::PR_REGISTER_AND_IGNORE, 0, 0, KEY_A)
            .and_then(|_| out(&mut b, ops::PR_REGISTER_AND_IGNORE, 0, 0, KEY_B))
            .and_then(|_| out(&mut a, ops::PR_RESERVE, kind, KEY_A, 0));
        if !matches!(&ready, Ok(o) if o.status == 0) {
            record(
                cases,
                SECTION,
                format!("{name} command set {label}: A reserves"),
                "GOOD",
                ready.map_or_else(|e| e, |o| super::cases::describe(&o)),
                false,
            );
            clean_slate(path, initiator, creds, name, cases);
            continue;
        }
        for (who, s, registered) in [
            ("B", &mut b, true),
            ("C (not registered, B's initiator name)", &mut c, false),
        ] {
            for (cmd, &works) in COMMANDS.iter().zip(&supported) {
                let conflict = match cmd.access {
                    Access::Always => false,
                    _ if registrants && registered => false,
                    Access::Reads => kind == 3 || kind == 6,
                    Access::Writes => true,
                };
                let answer = send(s, cmd, lba, &original);
                let actual = answer
                    .as_ref()
                    .map_or_else(|e| e.clone(), super::cases::describe);
                let label = format!("{name} {label}: {who} {}", cmd.name);
                if conflict {
                    // A command the device does not have may be refused rather than conflict.
                    let ok = answer.as_ref().is_ok_and(|o| {
                        Want::Conflict.matches(o) || (!works && refused(o, cmd.service_action))
                    });
                    record(cases, SECTION, label, "RESERVATION CONFLICT", actual, ok);
                } else {
                    let ok = answer.as_ref().is_ok_and(|o| !Want::Conflict.matches(o));
                    record_should(
                        cases,
                        SECTION,
                        label,
                        "completes (no RESERVATION CONFLICT)",
                        actual,
                        ok,
                    );
                }
                let _ = unit_attentions(s);
            }
        }
        let _ = out(&mut a, ops::PR_RELEASE, kind, KEY_A, 0);
        let _ = write_block(&mut a, lba as u32, &original);
        for s in [&mut a, &mut b, &mut c] {
            let _ = unit_attentions(s);
        }
    }
    let _ = out(&mut a, ops::PR_CLEAR, 0, KEY_A, 0);
    let _ = write_block(&mut a, lba as u32, &original);
    let _ = (a.logout(), b.logout(), c.logout());
    clean_slate(path, initiator, creds, name, cases);
}
