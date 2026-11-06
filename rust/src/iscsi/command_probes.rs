//! How the `sbc` checklist sends each block command it probes itself: the CDB forms it needs (12-byte,
//! 32-byte variable length (SBC-5 table 85), service action in), each command in a form that leaves
//! the device as it was (reads of the test block; writes of a pattern of their own, read back, then
//! the block's own data written back; an all-zero ORWRITE), and the judging of
//! an answer into a `Probe`: a clean refusal (INVALID COMMAND OPERATION CODE, or INVALID FIELD IN CDB
//! for a service action) means not supported; any other answer must be the right one. A probe that
//! loses the connection says so (`DROPPED`). The 32-byte commands SBC-5 restricts to type 2 protection
//! (4.21.2.2, 4.21.2.3: eight, WRITE SCATTERED (32) among them) must be refused on a LUN without it. RESERVE (6) and RELEASE (6) are probed as a pair
//! on two sessions.

use super::block_commands::SECTION;
use super::cases::{describe, refused, Want};
use super::command_set::Row;
use super::disk::Disk;
use super::lun_access::{pattern, read_block, write_block};
use super::port::Port;
use super::scsi_fields::be16;
use super::seen::Probe;
use super::session::{self, Outcome};
use crate::Result;

/// The start of a probe's answer when the connection failed under it.
pub(super) const DROPPED: &str = "connection failed";

/// Where the probes read and write: the second-to-last block and its original data.
pub(super) struct Block {
    pub(super) lba: u64,
    pub(super) len: usize,
    pub(super) original: Vec<u8>,
}

/// A 32-byte variable-length CDB (SBC-5 table 85): operation code 7Fh, ADDITIONAL CDB LENGTH 18h, the
/// service action at 8, the LBA at 12, the count at 28.
pub(super) fn cdb32(service_action: u16, lba: u64, count: u32) -> [u8; 32] {
    let mut c = [0u8; 32];
    c[0] = 0x7f;
    c[7] = 0x18;
    c[8..10].copy_from_slice(&service_action.to_be_bytes());
    c[12..20].copy_from_slice(&lba.to_be_bytes());
    c[28..32].copy_from_slice(&count.to_be_bytes());
    c
}

/// A SERVICE ACTION IN (16) or MAINTENANCE IN style CDB with the allocation length where these
/// commands put it.
pub(super) fn sa_in(op: u8, sa: u8, len: usize, alloc_at: usize, alloc: u32) -> Vec<u8> {
    let mut c = vec![0u8; len];
    c[0] = op;
    c[1] = sa;
    c[alloc_at..alloc_at + 4].copy_from_slice(&alloc.to_be_bytes());
    c
}

pub(super) fn cdb12(op: u8, lba: u32, count: u32) -> [u8; 12] {
    let mut c = [0u8; 12];
    c[0] = op;
    c[2..6].copy_from_slice(&lba.to_be_bytes());
    c[6..10].copy_from_slice(&count.to_be_bytes());
    c
}

/// The data for WRITE SCATTERED (16) of one range (SBC-5 tables 153, 154): a 32-byte header, one 32-byte
/// LBA range descriptor, padding to a block, then the block itself.
pub(super) fn scattered(lba: u64, block: &[u8]) -> Vec<u8> {
    let mut d = vec![0u8; block.len()];
    d[32..40].copy_from_slice(&lba.to_be_bytes());
    d[40..44].copy_from_slice(&1u32.to_be_bytes());
    d.extend_from_slice(block);
    d
}

/// One command's answer as a probe: what it found and whether that is right.
pub(super) fn judge(o: Result<Outcome>, ok: impl Fn(&Outcome) -> bool) -> Probe {
    match o {
        Ok(o) if refused(&o, true) => Probe {
            section: SECTION,
            supported: false,
            passed: true,
            actual: describe(&o),
        },
        Ok(o) => Probe {
            section: SECTION,
            supported: true,
            passed: ok(&o),
            actual: describe(&o),
        },
        // RFC 7143 11.17: a target may Reject a PDU it cannot take (reason 05h, command not supported),
        // which refuses the command cleanly.
        Err(e) if e.starts_with(session::REJECTED) => Probe {
            section: SECTION,
            supported: false,
            passed: true,
            actual: e,
        },
        Err(e) => Probe {
            section: SECTION,
            supported: false,
            passed: false,
            actual: format!("{DROPPED}: {e}"),
        },
    }
}

pub(super) fn good(o: &Outcome) -> bool {
    o.status == 0
}

/// Sends the row's command, in a form that changes nothing, and judges it.
pub(super) fn probe(s: &mut Port, row: &Row, b: &Block, disk: &Disk) -> Option<Probe> {
    let lba32 = b.lba as u32;
    let len = b.len as u32;
    let zeros = vec![0u8; b.len];
    let reads_back = |s: &mut Port| read_block(s, lba32, b.len).is_ok_and(|d| d == b.original);
    // A write probe writes a pattern of its own, so a target that accepts the command and stores
    // nothing is caught; `stored` reads it back and puts the block's own data back.
    let mine = pattern(b.len, 0x5a);
    let stored = |s: &mut Port| {
        let ok = read_block(s, lba32, b.len).is_ok_and(|d| d == mine);
        let _ = write_block(s, lba32, &b.original);
        ok
    };
    // SBC-5 4.21.2: without type 2 protection the type-2-only commands are INVALID COMMAND OPERATION
    // CODE.
    let type2 = type_2_protection(disk);
    let thirty_two = |o: &Outcome| {
        if type2 {
            o.status == 0
        } else {
            Want::Ascq(0x5, 0x20, 0x00).matches(o)
        }
    };
    let p = match row.name {
        "READ (12)" => judge(s.command(&cdb12(0xa8, lba32, 1), len, None), |o| {
            good(o) && o.data == b.original
        }),
        "WRITE (12)" => {
            let mut p = judge(s.command(&cdb12(0xaa, lba32, 1), 0, Some(&mine)), good);
            p.passed &= stored(s) || !p.supported;
            p
        }
        "VERIFY (12)" => judge(s.command(&cdb12(0xaf, lba32, 1), 0, None), good),
        "WRITE AND VERIFY (10)" => {
            let c = super::block_io::cdb10(0x2e, 0, lba32, 1);
            let mut p = judge(s.command(&c, 0, Some(&mine)), good);
            p.passed &= stored(s) || !p.supported;
            p
        }
        "WRITE AND VERIFY (12)" => {
            let mut p = judge(s.command(&cdb12(0xae, lba32, 1), 0, Some(&mine)), good);
            p.passed &= stored(s) || !p.supported;
            p
        }
        "WRITE AND VERIFY (16)" => {
            let c = super::block_io::cdb16(0x8e, 0, b.lba, 1);
            let mut p = judge(s.command(&c, 0, Some(&mine)), good);
            p.passed &= stored(s) || !p.supported;
            p
        }
        // SBC-5 5.13: CONDITION MET (04h) when the blocks are in the cache, else GOOD.
        "PRE-FETCH (10)" => judge(
            s.command(&super::block_io::cdb10(0x34, 0, lba32, 1), 0, None),
            |o| matches!(o.status, 0 | 4),
        ),
        "PRE-FETCH (16)" => judge(
            s.command(&super::block_io::cdb16(0x90, 0, b.lba, 1), 0, None),
            |o| matches!(o.status, 0 | 4),
        ),
        // SBC-5 5.10: OR with zeros leaves the block as it was.
        "ORWRITE (16)" => {
            let mut p = judge(
                s.command(&super::block_io::cdb16(0x8b, 0, b.lba, 1), 0, Some(&zeros)),
                good,
            );
            p.passed &= !p.supported || reads_back(s);
            p
        }
        "READ (32)" => judge(s.command(&cdb32(0x0009, b.lba, 1), len, None), thirty_two),
        "VERIFY (32)" => judge(s.command(&cdb32(0x000a, b.lba, 1), 0, None), thirty_two),
        "WRITE (32)" => judge(
            s.command(&cdb32(0x000b, b.lba, 1), 0, Some(&b.original)),
            thirty_two,
        ),
        "WRITE AND VERIFY (32)" => judge(
            s.command(&cdb32(0x000c, b.lba, 1), 0, Some(&b.original)),
            thirty_two,
        ),
        "WRITE SAME (32)" => judge(
            s.command(&cdb32(0x000d, b.lba, 1), 0, Some(&b.original)),
            thirty_two,
        ),
        // ORWRITE (32) and GET LBA STATUS (32) need no type 2 protection.
        "ORWRITE (32)" => judge(s.command(&cdb32(0x000e, b.lba, 1), 0, Some(&zeros)), good),
        "WRITE ATOMIC (32)" | "WRITE SCATTERED (32)" if type2 => Probe {
            section: SECTION,
            supported: true,
            passed: true,
            actual: "not sent on a type 2 protection LUN".into(),
        },
        "WRITE ATOMIC (32)" => judge(
            s.command(&cdb32(0x000f, b.lba, 1), 0, Some(&b.original)),
            thirty_two,
        ),
        // No ranges: nothing is written whether or not the command is taken.
        "WRITE SCATTERED (32)" => judge(s.command(&cdb32(0x0011, 0, 0), 0, None), thirty_two),
        "GET LBA STATUS (32)" => {
            let mut c = cdb32(0x0012, b.lba, 1);
            c[28..32].copy_from_slice(&64u32.to_be_bytes());
            c[20..24].copy_from_slice(&1u32.to_be_bytes());
            judge(s.command(&c, 64, None), good)
        }
        // SBC-5 5.48: one block, when Block Limits reports atomic writes.
        "WRITE ATOMIC (16)" => {
            let mut c = super::block_io::cdb16(0x9c, 0, b.lba, 0);
            c[12..14].copy_from_slice(&1u16.to_be_bytes());
            let limited = disk.limits().is_some_and(|l| l.max_atomic_transfer > 0);
            let mut p = judge(s.command(&c, 0, Some(&mine)), good);
            p.passed &= if p.supported { stored(s) } else { !limited };
            p
        }
        // SBC-5 5.55: LOGICAL BLOCK DATA OFFSET 1 (the data starts at the second block), one range.
        "WRITE SCATTERED (16)" => {
            let mut c = [0u8; 16];
            c[0] = 0x9f;
            c[1] = 0x12;
            c[4..6].copy_from_slice(&1u16.to_be_bytes());
            c[8..10].copy_from_slice(&1u16.to_be_bytes());
            c[10..14].copy_from_slice(&2u32.to_be_bytes());
            let mut p = judge(s.command(&c, 0, Some(&scattered(b.lba, &mine))), good);
            p.passed &= stored(s) || !p.supported;
            p
        }
        // SBC-5 5.15: PREVENT 00b allows removal; harmless on any disk.
        "PREVENT ALLOW MEDIUM REMOVAL" => judge(s.command(&[0x1e, 0, 0, 0, 0, 0], 0, None), good),
        // SBC-5 5.22, 5.23: REQ_PLIST and REQ_GLIST clear return only the header, with no defects; a
        // format the device does not use is RECOVERED ERROR, DEFECT LIST NOT FOUND (1Ch/00h).
        "READ DEFECT DATA (10)" => judge(
            s.command(&[0x37, 0, 0, 0, 0, 0, 0, 0, 4, 0], 4, None),
            |o| good(o) || Want::Ascq(0x1, 0x1c, 0x00).matches(o),
        ),
        "READ DEFECT DATA (12)" => judge(s.command(&cdb12(0xb7, 0, 8), 8, None), |o| {
            good(o) || Want::Ascq(0x1, 0x1c, 0x00).matches(o)
        }),
        // SPC-6 6.18.4: descriptor mode (03h) returns four bytes.
        "READ BUFFER (10)" => judge(
            s.command(&[0x3c, 0x03, 0, 0, 0, 0, 0, 0, 4, 0], 4, None),
            good,
        ),
        "READ BUFFER (16)" => {
            let mut c = [0u8; 16];
            c[0] = 0x9b;
            c[1] = 0x03;
            c[10..14].copy_from_slice(&4u32.to_be_bytes());
            judge(s.command(&c, 4, None), good)
        }
        // SPC-6 6.24: PCV with page 00h returns the supported diagnostic pages, page code 00h.
        "RECEIVE DIAGNOSTIC RESULTS" => judge(
            s.command(&[0x1c, 0x01, 0x00, 0x00, 0xff, 0], 255, None),
            |o| good(o) && o.data.first() == Some(&0),
        ),
        // SPC-6 6.37: security protocol 00h lists the supported protocols, 00h among them.
        "SECURITY PROTOCOL IN" => {
            judge(s.command(&sa_in(0xa2, 0x00, 12, 6, 512), 512, None), |o| {
                let n = be16(&o.data, 6) as usize;
                good(o) && o.data.get(8..8 + n).is_some_and(|l| l.contains(&0))
            })
        }
        "REPORT IDENTIFYING INFORMATION" => {
            judge(s.command(&sa_in(0xa3, 0x05, 12, 6, 256), 256, None), good)
        }
        // SPC-6 6.35: TIMESTAMP PARAMETER DATA LENGTH is 000Ah.
        "REPORT TIMESTAMP" => judge(s.command(&sa_in(0xa3, 0x0f, 12, 6, 12), 12, None), |o| {
            good(o) && be16(&o.data, 0) == 0x0a
        }),
        "REPORT PROVISIONING INITIALIZATION PATTERN" => {
            judge(s.command(&sa_in(0xa3, 0x1d, 12, 6, 512), 512, None), good)
        }
        // SBC-5 5.28: required when Extended INQUIRY Data sets R_SUP.
        "REPORT REFERRALS" => {
            let mut p = judge(s.command(&sa_in(0x9e, 0x13, 16, 10, 512), 512, None), good);
            p.passed &= p.supported || !disk.extended().is_some_and(|e| e.r_sup);
            p
        }
        "GET PHYSICAL ELEMENT STATUS" => {
            judge(s.command(&sa_in(0x9e, 0x17, 16, 10, 64), 64, None), good)
        }
        "GET STREAM STATUS" => judge(s.command(&sa_in(0x9e, 0x16, 16, 10, 64), 64, None), good),
        // SBC-5 5.2: BO_CTL 00b asks for no change.
        "BACKGROUND CONTROL" => judge(s.command(&sa_in(0x9e, 0x15, 16, 10, 0), 0, None), good),
        "READ MEDIA SERIAL NUMBER" => {
            judge(s.command(&sa_in(0xab, 0x01, 12, 6, 64), 64, None), good)
        }
        "READ ATTRIBUTE" => judge(s.command(&sa_in(0x8c, 0x00, 16, 10, 64), 64, None), good),
        "REPORT ALIASES" => judge(s.command(&sa_in(0xa3, 0x0b, 12, 6, 64), 64, None), good),
        "REPORT PRIORITY" => judge(s.command(&sa_in(0xa3, 0x0e, 12, 6, 64), 64, None), good),
        "REPORT ALL ROD TOKENS" => judge(s.command(&sa_in(0x84, 0x08, 16, 10, 64), 64, None), good),
        _ => return None,
    };
    // INVALID FIELD IN CDB refuses only a service action (SPC-6 4.2.5.2); for a command without one it
    // is an answer to the command, so the command is implemented.
    let mut p = p;
    if row.service_action.is_none() && !p.supported && p.actual == "CHECK CONDITION 5/24/00" {
        p.supported = true;
        p.passed = false;
    }
    Some(p)
}

/// RESERVE (6) and RELEASE (6) (SPC-2, obsolete since SPC-4): with an SPC-2 reservation held by this
/// nexus, another nexus's READ conflicts; after RELEASE it reads again.
pub(super) fn spc2(s: &mut Port, other: &mut Port, b: &Block) -> (Probe, Probe) {
    let lba = b.lba as u32;
    let reserve = s.command(&super::ops::RESERVE_6, 0, None);
    match reserve {
        Ok(o) if o.status == 0 => {
            let blocked = super::lun_access::read_cmd(other, lba, b.len)
                .is_ok_and(|o| Want::Conflict.matches(&o));
            let release = s.command(&super::ops::RELEASE_6, 0, None);
            let freed = super::lun_access::read_cmd(other, lba, b.len).is_ok_and(|o| good(&o));
            (
                Probe {
                    section: SECTION,
                    supported: true,
                    passed: blocked,
                    actual: format!(
                        "GOOD; another nexus's READ {}",
                        if blocked {
                            "conflicts"
                        } else {
                            "is not blocked"
                        }
                    ),
                },
                judge(release, |o| good(o) && freed),
            )
        }
        other_answer => {
            let r = judge(other_answer, |_| false);
            let release = judge(s.command(&super::ops::RELEASE_6, 0, None), good);
            (r, release)
        }
    }
}

/// Whether the LUN has type 2 protection enabled: PROT_EN set and P_TYPE 1 in READ CAPACITY (16).
pub(super) fn type_2_protection(disk: &Disk) -> bool {
    disk.rc16
        .get(12)
        .is_some_and(|p| p & 0x01 != 0 && (p >> 1) & 0x7 == 1)
}

/// The 32-byte commands a device server refuses as INVALID COMMAND OPERATION CODE without type 2
/// protection (SBC-5 4.21.2.2 and 4.21.2.3; SBC-3 4.22.2.2 lists the five it has).
pub(super) const TYPE_2_ONLY: [&str; 8] = [
    "READ (32)",
    "VERIFY (32)",
    "WRITE (32)",
    "WRITE AND VERIFY (32)",
    "WRITE ATOMIC (32)",
    "WRITE SAME (32)",
    "WRITE SCATTERED (32)",
    "WRITE STREAM (32)",
];

#[cfg(test)]
mod tests {
    use super::super::cases::REFUSED_SA;
    use super::*;

    #[test]
    fn a_32_byte_cdb_has_its_fixed_fields() {
        let c = cdb32(0x0009, 0x0102_0304_0506_0708, 1);
        assert_eq!((c[0], c[7]), (0x7f, 0x18));
        assert_eq!(&c[8..10], &[0, 9]);
        assert_eq!(&c[12..20], &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(&c[28..32], &[0, 0, 0, 1]);
    }

    #[test]
    fn scattered_data_has_header_descriptor_pad_and_block() {
        let block = vec![0xab; 512];
        let d = scattered(9, &block);
        assert_eq!(d.len(), 1024);
        assert_eq!(&d[32..40], &9u64.to_be_bytes());
        assert_eq!(&d[40..44], &1u32.to_be_bytes());
        assert!(d[44..512].iter().all(|&b| b == 0));
        assert_eq!(&d[512..], &block[..]);
    }

    #[test]
    fn refusals_pass_unless_the_rule_is_strict() {
        let refusal = Outcome {
            status: 2,
            sense: vec![0x72, 5, 0x20, 0],
            data: vec![],
            received: 0,
        };
        let p = judge(Ok(refusal), |_| false);
        assert!(!p.supported && p.passed);
        assert!(REFUSED_SA.iter().any(|w| w.matches(&Outcome {
            status: 2,
            sense: vec![0x72, 5, 0x24, 0],
            data: vec![],
            received: 0
        })));
    }
}
