//! The standby cases of the `mpio` section (SPC-3 5.8.2.4.4), through a port in the standby state:
//! the commands the standard lists must still work (INQUIRY, MODE SENSE, REPORT LUNS, REPORT TARGET
//! PORT GROUPS, REQUEST SENSE, PERSISTENT RESERVE IN and OUT, and the others the target supports), and
//! TEST UNIT READY should and READ must report LOGICAL UNIT NOT ACCESSIBLE, TARGET PORT IN STANDBY
//! STATE. Windows clustering registers its key down standby paths, so PERSISTENT RESERVE OUT matters.

use super::alua::SECTION;
use super::cases::{expect, expect_should, record, Want, REFUSED};
use super::lun_access::unit_attentions;
use super::mode_page_cases::{mode_page_of, mode_sense_10};
use super::ops;
use super::port::Port;
use super::port_groups::rtpg;
use super::scsi;
use crate::compliance::Case;

pub(super) const KEY_STANDBY: u64 = 0x5350_4954_0000_005b;

/// Through a port in the standby state: SPC-3 5.8.2.4.4 lists the commands that must still work
/// (here INQUIRY, REPORT LUNS, REPORT TARGET PORT GROUPS, MODE SENSE, REQUEST SENSE, LOG SENSE,
/// PERSISTENT RESERVE IN and OUT), and TEST UNIT READY and READ report LOGICAL UNIT NOT ACCESSIBLE,
/// TARGET PORT IN STANDBY STATE (04h/0Bh).
pub(super) fn run(s: &mut Port, name: &str, cases: &mut Vec<Case>) {
    let label = |what: &str| format!("{name} standby: {what}");
    let _ = unit_attentions(s);
    let not_ready = [Want::Ascq(0x2, 0x04, 0x0b)];
    // SPC-3 5.8.2.4.4: other commands shall be terminated; TEST UNIT READY "should" be.
    expect_should(
        cases,
        SECTION,
        label("TEST UNIT READY reports the standby state"),
        &not_ready,
        s.command(&[0, 0, 0, 0, 0, 0], 0, None),
    );
    expect(
        cases,
        SECTION,
        label("READ (10) reports the standby state"),
        &not_ready,
        s.command(&scsi::block_cdb(0x28, 0, 1), 4096, None),
    );
    expect(
        cases,
        SECTION,
        label("INQUIRY"),
        &[Want::Good],
        s.command(&[0x12, 0, 0, 0, 96, 0], 96, None),
    );
    expect(
        cases,
        SECTION,
        label("REPORT LUNS"),
        &[Want::Good],
        s.command(&[0xa0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0], 256, None),
    );
    expect(
        cases,
        SECTION,
        label("REPORT TARGET PORT GROUPS"),
        &[Want::Good],
        rtpg(s, false),
    );
    expect(
        cases,
        SECTION,
        label("MODE SENSE (10)"),
        &[Want::Good],
        s.command(&[0x5a, 0x08, 0x08, 0, 0, 0, 0, 0x01, 0, 0], 256, None),
    );
    expect(
        cases,
        SECTION,
        label("REQUEST SENSE"),
        &[Want::Good],
        s.command(&[0x03, 0, 0, 0, 252, 0], 252, None),
    );
    let mut log = vec![Want::Good];
    log.extend(REFUSED);
    expect(
        cases,
        SECTION,
        label("LOG SENSE"),
        &log,
        s.command(&[0x4d, 0, 0x40, 0, 0, 0, 0, 0x01, 0, 0], 256, None),
    );
    expect(
        cases,
        SECTION,
        label("PERSISTENT RESERVE IN, READ KEYS"),
        &[Want::Good],
        ops::persistent_reserve_in(s, ops::PR_IN_READ_KEYS, 512),
    );
    // Windows clustering registers its key on every path, the standby ones too.
    let register = ops::persistent_reserve_out(
        s,
        ops::PR_REGISTER_AND_IGNORE,
        0,
        &ops::pr_parameters(0, KEY_STANDBY),
    );
    if expect(
        cases,
        SECTION,
        label("PERSISTENT RESERVE OUT, REGISTER AND IGNORE EXISTING KEY"),
        &[Want::Good],
        register,
    )
    .0
    {
        let unregister = ops::persistent_reserve_out(
            s,
            ops::PR_REGISTER_AND_IGNORE,
            0,
            &ops::pr_parameters(0, 0),
        );
        expect(
            cases,
            SECTION,
            label("PERSISTENT RESERVE OUT, unregister"),
            &[Want::Good],
            unregister,
        );
    }
    standby_listed(s, name, cases);
}

/// The rest of the SPC-3 5.8.2.4.4 list: MODE SELECT (10), LOG SELECT, SEND and RECEIVE DIAGNOSTIC, and the
/// echo buffer modes of READ and WRITE BUFFER. Each must be served, or refused as an invalid operation code
/// when the target does not implement it (the same answer as in the active state). A standby port must not
/// answer with the standby state for these.
fn standby_listed(s: &mut Port, name: &str, cases: &mut Vec<Case>) {
    let label = |what: &str| format!("{name} standby: {what}");
    let mut served_or_refused = vec![Want::Good];
    served_or_refused.extend(REFUSED);

    // SPC-3 6.8: MODE SELECT (10) of the current Caching page, PS cleared, changes nothing.
    let caching = mode_sense_10(s, 0, 0x08, 0, true)
        .ok()
        .and_then(|o| mode_page_of(&o.data, true).map(|p| p.to_vec()));
    if let Some(mut page) = caching {
        page[0] &= 0x7f;
        let mut list = vec![0u8; 8];
        list.extend_from_slice(&page);
        let n = (list.len() as u16).to_be_bytes();
        expect(
            cases,
            SECTION,
            label("SPC-3 6.8 MODE SELECT (10) of the current Caching page"),
            &[Want::Good],
            s.command(&[0x55, 0x10, 0, 0, 0, 0, 0, n[0], n[1], 0], 0, Some(&list)),
        );
    }

    // SPC-3 6.5: LOG SELECT with an empty parameter list.
    expect(
        cases,
        SECTION,
        label("SPC-3 6.5 LOG SELECT, empty parameter list"),
        &served_or_refused,
        s.command(&[0x4c, 0, 0, 0, 0, 0, 0, 0, 0, 0], 0, None),
    );

    // SPC-3 6.28: SEND DIAGNOSTIC with PF set and no parameter list (the default self-test set).
    expect(
        cases,
        SECTION,
        label("SPC-3 6.28 SEND DIAGNOSTIC, PF set, no parameters"),
        &served_or_refused,
        s.command(&[0x1d, 0x10, 0, 0, 0, 0], 0, None),
    );

    // SPC-3 6.18: RECEIVE DIAGNOSTIC RESULTS, PCV clear, four bytes of allocation.
    expect(
        cases,
        SECTION,
        label("SPC-3 6.18 RECEIVE DIAGNOSTIC RESULTS, 4 bytes"),
        &served_or_refused,
        s.command(&[0x1c, 0, 0, 0, 4, 0], 4, None),
    );

    // SPC-3 6.35 and 6.15: the echo buffer mode. What WRITE BUFFER sends, READ BUFFER returns, when both are
    // served; a target that refuses either (not implemented) has nothing to compare.
    let echo = [0x5a, 0xa5, 0x01, 0xfe];
    let (_, wrote) = expect(
        cases,
        SECTION,
        label("SPC-3 6.35 WRITE BUFFER echo mode, 4 bytes"),
        &served_or_refused,
        s.command(&[0x3b, 0x0a, 0, 0, 0, 0, 0, 0, 4, 0], 0, Some(&echo)),
    );
    let (_, read) = expect(
        cases,
        SECTION,
        label("SPC-3 6.15 READ BUFFER echo mode, 4 bytes"),
        &served_or_refused,
        s.command(&[0x3c, 0x0a, 0, 0, 0, 0, 0, 0, 4, 0], 4, None),
    );
    let both_served =
        wrote.is_some_and(|o| o.status == 0) && read.as_ref().is_some_and(|o| o.status == 0);
    if both_served {
        let same = read.is_some_and(|o| o.data == echo);
        record(
            cases,
            SECTION,
            label("SPC-3 6.15 READ BUFFER echo returns what WRITE BUFFER sent"),
            "the bytes WRITE BUFFER sent",
            if same {
                "the same bytes"
            } else {
                "other bytes"
            },
            same,
        );
    }
}
