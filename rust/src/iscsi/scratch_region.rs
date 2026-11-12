//! The scratch window the bulk data cases of the `io` section write: the blocks just before the last
//! three of the LUN (which the single-block cases own), large enough for a 12 MiB run or 1000 blocks,
//! whichever is more, read in full before a case writes and written back after (`Scratch::save`,
//! `Scratch::restore`). Transfers are cut into commands of at most 1 MiB, the Block Limits MAXIMUM
//! TRANSFER LENGTH, and what SG_IO carries (`chunk`). Blocks written carry a stamp of their LBA and
//! the pass, so a block read back from the wrong place, or an old one, never matches by accident. A
//! seeded xorshift picks random LBAs; the seed is printed with the case.

use super::block_io::cdb16;
use super::cases::describe;
use super::disk::Disk;
use super::port::Port;
use crate::Result;

/// Bytes a 12 MiB run moves.
pub(super) const RUN_BYTES: u64 = 12 << 20;

/// Blocks the 1000-command runs touch.
pub(super) const COMMANDS: u32 = 1000;

/// The blocks per command: 1 MiB, or less when Block Limits or SG_IO allow less.
pub(super) fn chunk(s: &Port, disk: &Disk) -> u32 {
    let bl = disk.block_len.max(1);
    let mut blocks = ((1u32 << 20) / bl).max(1);
    if let Some(l) = disk.limits().filter(|l| l.max_transfer > 0) {
        blocks = blocks.min(l.max_transfer);
    }
    if let Some(bytes) = s.max_transfer() {
        blocks = blocks.min((bytes / bl).max(1));
    }
    blocks
}

/// A block of `len` bytes stamped with its LBA and `pass` and filled from both.
pub(super) fn stamp(len: usize, lba: u64, pass: u32) -> Vec<u8> {
    let mut b: Vec<u8> = (0..len)
        .map(|i| (i as u8).wrapping_mul(29) ^ (lba as u8) ^ (pass as u8).rotate_left(3))
        .collect();
    if len >= 12 {
        b[..8].copy_from_slice(&lba.to_be_bytes());
        b[8..12].copy_from_slice(&pass.to_be_bytes());
    }
    b
}

/// `count` blocks stamped from `lba` on.
pub(super) fn stamps(len: usize, lba: u64, count: u32, pass: u32) -> Vec<u8> {
    (0..u64::from(count))
        .flat_map(|i| stamp(len, lba + i, pass))
        .collect()
}

/// A xorshift64 draw for random LBAs, from a seed printed with the case.
pub(super) struct Draws(u64);

impl Draws {
    pub(super) fn new(seed: u64) -> Draws {
        Draws(seed.max(1))
    }

    /// A value below `n` (n > 0).
    pub(super) fn below(&mut self, n: u64) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x % n.max(1)
    }
}

/// A seed from the clock.
pub(super) fn seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |d| d.as_nanos() as u64)
}

/// Reads `count` blocks from `lba` with READ (16), `chunk` blocks per command.
pub(super) fn read(s: &mut Port, lba: u64, count: u32, len: usize, chunk: u32) -> Result<Vec<u8>> {
    read_with(s, false, lba, count, len, chunk)
}

/// `read`, with READ (10) when `ten`.
pub(super) fn read_with(
    s: &mut Port,
    ten: bool,
    lba: u64,
    count: u32,
    len: usize,
    chunk: u32,
) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(count as usize * len);
    let mut done = 0u32;
    while done < count {
        let n = chunk.min(count - done);
        let at = lba + u64::from(done);
        let cdb = if ten {
            let l = u32::try_from(at)
                .map_err(|_| "not run: READ (10) cannot address an LBA past 32 bits")?;
            super::block_io::cdb10(0x28, 0, l, n as u16).to_vec()
        } else {
            cdb16(0x88, 0, at, n).to_vec()
        };
        let o = s.command(&cdb, n * len as u32, None)?;
        if o.status != 0 {
            return Err(format!("READ at {at}: {}", describe(&o)));
        }
        out.extend_from_slice(&o.data);
        done += n;
    }
    Ok(out)
}

/// Writes `data` from `lba` with WRITE (10) (`op` 2Ah) or WRITE (16) (8Ah) and the CDB byte 1 `flags`,
/// `chunk` blocks per command.
pub(super) fn write(
    s: &mut Port,
    op: u8,
    flags: u8,
    lba: u64,
    data: &[u8],
    len: usize,
    chunk: u32,
) -> Result<()> {
    let count = (data.len() / len) as u32;
    let mut done = 0u32;
    while done < count {
        let n = chunk.min(count - done);
        let at = lba + u64::from(done);
        let bytes = &data[done as usize * len..(done + n) as usize * len];
        let o = if op == 0x2a {
            let l = u32::try_from(at)
                .map_err(|_| "not run: WRITE (10) cannot address an LBA past 32 bits")?;
            s.command(
                &super::block_io::cdb10(0x2a, flags, l, n as u16),
                0,
                Some(bytes),
            )?
        } else {
            s.command(&cdb16(0x8a, flags, at, n), 0, Some(bytes))?
        };
        if o.status != 0 {
            return Err(format!("WRITE at {at}: {}", describe(&o)));
        }
        done += n;
    }
    Ok(())
}

/// The scratch window and the data it held.
pub(super) struct Scratch {
    pub start: u64,
    pub blocks: u32,
    pub len: usize,
    pub chunk: u32,
    original: Vec<u8>,
}

impl Scratch {
    /// The window for `disk`, read in full. Fails when the LUN is too small to spare it twice over, or
    /// the read fails.
    pub(super) fn save(s: &mut Port, disk: &Disk) -> Result<Scratch> {
        let len = disk.block_len as usize;
        let end = disk.last_lba.saturating_sub(2);
        // The window is the run's bytes, and at least the command count when the LUN can spare it twice;
        // a small LUN (large blocks, few of them) gets the largest window that fits in half of it.
        let fits = (end / 2).min(u64::from(u32::MAX)) as u32;
        let blocks = ((RUN_BYTES / disk.block_len.max(1) as u64) as u32)
            .max(COMMANDS.min(fits))
            .min(fits);
        if blocks == 0 {
            return Err("the LUN is too small to spare a scratch window twice over".into());
        }
        let start = end - u64::from(blocks);
        let chunk = chunk(s, disk);
        let original = read(s, start, blocks, len, chunk)?;
        Ok(Scratch {
            start,
            blocks,
            len,
            chunk,
            original,
        })
    }

    /// Writes back what the window held.
    pub(super) fn restore(&self, s: &mut Port) -> Result<()> {
        write(s, 0x8a, 0, self.start, &self.original, self.len, self.chunk)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_differ_by_lba_and_pass() {
        assert_ne!(stamp(512, 7, 1), stamp(512, 8, 1));
        assert_ne!(stamp(512, 7, 1), stamp(512, 7, 2));
        assert_eq!(&stamp(512, 7, 1)[..8], &7u64.to_be_bytes());
        assert_eq!(stamps(512, 7, 2, 1)[512..], stamp(512, 8, 1)[..]);
    }

    #[test]
    fn draws_stay_below_the_bound_and_repeat_from_a_seed() {
        let mut a = Draws::new(42);
        let mut b = Draws::new(42);
        for _ in 0..100 {
            let x = a.below(1000);
            assert!(x < 1000);
            assert_eq!(x, b.below(1000));
        }
    }
}
