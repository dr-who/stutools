//! The nvme-io cases of `--compliance` (NVMe NVM Command Set Specification): what Windows sends a
//! namespace once it is a disk. For each namespace: the first and last blocks read, the namespace's
//! limits (`io_limit_cases`), a write and its read-back (NTFS), the FUA bit (NTFS metadata and log
//! writes), the optional commands ONCS lists (`oncs_cases`), and Flush (NTFS flushes the write cache);
//! then a read and a write of an inactive namespace. NVM 1.1: Read 3.3.4, Write 3.3.6 (FUA is CDW12
//! bit 30, Figure 53). SCSI-NVMe 1.5 maps Windows' READ/WRITE (5.3, 5.8) and SYNCHRONIZE CACHE (5.5,
//! Flush) onto these.
//! Data is changed only in the 8 blocks from 16 before the end (`AREA`), and what was there is
//! written back. Runs on its own association, as a host of its own.

use super::compliance::Target;
use super::controller;
use super::host::Host;
use super::identify::NamespaceInfo;
use super::identify_cases::Facts;
use super::link::Data;
use super::nvm;
use super::revisions as rev;
use super::sqe::{self, Completion};
use super::verdict::{status_case, INVALID_NAMESPACE, SUCCESS};
use super::{io_limit_cases, oncs_cases, DEFAULT_HOSTNQN};
use crate::compliance::{case, Case};
use crate::Result;

/// The blocks the cases may change: `AREA` blocks starting this many blocks before the end.
pub const AREA_FROM_END: u64 = 16;
pub const AREA: u16 = 8;

/// What the I/O cases found, for the windows section.
pub struct IoFacts {
    /// Flush succeeded on every namespace.
    pub flush: bool,
    /// Dataset Management deallocate: None when ONCS does not list it, else whether it worked.
    pub trim: Option<bool>,
}

/// Whether a command completed with success.
pub fn ok(done: &Result<Completion>) -> bool {
    matches!(done, Ok(d) if d.ok())
}

/// Reads `blocks` blocks of `len` bytes from `lba`, failing on any status but success.
pub fn read_blocks(h: &mut Host, nsid: u32, lba: u64, blocks: u16, len: usize) -> Result<Vec<u8>> {
    let mut buf = vec![0u8; len * blocks as usize];
    let done = h.io(nvm::read(nsid, lba, blocks), Data::In(&mut buf))?;
    if !done.ok() {
        let (sct, sc) = done.code();
        return Err(format!("Read: status type {sct:#x} code {sc:#04x}"));
    }
    Ok(buf)
}

/// One block: its namespace, LBA and length in bytes.
pub type Block = (u32, u64, usize);

/// A case that a read of `block` returns `want`, named by `what`.
pub fn holds(
    cases: &mut Vec<Case>,
    h: &mut Host,
    name: String,
    block: Block,
    want: &[u8],
    what: &str,
) {
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
    io_limit_cases::run(h, name, nsid, ns, facts, cases);

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

    oncs_cases::run(
        h,
        name,
        (nsid, base, len),
        ns,
        facts,
        [&a, &b, &c],
        out,
        cases,
    );

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
