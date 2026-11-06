//! The `unmap` cases of `--compliance`: logical block provisioning (SBC-5 4.7), which Windows uses for
//! TRIM (NTFS delete notifications and Optimize-Volume ReTrim), Linux for discard and VMware for space
//! reclamation. READ CAPACITY (16) LBPME and LBPRZ must agree with the Logical Block Provisioning and
//! Block Limits VPD pages; UNMAP, GET LBA STATUS (16) and WRITE SAME (16) with the UNMAP and NDOB bits
//! must work when those pages advertise them and be refused when they do not. Every block changed is
//! the second-to-last one, and its original data is written back.

use super::block_io::cdb16;
use super::cases::{expect, expect_should, record, Want, REFUSED, REFUSED_SA};
use super::disk::Disk;
use super::lun_access::{read_block, write_block};
use super::ops;
use super::port::Port;
use super::scsi_fields::{be32, be64};
use super::seen::Seen;
use crate::compliance::Case;

const SECTION: &str = "unmap";

/// UNMAP and NDOB in byte 1 of WRITE SAME (16) (SBC-5 table 150).
const WS_UNMAP: u8 = 0x08;
const WS_NDOB: u8 = 0x01;

/// What the provisioning cases found, for the Windows cases.
pub(super) struct Findings {
    /// The LUN advertises UNMAP and it works: Windows sends TRIM.
    pub trim: bool,
    /// No provisioning case failed (a LUN that does not provision passes by refusing cleanly).
    pub clean: bool,
}

pub(super) fn run(
    s: &mut Port,
    name: &str,
    disk: &Disk,
    cases: &mut Vec<Case>,
    seen: &mut Seen,
) -> Findings {
    let first = cases.len();
    let label = |what: &str| format!("{name} {what}");
    let lbpme = disk.lbpme();
    let p = disk.provisioning();
    let limits = disk.limits();
    let lbpu = p.is_some_and(|p| p.lbpu);
    let lbpws = p.is_some_and(|p| p.lbpws);

    // SBC-5 4.7.3.1 and 6.6.9: a LUN with LBPME set has the Logical Block Provisioning page and supports
    // UNMAP or WRITE SAME with UNMAP; one with LBPME clear is fully provisioned and supports neither.
    let unmaps = p.is_some_and(|p| p.lbpu || p.lbpws || p.lbpws10);
    record(
        cases,
        SECTION,
        label("LBPME agrees with VPD B2h"),
        if lbpme {
            "B2h with LBPU, LBPWS or LBPWS10"
        } else {
            "no unmap support advertised"
        },
        format!(
            "LBPME {}, B2h {}",
            lbpme as u8,
            match p {
                Some(p) => format!(
                    "LBPU {} LBPWS {} LBPWS10 {}",
                    p.lbpu as u8, p.lbpws as u8, p.lbpws10 as u8
                ),
                None => "absent".into(),
            }
        ),
        lbpme == unmaps,
    );
    // SBC-5 5.21.2: the READ CAPACITY (16) LBPRZ bit is one exactly when the B2h LBPRZ field is xx1b.
    if let Some(p) = p {
        record(
            cases,
            SECTION,
            label("LBPRZ agrees between READ CAPACITY (16) and VPD B2h"),
            format!("READ CAPACITY (16) LBPRZ {}", p.lbprz & 1),
            format!("{} (B2h field {:03b}b)", disk.lbprz() as u8, p.lbprz),
            disk.lbprz() == (p.lbprz & 1 != 0),
        );
    }
    // SBC-5 6.6.4: the unmap limits are zero exactly when UNMAP is not supported.
    if let Some(l) = limits {
        let advertised = l.max_unmap_lba > 0 && l.max_unmap_descriptors > 0;
        record(
            cases,
            SECTION,
            label("VPD B0h unmap limits agree with LBPU"),
            if lbpu {
                "non-zero limits"
            } else {
                "zero limits"
            },
            format!(
                "MAXIMUM UNMAP LBA COUNT {}, descriptors {}",
                l.max_unmap_lba, l.max_unmap_descriptors
            ),
            advertised == lbpu,
        );
    }

    let Ok(target) = u32::try_from(disk.last_lba.saturating_sub(1)) else {
        return Findings {
            trim: false,
            clean: false,
        };
    };
    let len = disk.block_len as usize;
    let original = match read_block(s, target, len) {
        Ok(d) => d,
        Err(err) => {
            record(
                cases,
                SECTION,
                label("read the test block"),
                "GOOD",
                err,
                false,
            );
            return Findings {
                trim: false,
                clean: false,
            };
        }
    };

    // SBC-5 5.35: UNMAP works when LBPU is set; otherwise it is refused (SBC-5 4.7.3.4).
    let unmap_wants: Vec<Want> = if lbpu {
        vec![Want::Good]
    } else {
        REFUSED.to_vec()
    };
    let answer = expect(
        cases,
        SECTION,
        label("UNMAP one block"),
        &unmap_wants,
        ops::unmap(s, target as u64, 1),
    );
    seen.answer("UNMAP", SECTION, &answer);
    let unmapped = answer.0 && lbpu;
    if unmapped {
        // SBC-5 4.7.4.5: an unmapped block reads as zeros when LBPRZ is set; otherwise zeros or the old
        // data are both allowed, since UNMAP may leave a block mapped (4.7.3.4).
        let after = read_block(s, target, len);
        let zeros = matches!(&after, Ok(d) if d.iter().all(|&b| b == 0));
        let old = matches!(&after, Ok(d) if *d == original);
        record(
            cases,
            SECTION,
            label("read after UNMAP"),
            if disk.lbprz() {
                "zeros, or the original data if it stayed mapped"
            } else {
                "zeros or the original data"
            },
            if zeros {
                "zeros"
            } else if old {
                "original data"
            } else {
                "other data"
            },
            zeros || old,
        );
        // SBC-5 5.35.2: a parameter list with no block descriptors unmaps nothing and is not an error.
        let header = [0u8, 6, 0, 0, 0, 0, 0, 0];
        let empty = s.command(&[0x42, 0, 0, 0, 0, 0, 0, 0, 8, 0], 0, Some(&header));
        let a = expect(
            cases,
            SECTION,
            label("UNMAP with no block descriptors"),
            &[Want::Good],
            empty,
        );
        seen.answer("UNMAP", SECTION, &a);
        // SBC-5 5.35.2: an LBA past the capacity is LOGICAL BLOCK ADDRESS OUT OF RANGE.
        let a = expect(
            cases,
            SECTION,
            label("UNMAP past the last LBA"),
            &[Want::Ascq(0x5, 0x21, 0x00)],
            ops::unmap(s, disk.last_lba + 1, 1),
        );
        seen.answer("UNMAP", SECTION, &a);
    }

    // SBC-5 5.6: GET LBA STATUS (16) for the test block: the first descriptor starts at the LBA asked
    // for. A fully provisioned logical unit (LBPME 0) may support it too (SBC-5 4.7.1), and then every
    // LBA it reports is mapped (4.7.2), or it refuses it cleanly.
    let alloc = 64u32.to_be_bytes();
    let mut cdb = [0u8; 16];
    cdb[0] = 0x9e;
    cdb[1] = 0x12;
    cdb[2..10].copy_from_slice(&(target as u64).to_be_bytes());
    cdb[10..14].copy_from_slice(&alloc);
    let mut wants = vec![Want::Good];
    if !lbpme {
        wants.extend_from_slice(&REFUSED_SA);
    }
    let answer = expect(
        cases,
        SECTION,
        label("GET LBA STATUS (16)"),
        &wants,
        s.command(&cdb, 64, None),
    );
    seen.answer("GET LBA STATUS (16)", SECTION, &answer);
    if let Some(o) = answer.1.filter(|o| o.status == 0) {
        // SBC-5 table 50, 52: the parameter data length is 4 plus a multiple of 16; the first descriptor's
        // LBA is the one asked for, and the provisioning status is 0h, 1h, 2h or 3h.
        let n = be32(&o.data, 0);
        let lba = be64(&o.data, 8);
        let status = o.data.get(20).copied().unwrap_or(0xff) & 0x0f;
        let most = if lbpme { 3 } else { 0 };
        record(
            cases,
            SECTION,
            label("GET LBA STATUS (16) descriptor"),
            format!("starts at {target}, status 0h to {most}h"),
            format!(
                "length {n}, starts at {lba}, status {status:x}h, {} blocks",
                be32(&o.data, 16)
            ),
            n >= 20 && (n - 4) % 16 == 0 && lba == target as u64 && status <= most,
        );
    }

    // SBC-5 5.53 and table 149: WRITE SAME (16) with UNMAP works when LBPWS is set. Without LBPWS the
    // device does not support that use (SBC-5 6.6.9), and SBC-5 names no sense code for it: an
    // unsupported value in a CDB field is INVALID FIELD IN CDB by SPC-6's general rule, so a different
    // answer only warns. On a fully provisioned LUN (LBPME 0) ignoring the UNMAP bit (writing) is also
    // allowed. NDOB sends no data; with UNMAP it unmaps (A.3.3.7 needs it then). Any block written is
    // restored below.
    let zeros = vec![0u8; len];
    let ws_wants: Vec<Want> = match (lbpws, lbpme) {
        (true, _) => vec![Want::Good],
        (false, false) => vec![Want::Good, Want::Ascq(0x5, 0x24, 0x00)],
        (false, true) => vec![Want::Ascq(0x5, 0x24, 0x00)],
    };
    let unmap = s.command(&cdb16(0x93, WS_UNMAP, target as u64, 1), 0, Some(&zeros));
    let answer = if lbpws {
        expect(
            cases,
            SECTION,
            label("WRITE SAME (16) with UNMAP"),
            &ws_wants,
            unmap,
        )
    } else {
        expect_should(
            cases,
            SECTION,
            label("WRITE SAME (16) with UNMAP"),
            &ws_wants,
            unmap,
        )
    };
    seen.fact("WRITE SAME (16) UNMAP", lbpws && answer.0);
    seen.answer("WRITE SAME (16)", SECTION, &answer);
    let mut ndob_wants = vec![Want::Good, Want::Ascq(0x5, 0x24, 0x00)];
    if !lbpws && lbpme {
        ndob_wants.remove(0);
    }
    let ndob = s.command(&cdb16(0x93, WS_UNMAP | WS_NDOB, target as u64, 1), 0, None);
    let answer = expect(
        cases,
        SECTION,
        label("WRITE SAME (16) with UNMAP and NDOB"),
        &ndob_wants,
        ndob,
    );
    seen.fact(
        "WRITE SAME (16) NDOB",
        matches!(&answer.1, Some(o) if o.status == 0),
    );

    if let Err(err) = write_block(s, target, &original) {
        record(
            cases,
            SECTION,
            label("restore the test block"),
            "written back",
            err,
            false,
        );
    }
    Findings {
        trim: lbpme && lbpu && unmapped,
        clean: super::cases::no_failures(&cases[first..]),
    }
}
