//! The 12 MiB cache runs of the `io` section, as Microsoft's SCSI compliance tests make them (SBC-5
//! 5.16, 5.18, 5.33, 5.40, 5.42, 6.5.6): 12 MiB read twice with READ (10) and with READ (16), 12 MiB
//! read elsewhere between, the same both times ("cache cleared"); 12 MiB written with WRITE (10) and
//! with WRITE (16) while the write cache is on (WCE 1), flushed with SYNCHRONIZE CACHE (10) over the
//! range, 12 MiB read elsewhere, and read back; the same with the write cache off (WCE 0), with no
//! flush; and 12 MiB written with FUA. A WCE the Caching page does not let change leaves only the runs
//! its value allows, which say so. The writes go to the scratch window (`scratch_region`), read first
//! and written back after, and the Caching page is written back as it was.

use super::block_io::cdb10;
use super::cases::{describe, record, record_info};
use super::disk::Disk;
use super::port::Port;
use super::scratch_region::{read, read_with, stamps, write, Scratch};
use super::write_cache::{self, caching_page, wce, CHANGEABLE, CURRENT, WCE};
use crate::compliance::Case;
use crate::Result;

const SECTION: &str = "io";

/// FUA in byte 1 of WRITE (10) and (16) (SBC-5 table 134).
const FUA: u8 = 0x08;

/// Writes `pass`'s stamps over the window with `op` and `flags`, flushes when `flush`, reads 12 MiB
/// elsewhere, and reads the window back: Ok(true) when it holds what was written.
fn round_trip(
    s: &mut Port,
    w: &Scratch,
    op: u8,
    flags: u8,
    flush: bool,
    pass: u32,
) -> Result<bool> {
    let data = stamps(w.len, w.start, w.blocks, pass);
    write(s, op, flags, w.start, &data, w.len, w.chunk)?;
    if flush {
        let l = u32::try_from(w.start).map_err(|_| {
            "not run: window past 32-bit LBAs, which READ (10) and WRITE (10) cannot address"
        })?;
        let o = s.command(
            &cdb10(0x35, 0, l, w.blocks.min(u32::from(u16::MAX)) as u16),
            0,
            None,
        )?;
        if o.status != 0 {
            return Err(format!("SYNCHRONIZE CACHE (10): {}", describe(&o)));
        }
    }
    read(s, w.start - u64::from(w.blocks), w.blocks, w.len, w.chunk)?;
    Ok(read(s, w.start, w.blocks, w.len, w.chunk)? == data)
}

fn show(r: &Result<bool>) -> String {
    match r {
        Ok(true) => "every byte as written".into(),
        Ok(false) => "other data".into(),
        Err(e) => e.clone(),
    }
}

pub(super) fn run(s: &mut Port, name: &str, disk: &Disk, cases: &mut Vec<Case>) {
    let w = match Scratch::save(s, disk) {
        Ok(w) => w,
        Err(e) => {
            record(
                cases,
                SECTION,
                format!("{name} 12 MiB runs: read the scratch window"),
                "GOOD",
                e,
                false,
            );
            return;
        }
    };
    let mib = (u64::from(w.blocks) * w.len as u64) >> 20;
    // Read, read elsewhere to push it out of any cache, read again (SBC-5 5.16, 5.18).
    for (ten, size, clause) in [(true, "(10)", "5.16"), (false, "(16)", "5.18")] {
        let mut pass = || -> Result<bool> {
            let first = read_with(s, ten, w.start, w.blocks, w.len, w.chunk)?;
            read_with(
                s,
                ten,
                w.start - u64::from(w.blocks),
                w.blocks,
                w.len,
                w.chunk,
            )?;
            Ok(read_with(s, ten, w.start, w.blocks, w.len, w.chunk)? == first)
        };
        let same = pass();
        record(
            cases,
            SECTION,
            format!(
                "{name} SBC-5 {clause} {mib} MiB READ {size}, {mib} MiB elsewhere, then the first again: the same"
            ),
            "the same data both times",
            match &same {
                Ok(true) => "the same".to_string(),
                Ok(false) => "different".to_string(),
                Err(e) => e.clone(),
            },
            matches!(same, Ok(true)),
        );
    }
    let current = caching_page(s, CURRENT).ok().flatten();
    let changeable = caching_page(s, CHANGEABLE)
        .ok()
        .flatten()
        .is_some_and(|m| m.get(2).is_some_and(|b| b & WCE != 0));
    let mut pass = 0x100;
    for on in [true, false] {
        let state = if on { "on (WCE 1)" } else { "off (WCE 0)" };
        let can = current.as_deref().is_some_and(|p| wce(p) == on) || changeable;
        if !can {
            record_info(
                cases,
                SECTION,
                format!("{name} SBC-5 6.5.6 {mib} MiB with the write cache {state}"),
                "not run: WCE is not changeable and has the other value",
            );
            continue;
        }
        let set = if current.as_deref().is_some_and(|p| wce(p) == on) {
            Ok(())
        } else {
            write_cache::set_wce(s, on).map(|_| ())
        };
        for (op, size, clause) in [(0x2au8, "(10)", "5.40"), (0x8a, "(16)", "5.42")] {
            pass += 1;
            let r = set
                .clone()
                .and_then(|()| round_trip(s, &w, op, 0, on, pass));
            record(
                cases,
                SECTION,
                format!(
                    "{name} SBC-5 {clause} {mib} MiB WRITE {size} with the write cache {state}{}, read back",
                    if on { ", flushed" } else { "" }
                ),
                "every byte as written",
                show(&r),
                matches!(r, Ok(true)),
            );
        }
    }
    if let Some(page) = &current {
        let _ = write_cache::restore(s, page);
    }
    // SBC-5 5.40: FUA writes reach the medium before GOOD.
    pass += 1;
    let r = round_trip(s, &w, 0x2a, FUA, false, pass);
    record(
        cases,
        SECTION,
        format!("{name} SBC-5 5.40 {mib} MiB WRITE (10) with FUA, read back"),
        "every byte as written",
        show(&r),
        matches!(r, Ok(true)),
    );
    if let Err(e) = w.restore(s) {
        record(
            cases,
            SECTION,
            format!("{name} scratch window written back"),
            "GOOD",
            e,
            false,
        );
    }
    if let Some(page) = &current {
        let now = caching_page(s, CURRENT).ok().flatten();
        if now.as_deref() != Some(&page[..]) {
            record(
                cases,
                SECTION,
                format!("{name} Caching page written back"),
                "as before",
                "different",
                false,
            );
        }
    }
}
