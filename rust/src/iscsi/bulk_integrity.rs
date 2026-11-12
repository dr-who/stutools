//! The bulk data integrity runs of the `io` section, as Microsoft's SCSI compliance tests make them
//! (SBC-5 5.16, 5.18, 5.40, 5.42; SBC-2 5.5, 5.24 for the 6-byte forms): for READ and WRITE (10) and
//! (16), and (6) when the device server takes them, 1000 sequential one-block writes each stamped with
//! its LBA, then 1000 sequential reads compared; and the same at 1000 random LBAs of the window (the
//! last write to an LBA is what it must hold), with the seed printed; and an overlapped re-read (blocks
//! 0-1, then 1-2: block 1 the same). The writes go to the scratch window (`scratch_region`), or for the
//! 6-byte forms to 1000 blocks below 2^21, each read first and written back after. The write cache is
//! turned on for the runs when it can be, so a disk with it off takes seconds rather than minutes, and
//! turned back after.

use super::block_io::{cdb10, cdb16};
use super::cases::{describe, record, record_info};
use super::disk::Disk;
use super::port::Port;
use super::scratch_region::{read, seed, stamp, write, Draws, Scratch, COMMANDS};
use super::six_byte_io::{cdb6, LBA_LIMIT};
use super::write_cache;
use crate::compliance::Case;
use std::collections::BTreeMap;

const SECTION: &str = "io";

/// One CDB size of READ and WRITE.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Size {
    Six,
    Ten,
    Sixteen,
}

impl Size {
    fn name(self) -> &'static str {
        match self {
            Size::Six => "(6)",
            Size::Ten => "(10)",
            Size::Sixteen => "(16)",
        }
    }

    fn clauses(self) -> &'static str {
        match self {
            Size::Six => "SBC-2 5.5, 5.24",
            Size::Ten => "SBC-5 5.16, 5.40",
            Size::Sixteen => "SBC-5 5.18, 5.42",
        }
    }

    fn read(self, lba: u64, blocks: u16) -> Vec<u8> {
        match self {
            Size::Six => cdb6(0x08, lba as u32, blocks).to_vec(),
            Size::Ten => cdb10(0x28, 0, lba as u32, blocks).to_vec(),
            Size::Sixteen => cdb16(0x88, 0, lba, u32::from(blocks)).to_vec(),
        }
    }

    fn write(self, lba: u64) -> Vec<u8> {
        match self {
            Size::Six => cdb6(0x0a, lba as u32, 1).to_vec(),
            Size::Ten => cdb10(0x2a, 0, lba as u32, 1).to_vec(),
            Size::Sixteen => cdb16(0x8a, 0, lba, 1).to_vec(),
        }
    }
}

/// The outcome of one run: the commands that went wrong, the first of them described.
#[derive(Default)]
struct Tally {
    bad: usize,
    first: Option<String>,
}

impl Tally {
    fn miss(&mut self, what: String) {
        self.bad += 1;
        self.first.get_or_insert(what);
    }

    fn show(&self, of: usize) -> String {
        match &self.first {
            None => format!("{of} of {of} equal"),
            Some(f) => format!("{} of {of} wrong; first: {f}", self.bad),
        }
    }
}

/// Writes one stamped block at each LBA of `lbas` in order, then reads each LBA written and compares it
/// with the last stamp written there.
fn write_then_read(s: &mut Port, size: Size, lbas: &[u64], len: usize, pass: u32) -> Tally {
    let mut t = Tally::default();
    let mut last: BTreeMap<u64, u32> = BTreeMap::new();
    for (i, &lba) in lbas.iter().enumerate() {
        let p = pass + i as u32;
        match s.command(&size.write(lba), 0, Some(&stamp(len, lba, p))) {
            Ok(o) if o.status == 0 => {
                last.insert(lba, p);
            }
            Ok(o) => t.miss(format!("WRITE at {lba}: {}", describe(&o))),
            Err(e) => t.miss(format!("WRITE at {lba}: {e}")),
        }
    }
    let order: Vec<u64> = lbas
        .iter()
        .copied()
        .filter(|l| last.contains_key(l))
        .collect();
    let mut checked = std::collections::BTreeSet::new();
    for lba in order {
        if !checked.insert(lba) {
            continue;
        }
        match s.command(&size.read(lba, 1), len as u32, None) {
            Ok(o) if o.status == 0 && o.data == stamp(len, lba, last[&lba]) => {}
            Ok(o) if o.status == 0 => t.miss(format!("READ at {lba}: other data")),
            Ok(o) => t.miss(format!("READ at {lba}: {}", describe(&o))),
            Err(e) => t.miss(format!("READ at {lba}: {e}")),
        }
    }
    t
}

/// The window of one size: the scratch window, or for the 6-byte forms 1000 blocks below 2^21.
fn window(s: &mut Port, disk: &Disk, size: Size, scratch: &Scratch) -> Option<(u64, u64, Vec<u8>)> {
    if size != Size::Six {
        return Some((scratch.start, u64::from(scratch.blocks), Vec::new()));
    }
    let start = (LBA_LIMIT - u64::from(COMMANDS) - 2)
        .min(scratch.start.saturating_sub(u64::from(COMMANDS)));
    let saved = read(s, start, COMMANDS, disk.block_len as usize, scratch.chunk).ok()?;
    Some((start, u64::from(COMMANDS), saved))
}

pub(super) fn run(s: &mut Port, name: &str, disk: &Disk, six: bool, cases: &mut Vec<Case>) {
    let len = disk.block_len as usize;
    let scratch = match Scratch::save(s, disk) {
        Ok(w) => w,
        Err(e) => {
            record(
                cases,
                SECTION,
                format!("{name} bulk runs: read the scratch window"),
                "GOOD",
                e,
                false,
            );
            return;
        }
    };
    let cache = write_cache::caching_page(s, write_cache::CHANGEABLE)
        .ok()
        .flatten()
        .filter(|m| m.get(2).is_some_and(|b| b & write_cache::WCE != 0))
        .and_then(|_| write_cache::set_wce(s, true).ok());
    let seed = seed();
    let mut draws = Draws::new(seed);
    let mut sizes = vec![Size::Ten, Size::Sixteen];
    if six {
        sizes.insert(0, Size::Six);
    }
    for size in sizes {
        let f = size.name();
        let cl = size.clauses();
        let Some((start, blocks, saved)) = window(s, disk, size, &scratch) else {
            continue;
        };
        if size == Size::Ten && start + blocks > u64::from(u32::MAX) {
            record_info(
                cases,
                SECTION,
                format!("{name} {cl} bulk runs {f}"),
                "not run: the window is past 32-bit LBAs",
            );
            continue;
        }
        // One command per block at most: a window smaller than COMMANDS blocks (a small LUN) runs fewer
        // commands, so no run walks past the end of the window (or of the LUN).
        let n = u64::from(COMMANDS).min(blocks).max(1);
        let sequential: Vec<u64> = (0..n).map(|i| start + i).collect();
        let t = write_then_read(s, size, &sequential, len, 1);
        record(
            cases,
            SECTION,
            format!("{name} {cl} {n} sequential WRITE {f}, then {n} sequential READ {f}, compared"),
            "every block as written",
            t.show(n as usize),
            t.bad == 0,
        );
        let random: Vec<u64> = (0..n).map(|_| start + draws.below(blocks)).collect();
        let t = write_then_read(s, size, &random, len, 5000);
        record(
            cases,
            SECTION,
            format!("{name} {cl} {n} random WRITE {f}, then READ {f} of each, compared"),
            "every block as last written",
            format!("{} (seed {seed})", t.show(n as usize)),
            t.bad == 0,
        );
        // Two reads that share a block return it the same.
        let a = s.command(&size.read(start, 2), 2 * len as u32, None);
        let b = s.command(&size.read(start + 1, 2), 2 * len as u32, None);
        let same = match (&a, &b) {
            (Ok(a), Ok(b)) if a.status == 0 && b.status == 0 => a.data[len..] == b.data[..len],
            _ => false,
        };
        record(
            cases,
            SECTION,
            format!("{name} {cl} overlapped READ {f}: blocks 0-1 then 1-2, block 1 the same"),
            "the same block",
            if same {
                "the same"
            } else {
                "different, or a read failed"
            },
            same,
        );
        if !saved.is_empty() && write(s, 0x2a, 0, start, &saved, len, scratch.chunk).is_err() {
            record(
                cases,
                SECTION,
                format!("{name} 6-byte window written back"),
                "GOOD",
                "failed",
                false,
            );
        }
    }
    if let Err(e) = scratch.restore(s) {
        record(
            cases,
            SECTION,
            format!("{name} scratch window written back"),
            "GOOD",
            e,
            false,
        );
    }
    if let Some(page) = cache {
        let _ = s.command(&cdb10(0x35, 0, 0, 0), 0, None);
        let _ = write_cache::restore(s, &page);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_size_builds_its_own_cdbs() {
        assert_eq!(Size::Six.read(5, 1), vec![0x08, 0, 0, 5, 1, 0]);
        assert_eq!(Size::Ten.write(5)[0], 0x2a);
        assert_eq!(Size::Sixteen.read(5, 2).len(), 16);
        let mut t = Tally::default();
        assert_eq!(t.show(3), "3 of 3 equal");
        t.miss("x".into());
        t.miss("y".into());
        assert_eq!(t.show(3), "2 of 3 wrong; first: x");
    }
}
