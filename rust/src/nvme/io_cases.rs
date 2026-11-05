//! The nvme-io cases of `--compliance` (NVMe NVM Command Set Specification): what Windows sends a
//! namespace once it is a disk. Reads and writes (NTFS), the FUA bit (NTFS metadata and log writes),
//! errors for a block past the end and an inactive namespace, a read of the largest transfer (MDTS)
//! and the refusal of a larger one, Flush (NTFS flushes the write cache), and the optional commands
//! ONCS lists: Compare, Write Zeroes and Dataset Management deallocate (Windows TRIM). A command ONCS
//! does not list must be refused as Invalid Command Opcode.
//! NVM 1.1: Read 3.3.4, Write 3.3.6, Compare 3.3.1, Dataset Management 3.3.3, Write Zeroes 3.3.8 (FUA is
//! CDW12 bit 30, Figure 53); Base 2.1 Figure 312 MDTS. SCSI-NVMe 1.5 maps Windows' SCSI commands onto
//! these: READ/WRITE 5.3 and 5.8, SYNCHRONIZE CACHE 5.5 (Flush), UNMAP 5.6 (Dataset Management).
//! Data is changed only in the 8 blocks from 16 before the end (`AREA`), and what was there is
//! written back. Runs on its own association, as a host of its own.

use super::compliance::{
    status_case, Target, COMPARE_FAILURE, INVALID_FIELD, INVALID_NAMESPACE, INVALID_OPCODE,
    LBA_OUT_OF_RANGE, SUCCESS,
};
use super::controller::{self, NamespaceInfo};
use super::host::Host;
use super::identify_cases::Facts;
use super::link::Data;
use super::nvm;
use super::revisions as rev;
use super::sqe::{self, Completion};
use super::DEFAULT_HOSTNQN;
use crate::compliance::{case, Case};
use crate::Result;

/// The blocks the cases may change: `AREA` blocks starting this many blocks before the end.
pub const AREA_FROM_END: u64 = 16;
pub const AREA: u16 = 8;

/// ONCS bits for the optional commands.
const ONCS_COMPARE: u16 = 1 << 0;
const ONCS_DSM: u16 = 1 << 2;
const ONCS_WRITE_ZEROES: u16 = 1 << 3;

/// The largest transfer tried: a larger MDTS is tested only up to this.
const LARGEST: usize = 1 << 20;

/// What the I/O cases found, for the windows section.
pub struct IoFacts {
    /// Flush succeeded on every namespace.
    pub flush: bool,
    /// Dataset Management deallocate: None when ONCS does not list it, else whether it worked.
    pub trim: Option<bool>,
}

fn ok(done: &Result<Completion>) -> bool {
    matches!(done, Ok(d) if d.ok())
}

fn read_blocks(h: &mut Host, nsid: u32, lba: u64, blocks: u16, len: usize) -> Result<Vec<u8>> {
    let mut buf = vec![0u8; len * blocks as usize];
    let done = h.io(nvm::read(nsid, lba, blocks), Data::In(&mut buf))?;
    if !done.ok() {
        let (sct, sc) = done.code();
        return Err(format!("Read: status type {sct:#x} code {sc:#04x}"));
    }
    Ok(buf)
}

/// One block: its namespace, LBA and length in bytes.
type Block = (u32, u64, usize);

/// A case that a read of `block` returns `want`, named by `what`.
fn holds(cases: &mut Vec<Case>, h: &mut Host, name: String, block: Block, want: &[u8], what: &str) {
    let (nsid, lba, len) = block;
    let got = read_blocks(h, nsid, lba, 1, len);
    cases.push(case(
        "nvme-io",
        name,
        what.into(),
        match &got {
            Ok(d) if d == want => what.into(),
            Ok(d) if d.iter().all(|&b| b == 0) => "zeros".into(),
            Ok(_) => "other data".into(),
            Err(e) => e.clone(),
        },
        matches!(&got, Ok(d) if d == want),
    ));
}

/// Runs the nvme-io cases for every namespace in `facts`.
pub fn run(t: &Target, name: &str, facts: &Facts, cases: &mut Vec<Case>) -> IoFacts {
    let mut out = IoFacts {
        flush: !facts.namespaces.is_empty(),
        trim: None,
    };
    let mut h = match controller::random_hostid()
        .and_then(|id| Host::open(t.addrs[0], &t.subnqn, DEFAULT_HOSTNQN, id))
    {
        Ok(h) => {
            cases.push(case(
                "nvme-io",
                format!("{name} I/O queue"),
                "connected".into(),
                "connected".into(),
                true,
            ));
            h
        }
        Err(e) => {
            cases.push(case(
                "nvme-io",
                format!("{name} I/O queue"),
                "connected".into(),
                e,
                false,
            ));
            out.flush = false;
            return out;
        }
    };
    for (nsid, ns) in &facts.namespaces {
        namespace(&mut h, name, *nsid, ns, facts, &mut out, cases);
    }
    if let Some((_, ns)) = facts.namespaces.first() {
        let inactive = (1..u32::MAX)
            .find(|n| !facts.active.contains(n))
            .unwrap_or(u32::MAX - 1);
        let len = ns.block_len as usize;
        let mut buf = vec![0u8; len];
        let done = h.io(nvm::read(inactive, 0, 1), Data::In(&mut buf));
        status_case(
            cases,
            "nvme-io",
            format!(
                "{name} {} read of inactive namespace {inactive}",
                rev::READ_WRITE.name()
            ),
            &[INVALID_NAMESPACE],
            "Invalid Namespace or Format",
            &done,
        );
        let done = h.send(nvm::write(inactive, 0, 1), &buf);
        status_case(
            cases,
            "nvme-io",
            format!(
                "{name} {} write to inactive namespace {inactive}",
                rev::READ_WRITE.name()
            ),
            &[INVALID_NAMESPACE],
            "Invalid Namespace or Format",
            &done,
        );
    }
    out
}

fn namespace(
    h: &mut Host,
    name: &str,
    nsid: u32,
    ns: &NamespaceInfo,
    facts: &Facts,
    out: &mut IoFacts,
    cases: &mut Vec<Case>,
) {
    let io = "nvme-io";
    let len = ns.block_len as usize;
    let mut one = vec![0u8; len];
    for (label, lba) in [("first", 0), ("last", ns.blocks - 1)] {
        let done = h.io(nvm::read(nsid, lba, 1), Data::In(&mut one));
        status_case(
            cases,
            io,
            format!(
                "{name} ns {nsid} {} read the {label} block",
                rev::READ_WRITE.name()
            ),
            &[SUCCESS],
            "success",
            &done,
        );
    }
    let done = h.io(nvm::read(nsid, ns.blocks, 1), Data::In(&mut one));
    status_case(
        cases,
        io,
        format!(
            "{name} ns {nsid} {} read past the end",
            rev::READ_WRITE.name()
        ),
        &[LBA_OUT_OF_RANGE],
        "LBA Out of Range",
        &done,
    );
    let mut two = vec![0u8; 2 * len];
    let done = h.io(nvm::read(nsid, ns.blocks - 1, 2), Data::In(&mut two));
    status_case(
        cases,
        io,
        format!(
            "{name} ns {nsid} {} read across the end",
            rev::READ_WRITE.name()
        ),
        &[LBA_OUT_OF_RANGE],
        "LBA Out of Range",
        &done,
    );
    let done = h.send(nvm::write(nsid, ns.blocks, 1), &one);
    status_case(
        cases,
        io,
        format!(
            "{name} ns {nsid} {} write past the end",
            rev::READ_WRITE.name()
        ),
        &[LBA_OUT_OF_RANGE],
        "LBA Out of Range",
        &done,
    );

    // Base 2.1 Figure 312: a transfer over MDTS is Invalid Field in Command. The largest transfer: MDTS in units of the minimum page size (CAP.MPSMIN), 0 for no limit.
    let page = 4096usize << ((facts.cap >> 48) & 0xf);
    let limit = (facts.ctrl.mdts != 0).then(|| page << facts.ctrl.mdts);
    let biggest = limit.unwrap_or(LARGEST).min(LARGEST) / len * len;
    let blocks = (biggest / len)
        .min(u16::MAX as usize + 1)
        .min(ns.blocks as usize);
    let mut big = vec![0u8; blocks * len];
    let done = h.io(nvm::read(nsid, 0, blocks as u16), Data::In(&mut big));
    status_case(
        cases,
        io,
        format!(
            "{name} ns {nsid} read of {} KiB (MDTS {})",
            blocks * len / 1024,
            limit.map_or("none".into(), |l| format!("{} KiB", l / 1024))
        ),
        &[SUCCESS],
        "success",
        &done,
    );
    if let Some(l) = limit.filter(|&l| l < LARGEST && (l / len + 1) as u64 <= ns.blocks) {
        let n = l / len + 1;
        let mut over = vec![0u8; n * len];
        let done = h.io(nvm::read(nsid, 0, n as u16), Data::In(&mut over));
        status_case(
            cases,
            io,
            format!("{name} ns {nsid} read larger than MDTS"),
            &[INVALID_FIELD],
            "Invalid Field in Command",
            &done,
        );
    }

    if ns.blocks < 2 * AREA_FROM_END {
        cases.push(case(
            io,
            format!("{name} ns {nsid} writes"),
            "room for the test blocks".into(),
            format!("{} blocks", ns.blocks),
            false,
        ));
        return;
    }
    let base = ns.blocks - AREA_FROM_END;
    let original = match read_blocks(h, nsid, base, AREA, len) {
        Ok(d) => d,
        Err(e) => {
            cases.push(case(
                io,
                format!("{name} ns {nsid} read the test blocks"),
                "success".into(),
                e,
                false,
            ));
            return;
        }
    };
    let a = vec![0xa5u8; len];
    let b = vec![0x5au8; len];
    let c = vec![0x3cu8; len];

    let done = h.send(nvm::write(nsid, base, 1), &a);
    status_case(
        cases,
        io,
        format!("{name} ns {nsid} {} write", rev::READ_WRITE.name()),
        &[SUCCESS],
        "success",
        &done,
    );
    holds(
        cases,
        h,
        format!(
            "{name} ns {nsid} {} read back the write",
            rev::READ_WRITE.name()
        ),
        (nsid, base, len),
        &a,
        "the data written",
    );
    let done = h.send(nvm::block_command(sqe::OP_WRITE, nsid, base, 1, true), &b);
    status_case(
        cases,
        io,
        format!("{name} ns {nsid} {} write", rev::FUA.name()),
        &[SUCCESS],
        "success",
        &done,
    );
    let mut fua = vec![0u8; len];
    let done = h.io(
        nvm::block_command(sqe::OP_READ, nsid, base, 1, true),
        Data::In(&mut fua),
    );
    cases.push(case(
        io,
        format!("{name} ns {nsid} {} read", rev::FUA.name()),
        "the FUA write's data".into(),
        match &done {
            Ok(d) if d.ok() && fua == b => "the FUA write's data".into(),
            Ok(d) if d.ok() => "other data".into(),
            Ok(d) => format!("status {:?}", d.code()),
            Err(e) => e.clone(),
        },
        ok(&done) && fua == b,
    ));

    // NVM 1.1 3.3.1: Compare; a miscompare is Compare Failure (Figure 19). The block holds B, so comparing B matches and comparing A miscompares.
    let oncs = facts.ctrl.oncs;
    if oncs & ONCS_COMPARE != 0 {
        let done = h.send(
            nvm::block_command(sqe::OP_COMPARE, nsid, base, 1, false),
            &b,
        );
        status_case(
            cases,
            io,
            format!("{name} ns {nsid} {}, data matches", rev::COMPARE.name()),
            &[SUCCESS],
            "success",
            &done,
        );
        let done = h.send(
            nvm::block_command(sqe::OP_COMPARE, nsid, base, 1, false),
            &a,
        );
        status_case(
            cases,
            io,
            format!("{name} ns {nsid} {}, data differs", rev::COMPARE.name()),
            &[COMPARE_FAILURE],
            "Compare Failure",
            &done,
        );
    } else {
        let done = h.send(
            nvm::block_command(sqe::OP_COMPARE, nsid, base, 1, false),
            &b,
        );
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
    let _ = h.send(nvm::write(nsid, zlba, 1), &c);
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
    let _ = h.send(nvm::write(nsid, dlba, 1), &c);
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
            ok_after = matches!(&after, Ok(d) if d.iter().all(|&x| x == 0) || *d == c);
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
                    Ok(d) if *d == c => "the data".into(),
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

    let done = h.io(nvm::flush(nsid), Data::None);
    out.flush &= status_case(
        cases,
        io,
        format!("{name} ns {nsid} {}", rev::FLUSH.name()),
        &[SUCCESS],
        "success",
        &done,
    );

    let restored = h.send(nvm::write(nsid, base, AREA), &original);
    if !ok(&restored) {
        cases.push(case(
            io,
            format!("{name} ns {nsid} restore the test blocks"),
            "written back".into(),
            format!("{:?}", restored.map(|d| d.code())),
            false,
        ));
    }
}
