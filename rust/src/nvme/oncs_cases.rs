//! The optional commands of the nvme-io section that ONCS lists (Base 2.1 Figure 312): Compare (bit 0;
//! a miscompare is Compare Failure, NVM 1.1 3.3.1 and Figure 19), Write Zeroes (bit 3; the block then
//! reads as zeros, NVM 1.1 3.3.8) and Dataset Management deallocate (bit 2; Windows TRIM through
//! SCSI-NVMe 1.5 5.6 UNMAP; afterwards the block reads as DLFEAT says or as the data that was there,
//! NVM 1.1 3.3.3). A command ONCS does not list must be refused as Invalid Command Opcode. Each case
//! uses one block of the test area, which the caller writes back.

use super::host::Host;
use super::identify::NamespaceInfo;
use super::identify_cases::Facts;
use super::io_cases::{holds, read_blocks, Block, IoFacts};
use super::link::Data;
use super::nvm;
use super::revisions as rev;
use super::sqe;
use super::verdict::{status_case, COMPARE_FAILURE, INVALID_OPCODE, SUCCESS};
use crate::compliance::{case, Case};

/// ONCS bits for the optional commands.
const ONCS_COMPARE: u16 = 1 << 0;
const ONCS_DSM: u16 = 1 << 2;
const ONCS_WRITE_ZEROES: u16 = 1 << 3;

/// Runs the ONCS cases on the test area starting at `block`, which holds pattern B; `patterns` are
/// A, B and C.
#[allow(clippy::too_many_arguments)]
pub fn run(
    h: &mut Host,
    name: &str,
    block: Block,
    ns: &NamespaceInfo,
    facts: &Facts,
    patterns: [&[u8]; 3],
    out: &mut IoFacts,
    cases: &mut Vec<Case>,
) {
    let io = "nvme-io";
    let (nsid, base, len) = block;
    let [a, b, c] = patterns;
    // NVM 1.1 3.3.1: Compare; a miscompare is Compare Failure (Figure 19). The block holds B, so comparing B matches and comparing A miscompares.
    let oncs = facts.ctrl.oncs;
    if oncs & ONCS_COMPARE != 0 {
        let done = h.send(nvm::block_command(sqe::OP_COMPARE, nsid, base, 1, false), b);
        status_case(
            cases,
            io,
            format!("{name} ns {nsid} {}, data matches", rev::COMPARE.name()),
            &[SUCCESS],
            "success",
            &done,
        );
        let done = h.send(nvm::block_command(sqe::OP_COMPARE, nsid, base, 1, false), a);
        status_case(
            cases,
            io,
            format!("{name} ns {nsid} {}, data differs", rev::COMPARE.name()),
            &[COMPARE_FAILURE],
            "Compare Failure",
            &done,
        );
    } else {
        let done = h.send(nvm::block_command(sqe::OP_COMPARE, nsid, base, 1, false), b);
        status_case(
            cases,
            io,
            format!(
                "{name} ns {nsid} {} (ONCS bit 0 clear)",
                rev::COMPARE.name()
            ),
            &[INVALID_OPCODE],
            "Invalid Command Opcode",
            &done,
        );
    }

    // Write Zeroes on the next block, which holds C first so the zeros are visible.
    let zlba = base + 1;
    let _ = h.send(nvm::write(nsid, zlba, 1), c);
    let done = h.io(nvm::write_zeroes(nsid, zlba, 1), Data::None);
    if oncs & ONCS_WRITE_ZEROES != 0 {
        if status_case(
            cases,
            io,
            format!("{name} ns {nsid} {}", rev::WRITE_ZEROES.name()),
            &[SUCCESS],
            "success",
            &done,
        ) {
            holds(
                cases,
                h,
                format!(
                    "{name} ns {nsid} {} reads back zeros",
                    rev::WRITE_ZEROES.name()
                ),
                (nsid, zlba, len),
                &vec![0u8; len],
                "zeros",
            );
        }
    } else {
        status_case(
            cases,
            io,
            format!(
                "{name} ns {nsid} {} (ONCS bit 3 clear)",
                rev::WRITE_ZEROES.name()
            ),
            &[INVALID_OPCODE],
            "Invalid Command Opcode",
            &done,
        );
    }

    // NVM 1.1 3.3.3 (Figure 45 AD: "may deallocate"): afterwards the block reads as DLFEAT.DRB says
    // a deallocated block does (zeros for 001b), or, if it was not deallocated, as the data that was there.
    let dlba = base + 2;
    let _ = h.send(nvm::write(nsid, dlba, 1), c);
    let done = h.send(nvm::deallocate(nsid), &nvm::range(dlba, 1));
    if oncs & ONCS_DSM != 0 {
        let worked = status_case(
            cases,
            io,
            format!(
                "{name} ns {nsid} {} deallocate",
                rev::DATASET_MANAGEMENT.name()
            ),
            &[SUCCESS],
            "success",
            &done,
        );
        let mut ok_after = false;
        if worked {
            let after = read_blocks(h, nsid, dlba, 1, len);
            let zeros_only = rev::DLFEAT_FIELD.defined_in(facts.vs) && ns.dlfeat & 0x7 == 1;
            ok_after = matches!(&after, Ok(d) if d.iter().all(|&x| x == 0) || d == c);
            cases.push(case(
                io,
                format!(
                    "{name} ns {nsid} {} read after deallocate",
                    rev::DATASET_MANAGEMENT.name()
                ),
                if zeros_only {
                    format!(
                        "zeros ({} DRB 001b), or the data if not deallocated",
                        rev::DLFEAT_FIELD.name()
                    )
                } else {
                    "zeros or the data".into()
                },
                match &after {
                    Ok(d) if d.iter().all(|&x| x == 0) => "zeros".into(),
                    Ok(d) if d == c => "the data".into(),
                    Ok(_) => "other data".into(),
                    Err(e) => e.clone(),
                },
                ok_after,
            ));
        }
        out.trim = Some(worked && ok_after);
    } else {
        status_case(
            cases,
            io,
            format!(
                "{name} ns {nsid} {} (ONCS bit 2 clear)",
                rev::DATASET_MANAGEMENT.name()
            ),
            &[INVALID_OPCODE],
            "Invalid Command Opcode",
            &done,
        );
    }
}
