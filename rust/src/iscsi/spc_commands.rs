//! The `spc` cases of `--compliance`: the SPC-6 commands every logical unit answers that the `disk`
//! section does not already check: TEST UNIT READY (6.46), REPORT LUNS (6.30), and the refusal of a
//! command no standard assigns (SPC-6 4.2.5: ILLEGAL REQUEST).

use super::cases::{expect, Want};
use super::port::Port;
use super::scsi;
use super::seen::Seen;
use crate::compliance::Case;

const SECTION: &str = "spc";

pub(super) fn run(s: &mut Port, name: &str, cases: &mut Vec<Case>, seen: &mut Seen) {
    let tur = expect(
        cases,
        SECTION,
        format!("{name} TEST UNIT READY"),
        &[Want::Good],
        s.command(&[0, 0, 0, 0, 0, 0], 0, None),
    );
    seen.answer("TEST UNIT READY", SECTION, &tur);
    // SPC-6 6.30: an 8-byte header, then 8 bytes per LUN; this target's LUN 0 is listed.
    let luns = s.command(&[0xa0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0], 256, None);
    let (passed, o) = expect(
        cases,
        SECTION,
        format!("{name} REPORT LUNS"),
        &[Want::Good],
        luns,
    );
    let listed = o.as_ref().is_some_and(|o| {
        let len = scsi::be32(&o.data, 0).unwrap_or(u32::MAX) as usize;
        len % 8 == 0 && len >= 8 && o.data.get(8..16) == Some(&[0u8; 8])
    });
    if passed {
        super::cases::record(
            cases,
            SECTION,
            format!("{name} REPORT LUNS lists LUN 0"),
            "length a multiple of 8, LUN 0",
            if listed { "listed" } else { "not listed" },
            listed,
        );
    }
    seen.note("REPORT LUNS", SECTION, passed, passed && listed, "see spc");
    // 7Fh with service action 0000h is assigned by neither SPC-6 nor SBC-5: INVALID COMMAND OPERATION
    // CODE, or INVALID FIELD IN CDB for the service action (SPC-6 4.2.5.2).
    expect(
        cases,
        SECTION,
        format!("{name} unimplemented operation code refused"),
        &[Want::Ascq(0x5, 0x20, 0x00), Want::Ascq(0x5, 0x24, 0x00)],
        s.command(&[0x7f, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 0, None),
    );
}
