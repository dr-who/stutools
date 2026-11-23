//! SPC-3 6.6 LOG SENSE. A log page the device does not list is refused with CHECK CONDITION, ILLEGAL REQUEST
//! with an invalid field in the CDB. The parameters within a log page are transferred in ascending order of
//! parameter code.

use super::cases::{Want, expect, record};
use super::disk::Disk;
use super::port::Port;
use super::session::Outcome;
use crate::Result;
use crate::compliance::Case;

const SECTION: &str = "disk";

/// LOG SENSE of one page, cumulative values (PC 01b), 512 bytes allocated.
fn log_sense(s: &mut Port, page: u8) -> Result<Outcome> {
    s.command(
        &[0x4d, 0, 0x40 | page, 0, 0, 0, 0, 0x02, 0x00, 0],
        512,
        None,
    )
}

/// The parameter codes of a log page, in the order they are returned.
fn parameter_codes(data: &[u8]) -> Vec<u16> {
    let total = data
        .get(2..4)
        .map_or(0, |b| usize::from(u16::from_be_bytes([b[0], b[1]])));
    let end = (4 + total).min(data.len());
    let mut codes = Vec::new();
    let mut at = 4;
    while at + 4 <= end {
        codes.push(u16::from_be_bytes([data[at], data[at + 1]]));
        at += 4 + usize::from(data[at + 3]);
    }
    codes
}

pub(super) fn run(s: &mut Port, name: &str, disk: &Disk, cases: &mut Vec<Case>) {
    let Some(pages) = disk.log_pages.as_ref() else {
        return;
    };
    // SPC-3 6.6: a page not listed by page 00h is refused, so the first unlisted page code is asked for.
    if let Some(unlisted) = (1u8..=0x3e).find(|p| !pages.contains(p)) {
        expect(
            cases,
            SECTION,
            format!(
                "{name} SPC-3 6.6 LOG SENSE of unlisted page {unlisted:02x}h: CHECK CONDITION, ILLEGAL REQUEST"
            ),
            &[Want::Asc(0x05, 0x24)],
            log_sense(s, unlisted),
        );
    }
    // SPC-3 6.6: the parameters of each page are transferred in ascending order of parameter code.
    let mut unordered = Vec::new();
    for &page in pages.iter().filter(|p| **p != 0) {
        let Ok(o) = log_sense(s, page) else {
            continue;
        };
        if o.status == 0 && o.data.len() >= 4 {
            let codes = parameter_codes(&o.data);
            if !codes.windows(2).all(|w| w[0] < w[1]) {
                unordered.push(format!("{page:02x}h"));
            }
        }
    }
    record(
        cases,
        SECTION,
        format!("{name} SPC-3 6.6 LOG SENSE parameters in ascending order of parameter code"),
        "ascending in every listed page",
        if unordered.is_empty() {
            "ascending in every listed page".to_string()
        } else {
            format!("out of order in {}", unordered.join(", "))
        },
        unordered.is_empty(),
    );
}
