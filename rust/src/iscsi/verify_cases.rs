//! The VERIFY edge cases of the `io` section that Microsoft's SCSI compliance tests and chkdsk /r make
//! (SBC-5 5.36, 5.38): a VERIFICATION LENGTH of zero (no blocks verified, not an error), two blocks from
//! the last LBA and one past it (LOGICAL BLOCK ADDRESS OUT OF RANGE), 16 random LBAs of one block
//! each (the seed printed), and the byte check forms: BYTCHK 01b with the block's own data (GOOD) and
//! with one byte changed (MISCOMPARE, with MISCOMPARE DURING VERIFY OPERATION), BYTCHK 11b with one
//! block for the range, and BYTCHK 01b of zero blocks. SBC-5 sets no INFORMATION field for a VERIFY
//! miscompare (only COMPARE AND WRITE reports the offset), so the offset is only reported. The test
//! block is the LUN's second-to-last; its own data is written back first, so it is mapped. A VERIFY
//! refused as unsupported ends that form's cases.

use super::block_io::{cdb10, cdb16};
use super::cases::{expect, information, record_info, refused, Want, REFUSED};
use super::disk::Disk;
use super::lun_access::{read_block, write_block};
use super::port::Port;
use super::scratch_region::{seed, Draws};
use crate::compliance::Case;

const SECTION: &str = "io";

/// BYTCHK in byte 1 of VERIFY (10) and (16) (SBC-5 tables 126, 132): bits 2 and 1.
const BYTCHK_01: u8 = 0x02;
const BYTCHK_11: u8 = 0x06;

/// MISCOMPARE with MISCOMPARE DURING VERIFY OPERATION (SPC-6 table 61).
const MISCOMPARE: Want = Want::Asc(0xe, 0x1d);

/// The byte of the test block the miscompare cases change.
const CHANGED_AT: usize = 100;

pub(super) fn run(s: &mut Port, name: &str, disk: &Disk, cases: &mut Vec<Case>) {
    let len = disk.block_len as usize;
    let last = disk.last_lba;
    let Ok(target) = u32::try_from(last.saturating_sub(1)) else {
        return;
    };
    let Ok(original) = read_block(s, target, len) else {
        return;
    };
    // A mapped block: BYTCHK on an unmapped one is MISCOMPARE VERIFY OF UNMAPPED LBA (SBC-5 5.36).
    if write_block(s, target, &original).is_err() {
        return;
    }
    let out = [Want::Ascq(0x5, 0x21, 0x00)];
    let mut changed = original.clone();
    changed[CHANGED_AT.min(len - 1)] ^= 0xff;
    let draw_seed = seed();
    let mut draws = Draws::new(draw_seed);
    for ten in [true, false] {
        let (f, clause) = if ten {
            ("VERIFY (10)", "SBC-5 5.36")
        } else {
            ("VERIFY (16)", "SBC-5 5.38")
        };
        let cdb = |flags: u8, lba: u64, n: u32| -> Vec<u8> {
            if ten {
                cdb10(0x2f, flags, lba as u32, n as u16).to_vec()
            } else {
                cdb16(0x8f, flags, lba, n).to_vec()
            }
        };
        let at = |what: &str| format!("{name} {clause} {f} {what}");
        // A VERIFICATION LENGTH of zero verifies nothing and is not an error.
        let mut wants = vec![Want::Good];
        wants.extend(REFUSED);
        let (_, o) = expect(
            cases,
            SECTION,
            at("of zero blocks"),
            &wants,
            s.command(&cdb(0, 0, 0), 0, None),
        );
        if o.as_ref().is_some_and(|o| refused(o, false)) {
            continue;
        }
        // A 10-byte CDB reaches the end only when the last LBA fits 32 bits.
        if !ten || last < u64::from(u32::MAX) {
            expect(
                cases,
                SECTION,
                at("of two blocks from the last LBA"),
                &out,
                s.command(&cdb(0, last, 2), 0, None),
            );
            expect(
                cases,
                SECTION,
                at("one block past the last LBA"),
                &out,
                s.command(&cdb(0, last + 1, 1), 0, None),
            );
        }
        let reach = if ten {
            (last + 1).min(1 << 32)
        } else {
            last + 1
        };
        let lbas: Vec<u64> = (0..8).map(|_| draws.below(reach)).collect();
        let bad: Vec<u64> = lbas
            .iter()
            .copied()
            .filter(|&l| {
                !s.command(&cdb(0, l, 1), 0, None)
                    .is_ok_and(|o| o.status == 0)
            })
            .collect();
        super::cases::record(
            cases,
            SECTION,
            at("of one block at 8 random LBAs"),
            "GOOD at every one",
            if bad.is_empty() {
                format!("all GOOD (seed {draw_seed})")
            } else {
                format!("not GOOD at {bad:?} (seed {draw_seed})")
            },
            bad.is_empty(),
        );
        // The byte check forms, when the device server takes BYTCHK.
        let (_, o) = expect(
            cases,
            SECTION,
            at("BYTCHK 01b with the block's own data"),
            &[Want::Good, Want::Ascq(0x5, 0x24, 0x00)],
            s.command(&cdb(BYTCHK_01, u64::from(target), 1), 0, Some(&original)),
        );
        if o.as_ref().is_some_and(|o| o.status != 0) {
            continue;
        }
        let (_, o) = expect(
            cases,
            SECTION,
            at("BYTCHK 01b with one byte changed: MISCOMPARE"),
            &[MISCOMPARE],
            s.command(&cdb(BYTCHK_01, u64::from(target), 1), 0, Some(&changed)),
        );
        if let Some(o) = o.filter(|o| o.status == 2) {
            record_info(
                cases,
                SECTION,
                at("BYTCHK 01b miscompare: INFORMATION"),
                match information(&o.sense) {
                    Some(v) => format!(
                        "{v} (byte {CHANGED_AT} was changed; SBC-5 sets no INFORMATION here)"
                    ),
                    None => "not valid (SBC-5 sets none for VERIFY)".to_string(),
                },
            );
        }
        expect(
            cases,
            SECTION,
            at("BYTCHK 11b with the block's own data"),
            &[Want::Good, Want::Ascq(0x5, 0x24, 0x00)],
            s.command(&cdb(BYTCHK_11, u64::from(target), 1), 0, Some(&original)),
        );
        expect(
            cases,
            SECTION,
            at("BYTCHK 11b with one byte changed: MISCOMPARE"),
            &[MISCOMPARE, Want::Ascq(0x5, 0x24, 0x00)],
            s.command(&cdb(BYTCHK_11, u64::from(target), 1), 0, Some(&changed)),
        );
        expect(
            cases,
            SECTION,
            at("BYTCHK 01b of zero blocks"),
            &[Want::Good],
            s.command(&cdb(BYTCHK_01, u64::from(target), 0), 0, None),
        );
    }
}
