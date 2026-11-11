//! The optional commands of the `io` and `sbc` sections that a block LUN may or may not implement: READ LONG
//! (10) and WRITE LONG (10) (SBC-3 5.19, 5.41), SEND DIAGNOSTIC (SPC-3 6.28) and SET PRIORITY (SPC-3 6.30).
//! For each, REPORT SUPPORTED OPERATION CODES (SPC-3 6.23.1, table 159) says whether the LUN implements it. A
//! command it does not implement must be refused as an invalid operation code, which is what the case checks
//! when it is sent. A command it does implement is recorded as supported; those that would change the LUN or its
//! task priority are not sent (WRITE LONG writes, SET PRIORITY sets the priority of later tasks).

use super::cases::{describe, expect, record, Want};
use super::disk::{label, SECTION};
use super::operation_codes::support;
use super::port::Port;
use crate::compliance::Case;

/// SUPPORT 001b: the device does not support the command.
const NOT_SUPPORTED: u8 = 0x01;

/// One optional command and how to check it.
struct Optional {
    clause: &'static str,
    name: &'static str,
    opcode: u8,
    service_action: Option<u16>,
    /// The CDB to send when the device does not implement it. Sent only where the command has no effect when
    /// refused, and, when it is implemented, only if `send_if_supported` says so.
    cdb: &'static [u8],
    send_if_supported: bool,
}

const OPTIONAL: [Optional; 4] = [
    // SBC-3 5.19: READ LONG (10) with a byte count of zero reads nothing.
    Optional {
        clause: "SBC-3 5.19",
        name: "READ LONG (10)",
        opcode: 0x3e,
        service_action: None,
        cdb: &[0x3e, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        send_if_supported: true,
    },
    // SBC-3 5.41: WRITE LONG (10) with a byte count of zero writes nothing. Never sent when implemented.
    Optional {
        clause: "SBC-3 5.41",
        name: "WRITE LONG (10)",
        opcode: 0x3f,
        service_action: None,
        cdb: &[0x3f, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        send_if_supported: false,
    },
    // SPC-3 6.28: SEND DIAGNOSTIC with the default self test and no parameter list.
    Optional {
        clause: "SPC-3 6.28",
        name: "SEND DIAGNOSTIC",
        opcode: 0x1d,
        service_action: None,
        cdb: &[0x1d, 0x10, 0, 0, 0, 0],
        send_if_supported: true,
    },
    // SPC-3 6.30: SET PRIORITY is a MAINTENANCE OUT service action. Never sent: it sets the priority of tasks.
    Optional {
        clause: "SPC-3 6.30",
        name: "SET PRIORITY",
        opcode: 0xa4,
        service_action: Some(0x0e),
        cdb: &[0xa4, 0x0e, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        send_if_supported: false,
    },
];

pub(super) fn run(s: &mut Port, name: &str, cases: &mut Vec<Case>) {
    let at = |clause: &str, what: &str| label(name, &format!("{clause} {what}"));
    for item in &OPTIONAL {
        let Some(support) = support(s, item.opcode, item.service_action) else {
            record(
                cases,
                SECTION,
                at(
                    item.clause,
                    &format!("{} REPORT SUPPORTED OPERATION CODES answers", item.name),
                ),
                "an answer",
                "no answer",
                false,
            );
            continue;
        };
        if support == NOT_SUPPORTED {
            // Not implemented: sending it must be refused as an invalid operation code (or invalid field).
            expect(
                cases,
                SECTION,
                at(
                    item.clause,
                    &format!("{} refused, not implemented", item.name),
                ),
                &[Want::Ascq(0x5, 0x20, 0x00), Want::Ascq(0x5, 0x24, 0x00)],
                s.command(item.cdb, 0, None),
            );
        } else if item.send_if_supported {
            let answer = s.command(item.cdb, 0, None);
            let text = match &answer {
                Ok(o) => describe(o),
                Err(e) => e.to_string(),
            };
            record(
                cases,
                SECTION,
                at(item.clause, &format!("{} implemented: answered", item.name)),
                "GOOD, or CHECK CONDITION",
                text,
                answer.is_ok(),
            );
        } else {
            record(
                cases,
                SECTION,
                at(
                    item.clause,
                    &format!(
                        "{} implemented, not sent (it changes the LUN or task priority)",
                        item.name
                    ),
                ),
                "supported",
                format!("supported (SUPPORT {support:03b}b)"),
                true,
            );
        }
    }
}
