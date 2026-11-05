//! The log page cases of the nvme-admin section (Base 2.1 5.1.12, Figure 202): Supported Log Pages
//! (00h) lists each page read here, Commands Supported and Effects (05h) lists every command the
//! compliance cases send, and the mandatory Error Information (01h), SMART / Health (02h) and
//! Firmware Slot (03h) pages answer with sane contents; a reserved page is refused. Windows' stornvme
//! reads these pages when it starts a controller, and SMART / Health for the disk's health.

use super::controller::Controller;
use super::identify_cases::Facts;
use super::revisions as rev;
use super::sqe::{self};
use super::verdict::{rev_case, status_case, INVALID_FIELD, INVALID_LOG_PAGE, SUCCESS};
use crate::compliance::{case, Case};

/// Log page identifiers (Figure 202).
const LOG_SUPPORTED: u8 = 0x00;
const LOG_EFFECTS: u8 = 0x05;
const LOG_ERROR: u8 = 0x01;
const LOG_SMART: u8 = 0x02;
const LOG_FIRMWARE: u8 = 0x03;
/// A log page identifier the base specification reserves.
const LOG_RESERVED: u8 = 0x6f;

/// Runs the log page cases on the enabled controller `c`.
pub fn run(c: &mut Controller, name: &str, facts: &Facts, cases: &mut Vec<Case>) {
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
}
