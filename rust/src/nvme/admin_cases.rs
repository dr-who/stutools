//! The nvme-admin cases of `--compliance` beyond log pages (`log_page_cases`) and features
//! (`feature_cases`): Abort of a command that is not outstanding, Keep Alive, a reserved admin
//! opcode, and `async_event`, which checks that an Asynchronous Event Request stays outstanding while
//! other commands complete; it runs on its own association, last, because that request never
//! completes on its own. Base 2.1: Abort 5.1.1 (Figures 143 and 144), Asynchronous Event Request
//! 5.1.2, Keep Alive 5.1.14.

use std::net::SocketAddr;

use super::controller::{self, Association, Controller};
use super::identify_cases::Facts;
use super::link::Data;
use super::revisions as rev;
use super::sqe::{self, Command};
use super::verdict::{status_case, INVALID_OPCODE};
use super::DEFAULT_HOSTNQN;
use crate::compliance::{case, Case};

/// An admin opcode the base specification reserves.
const RESERVED_ADMIN_OPCODE: u8 = 0x03;
/// The command ID Abort names: one this host never uses.
const ABSENT_CID: u16 = 0x7ffe;

/// Runs Abort, Keep Alive and the reserved opcode on the enabled controller `c`.
pub fn run(c: &mut Controller, name: &str, facts: &Facts, cases: &mut Vec<Case>) {
    let admin = "nvme-admin";
    let _vs = facts.vs;

    // 5.1.1, Figures 143 and 144: Abort names SQID and CID; for a command that is not found the
    // immediate abort is not performed, so DW0 bit 0 (IANP) is set.
    let mut abort = Command::new(sqe::OP_ABORT, 0);
    abort.set_u32(40, (ABSENT_CID as u32) << 16);
    let done = c.submit(abort, Data::None);
    cases.push(case(
        admin,
        format!("{name} {} of a command not outstanding", rev::ABORT.name()),
        "success, not aborted (DW0 bit 0)".into(),
        match &done {
            Ok(d) if d.ok() => format!("success, DW0 {:#x}", d.dw0),
            Ok(d) => {
                let (sct, sc) = d.code();
                format!("status type {sct:#x} code {sc:#04x}")
            }
            Err(e) => e.clone(),
        },
        matches!(&done, Ok(d) if d.ok() && d.dw0 & 1 == 1),
    ));

    let kept = c.keep_alive();
    cases.push(case(
        admin,
        format!("{name} {}", rev::KEEP_ALIVE.name()),
        "success".into(),
        match &kept {
            Ok(()) => "success".into(),
            Err(e) => e.clone(),
        },
        kept.is_ok(),
    ));
    let done = c.submit(Command::new(RESERVED_ADMIN_OPCODE, 0), Data::None);
    status_case(
        cases,
        admin,
        format!("{name} reserved admin opcode {RESERVED_ADMIN_OPCODE:#04x}"),
        &[INVALID_OPCODE],
        "Invalid Command Opcode",
        &done,
    );
}

/// An Asynchronous Event Request on a new association must stay outstanding while a Keep Alive sent
/// after it completes. A request that completes because an event is pending is also correct.
pub fn async_event(addr: SocketAddr, subnqn: &str, name: &str, cases: &mut Vec<Case>) {
    let result = (|| -> crate::Result<String> {
        let assoc = Association {
            addr,
            subnqn,
            hostnqn: DEFAULT_HOSTNQN,
            hostid: controller::random_hostid()?,
        };
        let mut c = Controller::connect(&assoc)?;
        c.enable()?;
        let aer = c.post(Command::new(sqe::OP_ASYNC_EVENT, 0))?;
        let ka = c.post(Command::new(sqe::OP_KEEP_ALIVE, 0))?;
        let first = c.next_completion()?;
        if first.cid == ka && first.ok() {
            return Ok("Keep Alive completed, the request is outstanding".into());
        }
        if first.cid == aer && first.ok() {
            // An event was pending: the request completed with it, and Keep Alive follows.
            let then = c.next_completion()?;
            if then.cid == ka && then.ok() {
                return Ok(format!(
                    "completed with event {:#x}, then Keep Alive",
                    first.dw0
                ));
            }
        }
        let (sct, sc) = first.code();
        Err(format!(
            "command {} completed with status type {sct:#x} code {sc:#04x}",
            first.cid
        ))
    })();
    cases.push(case(
        "nvme-admin",
        format!("{name} {}", rev::ASYNC_EVENT.name()),
        "outstanding while Keep Alive completes".into(),
        match &result {
            Ok(s) => s.clone(),
            Err(e) => e.clone(),
        },
        result.is_ok(),
    ));
}
