//! The standby cases of the `mpio` section (SPC-6 5.18.2.5.4), through a port in the standby state:
//! the commands the standard lists must still work (INQUIRY, MODE SENSE, REPORT LUNS, REPORT TARGET
//! PORT GROUPS, REQUEST SENSE, PERSISTENT RESERVE IN and OUT, and the others the target supports), and
//! TEST UNIT READY should and READ must report LOGICAL UNIT NOT ACCESSIBLE, TARGET PORT IN STANDBY
//! STATE. Windows clustering registers its key down standby paths, so PERSISTENT RESERVE OUT matters.

use super::alua::SECTION;
use super::cases::{expect, expect_should, Want, REFUSED};
use super::lun_access::unit_attentions;
use super::ops;
use super::port::Port;
use super::port_groups::rtpg;
use super::scsi;
use crate::compliance::Case;

pub(super) const KEY_STANDBY: u64 = 0x5350_4954_0000_005b;

/// Through a port in the standby state: SPC-6 5.18.2.5.4 lists the commands that must still work
/// (here INQUIRY, REPORT LUNS, REPORT TARGET PORT GROUPS, MODE SENSE, REQUEST SENSE, LOG SENSE,
/// PERSISTENT RESERVE IN and OUT), and TEST UNIT READY and READ report LOGICAL UNIT NOT ACCESSIBLE,
/// TARGET PORT IN STANDBY STATE (04h/0Bh).
pub(super) fn run(s: &mut Port, name: &str, cases: &mut Vec<Case>) {
    let label = |what: &str| format!("{name} standby: {what}");
    let _ = unit_attentions(s);
    let not_ready = [Want::Ascq(0x2, 0x04, 0x0b)];
    // SPC-6 5.18.2.5.4: other commands shall be terminated; TEST UNIT READY "should" be.
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
}
