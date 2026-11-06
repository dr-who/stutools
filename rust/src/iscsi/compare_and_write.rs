//! The `ats` cases of `--compliance`: COMPARE AND WRITE (SBC-5 5.3), the atomic test and set VMware
//! ESXi uses for its locks. A compare that matches writes the second half of the data; one that does
//! not reports MISCOMPARE (sense key Eh, ASC 1Dh) and writes nothing. When Block Limits reports MAXIMUM
//! COMPARE AND WRITE LENGTH 0 the command is not supported and must be refused (SBC-5 6.6.4). The
//! second-to-last block is used and its original data written back.

use super::cases::{expect, record, Want, REFUSED};
use super::disk::Disk;
use super::lun_access::{read_block, write_block};
use super::ops;
use super::seen::Seen;
use super::session::Session;
use crate::compliance::Case;

const SECTION: &str = "ats";

pub(super) fn run(
    s: &mut Session,
    name: &str,
    disk: &Disk,
    cases: &mut Vec<Case>,
    seen: &mut Seen,
) {
    let label = |what: &str| format!("{name} {what}");
    let Ok(target) = u32::try_from(disk.last_lba.saturating_sub(1)) else {
        return;
    };
    let len = disk.block_len as usize;
    let original = match read_block(s, target, len) {
        Ok(d) => d,
        Err(err) => {
            record(
                cases,
                SECTION,
                label("read the test block"),
                "GOOD",
                err,
                false,
            );
            return;
        }
    };
    // SBC-5 6.6.4: MAXIMUM COMPARE AND WRITE LENGTH 0 says the command is not supported. Without a
    // Block Limits page nothing is advertised either way, so the optional command may work or be refused.
    let advertised = disk.limits().map(|l| l.max_compare_and_write > 0);
    let a = vec![0xa5u8; len];
    let b = vec![0x5au8; len];
    let c = vec![0x3cu8; len];
    if write_block(s, target, &a).is_err() {
        record(
            cases,
            SECTION,
            label("set up block A"),
            "GOOD",
            "failed",
            false,
        );
        return;
    }
    let mut matching = a.clone();
    matching.extend_from_slice(&b);
    let wants: Vec<Want> = match advertised {
        Some(true) => vec![Want::Good],
        Some(false) => REFUSED.to_vec(),
        None => [Want::Good].iter().chain(REFUSED.iter()).cloned().collect(),
    };
    let answer = expect(
        cases,
        SECTION,
        label("COMPARE AND WRITE, compare matches"),
        &wants,
        ops::compare_and_write(s, target as u64, 1, &matching),
    );
    seen.answer("COMPARE AND WRITE", SECTION, &answer);
    if matches!(&answer.1, Some(o) if o.status == 0) {
        let now = read_block(s, target, len);
        let ok = matches!(&now, Ok(d) if *d == b);
        record(
            cases,
            SECTION,
            label("block holds the written data"),
            "B",
            if ok { "B" } else { "other" },
            ok,
        );
        seen.note("COMPARE AND WRITE", SECTION, true, ok, "see ats");
        // SBC-5 5.3: the block is B, so comparing with A miscompares; nothing is written.
        let mut mismatching = a.clone();
        mismatching.extend_from_slice(&c);
        let answer = expect(
            cases,
            SECTION,
            label("COMPARE AND WRITE, compare fails"),
            &[Want::Asc(0xe, 0x1d)],
            ops::compare_and_write(s, target as u64, 1, &mismatching),
        );
        seen.answer("COMPARE AND WRITE", SECTION, &answer);
        let after = read_block(s, target, len);
        let kept = matches!(&after, Ok(d) if *d == b);
        record(
            cases,
            SECTION,
            label("failed compare writes nothing"),
            "B",
            match &after {
                Ok(d) if *d == c => "C (written)",
                Ok(_) if kept => "B",
                _ => "other",
            },
            kept,
        );
    }
    if let Err(err) = write_block(s, target, &original) {
        record(
            cases,
            SECTION,
            label("restore the test block"),
            "written back",
            err,
            false,
        );
    }
}
