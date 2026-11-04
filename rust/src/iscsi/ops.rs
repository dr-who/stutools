//! SCSI commands beyond READ and WRITE: UNMAP (SBC), COMPARE AND WRITE (SBC, the ATS primitive),
//! PERSISTENT RESERVE IN and OUT (SPC-4), and SET TARGET PORT GROUPS (SPC-4). Each function sends
//! one command and returns its status and sense. The caller decides whether the answer is right.

use super::session::{Outcome, Session};
use crate::Result;

const UNMAP: u8 = 0x42;
const COMPARE_AND_WRITE: u8 = 0x89;
const PERSISTENT_RESERVE_IN: u8 = 0x5e;
const PERSISTENT_RESERVE_OUT: u8 = 0x5f;
const MAINTENANCE_OUT: u8 = 0xa4;
const SET_TARGET_PORT_GROUPS: u8 = 0x0a;

pub const CHECK_CONDITION: u8 = 0x02;
pub const RESERVATION_CONFLICT: u8 = 0x18;

/// PERSISTENT RESERVE OUT service actions (SPC-4 6.16).
pub const PR_REGISTER: u8 = 0x00;
pub const PR_RESERVE: u8 = 0x01;
pub const PR_RELEASE: u8 = 0x02;
pub const PR_CLEAR: u8 = 0x03;
pub const PR_PREEMPT: u8 = 0x04;

/// Reservation type for the scope field: write exclusive.
pub const PR_WRITE_EXCLUSIVE: u8 = 0x01;

/// PERSISTENT RESERVE IN service actions (SPC-4 6.15).
pub const PR_IN_READ_KEYS: u8 = 0x00;
pub const PR_IN_READ_RESERVATIONS: u8 = 0x01;
pub const PR_IN_REPORT_CAPABILITIES: u8 = 0x02;

/// UNMAP of one block descriptor (SBC-3 5.25). The parameter list is an 8-byte header (the data
/// length excludes its own two bytes, then the block descriptor length) and one 16-byte descriptor.
pub fn unmap(s: &mut Session, lba: u64, blocks: u32) -> Result<Outcome> {
    let mut list = [0u8; 24];
    list[1] = 22;
    list[3] = 16;
    list[8..16].copy_from_slice(&lba.to_be_bytes());
    list[16..20].copy_from_slice(&blocks.to_be_bytes());
    let cdb = [UNMAP, 0, 0, 0, 0, 0, 0, 0, 24, 0];
    s.command(&cdb, 0, Some(&list))
}

/// COMPARE AND WRITE (SBC-3 5.3): `data` is `blocks` blocks to compare, then `blocks` to write.
/// The LBA is 8 bytes at 2 to 9 and the block count is one byte at 13.
pub fn compare_and_write(s: &mut Session, lba: u64, blocks: u8, data: &[u8]) -> Result<Outcome> {
    let mut cdb = [0u8; 16];
    cdb[0] = COMPARE_AND_WRITE;
    cdb[2..10].copy_from_slice(&lba.to_be_bytes());
    cdb[13] = blocks;
    s.command(&cdb, 0, Some(data))
}

/// PERSISTENT RESERVE IN. Reads `alloc` bytes of the service action's answer.
pub fn persistent_reserve_in(s: &mut Session, action: u8, alloc: u16) -> Result<Outcome> {
    let a = alloc.to_be_bytes();
    let cdb = [PERSISTENT_RESERVE_IN, action, 0, 0, 0, 0, 0, a[0], a[1], 0];
    s.command(&cdb, alloc as u32, None)
}

/// PERSISTENT RESERVE OUT parameter list (SPC-4 6.16.4). Bytes 0 to 7 hold the reservation key and
/// 8 to 15 the service action reservation key. Byte 20 carries the flags; this list sets none.
pub fn pr_parameters(key: u64, service_key: u64) -> [u8; 24] {
    let mut list = [0u8; 24];
    list[0..8].copy_from_slice(&key.to_be_bytes());
    list[8..16].copy_from_slice(&service_key.to_be_bytes());
    list
}

/// PERSISTENT RESERVE OUT. `kind` is the scope (LU, 0) in the high nibble and the reservation type
/// in the low nibble. The parameter list is 24 bytes, see `pr_parameters`.
pub fn persistent_reserve_out(
    s: &mut Session,
    action: u8,
    kind: u8,
    list: &[u8],
) -> Result<Outcome> {
    let n = (list.len() as u32).to_be_bytes();
    let cdb = [
        PERSISTENT_RESERVE_OUT,
        action,
        kind,
        0,
        0,
        n[0],
        n[1],
        n[2],
        n[3],
        0,
    ];
    s.command(&cdb, 0, Some(list))
}

/// SET TARGET PORT GROUPS (SPC-4 6.36) for one group. Each group descriptor is four bytes: the
/// asymmetric access state (low nibble of byte 0), then the group ID in bytes 2 and 3.
pub fn set_port_group_state(s: &mut Session, group: u16, state: u8) -> Result<Outcome> {
    let g = group.to_be_bytes();
    let list = [state & 0x0f, 0, g[0], g[1]];
    let n = (list.len() as u32).to_be_bytes();
    let cdb = [
        MAINTENANCE_OUT,
        SET_TARGET_PORT_GROUPS,
        0,
        0,
        0,
        0,
        n[0],
        n[1],
        n[2],
        n[3],
        0,
        0,
    ];
    s.command(&cdb, 0, Some(&list))
}
