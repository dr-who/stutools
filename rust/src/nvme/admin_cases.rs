//! The nvme-admin cases of `--compliance` (NVMe base specification, admin command set): the mandatory
//! log pages (Error Information, SMART / Health, Firmware Slot) and the refusal of a reserved one, the
//! features a host reads at start-up (Number of Queues, Keep Alive Timer, Volatile Write Cache,
//! Asynchronous Event Configuration, Host Identifier) and the refusal of a reserved one, Set Features
//! Number of Queues, Abort, Keep Alive and a reserved opcode. Windows' stornvme reads these pages and
//! features when it starts a controller, and reads SMART / Health for the disk's health.
//! `async_event` checks that an Asynchronous Event Request stays outstanding while other commands
//! complete; it runs on its own association, last, because that request never completes on its own.
//! Base 2.1: log pages 5.1.12 (Figure 202), features 5.1.25 (Figure 385), Abort 5.1.1 (Figures 143
//! and 144), Asynchronous Event Request 5.1.2, Keep Alive 5.1.14.

use std::net::SocketAddr;

use super::compliance::{
    rev_case, status_case, INVALID_FIELD, INVALID_LOG_PAGE, INVALID_OPCODE, SUCCESS,
};
use super::controller::{self, Association, Controller};
use super::identify_cases::Facts;
use super::link::Data;
use super::revisions as rev;
use super::sqe::{self, Command};
use super::DEFAULT_HOSTNQN;
use crate::compliance::{case, Case};

/// Log page identifiers (Figure 202).
const LOG_SUPPORTED: u8 = 0x00;
const LOG_EFFECTS: u8 = 0x05;
const LOG_ERROR: u8 = 0x01;
const LOG_SMART: u8 = 0x02;
const LOG_FIRMWARE: u8 = 0x03;
/// A log page identifier the base specification reserves.
const LOG_RESERVED: u8 = 0x6f;

/// Feature identifiers.
const FEAT_VOLATILE_WRITE_CACHE: u8 = 0x06;
const FEAT_NUMBER_OF_QUEUES: u8 = 0x07;
const FEAT_ASYNC_EVENT_CONFIG: u8 = 0x0b;
const FEAT_KEEP_ALIVE_TIMER: u8 = 0x0f;
const FEAT_HOST_IDENTIFIER: u8 = 0x81;
/// A feature identifier the base specification reserves.
const FEAT_RESERVED: u8 = 0x6f;

/// An admin opcode the base specification reserves.
const RESERVED_ADMIN_OPCODE: u8 = 0x03;
/// The command ID Abort names: one this host never uses.
const ABSENT_CID: u16 = 0x7ffe;

/// Runs the nvme-admin cases on the enabled controller `c`, connected with `hostid`.
pub fn run(c: &mut Controller, name: &str, hostid: [u8; 16], facts: &Facts, cases: &mut Vec<Case>) {
    let admin = "nvme-admin";
    let vs = facts.vs;

    // Figure 31: Supported Log Pages (00h) lists each page with LSUPP (Figure 204 bit 0); the pages
    // this section reads must be listed.
    let mut pages = [0u8; 1024];
    let done = c.log_page(LOG_SUPPORTED, 0, 0, &mut pages);
    if rev_case(
        cases,
        admin,
        name,
        &rev::LOG_SUPPORTED,
        vs,
        &[SUCCESS],
        "success",
        &done,
    ) && done.as_ref().is_ok_and(|d| d.ok())
    {
        let listed = |lid: u8| pages[4 * lid as usize] & 1 != 0;
        let mut want = vec![LOG_ERROR, LOG_SMART, LOG_FIRMWARE, LOG_EFFECTS];
        if facts.ctrl.cmic & 0x08 != 0 {
            want.push(0x0c);
        }
        if facts.ctrl.oncs & (1 << 5) != 0 {
            want.push(0x80);
        }
        let show = |l: &u8| format!("{l:02x}h");
        let missing: Vec<String> = want.iter().filter(|&&l| !listed(l)).map(show).collect();
        cases.push(case(
            admin,
            format!("{name} Supported Log Pages lists the pages read"),
            format!(
                "{} listed",
                want.iter().map(show).collect::<Vec<_>>().join(", ")
            ),
            if missing.is_empty() {
                "all listed".into()
            } else {
                format!("missing {}", missing.join(", "))
            },
            missing.is_empty(),
        ));
    }
    // Figure 209: Commands Supported and Effects, 4 bytes per admin opcode then per I/O opcode, CSUPP
    // in bit 0 (Figure 210). Every command these cases send, and each optional one ONCS lists, must
    // be listed; stornvme reads this page.
    let mut effects = vec![0u8; 4096];
    let done = c.log_page(LOG_EFFECTS, 0, 0, &mut effects);
    if rev_case(
        cases,
        admin,
        name,
        &rev::LOG_EFFECTS,
        vs,
        &[SUCCESS],
        "success",
        &done,
    ) && done.as_ref().is_ok_and(|d| d.ok())
    {
        let oncs = facts.ctrl.oncs;
        let mut io = vec![sqe::OP_FLUSH, sqe::OP_WRITE, sqe::OP_READ];
        let optional: [(u16, &[u8]); 4] = [
            (0, &[sqe::OP_COMPARE]),
            (2, &[sqe::OP_DATASET_MANAGEMENT]),
            (3, &[sqe::OP_WRITE_ZEROES]),
            (
                5,
                &[
                    sqe::OP_RESV_REGISTER,
                    sqe::OP_RESV_REPORT,
                    sqe::OP_RESV_ACQUIRE,
                    sqe::OP_RESV_RELEASE,
                ],
            ),
        ];
        for (bit, ops) in optional {
            if oncs & (1 << bit) != 0 {
                io.extend_from_slice(ops);
            }
        }
        let admin_ops = [
            sqe::OP_GET_LOG_PAGE,
            sqe::OP_IDENTIFY,
            sqe::OP_ABORT,
            sqe::OP_SET_FEATURES,
            sqe::OP_GET_FEATURES,
            sqe::OP_ASYNC_EVENT,
            sqe::OP_KEEP_ALIVE,
        ];
        let mut missing: Vec<String> = admin_ops
            .iter()
            .filter(|&&o| effects[4 * o as usize] & 1 == 0)
            .map(|o| format!("admin {o:02x}h"))
            .collect();
        missing.extend(
            io.iter()
                .filter(|&&o| effects[1024 + 4 * o as usize] & 1 == 0)
                .map(|o| format!("I/O {o:02x}h")),
        );
        cases.push(case(
            admin,
            format!("{name} Commands Supported and Effects lists the commands used"),
            "every admin and I/O command these cases send, and each ONCS lists".into(),
            if missing.is_empty() {
                "all listed".into()
            } else {
                format!("missing {}", missing.join(", "))
            },
            missing.is_empty(),
        ));
    }

    let mut error_log = [0u8; 64];
    let done = c.log_page(LOG_ERROR, 0, 0, &mut error_log);
    rev_case(
        cases,
        admin,
        name,
        &rev::LOG_ERROR,
        vs,
        &[SUCCESS],
        "success",
        &done,
    );
    let mut smart = [0u8; 512];
    let done = c.log_page(LOG_SMART, 0xffff_ffff, 0, &mut smart);
    if rev_case(
        cases,
        admin,
        name,
        &rev::LOG_SMART,
        vs,
        &[SUCCESS],
        "success",
        &done,
    ) && done.as_ref().is_ok_and(|d| d.ok())
    {
        // Figure 202 note 1: NSID FFFFFFFFh asks for the controller-wide SMART / Health page.
        // Composite temperature in kelvins at bytes 1 and 2: 0 means not reported.
        let kelvin = u16::from_le_bytes([smart[1], smart[2]]);
        cases.push(case(
            admin,
            format!("{name} SMART / Health contents"),
            "critical warning and temperature reported".into(),
            format!("critical warning {:#04x}, temperature {kelvin} K", smart[0]),
            true,
        ));
    }
    let mut slots = [0u8; 512];
    let done = c.log_page(LOG_FIRMWARE, 0, 0, &mut slots);
    if rev_case(
        cases,
        admin,
        name,
        &rev::LOG_FIRMWARE,
        vs,
        &[SUCCESS],
        "success",
        &done,
    ) && done.as_ref().is_ok_and(|d| d.ok())
    {
        // Figure 208 AFI bits 2:0 (CAFS): the slot the running firmware came from; slots are 1 to 7,
        // and the page is mandatory for an I/O controller (Figure 31).
        let slot = slots[0] & 0x7;
        cases.push(case(
            admin,
            format!("{name} active firmware slot"),
            "1 to 7".into(),
            format!("slot {slot}"),
            (1..=7).contains(&slot),
        ));
    }
    // Figure 202: 24h to 6Fh are reserved; an unsupported page is Invalid Log Page (Figure 103).
    let mut reserved = [0u8; 4];
    let done = c.log_page(LOG_RESERVED, 0, 0, &mut reserved);
    status_case(
        cases,
        admin,
        format!("{name} reserved log page {LOG_RESERVED:#04x}"),
        &[INVALID_LOG_PAGE, INVALID_FIELD],
        "Invalid Log Page or Invalid Field",
        &done,
    );

    // 5.1.25.2.1: Set Features Number of Queues before any I/O queue exists, here one submission and
    // one completion queue (0-based); DW0 says what was allocated (Figure 444).
    let mut cmd = Command::new(sqe::OP_SET_FEATURES, 0);
    cmd.set_u32(40, FEAT_NUMBER_OF_QUEUES as u32);
    cmd.set_u32(44, 0);
    let done = c.submit(cmd, Data::None);
    status_case(
        cases,
        admin,
        format!("{name} {} (set)", rev::FEAT_QUEUES.name()),
        &[SUCCESS],
        "success",
        &done,
    );
    let done = c.get_features(FEAT_NUMBER_OF_QUEUES, 0, None);
    if rev_case(
        cases,
        admin,
        name,
        &rev::FEAT_QUEUES,
        vs,
        &[SUCCESS],
        "success",
        &done,
    ) && done.as_ref().is_ok_and(|d| d.ok())
    {
        let dw0 = done.as_ref().map(|d| d.dw0).unwrap_or(0);
        cases.push(case(
            admin,
            format!("{name} queues allocated"),
            "1 or more of each".into(),
            format!(
                "{} submission, {} completion",
                (dw0 & 0xffff) + 1,
                (dw0 >> 16) + 1
            ),
            true,
        ));
    }
    // 5.1.25.1.8: the KATO Connect asked for, rounded up to the KAS granularity (100 ms units).
    let done = c.get_features(FEAT_KEEP_ALIVE_TIMER, 0, None);
    let step = 100 * facts.ctrl.kas.max(1) as u32;
    let kato = done.as_ref().ok().filter(|d| d.ok()).map(|d| d.dw0);
    cases.push(case(
        admin,
        format!("{name} {}", rev::FEAT_KATO.name()),
        format!(
            "{} ms, or rounded up to {step} ms steps",
            controller::KATO_MS
        ),
        match (&done, kato) {
            (_, Some(v)) => format!("{v} ms"),
            (Ok(d), None) => {
                let (sct, sc) = d.code();
                format!("status type {sct:#x} code {sc:#04x}")
            }
            (Err(e), None) => e.clone(),
        },
        kato.is_some_and(|v| v >= controller::KATO_MS && v < controller::KATO_MS + step),
    ));
    // 5.1.25.1.4: a controller with a volatile write cache (VWC bit 0) supports the feature; one
    // without aborts Get Features with Invalid Field in Command.
    let has_cache = facts.ctrl.vwc & 1 != 0;
    let done = c.get_features(FEAT_VOLATILE_WRITE_CACHE, 0, None);
    status_case(
        cases,
        admin,
        format!("{name} {}", rev::FEAT_VWC.name()),
        if has_cache {
            &[SUCCESS]
        } else {
            &[INVALID_FIELD]
        },
        if has_cache {
            "success: VWC says there is a cache"
        } else {
            "Invalid Field: VWC says there is no cache"
        },
        &done,
    );
    let done = c.get_features(FEAT_ASYNC_EVENT_CONFIG, 0, None);
    rev_case(
        cases,
        admin,
        name,
        &rev::FEAT_AEC,
        vs,
        &[SUCCESS],
        "success",
        &done,
    );
    // 5.1.25.1.28.2: mandatory over fabrics, with EXHID (CDW11 bit 0) for the 128-bit identifier
    // Connect gave, which reservations use to tell hosts apart.
    let mut id = [0u8; 16];
    let done = c.get_features(FEAT_HOST_IDENTIFIER, 1, Some(&mut id));
    let ok = matches!(&done, Ok(d) if d.ok());
    cases.push(case(
        admin,
        format!("{name} {}", rev::FEAT_HOST_ID.name()),
        "the host ID Connect gave".into(),
        match &done {
            Ok(d) if d.ok() && id == hostid => "the same".into(),
            Ok(d) if d.ok() => "a different identifier".into(),
            Ok(d) => {
                let (sct, sc) = d.code();
                format!("status type {sct:#x} code {sc:#04x}")
            }
            Err(e) => e.clone(),
        },
        ok && id == hostid,
    ));
    // 5.1.25: an unsupported Feature Identifier is Invalid Field in Command; 22h to 77h are reserved.
    let done = c.get_features(FEAT_RESERVED, 0, None);
    status_case(
        cases,
        admin,
        format!("{name} Get Features reserved {FEAT_RESERVED:#04x}"),
        &[INVALID_FIELD],
        "Invalid Field in Command",
        &done,
    );

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
