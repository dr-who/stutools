//! READ (6) and WRITE (6) in the `io` section, which Microsoft's SCSI compliance tests send "if
//! implemented" (SBC-2 5.5 and 5.24; SBC-3 made both obsolete, so a device server may refuse them as
//! INVALID COMMAND OPERATION CODE). They reach only LBAs below 2^21, so their scratch block lies below
//! that, read first and written back. When READ (6) is taken: one block equals what READ (10) returns,
//! and a TRANSFER LENGTH of 0 moves 256 blocks; when WRITE (6) is taken, READ (10) returns what it
//! wrote. `cdb6` builds the CDB the bulk runs use too.

use super::block_io::cdb10;
use super::cases::{describe, expect, record, refused, Want};
use super::disk::Disk;
use super::lun_access::pattern;
use super::port::Port;
use crate::compliance::Case;

const SECTION: &str = "io";

/// The largest LBA a 6-byte CDB carries (21 bits).
pub(super) const LBA_LIMIT: u64 = 1 << 21;

/// A 6-byte READ (08h) or WRITE (0Ah) CDB (SBC-2 tables 33 and 70): LBA bits 20 to 16 in byte 1,
/// 15 to 0 in bytes 2 and 3, and the transfer length in byte 4 (0 means 256).
pub(super) fn cdb6(op: u8, lba: u32, blocks: u16) -> [u8; 6] {
    let l = lba.to_be_bytes();
    [op, l[1] & 0x1f, l[2], l[3], (blocks & 0xff) as u8, 0]
}

/// The block the cases write: the highest below 2^21 that leaves room for 256 blocks, or the
/// second-to-last of a smaller LUN.
pub(super) fn low_block(disk: &Disk) -> u32 {
    (LBA_LIMIT - 512).min(disk.last_lba.saturating_sub(300)) as u32
}

/// Runs the READ (6) and WRITE (6) cases and returns which of the two the device server takes.
pub(super) fn run(s: &mut Port, name: &str, disk: &Disk, cases: &mut Vec<Case>) -> (bool, bool) {
    let at = |what: &str| format!("{name} SBC-2 {what}");
    let len = disk.block_len as usize;
    let lba = low_block(disk);
    let ok_or_refused = [Want::Good, Want::Ascq(0x5, 0x20, 0x00)];
    let (_, r6) = expect(
        cases,
        SECTION,
        at("5.5 READ (6) of one block (obsolete since SBC-3: taken or refused)"),
        &ok_or_refused,
        s.command(&cdb6(0x08, lba, 1), len as u32, None),
    );
    let reads = r6
        .as_ref()
        .is_some_and(|o| !refused(o, false) && o.status == 0);
    let one = match s.command(&cdb10(0x28, 0, lba, 1), len as u32, None) {
        Ok(o) if o.status == 0 => o.data,
        other => {
            record(
                cases,
                SECTION,
                at("READ (10) of the 6-byte scratch block"),
                "GOOD",
                other.map_or_else(|e| e, |o| describe(&o)),
                false,
            );
            return (reads, false);
        }
    };
    if let Some(o) = r6.filter(|_| reads) {
        record(
            cases,
            SECTION,
            at("5.5 READ (6) returns what READ (10) does"),
            "the same block",
            if o.data == one {
                "the same"
            } else {
                "different"
            },
            o.data == one,
        );
        // TRANSFER LENGTH 0 is 256 blocks, when one command here carries that many.
        let ten = (super::scratch_region::chunk(s, disk) >= 256)
            .then(|| {
                s.command(&cdb10(0x28, 0, lba, 256), 256 * len as u32, None)
                    .ok()
            })
            .flatten()
            .filter(|o| o.status == 0);
        if let Some(ten) = ten {
            let o = s.command(&cdb6(0x08, lba, 0), 256 * len as u32, None);
            let (actual, passed) = match &o {
                Ok(o) if o.status == 0 => {
                    let same = o.received == 256 * len && o.data == ten.data;
                    (
                        format!(
                            "{} bytes{}",
                            o.received,
                            if same { ", the same" } else { "" }
                        ),
                        same,
                    )
                }
                Ok(o) => (describe(o), false),
                Err(e) => (e.clone(), false),
            };
            record(
                cases,
                SECTION,
                at("5.5 READ (6) TRANSFER LENGTH 0 reads 256 blocks"),
                format!("{} bytes, as READ (10) of 256 blocks", 256 * len),
                actual,
                passed,
            );
        }
    }
    let original = one;
    let p = pattern(len, 0x66);
    let (_, w6) = expect(
        cases,
        SECTION,
        at("5.24 WRITE (6) of one block (obsolete since SBC-3: taken or refused)"),
        &ok_or_refused,
        s.command(&cdb6(0x0a, lba, 1), 0, Some(&p)),
    );
    let writes = w6.as_ref().is_some_and(|o| o.status == 0);
    if writes {
        let back = s.command(&cdb10(0x28, 0, lba, 1), len as u32, None);
        let same = back.as_ref().is_ok_and(|o| o.status == 0 && o.data == p);
        record(
            cases,
            SECTION,
            at("5.24 READ (10) returns what WRITE (6) wrote"),
            "the data written",
            if same {
                "the data written"
            } else {
                "other data"
            },
            same,
        );
        let restored = s.command(&cdb10(0x2a, 0, lba, 1), 0, Some(&original));
        if !restored.as_ref().is_ok_and(|o| o.status == 0) {
            record(
                cases,
                SECTION,
                at("WRITE (6) block written back"),
                "GOOD",
                "failed",
                false,
            );
        }
    }
    (reads, writes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_six_byte_cdb_carries_21_bits_of_lba() {
        assert_eq!(cdb6(0x08, 0x1f_1234, 1), [0x08, 0x1f, 0x12, 0x34, 1, 0]);
        assert_eq!(cdb6(0x0a, 0x0a_0000, 256), [0x0a, 0x0a, 0, 0, 0, 0]);
    }
}
