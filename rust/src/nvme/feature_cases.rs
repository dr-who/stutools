//! The feature cases of the nvme-admin section (Base 2.1 5.1.25, Figure 385): Set Features Number of
//! Queues before any I/O queue exists, and Get Features of what a host reads at start-up: Number of
//! Queues, Keep Alive Timer (the KATO Connect asked for), Volatile Write Cache (against Identify
//! VWC), Asynchronous Event Configuration and Host Identifier (the 128-bit ID Connect gave); a
//! reserved feature is refused.

use super::controller::{self, Controller};
use super::identify_cases::Facts;
use super::link::Data;
use super::revisions as rev;
use super::sqe::{self, Command};
use super::verdict::{rev_case, status_case, INVALID_FIELD, SUCCESS};
use crate::compliance::{case, Case};

/// Feature identifiers.
const FEAT_VOLATILE_WRITE_CACHE: u8 = 0x06;
const FEAT_NUMBER_OF_QUEUES: u8 = 0x07;
const FEAT_ASYNC_EVENT_CONFIG: u8 = 0x0b;
const FEAT_KEEP_ALIVE_TIMER: u8 = 0x0f;
const FEAT_HOST_IDENTIFIER: u8 = 0x81;
/// A feature identifier the base specification reserves.
const FEAT_RESERVED: u8 = 0x6f;

/// Runs the feature cases on the enabled controller `c`, connected with `hostid`.
pub fn run(c: &mut Controller, name: &str, hostid: [u8; 16], facts: &Facts, cases: &mut Vec<Case>) {
    let admin = "nvme-admin";
    let vs = facts.vs;

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
}
