//! The mode page cases of the `disk` section (SPC-6 6.11 to 6.14, 7.5; SBC-5 6.5): MODE SENSE (6) and
//! (10) of each page a disk driver reads (Caching, Control, Control Extension, Informational
//! Exceptions Control, Read-Write Error Recovery), current and changeable values, the two forms
//! agreeing, and a MODE SELECT of the Caching page that changes nothing. A page is found after the
//! mode parameter header and block descriptors (`mode_page_of`).

use super::cases::{expect, record, refused, Want};
use super::disk::{label, Disk, SECTION};
use super::port::Port;
use super::scsi_fields::be16;
use super::seen::Seen;
use super::session::Outcome;
use crate::compliance::Case;
use crate::Result;

/// The page itself from a MODE SENSE answer: after the header (4 bytes for (6), 8 for (10)) and the
/// block descriptors (SPC-6 7.5.6).
pub(super) fn mode_page_of(d: &[u8], ten: bool) -> Option<&[u8]> {
    let (header, bdl) = if ten {
        (8, be16(d, 6) as usize)
    } else {
        (4, d.get(3).copied().unwrap_or(0) as usize)
    };
    let start = header + bdl;
    let b0 = *d.get(start)?;
    let len = if b0 & 0x40 != 0 {
        4 + be16(d, start + 2) as usize
    } else {
        2 + *d.get(start + 1)? as usize
    };
    d.get(start..start + len)
}

pub(super) fn mode_sense_10(
    s: &mut Port,
    control: u8,
    page: u8,
    subpage: u8,
    dbd: bool,
) -> Result<Outcome> {
    let cdb = [
        0x5a,
        if dbd { 0x08 } else { 0 },
        (control << 6) | page,
        subpage,
        0,
        0,
        0,
        0x10,
        0x00,
        0,
    ];
    s.command(&cdb, 4096, None)
}

pub(super) fn mode_sense_6(s: &mut Port, page: u8) -> Result<Outcome> {
    s.command(&[0x1a, 0x08, page, 0, 255, 0], 255, None)
}

/// SPC-5 6.14, 6.15, 7.5; SBC-5 6.5: the mode pages, and MODE SELECT of the Caching page unchanged.
pub(super) fn mode_pages(
    s: &mut Port,
    name: &str,
    cases: &mut Vec<Case>,
    seen: &mut Seen,
    disk: &mut Disk,
) {
    // All pages with block descriptors (DBD clear): the header and every page fit the length.
    let all = mode_sense_10(s, 0, 0x3f, 0, false);
    let answer = expect(
        cases,
        SECTION,
        label(name, "MODE SENSE (10) all pages"),
        &[Want::Good],
        all,
    );
    seen.answer("MODE SENSE (10)", SECTION, &answer);
    if let Some(o) = answer.1.filter(|o| o.status == 0) {
        let len = be16(&o.data, 0) as usize + 2;
        let bdl = be16(&o.data, 6);
        disk.device_specific = o.data.get(3).copied().unwrap_or(0);
        disk.block_descriptor_len = Some(bdl);
        // SPC-6 7.5.6: MODE DATA LENGTH excludes itself; block descriptors are 8 bytes, or 16 with LONGLBA.
        record(
            cases,
            SECTION,
            label(name, "MODE SENSE (10) header and block descriptor"),
            "length within the data; descriptor 0, 8 or 16 bytes",
            format!("length {len}, block descriptors {bdl}"),
            len >= 8 && len <= o.data.len() && matches!(bdl, 0 | 8 | 16),
        );
        // SBC-5 table 231: WP clear means the LUN may be written; hosts mount it read-only otherwise.
        record(
            cases,
            SECTION,
            label(name, "LUN is writable (WP clear)"),
            "0",
            format!("{}", disk.device_specific >> 7),
            disk.device_specific & 0x80 == 0,
        );
    }
    for (page, subpage, what) in [
        (0x08u8, 0u8, "Caching"),
        (0x0a, 0, "Control"),
        (0x0a, 0x01, "Control Extension"),
        (0x1c, 0, "Informational Exceptions Control"),
        (0x01, 0, "Read-Write Error Recovery"),
    ] {
        let o = mode_sense_10(s, 0, page, subpage, true);
        // A page the device server does not have is an invalid field in the CDB (SPC-6 6.13).
        let wants: &[Want] = if page == 0x08 {
            &[Want::Good]
        } else {
            &[Want::Good, Want::Ascq(0x5, 0x24, 0x00)]
        };
        let (passed, o) = expect(
            cases,
            SECTION,
            label(name, &format!("MODE SENSE (10) {what} page")),
            wants,
            o,
        );
        let Some(o) = o.filter(|o| o.status == 0) else {
            continue;
        };
        let Some(p) = mode_page_of(&o.data, true) else {
            continue;
        };
        let code_ok = p[0] & 0x3f == page && (subpage == 0 || p.get(1) == Some(&subpage));
        // SBC-5 table 239: Caching 12h; SPC-6 table 461: Control 0Ah; table 458: Control Extension 1Ch;
        // SBC-5 tables 246 and 254: Informational Exceptions Control and Read-Write Error Recovery 0Ah.
        let want_len = match (page, subpage) {
            (0x08, _) => 0x12 + 2,
            (0x0a, 0x01) => 0x1c + 4,
            _ => 0x0a + 2,
        };
        if passed && !(code_ok && p.len() == want_len) {
            record(
                cases,
                SECTION,
                label(name, &format!("{what} page code and length")),
                format!("page {page:02x}h, {} bytes", want_len),
                format!("page {:02x}h, {} bytes", p[0] & 0x3f, p.len()),
                false,
            );
        }
        disk.mode.push(((page, subpage), p.to_vec()));
    }
    // SPC-6 7.5.3: PC 01b returns the changeable values.
    for (page, what) in [
        (0x08u8, "Caching"),
        (0x0a, "Control"),
        (0x1c, "Informational Exceptions Control"),
        (0x01, "Read-Write Error Recovery"),
    ] {
        if disk.mode_page(page, 0).is_none() {
            continue;
        }
        let o = mode_sense_10(s, 1, page, 0, true);
        let (_, o) = expect(
            cases,
            SECTION,
            label(name, &format!("MODE SENSE (10) {what} changeable values")),
            &[Want::Good],
            o,
        );
        if let Some(p) = o.and_then(|o| mode_page_of(&o.data, true).map(|p| p.to_vec())) {
            disk.changeable.push(((page, 0), p));
        }
    }

    // MODE SENSE (6): SBC-5 no longer lists it (SBC-5 table 34 did); Windows and macOS still send it and
    // fall back to MODE SENSE (10) when it is refused. GOOD must agree with (10).
    let six = mode_sense_6(s, 0x08);
    let (passed, o) = expect(
        cases,
        SECTION,
        label(name, "MODE SENSE (6) Caching page"),
        &[Want::Good, Want::Ascq(0x5, 0x20, 0x00)],
        six,
    );
    let mut six_ok = passed;
    let six_supported = o.as_ref().is_some_and(|o| !refused(o, false));
    if let Some(o) = o.filter(|o| o.status == 0) {
        let page = mode_page_of(&o.data, false).map(|p| p.to_vec());
        let same = page.as_deref() == disk.mode_page(0x08, 0)
            && o.data.get(2).copied() == Some(disk.device_specific);
        six_ok &= record(
            cases,
            SECTION,
            label(name, "MODE SENSE (6) agrees with MODE SENSE (10)"),
            "same Caching page and DEVICE-SPECIFIC PARAMETER",
            if same { "same" } else { "different" },
            same,
        );
    }
    seen.note("MODE SENSE (6)", SECTION, six_supported, six_ok, "see disk");

    // MODE SELECT (10) and (6) of the current Caching page, PS cleared (SPC-6 7.5.8: PS is reserved in
    // MODE SELECT). Sending the current values changes nothing.
    if let Some(page) = disk.mode_page(0x08, 0).map(|p| p.to_vec()) {
        let mut body = page.clone();
        body[0] &= 0x7f;
        let mut list10 = vec![0u8; 8];
        list10.extend_from_slice(&body);
        let n = (list10.len() as u16).to_be_bytes();
        let select10 = s.command(
            &[0x55, 0x10, 0, 0, 0, 0, 0, n[0], n[1], 0],
            0,
            Some(&list10),
        );
        let answer = expect(
            cases,
            SECTION,
            label(name, "MODE SELECT (10) of the current Caching page"),
            &[Want::Good],
            select10,
        );
        seen.answer("MODE SELECT (10)", SECTION, &answer);
        let mut list6 = vec![0u8; 4];
        list6.extend_from_slice(&body);
        let select6 = s.command(&[0x15, 0x10, 0, 0, list6.len() as u8, 0], 0, Some(&list6));
        let answer = expect(
            cases,
            SECTION,
            label(name, "MODE SELECT (6) of the current Caching page"),
            &[Want::Good, Want::Ascq(0x5, 0x20, 0x00)],
            select6,
        );
        seen.answer("MODE SELECT (6)", SECTION, &answer);
        let after = mode_sense_10(s, 0, 0x08, 0, true)
            .ok()
            .and_then(|o| mode_page_of(&o.data, true).map(|p| p.to_vec()));
        record(
            cases,
            SECTION,
            label(name, "Caching page unchanged by the MODE SELECTs"),
            "unchanged",
            if after.as_deref() == Some(&page[..]) {
                "unchanged"
            } else {
                "changed"
            },
            after.as_deref() == Some(&page[..]),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_pages_are_found_after_header_and_descriptors() {
        let mut ten = vec![0, 0, 0, 0x10, 0, 0, 0, 8];
        ten.extend_from_slice(&[0; 8]);
        ten.extend_from_slice(&[0x08, 0x12]);
        ten.extend_from_slice(&[0x04; 0x12]);
        let page = mode_page_of(&ten, true).unwrap();
        assert_eq!(page.len(), 0x14);
        assert_eq!(page[0], 0x08);
        let six = [0, 0, 0, 0, 0x4a, 0x01, 0x00, 0x1c, 1, 2];
        let sub = mode_page_of(&six, false);
        assert!(
            sub.is_none(),
            "a subpage longer than the data is not returned"
        );
    }
}
