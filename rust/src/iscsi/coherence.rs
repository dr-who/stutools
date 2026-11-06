//! The `coherence` cases of `--compliance`: what one I_T nexus writes, every other nexus to the
//! logical unit reads back at once. A logical unit has one medium (SAM-6), so a write that has
//! completed is the block's content for every nexus; cluster file systems (GPFS, GFS2, OCFS2) and
//! active/active multipath depend on it. Each nexus in turn writes a pattern of its own to one block,
//! and every other nexus reads it, round robin. The nexuses are A and A's second port, and with a
//! second initiator name B and C too: on a local multipath disk, separate host adapters and disk
//! ports. The block, the third from the end, gets its own data back afterwards.

use super::cases::record;
use super::lun_access::{
    last_block, open, pattern, read_block, unit_attentions, write_block, Creds,
};
use super::path::Path;
use super::port::Port;
use super::pr_ports::{ISID_A, ISID_A2, ISID_B, ISID_C};
use crate::compliance::Case;

const SECTION: &str = "coherence";

pub(super) fn run(
    path: &Path,
    initiator: &str,
    second: Option<&str>,
    creds: Creds,
    name: &str,
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
        match open(path, who, creds, isid) {
            Ok(mut p) => {
                // A new nexus's first answer may be a unit attention, not the data.
                let _ = unit_attentions(&mut p);
                ports.push((label, p));
            }
            Err(e) => {
                record(
                    cases,
                    SECTION,
                    format!("{name} {label} opens"),
                    "open",
                    e,
                    false,
                );
            }
        }
    }
    if ports.len() < 2 {
        return;
    }
    let Ok((last, len)) = last_block(&mut ports[0].1) else {
        return;
    };
    let Some(lba) = last.checked_sub(2) else {
        return;
    };
    let original = match read_block(&mut ports[0].1, lba, len) {
        Ok(d) => d,
        Err(e) => {
            record(
                cases,
                SECTION,
                format!("{name} read the block"),
                "GOOD",
                e,
                false,
            );
            return;
        }
    };
    for w in 0..ports.len() {
        let data = pattern(len, 0x40 + w as u8);
        let writer = ports[w].0;
        if let Err(e) = write_block(&mut ports[w].1, lba, &data) {
            record(
                cases,
                SECTION,
                format!("{name} {writer} writes"),
                "GOOD",
                e,
                false,
            );
            continue;
        }
        for r in (0..ports.len()).filter(|&r| r != w) {
            let reader = ports[r].0;
            let (actual, passed) = match read_block(&mut ports[r].1, lba, len) {
                Ok(got) if got == data => ("the same data".to_string(), true),
                Ok(_) => ("other data".to_string(), false),
                Err(e) => (e, false),
            };
            record(
                cases,
                SECTION,
                format!("{name} {writer} writes, {reader} reads it"),
                "what was written",
                actual,
                passed,
            );
        }
    }
    let restored = write_block(&mut ports[0].1, lba, &original);
    record(
        cases,
        SECTION,
        format!("{name} the block's own data written back"),
        "GOOD",
        restored
            .as_ref()
            .err()
            .cloned()
            .unwrap_or_else(|| "GOOD".into()),
        restored.is_ok(),
    );
    for (_, p) in ports {
        let _ = p.logout();
    }
}
