//! The unavailable cases of the `mpio` section (SPC-3 5.8.2.4.5), through a port in the unavailable state: INQUIRY
//! reports the peripheral qualifier 001b, the commands the standard lists still answer, and the rest are refused
//! with NOT READY, LOGICAL UNIT NOT ACCESSIBLE, TARGET PORT IN UNAVAILABLE STATE (04h, 0Ch).

use super::alua::SECTION;
use super::cases::{Want, expect, record};
use super::lun_access::unit_attentions;
use super::port::Port;
use super::port_groups::rtpg;
use super::scsi;
use crate::compliance::Case;

pub(super) fn run(s: &mut Port, name: &str, cases: &mut Vec<Case>) {
    let label = |what: &str| format!("{name} unavailable: {what}");
    let _ = unit_attentions(s);
    let not_accessible = [Want::Ascq(0x2, 0x04, 0x0c)];

    // SPC-3 5.8.2.4.5 a): INQUIRY answers, with the peripheral qualifier 001b.
    let inquiry = s.command(&[0x12, 0, 0, 0, 96, 0], 96, None);
    let qualifier = match &inquiry {
        Ok(o) if o.status == 0 => Some(o.data[0] >> 5),
        _ => None,
    };
    record(
        cases,
        SECTION,
        label("INQUIRY, peripheral qualifier 001b"),
        "GOOD, qualifier 001b",
        format!(
            "qualifier {}",
            qualifier.map_or("none".to_string(), |q| format!("{q:03b}b"))
        ),
        qualifier == Some(0b001),
    );
    // Commands the standard lists (SPC-3 5.8.2.4.5 b) to h)) must answer.
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
        label("REQUEST SENSE"),
        &[Want::Good],
        s.command(&[0x03, 0, 0, 0, 252, 0], 252, None),
    );
    // Commands the standard does not list are refused with the unavailable state (SPC-3 5.8.2.4.5).
    expect(
        cases,
        SECTION,
        label("TEST UNIT READY reports the unavailable state"),
        &not_accessible,
        s.command(&[0; 6], 0, None),
    );
    expect(
        cases,
        SECTION,
        label("READ (10) reports the unavailable state"),
        &not_accessible,
        s.command(&scsi::block_cdb(0x28, 0, 1), 4096, None),
    );
}
