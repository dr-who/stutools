//! The VPD framing cases of the `disk` section that the Microsoft SCSI compliance tests check (SPC-6
//! 7.7.2, 7.7.6, 4.2.5.6): every page the Supported VPD Pages page lists, whether spit knows it or not,
//! read at allocation 255 (GOOD, its page code, the peripheral qualifier and device type of standard
//! INQUIRY, and PAGE LENGTH + 4 bytes or the allocation length returned); the Device Identification
//! page at allocation 4 (its header only, PAGE LENGTH unchanged); and each designation descriptor of
//! that page held to the rules of its designator type: the code set, the length, the association
//! and the PIV bit (`designator_faults`).

use super::cases::{describe, record};
use super::disk::{label, Disk, SECTION};
use super::lun_access::vpd;
use super::port::Port;
use super::scsi_fields::be16;
use crate::compliance::Case;

/// What breaks the rules of SPC-6 7.7.6.3 to 7.7.6.13 in one designation descriptor (table 525) that
/// starts at `d[0]`: none when it keeps them all.
pub(super) fn designator_faults(d: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    if d.len() < 4 {
        return vec!["shorter than its 4-byte header".into()];
    }
    let code_set = d[0] & 0x0f;
    let piv = d[1] & 0x80 != 0;
    let association = (d[1] >> 4) & 0x3;
    let kind = d[1] & 0x0f;
    let len = d[3] as usize;
    let value = d.get(4..4 + len).unwrap_or(&[]);
    let mut need = |cond: bool, what: &str| {
        if !cond {
            out.push(format!("type {kind:x}h: {what}"));
        }
    };
    need(association != 3, "association 11b is reserved");
    match kind {
        2 => {
            // 7.7.6.5: EUI-64 based, binary, 8, 12 or 16 bytes.
            need(code_set == 1, "code set 1h (binary)");
            need(matches!(len, 8 | 12 | 16), "length 8, 12 or 16");
        }
        3 => {
            // 7.7.6.6: NAA, binary; 8 bytes for NAA 2h, 3h and 5h, 16 for 6h.
            need(code_set == 1, "code set 1h (binary)");
            match value.first().map(|b| b >> 4) {
                Some(2 | 3 | 5) => need(len == 8, "length 8 for NAA 2h, 3h or 5h"),
                Some(6) => need(len == 16, "length 16 for NAA 6h"),
                _ => need(false, "an NAA field of 2h, 3h, 5h or 6h"),
            }
        }
        4 | 5 => {
            // 7.7.6.7, 7.7.6.8: relative target port and target port group, of a target port.
            need(association == 1, "association 01b (target port)");
            need(code_set == 1, "code set 1h (binary)");
            need(len == 4, "length 4");
        }
        6 => {
            // 7.7.6.9: logical unit group, of the logical unit.
            need(association == 0, "association 00b (logical unit)");
            need(code_set == 1, "code set 1h (binary)");
            need(len == 4, "length 4");
        }
        7 => {
            // 7.7.6.10: MD5 logical unit identifier, of the logical unit.
            need(association == 0, "association 00b (logical unit)");
            need(code_set == 1, "code set 1h (binary)");
        }
        8 => {
            // 7.7.6.11: SCSI name string, UTF-8, null-terminated, a multiple of 4 below 256.
            need(code_set == 3, "code set 3h (UTF-8)");
            need(len % 4 == 0 && len < 256, "length a multiple of 4");
            need(value.contains(&0), "null-terminated");
            let starts = [b"eui.", b"naa.", b"iqn."]
                .iter()
                .any(|p| value.len() >= 4 && value[..4].eq_ignore_ascii_case(&p[..]));
            need(starts, "starting eui., naa. or iqn.");
        }
        9 => {
            // 7.7.6.12: protocol specific port identifier, of a target port, with PIV.
            need(association == 1, "association 01b (target port)");
            need(piv, "PIV 1");
        }
        0xa => {
            // 7.7.6.13: UUID, binary, 18 bytes for a locally assigned RFC 4122 UUID.
            need(code_set == 1, "code set 1h (binary)");
            if value.first().map(|b| b >> 4) == Some(1) {
                need(len == 0x12, "length 12h");
            }
        }
        _ => {}
    }
    out
}

/// Each designation descriptor of a Device Identification page and what is wrong with it.
pub(super) fn page_faults(page: &[u8]) -> Vec<String> {
    let end = (4 + be16(page, 2) as usize).min(page.len());
    let mut out = Vec::new();
    let mut i = 4;
    while i + 4 <= end {
        let len = page[i + 3] as usize;
        out.extend(designator_faults(&page[i..(i + 4 + len).min(end)]));
        i += 4 + len;
    }
    out
}

pub(super) fn run(s: &mut Port, name: &str, disk: &Disk, cases: &mut Vec<Case>) {
    let at = |clause: &str, what: &str| label(name, &format!("{clause} {what}"));
    let qualifier = disk.inquiry.first().copied().unwrap_or(0);
    for &page in &disk.pages {
        let o = vpd(s, page, 255);
        let what = at(
            "SPC-6 7.7.2",
            &format!("VPD {page:02x}h at allocation 255: framing"),
        );
        let (actual, passed) = match &o {
            Ok(o) if o.status == 0 => {
                let d = &o.data[..o.received.min(o.data.len())];
                let length = be16(d, 2) as usize;
                let want = (length + 4).min(255);
                let ok =
                    d.len() == want && d.get(1) == Some(&page) && d.first() == Some(&qualifier);
                (
                    format!(
                        "byte 0 {:02x}h, page {:02x}h, PAGE LENGTH {length}, {} bytes",
                        d.first().copied().unwrap_or(0),
                        d.get(1).copied().unwrap_or(0),
                        d.len()
                    ),
                    ok,
                )
            }
            Ok(o) => (describe(o), false),
            Err(e) => (e.clone(), false),
        };
        record(
            cases,
            SECTION,
            what,
            format!(
                "GOOD; byte 0 {qualifier:02x}h as standard INQUIRY, page {page:02x}h, PAGE LENGTH + 4 bytes (at most 255)"
            ),
            actual,
            passed,
        );
    }
    if !disk.pages.contains(&0x83) {
        return;
    }
    // SPC-6 4.2.5.6, 7.7.6.1: four bytes asked for, four returned; PAGE LENGTH is not cut.
    let full = disk.page(0x83).map(|d| be16(d, 2));
    let o = vpd(s, 0x83, 4);
    let (actual, passed) = match &o {
        Ok(o) if o.status == 0 => {
            let d = &o.data[..o.received.min(o.data.len())];
            (
                format!("{} bytes, PAGE LENGTH {}", d.len(), be16(d, 2)),
                d.len() == 4 && Some(be16(d, 2)) == full,
            )
        }
        Ok(o) => (describe(o), false),
        Err(e) => (e.clone(), false),
    };
    record(
        cases,
        SECTION,
        at(
            "SPC-6 4.2.5.6",
            "VPD 83h at allocation 4: the header, PAGE LENGTH unchanged",
        ),
        format!("4 bytes, PAGE LENGTH {}", full.unwrap_or(0)),
        actual,
        passed,
    );
    if let Some(d) = disk.page(0x83) {
        let faults = page_faults(d);
        record(
            cases,
            SECTION,
            at(
                "SPC-6 7.7.6",
                "VPD 83h each designator's code set, length and association",
            ),
            "as its designator type sets",
            if faults.is_empty() {
                "all as set".to_string()
            } else {
                faults.join("; ")
            },
            faults.is_empty(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn designators_keep_the_rules_of_their_type() {
        let mut naa6 = vec![0x01, 0x03, 0, 16, 0x60];
        naa6.extend_from_slice(&[0; 15]);
        assert!(designator_faults(&naa6).is_empty());
        let naa6_ascii = {
            let mut d = naa6.clone();
            d[0] = 0x02;
            d
        };
        assert_eq!(designator_faults(&naa6_ascii).len(), 1);
        let short_naa6 = [0x01, 0x03, 0, 8, 0x60, 0, 0, 0, 0, 0, 0, 0];
        assert!(!designator_faults(&short_naa6).is_empty());
        let port = [0x01, 0x14, 0, 4, 0, 0, 0, 1];
        assert!(designator_faults(&port).is_empty());
        let port_of_lun = [0x01, 0x04, 0, 4, 0, 0, 0, 1];
        assert!(!designator_faults(&port_of_lun).is_empty());
        let mut name = vec![0x03, 0x08, 0, 8];
        name.extend_from_slice(b"iqn.a\0\0\0");
        assert!(designator_faults(&name).is_empty());
        let mut unterminated = vec![0x03, 0x08, 0, 4];
        unterminated.extend_from_slice(b"iqn.");
        assert_eq!(
            designator_faults(&unterminated),
            vec!["type 8h: null-terminated"]
        );
        let eui = [0x01, 0x02, 0, 10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(designator_faults(&eui), vec!["type 2h: length 8, 12 or 16"]);
    }

    #[test]
    fn a_page_is_judged_descriptor_by_descriptor() {
        let mut page = vec![0, 0x83, 0, 16];
        page.extend_from_slice(&[0x01, 0x14, 0, 4, 0, 0, 0, 1]);
        page.extend_from_slice(&[0x01, 0x15, 0, 4, 0, 0, 0, 2]);
        assert!(page_faults(&page).is_empty());
        page[5] = 0x05;
        assert_eq!(page_faults(&page).len(), 1);
    }
}
