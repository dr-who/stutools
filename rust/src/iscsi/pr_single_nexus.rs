//! The `pr` cases that one I_T nexus can make: the reservation key of one initiator port, a reservation
//! held by it, and the rules for releasing, unregistering and clearing (SPC-3 5.6.6, 5.6.8 to 5.6.10.2,
//! 5.6.10.3, 5.6.10.6). A local disk has one nexus, so the three-port cases in `reservations` cannot run
//! there; these do. Every command is sent through the session the conformance run already holds, and the
//! LUN is left with no registration and no reservation.

use super::cases::{expect, record, record_info, sense, Want};
use super::ops;
use super::port::Port;
use super::pr_commands::{out, read_holder, read_keys, show_holder, show_keys, TYPES};
use super::pr_ports::{KEY_A, KEY_B2, SECTION};
use crate::compliance::Case;

/// Runs the single-nexus cases on `s`. Records one "not run" when PERSISTENT RESERVE OUT is not served.
pub(super) fn run(s: &mut Port, name: &str, cases: &mut Vec<Case>) {
    let at = |what: &str| format!("{name} {what}");
    let probe = out(s, ops::PR_REGISTER, 0, 0, KEY_A);
    if let Ok(o) = &probe {
        let (key, asc, _) = sense(&o.sense);
        if o.status == 2 && key == 0x5 && asc == 0x20 {
            record_info(
                cases,
                SECTION,
                at("PERSISTENT RESERVE OUT, one initiator"),
                "not run: PERSISTENT RESERVE OUT is refused as an invalid operation code",
            );
            return;
        }
    }
    expect(
        cases,
        SECTION,
        at("SPC-3 5.6.6 REGISTER a reservation key for this nexus"),
        &[Want::Good],
        probe,
    );
    registered(s, name, cases, true, "after REGISTER (5.6.6)");

    // 5.6.8: a holder reserves with its key, and the holder's key is returned by READ RESERVATION
    // (5.6.9), except for the all-registrants types, where it is zero.
    for (kind, label) in TYPES {
        let reserve = out(s, ops::PR_RESERVE, kind, KEY_A, 0);
        expect(
            cases,
            SECTION,
            at(&format!("SPC-3 5.6.8 RESERVE, {label}")),
            &[Want::Good],
            reserve,
        );
        let all = matches!(kind, 7 | 8);
        let want_key = if all { 0 } else { KEY_A };
        let holder = read_holder(s);
        let ok = matches!(&holder, Ok(Some((k, t))) if *k == want_key && *t == kind);
        record(
            cases,
            SECTION,
            at(&format!("SPC-3 5.6.9 READ RESERVATION holds {label}")),
            format!("key {want_key:#x}, type {kind}"),
            show_holder(&holder),
            ok,
        );
        // Reserving again with the same type and scope changes nothing (5.6.8).
        let again = out(s, ops::PR_RESERVE, kind, KEY_A, 0);
        expect(
            cases,
            SECTION,
            at(&format!("SPC-3 5.6.8 RESERVE again, same type, {label}")),
            &[Want::Good],
            again,
        );
        // A holder may not change the type of its reservation (5.6.8).
        let other = if kind == 1 { 5 } else { 1 };
        let changed = out(s, ops::PR_RESERVE, other, KEY_A, 0);
        expect(
            cases,
            SECTION,
            at(&format!(
                "SPC-3 5.6.8 RESERVE with another type while {label} is held"
            )),
            &[Want::Conflict],
            changed,
        );
        // A release that names another type is refused and leaves the reservation (5.6.10.2).
        let wrong = out(s, ops::PR_RELEASE, other, KEY_A, 0);
        expect(
            cases,
            SECTION,
            at(&format!(
                "SPC-3 5.6.10.2 RELEASE with another type while {label} is held"
            )),
            &[Want::Ascq(0x5, 0x26, 0x04)],
            wrong,
        );
        let release = out(s, ops::PR_RELEASE, kind, KEY_A, 0);
        expect(
            cases,
            SECTION,
            at(&format!("SPC-3 5.6.10.2 RELEASE by the holder, {label}")),
            &[Want::Good],
            release,
        );
        let after = read_holder(s);
        record(
            cases,
            SECTION,
            at(&format!(
                "SPC-3 5.6.10.2 after RELEASE of {label}, no reservation"
            )),
            "no reservation",
            show_holder(&after),
            matches!(after, Ok(None)),
        );
        // The registration stays after a release (5.6.10.2 b).
        registered(
            s,
            name,
            cases,
            true,
            &format!("after RELEASE of {label} (5.6.10.2)"),
        );
    }

    // With no reservation, RELEASE from a registered nexus returns GOOD and changes nothing (5.6.10.2).
    let idle = out(s, ops::PR_RELEASE, 1, KEY_A, 0);
    expect(
        cases,
        SECTION,
        at("SPC-3 5.6.10.2 RELEASE with no reservation"),
        &[Want::Good],
        idle,
    );

    // A registration with the key changed to a new one (5.6.6), then unregistered with a zero key (5.6.10.3).
    let moved = out(s, ops::PR_REGISTER, 0, KEY_A, KEY_B2);
    expect(
        cases,
        SECTION,
        at("SPC-3 5.6.6 REGISTER AND replace key A with key B2"),
        &[Want::Good],
        moved,
    );
    registered_only(s, name, cases, KEY_B2, "after replacing the key (5.6.6)");
    let gone = out(s, ops::PR_REGISTER, 0, KEY_B2, 0);
    expect(
        cases,
        SECTION,
        at("SPC-3 5.6.10.3 REGISTER with a zero service key unregisters"),
        &[Want::Good],
        gone,
    );
    registered(s, name, cases, false, "after unregistering (5.6.10.3)");

    // CLEAR removes every registration and any reservation (5.6.10.6). Register, reserve, then clear.
    let again = out(s, ops::PR_REGISTER, 0, 0, KEY_A);
    expect(
        cases,
        SECTION,
        at("SPC-3 5.6.6 REGISTER again before CLEAR"),
        &[Want::Good],
        again,
    );
    let reserve = out(s, ops::PR_RESERVE, 1, KEY_A, 0);
    expect(
        cases,
        SECTION,
        at("SPC-3 5.6.8 RESERVE before CLEAR"),
        &[Want::Good],
        reserve,
    );
    let clear = out(s, ops::PR_CLEAR, 0, KEY_A, 0);
    expect(
        cases,
        SECTION,
        at("SPC-3 5.6.10.6 CLEAR by the registered nexus"),
        &[Want::Good],
        clear,
    );
    registered(s, name, cases, false, "after CLEAR (5.6.10.6)");
    let held = read_holder(s);
    record(
        cases,
        SECTION,
        at("SPC-3 5.6.10.6 after CLEAR, no reservation"),
        "no reservation",
        show_holder(&held),
        matches!(held, Ok(None)),
    );
}

/// Whether the reservation key of this nexus is registered, as READ KEYS reports it.
fn registered(s: &mut Port, name: &str, cases: &mut Vec<Case>, want: bool, when: &str) {
    let keys = read_keys(s);
    let has = matches!(&keys, Ok((_, k)) if k.contains(&KEY_A));
    record(
        cases,
        SECTION,
        format!("{name} SPC-3 5.6.6 READ KEYS {when}"),
        if want {
            "key A registered"
        } else {
            "key A not registered"
        },
        show_keys(&keys),
        has == want,
    );
}

/// Whether `key` is the only registration, as READ KEYS reports it.
fn registered_only(s: &mut Port, name: &str, cases: &mut Vec<Case>, key: u64, when: &str) {
    let keys = read_keys(s);
    let only = matches!(&keys, Ok((_, k)) if k.as_slice() == [key]);
    record(
        cases,
        SECTION,
        format!("{name} SPC-3 5.6.6 READ KEYS {when}"),
        format!("only key {key:#x} registered"),
        show_keys(&keys),
        only,
    );
}
