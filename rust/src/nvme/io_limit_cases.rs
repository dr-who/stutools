//! The limit cases of the nvme-io section (NVM 1.1 Figure 17; Base 2.1 Figure 312 MDTS): a read past
//! the end, a read across the end and a write past the end are LBA Out of Range; the largest transfer
//! MDTS allows (up to 1 MiB) reads, and one block more is Invalid Field in Command. Only reads change
//! nothing; the write past the end must not reach the medium.

use super::host::Host;
use super::identify::NamespaceInfo;
use super::identify_cases::Facts;
use super::link::Data;
use super::nvm;
use super::revisions as rev;
use super::verdict::{status_case, INVALID_FIELD, LBA_OUT_OF_RANGE, SUCCESS};
use crate::compliance::Case;

/// The largest transfer tried: a larger MDTS is tested only up to this.
const LARGEST: usize = 1 << 20;

/// Runs the limit cases for namespace `nsid`.
pub fn run(
    h: &mut Host,
    name: &str,
    nsid: u32,
    ns: &NamespaceInfo,
    facts: &Facts,
    cases: &mut Vec<Case>,
) {
    let io = "nvme-io";
    let len = ns.block_len as usize;
    let mut one = vec![0u8; len];
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
}
