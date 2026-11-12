//! The block descriptor of MODE SENSE (SBC-3 6.5.2.2 and 6.5.2.3): the NUMBER OF LOGICAL BLOCKS and the
//! LOGICAL BLOCK LENGTH of the short form (MODE SENSE (10) with LONGLBA clear) and of the long form (LLBAA set, so
//! LONGLBA is set in the header). The short form reports the current number of logical blocks, or zero when the device
//! does not report it, or FFFF_FFFFh when the count does not fit in 32 bits (SBC-3 6.5.2.2). The long form
//! reports the current number of logical blocks exactly (SBC-3 6.5.2.3). Both report the current logical block length. READ
//! CAPACITY (SBC-3 5.16) gives the same numbers, which the cases compare against.

use super::cases::record;
use super::disk::{label, Disk, SECTION};
use super::port::Port;
use crate::compliance::Case;

/// MODE SENSE (10) header: MODE DATA LENGTH (2), medium type, device-specific parameter, LONGLBA bit 4 of
/// byte 4, and BLOCK DESCRIPTOR LENGTH at bytes 6 and 7 (SPC-3 7.4.3).
const HEADER_LEN: usize = 8;
const LONGLBA: u8 = 0x01;
/// The short descriptor: 4 bytes of number of logical blocks, density code, 3 bytes of block length (SBC-3 table 181).
const SHORT_LEN: usize = 8;
/// The long descriptor: 8 bytes of number of logical blocks, 4 reserved, 4 bytes of block length (SBC-3 table 182).
const LONG_LEN: usize = 16;

/// The value a short descriptor reports when the number of logical blocks does not fit (SBC-3 6.5.2.2).
const DOES_NOT_FIT: u64 = 0xffff_ffff;

/// MODE SENSE (10) for all pages with the block descriptor (DBD 0). `llbaa` sets LLBAA in byte 1, which
/// asks for the long descriptor form (SPC-3 6.12).
fn mode_sense_blocks(s: &mut Port, llbaa: bool) -> Option<Vec<u8>> {
    let cdb = [
        0x5a,
        if llbaa { 0x10 } else { 0 },
        0x3f,
        0,
        0,
        0,
        0,
        0x10,
        0x00,
        0,
    ];
    s.command(&cdb, 4096, None).ok().map(|o| o.data)
}

/// The number of logical blocks and block length in the descriptor, and whether the header says long, or None when
/// the answer does not hold the descriptor it claims.
struct Descriptor {
    long: bool,
    blocks: u64,
    block_len: u32,
}

fn parse(d: &[u8]) -> Option<Descriptor> {
    let long = d.get(4)? & LONGLBA != 0;
    let length = u16::from_be_bytes([*d.get(6)?, *d.get(7)?]) as usize;
    let body = d.get(HEADER_LEN..HEADER_LEN + length)?;
    if long {
        if length != LONG_LEN || body.len() < LONG_LEN {
            return None;
        }
        Some(Descriptor {
            long,
            blocks: u64::from_be_bytes(body[0..8].try_into().ok()?),
            block_len: u32::from_be_bytes(body[12..16].try_into().ok()?),
        })
    } else {
        if length != SHORT_LEN || body.len() < SHORT_LEN {
            return None;
        }
        Some(Descriptor {
            long,
            blocks: u32::from_be_bytes(body[0..4].try_into().ok()?) as u64,
            block_len: u32::from_be_bytes([0, body[5], body[6], body[7]]),
        })
    }
}

pub(super) fn run(s: &mut Port, name: &str, disk: &Disk, cases: &mut Vec<Case>) {
    let at = |what: &str| label(name, what);
    let capacity = disk.last_lba + 1;
    let block_len = disk.block_len;

    if let Some(d) = mode_sense_blocks(s, false).and_then(|d| parse(&d)) {
        let reports_count = d.blocks == 0
            || d.blocks == capacity
            || (capacity > DOES_NOT_FIT && d.blocks == DOES_NOT_FIT);
        record(
            cases,
            SECTION,
            at("SBC-3 6.5.2.2 MODE SENSE (10) short block descriptor: NUMBER OF LOGICAL BLOCKS"),
            format!("0, {capacity}, or FFFF_FFFFh when capacity does not fit"),
            d.blocks.to_string(),
            reports_count && !d.long,
        );
        record(
            cases,
            SECTION,
            at("SBC-3 6.5.2.2 MODE SENSE (10) short block descriptor: LOGICAL BLOCK LENGTH"),
            block_len.to_string(),
            d.block_len.to_string(),
            d.block_len == block_len,
        );
    } else {
        record(
            cases,
            SECTION,
            at("SBC-3 6.5.2.2 MODE SENSE (10) short block descriptor present"),
            "an 8-byte short descriptor",
            "none",
            false,
        );
    }

    let long_answer = mode_sense_blocks(s, true).and_then(|d| parse(&d));
    if long_answer.as_ref().is_some_and(|d| !d.long) {
        // SPC-3 6.10: with LLBAA set the device server is allowed to return LONGLBA 1; a short descriptor is a
        // valid answer, and the short form was judged above.
        super::cases::record_info(
            cases,
            SECTION,
            at("SBC-3 6.5.2.3 MODE SENSE (10) long block descriptor"),
            "not run: LONGLBA 0; LLBAA allows, and does not require, a long block descriptor (SPC-3 6.10)",
        );
    } else if let Some(d) = long_answer {
        record(
            cases,
            SECTION,
            at("SBC-3 6.5.2.3 MODE SENSE (10) long block descriptor: NUMBER OF LOGICAL BLOCKS"),
            capacity.to_string(),
            d.blocks.to_string(),
            d.long && d.blocks == capacity,
        );
        record(
            cases,
            SECTION,
            at("SBC-3 6.5.2.3 MODE SENSE (10) long block descriptor: LOGICAL BLOCK LENGTH"),
            block_len.to_string(),
            d.block_len.to_string(),
            d.block_len == block_len,
        );
    } else {
        record(
            cases,
            SECTION,
            at("SBC-3 6.5.2.3 MODE SENSE (10) long block descriptor present with LLBAA"),
            "a 16-byte long descriptor",
            "none",
            false,
        );
    }
}
