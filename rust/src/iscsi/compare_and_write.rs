//! The `ats` cases of `--compliance`: COMPARE AND WRITE (SBC-5 5.3), the atomic test and set VMware
//! ESXi uses for its locks. A compare that matches writes the second half of the data; one that does
//! not reports MISCOMPARE (sense key Eh, ASC 1Dh) and writes nothing. When Block Limits reports MAXIMUM
//! COMPARE AND WRITE LENGTH 0 the command is not supported and must be refused (SBC-5 6.6.4). The
//! second-to-last block is used and its original data written back. When the command works, the rest
//! of 5.3 too: a miscompare reports the offset of the first unequal byte in the INFORMATION field;
//! NUMBER OF LOGICAL BLOCKS 0 does nothing and is no error; two blocks compare and write together when
//! the limit allows; more than MAXIMUM COMPARE AND WRITE LENGTH is INVALID FIELD IN CDB, and an LBA
//! past the capacity LOGICAL BLOCK ADDRESS OUT OF RANGE. Two hosts at once are `ats_race`.

use super::cases::{describe, expect, information, record, Want, REFUSED};
use super::disk::Disk;
use super::lun_access::{read_block, write_block};
use super::ops;
use super::port::Port;
use super::seen::Seen;
use crate::compliance::Case;

const SECTION: &str = "ats";

pub(super) fn run(s: &mut Port, name: &str, disk: &Disk, cases: &mut Vec<Case>, seen: &mut Seen) {
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
    if matches!(&answer.1, Some(o) if o.status == 0) {
        depth(s, name, disk, target, cases);
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

/// SBC-5 5.3 beyond a one-block match and mismatch. The test block holds B when this starts.
fn depth(s: &mut Port, name: &str, disk: &Disk, target: u32, cases: &mut Vec<Case>) {
    let label = |what: &str| format!("{name} {what}");
    let len = disk.block_len as usize;
    let lba = u64::from(target);
    let Ok(now) = read_block(s, target, len) else {
        return;
    };
    // The first unequal byte is byte 100 of the compare instance.
    let at = 100.min(len - 1);
    let mut probe = now.clone();
    probe[at] ^= 0xff;
    probe.extend_from_slice(&vec![0x3cu8; len]);
    let answer = ops::compare_and_write(s, lba, 1, &probe);
    let offset = answer.as_ref().ok().and_then(|o| information(&o.sense));
    record(
        cases,
        SECTION,
        label(&format!(
            "miscompare at byte {at} reports offset {at} (INFORMATION)"
        )),
        format!("CHECK CONDITION E/1D/00, INFORMATION {at}"),
        match (&answer, offset) {
            (Ok(o), Some(i)) => format!("{}, INFORMATION {i}", describe(o)),
            (Ok(o), None) => format!("{}, no INFORMATION", describe(o)),
            (Err(e), _) => e.clone(),
        },
        answer
            .as_ref()
            .is_ok_and(|o| Want::Asc(0xe, 0x1d).matches(o))
            && offset == Some(at as u64),
    );
    // NUMBER OF LOGICAL BLOCKS 0: nothing transferred, compared or written.
    let mut cdb = [0u8; 16];
    cdb[0] = 0x89;
    cdb[2..10].copy_from_slice(&lba.to_be_bytes());
    let zero = s.command(&cdb, 0, None);
    let unchanged = read_block(s, target, len).is_ok_and(|d| d == now);
    record(
        cases,
        SECTION,
        label("COMPARE AND WRITE of 0 blocks"),
        "GOOD, nothing written",
        format!(
            "{}, {}",
            zero.as_ref().map_or_else(|e| e.clone(), describe),
            if unchanged {
                "nothing written"
            } else {
                "the block changed"
            }
        ),
        zero.as_ref().is_ok_and(|o| o.status == 0) && unchanged,
    );
    let max = disk.limits().map_or(1, |l| l.max_compare_and_write);
    // Two blocks: the test block and the one before it.
    if max >= 2 {
        let first = target - 1;
        if let Ok(before) = read_block(s, first, len) {
            let mut compare = before.clone();
            compare.extend_from_slice(&now);
            let mut write = vec![0x77u8; len];
            write.extend_from_slice(&vec![0x88u8; len]);
            let mut both = compare;
            both.extend_from_slice(&write);
            let answer = ops::compare_and_write(s, u64::from(first), 2, &both);
            let held = read_block(s, first, len).is_ok_and(|d| d == write[..len])
                && read_block(s, target, len).is_ok_and(|d| d == write[len..]);
            record(
                cases,
                SECTION,
                label("COMPARE AND WRITE of 2 blocks writes both"),
                "GOOD, both blocks written",
                format!(
                    "{}, {}",
                    answer.as_ref().map_or_else(|e| e.clone(), describe),
                    if held {
                        "both written"
                    } else {
                        "not both written"
                    }
                ),
                answer.as_ref().is_ok_and(|o| o.status == 0) && held,
            );
            let _ = write_block(s, first, &before);
            let _ = write_block(s, target, &now);
        }
    }
    // One block more than the limit.
    if max > 0 && max < 255 {
        let blocks = max + 1;
        let data = vec![0u8; 2 * usize::from(blocks) * len];
        let answer =
            ops::compare_and_write(s, lba.saturating_sub(u64::from(blocks)), blocks, &data);
        record(
            cases,
            SECTION,
            label(&format!(
                "COMPARE AND WRITE of {blocks} blocks, over the limit of {max}"
            )),
            "CHECK CONDITION 5/24/00",
            answer.as_ref().map_or_else(|e| e.clone(), describe),
            answer
                .as_ref()
                .is_ok_and(|o| Want::Ascq(0x5, 0x24, 0x00).matches(o)),
        );
    }
    // Past the capacity.
    let mut data = now.clone();
    data.extend_from_slice(&now);
    let answer = ops::compare_and_write(s, disk.last_lba + 1, 1, &data);
    record(
        cases,
        SECTION,
        label("COMPARE AND WRITE past the last LBA"),
        "CHECK CONDITION 5/21/00",
        answer.as_ref().map_or_else(|e| e.clone(), describe),
        answer
            .as_ref()
            .is_ok_and(|o| Want::Ascq(0x5, 0x21, 0x00).matches(o)),
    );
}
