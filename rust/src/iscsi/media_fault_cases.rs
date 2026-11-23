//! SBC-3 4.18.1 and table 13: a command that meets a condition in table 13 terminates with CHECK CONDITION, and
//! the sense key is the one the table gives. A block with a media fault (a LUN under fault injection, named by
//! `--fault-lba`) is read, and its MEDIUM ERROR (sense key 03h) must be reported. The block before it is read
//! too, to show the fault is local to the block.

use super::cases::{Want, expect};
use super::lun_access::read_cmd;
use super::port::Port;
use crate::compliance::Case;

const SECTION: &str = "sbc";

pub(super) fn run(s: &mut Port, name: &str, lba: u32, cases: &mut Vec<Case>) {
    expect(
        cases,
        SECTION,
        format!(
            "{name} SBC-3 4.18.1 READ (10) of the injected media fault at LBA {lba}: CHECK CONDITION, MEDIUM ERROR"
        ),
        &[Want::Key(0x03)],
        read_cmd(s, lba, 512),
    );
    if let Some(before) = lba.checked_sub(1) {
        expect(
            cases,
            SECTION,
            format!(
                "{name} SBC-3 4.18.1 READ (10) of LBA {before}, the block before the fault: GOOD"
            ),
            &[Want::Good],
            read_cmd(s, before, 512),
        );
    }
}
