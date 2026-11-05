//! The logical unit state cases of the `disk` section: REQUEST SENSE in fixed and descriptor format
//! with nothing to report (SPC-6 6.36), START STOP UNIT with START (SBC-5 5.31; Windows and macOS send
//! it), and LOG SENSE of the supported log pages and subpages (SPC-6 6.9).

use super::cases::{expect, record, Want, REFUSED};
use super::disk::{hex_list, label, Disk, SECTION};
use super::scsi_fields::be16;
use super::seen::Seen;
use super::session::{Outcome, Session};
use crate::compliance::Case;

/// SPC-6 6.36 REQUEST SENSE, SBC-5 5.31 START STOP UNIT, SPC-6 6.9 LOG SENSE.
pub(super) fn run(
    s: &mut Session,
    name: &str,
    cases: &mut Vec<Case>,
    seen: &mut Seen,
    disk: &mut Disk,
) {
    // SPC-6 6.36: with nothing to report, REQUEST SENSE returns sense data in the format DESC asks for.
    let fixed = s.command(&[0x03, 0, 0, 0, 252, 0], 252, None);
    let (passed, o) = expect(
        cases,
        SECTION,
        label(name, "REQUEST SENSE, fixed format (DESC 0)"),
        &[Want::Good],
        fixed,
    );
    let fixed_ok = passed
        && o.as_ref()
            .is_some_and(|o| matches!(o.data.first().map(|b| b & 0x7f), Some(0x70 | 0x71)));
    record(
        cases,
        SECTION,
        label(name, "REQUEST SENSE DESC 0 response code"),
        "70h or 71h",
        show_byte(&o, 0),
        fixed_ok,
    );
    seen.fact("REQUEST SENSE DESC 0", fixed_ok);
    let desc = s.command(&[0x03, 0x01, 0, 0, 252, 0], 252, None);
    let (passed, o) = expect(
        cases,
        SECTION,
        label(name, "REQUEST SENSE, descriptor format (DESC 1)"),
        &[Want::Good, Want::Ascq(0x5, 0x24, 0x00)],
        desc,
    );
    let desc_ok = o.as_ref().is_some_and(|o| {
        o.status == 0 && matches!(o.data.first().map(|b| b & 0x7f), Some(0x72 | 0x73))
    });
    if o.as_ref().is_some_and(|o| o.status == 0) {
        record(
            cases,
            SECTION,
            label(name, "REQUEST SENSE DESC 1 response code"),
            "72h or 73h",
            show_byte(&o, 0),
            desc_ok,
        );
    }
    seen.fact("REQUEST SENSE DESC 1", desc_ok);
    seen.note(
        "REQUEST SENSE",
        SECTION,
        true,
        fixed_ok && passed,
        "see disk",
    );

    // SBC-5 5.31: START with POWER CONDITION 0h and no load or eject.
    let start = s.command(&[0x1b, 0, 0, 0, 0x01, 0], 0, None);
    let mut wants = vec![Want::Good];
    wants.extend(REFUSED);
    let answer = expect(
        cases,
        SECTION,
        label(name, "START STOP UNIT, START"),
        &wants,
        start,
    );
    seen.answer("START STOP UNIT", SECTION, &answer);

    // SPC-6 6.9: LOG SENSE of page 00h, current cumulative values; then 00h/FFh, pages and subpages.
    let log = s.command(&[0x4d, 0, 0x40, 0, 0, 0, 0, 0x04, 0x00, 0], 1024, None);
    let answer = expect(
        cases,
        SECTION,
        label(name, "LOG SENSE supported pages"),
        &wants,
        log,
    );
    seen.answer("LOG SENSE", SECTION, &answer);
    if let Some(o) = answer.1.filter(|o| o.status == 0) {
        let n = be16(&o.data, 2) as usize;
        let list = o.data.get(4..4 + n).unwrap_or(&[]).to_vec();
        record(
            cases,
            SECTION,
            label(name, "LOG SENSE page 00h ascending"),
            "page code 00h, ascending",
            hex_list(&list),
            o.data[0] & 0x3f == 0 && list.windows(2).all(|w| w[0] < w[1]),
        );
        disk.log_pages = Some(list);
        let sub = s.command(&[0x4d, 0, 0x40, 0xff, 0, 0, 0, 0x04, 0x00, 0], 1024, None);
        if let Some(o) = sub.ok().filter(|o| o.status == 0) {
            let n = be16(&o.data, 2) as usize;
            disk.log_subpages = Some(
                o.data
                    .get(4..4 + n)
                    .unwrap_or(&[])
                    .chunks_exact(2)
                    .map(|c| (c[0] & 0x3f, c[1]))
                    .collect(),
            );
        }
    }
}

pub(super) fn show_byte(o: &Option<Outcome>, at: usize) -> String {
    match o {
        Some(o) if o.status == 0 => format!("{:02x}h", o.data.get(at).copied().unwrap_or(0) & 0x7f),
        Some(o) => super::cases::describe(o),
        None => "no answer".into(),
    }
}
