//! The `pr` cases that one I_T nexus can make: the reservation key of one initiator port, a reservation
//! held by it, and the rules for releasing, unregistering and clearing (SPC-3 5.6.6, 5.6.8 to 5.6.10.2,
//! 5.6.10.3, 5.6.10.6). A local disk has one nexus, so the three-port cases in `reservations` cannot run
//! there; these do. Every command is sent through the session the conformance run already holds, and the
//! LUN is left with no registration and no reservation.

use super::cases::{expect, record, record_info, sense, Want, REFUSED_SA};
use super::ops::{self, persistent_reserve_in, persistent_reserve_out, pr_parameters};
use super::port::Port;
use super::pr_commands::{out, read_holder, read_keys, show_holder, show_keys, TYPES};
use super::pr_ports::{KEY_A, KEY_B2, KEY_WRONG, SECTION};
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

    // SPC-3 6.12.3: once registered, a RESERVATION KEY other than this nexus's own gets RESERVATION CONFLICT,
    // whatever the service action. Neither command changes the registration or the reservation.
    let wrong_register = out(s, ops::PR_REGISTER, 0, KEY_WRONG, KEY_B2);
    expect(
        cases,
        SECTION,
        at("SPC-3 6.12.3 REGISTER with a RESERVATION KEY that is not registered for this nexus"),
        &[Want::Conflict],
        wrong_register,
    );
    let wrong_reserve = out(s, ops::PR_RESERVE, 1, KEY_WRONG, 0);
    expect(
        cases,
        SECTION,
        at("SPC-3 6.12.3 RESERVE with a RESERVATION KEY that is not registered for this nexus"),
        &[Want::Conflict],
        wrong_reserve,
    );
    registered_only(s, name, cases, KEY_A, "after the refused commands (6.12.3)");

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
    rules(s, name, cases);
}

/// The rules a single nexus can show, each checked on a clean LUN and left clean: the length and service
/// action refusals (6.12.2, 6.12.3), the generation counter (6.11.2), and what unregistering or preempting
/// the holder does to the reservation (5.6.9, 5.6.10.1 to 5.6.10.4).
fn rules(s: &mut Port, name: &str, cases: &mut Vec<Case>) {
    let at = |what: &str| format!("{name} {what}");
    let list = pr_parameters(0, KEY_A);

    // A parameter list that is not 24 bytes is refused and changes nothing (6.12.3).
    let short = persistent_reserve_out(s, ops::PR_REGISTER, 0, &list[..16]);
    expect(
        cases,
        SECTION,
        at("SPC-3 6.12 REGISTER with a 16-byte parameter list refused, PARAMETER LIST LENGTH ERROR"),
        &[Want::Ascq(0x5, 0x1a, 0x00)],
        short,
    );
    registered(s, name, cases, false, "after the refused short list");

    // An unimplemented service action is refused, for both commands.
    let bad_out = persistent_reserve_out(s, 0x0f, 0, &list);
    expect(
        cases,
        SECTION,
        at("SPC-3 6.12.2 PERSISTENT RESERVE OUT, service action 0Fh refused"),
        &REFUSED_SA,
        bad_out,
    );
    let bad_in = persistent_reserve_in(s, 0x0f, 8);
    expect(
        cases,
        SECTION,
        at("SPC-3 6.11 PERSISTENT RESERVE IN, service action 0Fh refused"),
        &REFUSED_SA,
        bad_in,
    );

    // The generation counter moves on REGISTER (6.11.2) and does not move on RESERVE.
    let (g0, _) = read_keys(s).unwrap_or((0, Vec::new()));
    let reg = out(s, ops::PR_REGISTER, 0, 0, KEY_A);
    expect(
        cases,
        SECTION,
        at("SPC-3 6.11.2 REGISTER for the generation check"),
        &[Want::Good],
        reg,
    );
    let (g1, _) = read_keys(s).unwrap_or((0, Vec::new()));
    record(
        cases,
        SECTION,
        at("SPC-3 6.11.2 PR generation after REGISTER"),
        format!("above {g0}"),
        format!("{g1}"),
        g1 != g0,
    );
    let res = out(s, ops::PR_RESERVE, 5, KEY_A, 0);
    expect(
        cases,
        SECTION,
        at("SPC-3 5.6.8 RESERVE, registrants only, for the generation check"),
        &[Want::Good],
        res,
    );
    let (g2, _) = read_keys(s).unwrap_or((0, Vec::new()));
    record(
        cases,
        SECTION,
        at("SPC-3 6.11.2 PR generation after RESERVE (not incremented)"),
        format!("{g1}"),
        format!("{g2}"),
        g2 == g1,
    );

    // REGISTER AND IGNORE EXISTING KEY ignores the RESERVATION KEY (6.12.3 a).
    let ignored = out(s, ops::PR_REGISTER_AND_IGNORE, 0, KEY_WRONG, KEY_A);
    expect(
        cases,
        SECTION,
        at("SPC-3 6.12.3 REGISTER AND IGNORE EXISTING KEY ignores the RESERVATION KEY"),
        &[Want::Good],
        ignored,
    );
    registered_only(s, name, cases, KEY_A, "after REGISTER AND IGNORE (6.12.3)");
    // REGISTER AND IGNORE is a registration: the counter moves, so the next check starts from its new value.
    let (g2, _) = read_keys(s).unwrap_or((0, Vec::new()));

    // A PREEMPT whose service action key matches no registration is a RESERVATION CONFLICT (5.6.10.4).
    let unmatched = out(s, ops::PR_PREEMPT, 1, KEY_A, KEY_WRONG);
    expect(
        cases,
        SECTION,
        at("SPC-3 5.6.10.4 PREEMPT with an unregistered service action key"),
        &[Want::Conflict],
        unmatched,
    );
    let (g3, _) = read_keys(s).unwrap_or((0, Vec::new()));
    record(
        cases,
        SECTION,
        at("SPC-3 6.11.2 PR generation after the conflict (not incremented)"),
        format!("{g2}"),
        format!("{g3}"),
        g3 == g2,
    );

    // The holder preempts itself with a new type: the reservation becomes that type (5.6.10.4).
    let self_preempt = out(s, ops::PR_PREEMPT, 1, KEY_A, KEY_A);
    expect(
        cases,
        SECTION,
        at("SPC-3 5.6.10.4 PREEMPT by the holder with its own key, type 1"),
        &[Want::Good],
        self_preempt,
    );
    let held = read_holder(s);
    record(
        cases,
        SECTION,
        at("SPC-3 5.6.10.4 after the holder preempts itself, type 1 held by key A"),
        format!("key {KEY_A:#x}, type 1"),
        show_holder(&held),
        matches!(held, Ok(Some((k, 1))) if k == KEY_A),
    );
    let _ = out(s, ops::PR_CLEAR, 0, KEY_A, 0);

    // Unregistering the holder releases the reservation, for each type that releases on the last registrant
    // (5.6.10.1.2 to 5.6.10.1.4). The nexus is unregistered at the end, so the LUN is left clean.
    for (kind, label) in [
        (1u8, "write exclusive"),
        (5, "write exclusive, registrants only"),
        (7, "write exclusive, all registrants"),
    ] {
        let _ = out(s, ops::PR_REGISTER, 0, 0, KEY_A);
        let _ = out(s, ops::PR_RESERVE, kind, KEY_A, 0);
        let gone = out(s, ops::PR_REGISTER, 0, KEY_A, 0);
        expect(
            cases,
            SECTION,
            at(&format!("SPC-3 5.6.10.3 unregister the holder, {label}")),
            &[Want::Good],
            gone,
        );
        let after = read_holder(s);
        record(
            cases,
            SECTION,
            at(&format!(
                "SPC-3 5.6.10.1 holder unregistered, {label}: reservation released"
            )),
            "no reservation",
            show_holder(&after),
            matches!(after, Ok(None)),
        );
    }
    registered(
        s,
        name,
        cases,
        false,
        "after the holder unregisters (5.6.10.3)",
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
