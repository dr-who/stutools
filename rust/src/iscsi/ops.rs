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

/// PERSISTENT RESERVE OUT service actions (SPC-6 6.15).
pub const PR_REGISTER: u8 = 0x00;
pub const PR_RESERVE: u8 = 0x01;
pub const PR_RELEASE: u8 = 0x02;
pub const PR_CLEAR: u8 = 0x03;
pub const PR_PREEMPT: u8 = 0x04;
pub const PR_PREEMPT_AND_ABORT: u8 = 0x05;
pub const PR_REGISTER_AND_IGNORE: u8 = 0x06;

/// Reservation types (SPC-6 6.15.3), in the type field with the LU scope (0).
pub const PR_WRITE_EXCLUSIVE: u8 = 0x01;
pub const PR_WRITE_EXCLUSIVE_REGISTRANTS: u8 = 0x05;

/// PERSISTENT RESERVE IN service actions (SPC-6 6.14).
pub const PR_IN_READ_KEYS: u8 = 0x00;
pub const PR_IN_READ_RESERVATIONS: u8 = 0x01;
pub const PR_IN_REPORT_CAPABILITIES: u8 = 0x02;
pub const PR_IN_READ_FULL_STATUS: u8 = 0x03;

/// The SPC-2 reservation commands, RESERVE(6) and RELEASE(6), with nothing in the CDB but the opcode.
pub const RESERVE_6: [u8; 6] = [0x16, 0, 0, 0, 0, 0];
pub const RELEASE_6: [u8; 6] = [0x17, 0, 0, 0, 0, 0];

/// UNMAP of one block descriptor (SBC-5 5.31). The parameter list is an 8-byte header (the data
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

/// COMPARE AND WRITE (SBC-5 5.4): `data` is `blocks` blocks to compare, then `blocks` to write.
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

/// PERSISTENT RESERVE OUT parameter list (SPC-6 6.15.4). Bytes 0 to 7 hold the reservation key and
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

/// The SET TARGET PORT GROUPS parameter list for one group (SPC-5 tables 297, 298): four reserved
/// bytes, then the descriptor: the asymmetric access state in the low nibble of byte 0 and the group in
/// bytes 2 and 3.
pub fn port_group_list(group: u16, state: u8) -> [u8; 8] {
    let g = group.to_be_bytes();
    [0, 0, 0, 0, state & 0x0f, 0, g[0], g[1]]
}

/// SET TARGET PORT GROUPS (SPC-6 6.43) for one group.
pub fn set_port_group_state(s: &mut Session, group: u16, state: u8) -> Result<Outcome> {
    let list = port_group_list(group, state);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_port_groups_list_starts_with_four_reserved_bytes() {
        assert_eq!(
            port_group_list(0x0102, 0x1),
            [0, 0, 0, 0, 0x01, 0, 0x01, 0x02]
        );
    }
}
