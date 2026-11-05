//! NVM command set I/O commands the compliance tests send (NVMe NVM Command Set Specification): Read,
//! Write and Compare with their FUA bit, Write Zeroes, Dataset Management (deallocate) and Flush.
//! Each function builds the 64-byte command; the data, when there is any, goes with it on submit.
//! `blocks` is the number of logical blocks, 1 or more; the command carries it 0-based.

use super::sqe::{self, Command};

/// CDW12 bit 30 (byte 51, bit 6): Force Unit Access.
const FUA: u8 = 0x40;
/// Dataset Management CDW11 bit 2: the ranges are deallocated (AD).
const ATTR_DEALLOCATE: u32 = 0x04;

/// A Read, Write or Compare of `blocks` blocks at `lba`.
pub fn block_command(opcode: u8, nsid: u32, lba: u64, blocks: u16, fua: bool) -> Command {
    let mut cmd = Command::new(opcode, 0);
    cmd.set_nsid(nsid);
    cmd.set_u64(40, lba);
    cmd.set_u16(48, blocks.saturating_sub(1));
    if fua {
        cmd.set_u8(51, FUA);
    }
    cmd
}

pub fn read(nsid: u32, lba: u64, blocks: u16) -> Command {
    block_command(sqe::OP_READ, nsid, lba, blocks, false)
}

pub fn write(nsid: u32, lba: u64, blocks: u16) -> Command {
    block_command(sqe::OP_WRITE, nsid, lba, blocks, false)
}

/// Write Zeroes of `blocks` blocks at `lba`, without deallocating them.
pub fn write_zeroes(nsid: u32, lba: u64, blocks: u16) -> Command {
    block_command(sqe::OP_WRITE_ZEROES, nsid, lba, blocks, false)
}

pub fn flush(nsid: u32) -> Command {
    let mut cmd = Command::new(sqe::OP_FLUSH, 0);
    cmd.set_nsid(nsid);
    cmd
}

/// Dataset Management with the deallocate attribute over one range; send `range(lba, blocks)` with it.
pub fn deallocate(nsid: u32) -> Command {
    let mut cmd = Command::new(sqe::OP_DATASET_MANAGEMENT, 0);
    cmd.set_nsid(nsid);
    cmd.set_u32(40, 0); // NR, 0-based: one range
    cmd.set_u32(44, ATTR_DEALLOCATE);
    cmd
}

/// One 16-byte Dataset Management range: context attributes, the length in blocks, the first block.
pub fn range(lba: u64, blocks: u32) -> [u8; 16] {
    let mut r = [0u8; 16];
    r[4..8].copy_from_slice(&blocks.to_le_bytes());
    r[8..16].copy_from_slice(&lba.to_le_bytes());
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_commands_carry_lba_count_and_fua() {
        let c = block_command(sqe::OP_WRITE, 3, 0x0102_0304_0506, 8, true);
        assert_eq!(c.0[0], sqe::OP_WRITE);
        assert_eq!(&c.0[4..8], &3u32.to_le_bytes());
        assert_eq!(&c.0[40..48], &0x0102_0304_0506u64.to_le_bytes());
        assert_eq!(&c.0[48..50], &7u16.to_le_bytes());
        assert_eq!(c.0[51], 0x40);
        assert_eq!(read(1, 0, 1).0[51], 0);
        assert_eq!(&read(1, 0, 1).0[48..50], &[0, 0]);
    }

    #[test]
    fn deallocate_sets_the_attribute_and_the_range_layout() {
        let c = deallocate(1);
        assert_eq!(c.0[0], sqe::OP_DATASET_MANAGEMENT);
        assert_eq!(&c.0[44..48], &4u32.to_le_bytes());
        let r = range(0x10, 2);
        assert_eq!(&r[4..8], &2u32.to_le_bytes());
        assert_eq!(&r[8..16], &0x10u64.to_le_bytes());
    }
}
