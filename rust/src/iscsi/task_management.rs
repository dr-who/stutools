//! The `tmf` cases of `--compliance`: iSCSI task management (RFC 7143 11.5, 11.6; SAM-6 7), which
//! Windows MSDSM and Linux use to recover a path: ABORT TASK of a task that no longer exists, ABORT TASK
//! SET, CLEAR TASK SET, LOGICAL UNIT RESET (other nexuses are told BUS DEVICE RESET FUNCTION OCCURRED,
//! and a persistent reservation survives it, SPC-6 5.14.1), an undefined function, and the RFC 7144
//! functions (QUERY TASK, QUERY TASK SET, QUERY ASYNCHRONOUS EVENT, I_T NEXUS RESET), which are required
//! only when the target negotiated iSCSIProtocolLevel 1 or more. TARGET RESET is obsolete (SAM-6) and is
//! never sent.

use super::cases::{expect, record, Want};
use super::login;
use super::lun_access::{open, show_attentions, unit_attentions, Creds};
use super::ops;
use super::path::Path;
use super::pdu;
use super::seen::Seen;
use super::session::{Session, TaskAnswer};
use crate::compliance::Case;
use crate::Result;

const SECTION: &str = "tmf";
const KEY: u64 = 0x5350_4954_0000_007e;
/// An ISID for the reset test's holder, apart from the reservation section's.
const ISID_HOLDER: [u8; 6] = [0x80, 0x7e, 0x53, 0x50, 0x00, 0x01];

fn show(r: &Result<TaskAnswer>) -> String {
    match r {
        Ok(TaskAnswer::Response(0)) => "function complete (0)".into(),
        Ok(TaskAnswer::Response(1)) => "task does not exist (1)".into(),
        Ok(TaskAnswer::Response(5)) => "function not supported (5)".into(),
        Ok(TaskAnswer::Response(7)) => "function succeeded (7)".into(),
        Ok(TaskAnswer::Response(255)) => "function rejected (255)".into(),
        Ok(TaskAnswer::Response(n)) => format!("response {n}"),
        Ok(TaskAnswer::Rejected(reason)) => format!("Reject, reason {reason:#04x}"),
        Err(e) => e.clone(),
    }
}

fn answer_is(r: &Result<TaskAnswer>, allowed: &[u8]) -> bool {
    matches!(r, Ok(TaskAnswer::Response(n)) if allowed.contains(n))
}

/// Runs the task management cases on one target. `protocol_level` is the iSCSIProtocolLevel the target
/// negotiated, if it answered the key.
pub(super) fn run(
    path: &Path,
    initiator: &str,
    creds: Creds,
    name: &str,
    protocol_level: Option<u32>,
    cases: &mut Vec<Case>,
    seen: &mut Seen,
) {
    let label = |what: &str| format!("{name} {what}");
    let mut s = match open(path, initiator, creds, login::random_isid()) {
        Ok(s) => s,
        Err(err) => {
            record(cases, SECTION, label("login"), "login ok", err, false);
            return;
        }
    };
    let _ = unit_attentions(&mut s);
    // RFC 7143 11.6.1 c): a task tag that names no task, with RefCmdSN outside the CmdSN window (a CmdSN
    // already used), is "Task does not exist".
    let used = s.cmd_sn().wrapping_sub(1);
    let abort = s.task_management(pdu::TMF_ABORT_TASK, 0x7fff_fff0, used);
    let ok = answer_is(&abort, &[1]);
    seen.fact("TMF ABORT TASK", answer_is(&abort, &[0, 1]));
    record(
        cases,
        SECTION,
        label("ABORT TASK of a task that does not exist"),
        "task does not exist (1)",
        show(&abort),
        ok,
    );
    // SAM-6 7.2, 7.3: with nothing in the task set these complete. RFC 7143 11.6.1 lets a target
    // answer "function not supported" (5); Discovery 2016 requires both (see features).
    let set = s.task_management(pdu::TMF_ABORT_TASK_SET, 0, 0);
    seen.fact("TMF ABORT TASK SET", answer_is(&set, &[0]));
    record(
        cases,
        SECTION,
        label("ABORT TASK SET"),
        "function complete (0), or not supported (5)",
        show(&set),
        answer_is(&set, &[0, 5]),
    );
    let clear = s.task_management(pdu::TMF_CLEAR_TASK_SET, 0, 0);
    seen.fact("TMF CLEAR TASK SET", answer_is(&clear, &[0]));
    record(
        cases,
        SECTION,
        label("CLEAR TASK SET"),
        "function complete (0), or not supported (5)",
        show(&clear),
        answer_is(&clear, &[0, 5]),
    );
    let _ = unit_attentions(&mut s);
    logical_unit_reset(path, initiator, creds, name, &mut s, cases, seen);
    rfc7144(path, initiator, creds, name, protocol_level, cases);
    let _ = s.logout();
    // RFC 7143 11.5.1: function codes past those defined are not task management functions the target
    // supports; it answers 5, 255, or rejects the PDU. A throwaway session, in case it drops it.
    if let Ok(mut t) = open(path, initiator, creds, login::random_isid()) {
        let odd = t.task_management(14, 0, 0);
        let ok = answer_is(&odd, &[5, 255]) || matches!(odd, Ok(TaskAnswer::Rejected(_)));
        record(
            cases,
            SECTION,
            label("undefined task management function 14"),
            "not supported (5), rejected (255) or Reject",
            show(&odd),
            ok,
        );
    }
}

/// SAM-6 6.3.3: a LOGICAL UNIT RESET establishes a unit attention for the other nexuses (BUS DEVICE
/// RESET FUNCTION OCCURRED, 29h/03h, or the general 29h/00h, SAM-6 table 49); SPC-6 5.14.1: persistent
/// reservations are not reset by it.
fn logical_unit_reset(
    path: &Path,
    initiator: &str,
    creds: Creds,
    name: &str,
    s: &mut Session,
    cases: &mut Vec<Case>,
    seen: &mut Seen,
) {
    let label = |what: &str| format!("{name} {what}");
    let Ok(mut holder) = open(path, initiator, creds, ISID_HOLDER) else {
        record(
            cases,
            SECTION,
            label("a second nexus for LOGICAL UNIT RESET"),
            "login ok",
            "refused",
            false,
        );
        return;
    };
    let _ = unit_attentions(&mut holder);
    let reg = ops::persistent_reserve_out(
        &mut holder,
        ops::PR_REGISTER_AND_IGNORE,
        0,
        &ops::pr_parameters(0, KEY),
    );
    let res = ops::persistent_reserve_out(
        &mut holder,
        ops::PR_RESERVE,
        ops::PR_WRITE_EXCLUSIVE,
        &ops::pr_parameters(KEY, 0),
    );
    let reserved = matches!((&reg, &res), (Ok(a), Ok(b)) if a.status == 0 && b.status == 0);
    let reset = s.task_management(pdu::TMF_LOGICAL_UNIT_RESET, 0, 0);
    seen.fact("TMF LOGICAL UNIT RESET", answer_is(&reset, &[0]));
    record(
        cases,
        SECTION,
        label("LOGICAL UNIT RESET"),
        "function complete (0)",
        show(&reset),
        answer_is(&reset, &[0]),
    );
    let told = unit_attentions(&mut holder);
    record(
        cases,
        SECTION,
        label("the other nexus is told of the reset"),
        "29h/03h or 29h/00h",
        show_attentions(&told),
        told.iter()
            .any(|&(asc, ascq)| asc == 0x29 && matches!(ascq, 0x00 | 0x03)),
    );
    let _ = unit_attentions(s);
    if reserved {
        let o = ops::persistent_reserve_in(s, ops::PR_IN_READ_RESERVATIONS, 512);
        let kept = matches!(&o, Ok(o) if o.status == 0 && o.data.len() >= 22 && o.data[8..16] == KEY.to_be_bytes() && o.data[21] & 0x0f == 1);
        record(
            cases,
            SECTION,
            label("persistent reservation survives LOGICAL UNIT RESET"),
            format!("key {KEY:#x}, type 1"),
            if kept {
                "kept".to_string()
            } else {
                "lost".to_string()
            },
            kept,
        );
    } else {
        record(
            cases,
            SECTION,
            label("reserve before LOGICAL UNIT RESET"),
            "GOOD",
            "refused",
            false,
        );
    }
    let clear =
        ops::persistent_reserve_out(&mut holder, ops::PR_CLEAR, 0, &ops::pr_parameters(KEY, 0));
    expect(
        cases,
        SECTION,
        label("CLEAR after the reset test"),
        &[Want::Good],
        clear,
    );
    let _ = unit_attentions(s);
    let _ = holder.logout();
}

/// RFC 7144 6.3, 6.4.2: QUERY TASK of a task not in the task set and QUERY TASK SET with nothing in it
/// are "Function complete"; QUERY ASYNCHRONOUS EVENT is "Function complete" or, with a unit attention
/// pending, "Function succeeded"; I_T NEXUS RESET is "Function complete" and drops the connections, so
/// it runs on a session of its own. Without iSCSIProtocolLevel 1 they may be "not supported".
fn rfc7144(
    path: &Path,
    initiator: &str,
    creds: Creds,
    name: &str,
    level: Option<u32>,
    cases: &mut Vec<Case>,
) {
    let label = |what: &str| format!("{name} {what}");
    let required = level.is_some_and(|l| l >= 1);
    let note = if required {
        "(iSCSIProtocolLevel >= 1)"
    } else {
        "(no iSCSIProtocolLevel: may be not supported)"
    };
    let with = |codes: &[u8]| -> Vec<u8> {
        let mut v = codes.to_vec();
        if !required {
            v.push(5);
        }
        v
    };
    let checks: [(&str, u8, Vec<u8>); 3] = [
        (
            "QUERY TASK of a task that does not exist",
            pdu::TMF_QUERY_TASK,
            with(&[0]),
        ),
        ("QUERY TASK SET", pdu::TMF_QUERY_TASK_SET, with(&[0])),
        (
            "QUERY ASYNCHRONOUS EVENT",
            pdu::TMF_QUERY_ASYNC_EVENT,
            with(&[0, 7]),
        ),
    ];
    // Each on a session of its own: a target that does not know a function may drop the connection.
    for (what, function, allowed) in checks {
        let r = open(path, initiator, creds, login::random_isid()).and_then(|mut t| {
            let cmd_sn = t.cmd_sn();
            let r = t.task_management(function, 0x7fff_fff1, cmd_sn);
            if r.is_ok() {
                let _ = t.logout();
            }
            r
        });
        let expected = allowed
            .iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(" or ");
        record(
            cases,
            SECTION,
            label(&format!("{what} {note}")),
            expected,
            show(&r),
            answer_is(&r, &allowed),
        );
    }
    if let Ok(mut t) = open(path, initiator, creds, login::random_isid()) {
        let r = t.task_management(pdu::TMF_I_T_NEXUS_RESET, 0, 0);
        // RFC 7144 6.4.2: the response may be lost as the target drops the connections.
        let ok = answer_is(&r, &with(&[0])) || r.is_err();
        record(
            cases,
            SECTION,
            label(&format!("I_T NEXUS RESET {note}")),
            "function complete (0), or the connection dropped",
            show(&r),
            ok,
        );
    }
}
