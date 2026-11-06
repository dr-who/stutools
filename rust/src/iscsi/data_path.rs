//! Multi-block reads and writes, as hosts send them (SBC-5 5.16 to 5.42): a window of up to 1 MiB,
//! which on iSCSI crosses FirstBurstLength, MaxBurstLength and the R2T sequence (RFC 7143 11.7, 11.8),
//! where every case elsewhere moves one block. Each I_T nexus in turn writes the window with WRITE (16)
//! and every nexus reads it back with READ (16), and one reads it again with READ (10) in three
//! unequal pieces (the `coherence` section). Every block carries its LBA and the writer's seed, so a
//! block written to the wrong LBA, or a stale one, shows. Then through A (the `io` section): WRITE
//! (16) with FUA, DPO and both, and READ (16) with each, which a device that does not report DPOFUA
//! may refuse as INVALID FIELD IN CDB (SBC-5 6.5.1); and the boundaries: a two-block WRITE (16) at
//! the last LBA, READ (16) at the highest LBA there is, and a WRITE (16) of no blocks, none of which
//! may change anything. The window ends four blocks before the last, clear of the blocks the other
//! sections use, and gets its own data back afterwards.

use super::block_io::{cdb10, cdb16};
use super::cases::{describe, record, Want};
use super::disk::Disk;
use super::lun_access::{open, pattern, unit_attentions, Creds};
use super::path::Path;
use super::port::Port;
use super::pr_ports::{ISID_A, ISID_A2, ISID_B, ISID_C};
use crate::compliance::Case;

const DPO: u8 = 0x10;
const FUA: u8 = 0x08;
const LBA_OUT_OF_RANGE: Want = Want::Ascq(0x5, 0x21, 0x00);

/// `blocks` blocks from `lba`, each starting with its LBA and `seed`, then a pattern.
fn stamped(lba: u64, blocks: u32, len: usize, seed: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(blocks as usize * len);
    for i in 0..u64::from(blocks) {
        let mut b = pattern(len, seed ^ (i as u8));
        b[..8].copy_from_slice(&(lba + i).to_be_bytes());
        b[8..16].copy_from_slice(&u64::from(seed).to_be_bytes());
        out.extend_from_slice(&b);
    }
    out
}

fn read16(s: &mut Port, flags: u8, lba: u64, blocks: u32, len: usize) -> crate::Result<Vec<u8>> {
    let o = s.command(&cdb16(0x88, flags, lba, blocks), blocks * len as u32, None)?;
    if o.status != 0 {
        return Err(describe(&o));
    }
    Ok(o.data)
}

fn write16(s: &mut Port, flags: u8, lba: u64, data: &[u8], len: usize) -> crate::Result<()> {
    let blocks = (data.len() / len) as u32;
    let o = s.command(&cdb16(0x8a, flags, lba, blocks), 0, Some(data))?;
    if o.status != 0 {
        return Err(describe(&o));
    }
    Ok(())
}

/// What a read gave against what it should: the same, or where it first differs.
fn compare(got: &crate::Result<Vec<u8>>, want: &[u8], len: usize) -> (String, bool) {
    match got {
        Ok(d) if d == want => ("the same data".into(), true),
        Ok(d) => {
            let at = d
                .iter()
                .zip(want)
                .position(|(a, b)| a != b)
                .unwrap_or(d.len().min(want.len()));
            (format!("differs from block {} on", at / len.max(1)), false)
        }
        Err(e) => (e.clone(), false),
    }
}

pub(super) fn run(
    path: &Path,
    initiator: &str,
    second: Option<&str>,
    creds: Creds,
    name: &str,
    disk: &Disk,
    cases: &mut Vec<Case>,
) {
    let mut wanted = vec![
        (initiator, ISID_A, "A"),
        (initiator, ISID_A2, "A's second port"),
    ];
    if let Some(b) = second {
        wanted.push((b, ISID_B, "B"));
        wanted.push((b, ISID_C, "C"));
    }
    let mut ports: Vec<(&str, Port)> = Vec::new();
    for (who, isid, label) in wanted {
        if let Ok(mut p) = open(path, who, creds, isid) {
            let _ = unit_attentions(&mut p);
            ports.push((label, p));
        }
    }
    if ports.is_empty() {
        return;
    }
    let len = disk.block_len as usize;
    let last = disk.last_lba;
    let limits = disk.limits().unwrap_or_default();
    let mut n = ((1u32 << 20) / disk.block_len.max(1)).max(1);
    if limits.max_transfer > 0 {
        n = n.min(limits.max_transfer);
    }
    if let Some(bytes) = ports[0].1.max_transfer() {
        n = n.min((bytes / disk.block_len.max(1)).max(1));
    }
    if last < u64::from(n) + 8 || len < 16 {
        return;
    }
    let lba = last - 4 - u64::from(n) + 1;
    let original = match read16(&mut ports[0].1, 0, lba, n, len) {
        Ok(d) => d,
        Err(e) => {
            record(
                cases,
                "io",
                format!("{name} read the {n}-block window"),
                "GOOD",
                e,
                false,
            );
            return;
        }
    };

    // Round robin: each nexus writes the window, every nexus reads it back.
    for w in 0..ports.len() {
        let data = stamped(lba, n, len, 0x10 + w as u8);
        let writer = ports[w].0;
        if let Err(e) = write16(&mut ports[w].1, 0, lba, &data, len) {
            record(
                cases,
                "coherence",
                format!("{name} {writer} writes {n} blocks"),
                "GOOD",
                e,
                false,
            );
            continue;
        }
        for r in 0..ports.len() {
            let reader = ports[r].0;
            let (actual, passed) = compare(&read16(&mut ports[r].1, 0, lba, n, len), &data, len);
            record(
                cases,
                "coherence",
                format!("{name} {n} blocks written through {writer}, read through {reader}"),
                "what was written",
                actual,
                passed,
            );
        }
        // READ (10) in three unequal pieces, through the next nexus round.
        let r = (w + 1) % ports.len();
        if let Ok(start) = u32::try_from(lba) {
            let cut = [n / 5, n / 2, n];
            let mut got = Vec::with_capacity(data.len());
            let mut from = 0;
            let mut failed = None;
            for to in cut {
                let count = (to - from) as u16;
                if count > 0 {
                    match ports[r].1.command(
                        &cdb10(0x28, 0, start + from, count),
                        u32::from(count) * len as u32,
                        None,
                    ) {
                        Ok(o) if o.status == 0 => got.extend_from_slice(&o.data),
                        Ok(o) => failed = Some(describe(&o)),
                        Err(e) => failed = Some(e),
                    }
                }
                from = to;
            }
            let result = failed.map_or(Ok(got), Err);
            let (actual, passed) = compare(&result, &data, len);
            record(
                cases,
                "coherence",
                format!(
                    "{name} written through {writer}, READ (10) in three pieces through {}",
                    ports[r].0
                ),
                "what was written",
                actual,
                passed,
            );
        }
    }

    let a = &mut ports[0].1;
    // SBC-5 6.5.1: DPO and FUA are accepted when DPOFUA is set; without it they may be refused.
    let mut flag_wants = vec![Want::Good];
    if !disk.dpofua() {
        flag_wants.push(Want::Ascq(0x5, 0x24, 0x00));
    }
    for (flags, label, seed) in [
        (FUA, "FUA", 0x31u8),
        (DPO, "DPO", 0x32),
        (DPO | FUA, "DPO and FUA", 0x33),
    ] {
        let data = stamped(lba, n, len, seed);
        let answer = a.command(&cdb16(0x8a, flags, lba, n), 0, Some(&data));
        let took = answer.as_ref().is_ok_and(|o| o.status == 0);
        let ok = answer
            .as_ref()
            .is_ok_and(|o| flag_wants.iter().any(|w| w.matches(o)));
        record(
            cases,
            "io",
            format!("{name} WRITE (16) of {n} blocks with {label}"),
            if disk.dpofua() {
                "GOOD".to_string()
            } else {
                "GOOD or CHECK CONDITION 5/24/00 (DPOFUA 0)".into()
            },
            answer.as_ref().map_or_else(|e| e.clone(), describe),
            ok,
        );
        if took {
            let (actual, passed) = compare(&read16(a, flags, lba, n, len), &data, len);
            record(
                cases,
                "io",
                format!("{name} READ (16) with {label} returns what WRITE (16) with {label} wrote"),
                "what was written",
                actual,
                passed,
            );
        }
    }

    // Boundaries: nothing may change.
    let last_block = read16(a, 0, last, 1, len);
    let over = a.command(
        &cdb16(0x8a, 0, last, 2),
        0,
        Some(&stamped(last, 2, len, 0x41)),
    );
    record(
        cases,
        "io",
        format!("{name} WRITE (16) of 2 blocks at the last LBA"),
        "CHECK CONDITION 5/21/00",
        over.as_ref().map_or_else(|e| e.clone(), describe),
        over.as_ref().is_ok_and(|o| LBA_OUT_OF_RANGE.matches(o)),
    );
    if let Ok(before) = &last_block {
        let (actual, passed) = compare(&read16(a, 0, last, 1, len), before, len);
        record(
            cases,
            "io",
            format!("{name} the last block is unchanged by it"),
            "the same data",
            actual,
            passed,
        );
    }
    let wrap = a.command(&cdb16(0x88, 0, u64::MAX, 2), 2 * len as u32, None);
    record(
        cases,
        "io",
        format!("{name} READ (16) of 2 blocks at LBA FFFFFFFFFFFFFFFFh"),
        "CHECK CONDITION 5/21/00",
        wrap.as_ref().map_or_else(|e| e.clone(), describe),
        wrap.as_ref().is_ok_and(|o| LBA_OUT_OF_RANGE.matches(o)),
    );
    let before = read16(a, 0, lba, 1, len);
    let none = a.command(&cdb16(0x8a, 0, lba, 0), 0, None);
    let unchanged = before
        .as_ref()
        .is_ok_and(|b| read16(a, 0, lba, 1, len).is_ok_and(|now| &now == b));
    record(
        cases,
        "io",
        format!("{name} WRITE (16) of 0 blocks"),
        "GOOD, nothing written",
        format!(
            "{}, {}",
            none.as_ref().map_or_else(|e| e.clone(), describe),
            if unchanged {
                "nothing written"
            } else {
                "the block changed"
            }
        ),
        none.as_ref().is_ok_and(|o| o.status == 0) && unchanged,
    );

    // The window's own data back.
    let back = write16(a, FUA, lba, &original, len);
    let (actual, passed) = match back {
        Ok(()) => compare(&read16(a, 0, lba, n, len), &original, len),
        Err(e) => (e, false),
    };
    record(
        cases,
        "io",
        format!("{name} the {n}-block window written back"),
        "the same data",
        actual,
        passed,
    );
    for (_, p) in ports {
        let _ = p.logout();
    }
}
