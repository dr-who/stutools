//! MODE SELECT of the mode pages whose changeable bits the Caching-page test does not reach: Control
//! (SPC-3 7.4.6), Informational Exceptions Control (SPC-3 7.4.11, SBC-3 6.5.6) and Read-Write Error
//! Recovery (SBC-3 6.5.8). For each page the changeable mask is read with PC 01b (SPC-3 6.8, 7.4). The
//! first changeable bit of the page is flipped, MODE SELECT (10) sends it (GOOD, SPC-3 6.8), MODE SENSE
//! reads it back, and the page is restored. The first bit that is not changeable is flipped and sent: it
//! is refused with INVALID FIELD IN PARAMETER LIST (5/26/00, SPC-3 6.8) and changes nothing. A page with
//! no changeable bit in its body gets no case, and the page's values are written back at the end.

use super::cases::{Want, expect, record};
use super::disk::{SECTION, label};
use super::mode_page_cases::{mode_page_of, mode_sense_10};
use super::port::Port;
use super::session::Outcome;
use crate::Result;
use crate::compliance::Case;

/// Page code, the clause of the page, and what it is called in the report.
const PAGES: [(u8, &str, &str); 4] = [
    (0x0a, "SPC-3 7.4.6", "Control"),
    (
        0x1c,
        "SPC-3 7.4.11 / SBC-3 6.5.6",
        "Informational Exceptions Control",
    ),
    (0x01, "SBC-3 6.5.8", "Read-Write Error Recovery"),
    (0x07, "SBC-3 6.5.9", "Verify Error Recovery"),
];

/// The first body byte of a mode page: bytes 0 and 1 are the page code and length.
const FIRST_BODY: usize = 2;

/// Flips one bit of a page body, found from the changeable mask.
fn flip(page: &[u8], byte: usize, bit: u8) -> Vec<u8> {
    let mut out = page.to_vec();
    out[byte] ^= 1 << bit;
    out
}

/// MODE SELECT (10) of one page: the header, then the page with PS cleared (SPC-3 7.5.3).
fn select(s: &mut Port, page: &[u8]) -> Result<Outcome> {
    let mut body = page.to_vec();
    body[0] &= 0x7f;
    let mut list = vec![0u8; 8];
    list.extend_from_slice(&body);
    let n = (list.len() as u16).to_be_bytes();
    s.command(&[0x55, 0x10, 0, 0, 0, 0, 0, n[0], n[1], 0], 0, Some(&list))
}

/// The current page, or None when the device does not have it.
fn current(s: &mut Port, page: u8) -> Option<Vec<u8>> {
    let o = mode_sense_10(s, 0, page, 0, true).ok().filter(|o| o.status == 0)?;
    mode_page_of(&o.data, true).map(|p| p.to_vec())
}

/// The changeable mask of a page: PC 01b, one bit per changeable bit of the page.
fn changeable(s: &mut Port, page: u8) -> Option<Vec<u8>> {
    let o = mode_sense_10(s, 1, page, 0, true).ok().filter(|o| o.status == 0)?;
    mode_page_of(&o.data, true).map(|p| p.to_vec())
}

/// The first (byte, bit) of the body that the mask says is changeable, or not changeable.
fn first_bit(mask: &[u8], changeable: bool) -> Option<(usize, u8)> {
    (FIRST_BODY..mask.len()).find_map(|byte| {
        (0..8u8).find_map(|bit| {
            let on = mask[byte] & (1 << bit) != 0;
            (on == changeable).then_some((byte, bit))
        })
    })
}

/// Byte 2 of the Read-Write Error Recovery page: the error recovery bits EER, PER, DTE and DCR (SBC-3 6.5.8).
const ERROR_BITS_BYTE: usize = 2;
const EER: u8 = 0x08;
const PER: u8 = 0x04;
const DTE: u8 = 0x02;
const DCR: u8 = 0x01;

/// The SBC-3 6.5.8 combination rules, each a combination of the error recovery bits the standard forbids:
/// "If the EER bit is set to one, the DCR bit shall be set to zero", and "If the DTE bit is set to one, then
/// the PER bit shall be set to one". Each is sent alone, so the refusal is for that combination.
const FORBIDDEN: [(&str, u8, &str); 2] = [
    ("EER 1 with DCR 1", EER | DCR, "EER 1 requires DCR 0"),
    ("DTE 1 with PER 0", DTE, "DTE 1 requires PER 1"),
];

/// Sends each forbidden error recovery combination of the page and expects it refused with INVALID FIELD IN
/// PARAMETER LIST (SPC-3 6.8). A device that accepts one is restored to the page it had.
fn combinations(s: &mut Port, name: &str, page_code: u8, original: &[u8], cases: &mut Vec<Case>) {
    let (clause, title) = if page_code == 0x01 {
        ("SBC-3 6.5.8", "Read-Write Error Recovery")
    } else {
        ("SBC-3 6.5.9", "Verify Error Recovery")
    };
    // A page too short to carry the error recovery byte has no combination to send; its length is judged elsewhere.
    if original.len() <= ERROR_BITS_BYTE {
        return;
    }
    for (what, bits, rule) in FORBIDDEN {
        let mut page = original.to_vec();
        // Keep the other error recovery bits as they are, and set the forbidden pair.
        page[ERROR_BITS_BYTE] = (original[ERROR_BITS_BYTE] & !(EER | PER | DTE | DCR)) | bits;
        let sent = select(s, &page);
        expect(
            cases,
            SECTION,
            label(
                name,
                &format!("{clause} {title} MODE SELECT (10) with {what} refused ({rule})"),
            ),
            &[Want::Ascq(0x5, 0x26, 0x00)],
            sent,
        );
        if current(s, page_code).is_some_and(|p| p != original) {
            let _ = select(s, original);
        }
    }
}

pub(super) fn run(s: &mut Port, name: &str, cases: &mut Vec<Case>) {
    for (page, clause, title) in PAGES {
        let (Some(original), Some(mask)) = (current(s, page), changeable(s, page)) else {
            continue;
        };
        let at = |what: &str| label(name, &format!("{clause} {title} {what}"));
        if let Some((byte, bit)) = first_bit(&mask, true) {
            let flipped = flip(&original, byte, bit);
            let sent = select(s, &flipped);
            expect(
                cases,
                SECTION,
                at(&format!(
                    "MODE SELECT (10) flips changeable byte {byte} bit {bit}"
                )),
                &[Want::Good],
                sent,
            );
            let read_back = current(s, page).is_some_and(|p| p.get(byte) == flipped.get(byte));
            record(
                cases,
                SECTION,
                at(&format!(
                    "MODE SENSE (10) reads byte {byte} bit {bit} back changed"
                )),
                "the changed bit",
                if read_back { "as sent" } else { "not as sent" },
                read_back,
            );
            let restored = select(s, &original);
            expect(
                cases,
                SECTION,
                at("MODE SELECT (10) restores the page"),
                &[Want::Good],
                restored,
            );
        }
        if let Some((byte, bit)) = first_bit(&mask, false) {
            let before = current(s, page);
            let sent = select(s, &flip(&original, byte, bit));
            expect(
                cases,
                SECTION,
                at(&format!(
                    "MODE SELECT (10) of non-changeable byte {byte} bit {bit} refused"
                )),
                &[Want::Ascq(0x5, 0x26, 0x00)],
                sent,
            );
            let unchanged = current(s, page) == before;
            record(
                cases,
                SECTION,
                at("the refused MODE SELECT changed nothing"),
                "page unchanged",
                if unchanged { "unchanged" } else { "changed" },
                unchanged,
            );
        }
        // SBC-3 table 199: the same error recovery combinations govern the Verify page (6.5.9).
        if page == 0x01 || page == 0x07 {
            combinations(s, name, page, &original, cases);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The first changeable bit is searched from the first body byte, so the page code and length bytes
    /// are never flipped.
    #[test]
    fn first_bit_skips_the_page_header_and_finds_each_kind() {
        let mask = [0xffu8, 0xff, 0x00, 0x04];
        assert_eq!(first_bit(&mask, true), Some((3, 2)));
        assert_eq!(first_bit(&mask, false), Some((2, 0)));
        assert_eq!(first_bit(&[0xff, 0xff, 0xff], false), None);
        assert_eq!(first_bit(&[0, 0, 0], true), None);
    }

    #[test]
    fn flip_changes_exactly_one_bit() {
        let page = [0x1c, 0x0a, 0x00, 0x10];
        let out = flip(&page, 2, 4);
        assert_eq!(out, vec![0x1c, 0x0a, 0x10, 0x10]);
        assert_eq!(flip(&out, 2, 4), page.to_vec());
    }
}
