//! The READ CAPACITY cases of the `disk` section (SBC-5 5.20, 5.21): READ CAPACITY (10) and (16)
//! agree, (10) reports FFFF_FFFFh when the LUN is larger, the block length is at least one byte and even
//! bytes or more, and the (16) fields (protection, LBPME, LBPRZ, the physical block exponent and the
//! lowest aligned LBA) are recorded in `Disk`; and READ CAPACITY (16) with an ALLOCATION LENGTH of
//! zero is not an error and returns nothing (SPC-6 4.2.5.6).

use super::cases::{expect, record, Want};
use super::disk::{label, Disk, SECTION};
use super::port::Port;
use super::scsi_fields::{be16, be32, be64};
use super::seen::Seen;
use crate::compliance::Case;

/// SBC-5 5.20, 5.21: READ CAPACITY (10) and (16).
pub(super) fn capacity(
    s: &mut Port,
    name: &str,
    cases: &mut Vec<Case>,
    seen: &mut Seen,
    disk: &mut Disk,
) {
    let cdb16 = [0x9e, 0x10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 32, 0, 0];
    let answer = expect(
        cases,
        SECTION,
        label(name, "READ CAPACITY (16)"),
        &[Want::Good],
        s.command(&cdb16, 32, None),
    );
    seen.answer("READ CAPACITY (16)", SECTION, &answer);
    disk.rc16 = answer.1.map(|o| o.data).unwrap_or_default();
    disk.last_lba = be64(&disk.rc16, 0);
    disk.block_len = be32(&disk.rc16, 8);
    let exponent = disk.rc16.get(13).copied().unwrap_or(0) & 0x0f;
    let lowest = be16(&disk.rc16, 14) & 0x3fff;
    record(
        cases,
        SECTION,
        label(name, "READ CAPACITY (16) block size and alignment"),
        "at least 1 B and an even number of bytes (SBC-3 4.5); lowest aligned LBA below 2^exponent",
        format!(
            "{} B, exponent {exponent}, lowest aligned {lowest}",
            disk.block_len
        ),
        // SBC-3 4.5: the logical block length is at least one byte and should be even; it need not be a
        // power of two (a 64 KiB block is legal).
        disk.block_len >= 1 && disk.block_len % 2 == 0 && (lowest as u32) < (1u32 << exponent),
    );
    // SPC-6 4.2.5.6: an allocation length of zero transfers nothing and is not an error.
    let zero = [0x9e, 0x10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let (passed, o) = expect(
        cases,
        SECTION,
        label(
            name,
            "SPC-6 4.2.5.6 READ CAPACITY (16) with ALLOCATION LENGTH 0",
        ),
        &[Want::Good],
        s.command(&zero, 0, None),
    );
    if passed && o.is_some_and(|o| o.received != 0) {
        record(
            cases,
            SECTION,
            label(
                name,
                "SPC-6 4.2.5.6 READ CAPACITY (16) with ALLOCATION LENGTH 0 returns no data",
            ),
            "no data",
            "data",
            false,
        );
    }
    let answer = expect(
        cases,
        SECTION,
        label(name, "READ CAPACITY (10)"),
        &[Want::Good],
        s.command(&[0x25, 0, 0, 0, 0, 0, 0, 0, 0, 0], 8, None),
    );
    seen.answer("READ CAPACITY (10)", SECTION, &answer);
    if let Some(o) = answer.1.filter(|o| o.status == 0) {
        // SBC-5 5.20.2: the last LBA, or FFFF_FFFFh when it does not fit, and the same block length.
        let want = disk.last_lba.min(0xffff_ffff) as u32;
        record(
            cases,
            SECTION,
            label(name, "READ CAPACITY (10) agrees with (16)"),
            format!("{want} and {} B", disk.block_len),
            format!("{} and {} B", be32(&o.data, 0), be32(&o.data, 4)),
            be32(&o.data, 0) == want && be32(&o.data, 4) == disk.block_len,
        );
    }
}
