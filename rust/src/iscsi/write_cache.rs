//! The write cache of the logical unit under test, as the cases that change it reach it: the Caching
//! mode page (SBC-5 6.5.6) read with MODE SENSE (10) for one page control (current, changeable,
//! default or saved values, SPC-6 6.13), written with MODE SELECT (10) or (6) (SPC-6 6.12, SPC-5
//! 6.12) as a mode parameter list of one page, and its WCE bit (byte 2 bit 2) set or cleared. A page
//! sent back has PS cleared, since PS is reserved in MODE SELECT (SPC-6 7.5.8). Every function sends
//! one or two commands and fails only when the transport does or the device answers other than GOOD.

use super::cases::describe;
use super::mode_page_cases::{mode_page_of, mode_sense_10};
use super::port::Port;
use super::session::Outcome;
use crate::Result;

/// WCE in byte 2 of the Caching mode page (SBC-5 table 239).
pub(super) const WCE: u8 = 0x04;

/// The page control field of MODE SENSE (SPC-6 table 169).
pub(super) const CURRENT: u8 = 0;
pub(super) const CHANGEABLE: u8 = 1;
pub(super) const SAVED: u8 = 3;

/// The Caching mode page with page control `pc`: Ok(None) when the device answers other than GOOD.
pub(super) fn caching_page(s: &mut Port, pc: u8) -> Result<Option<Vec<u8>>> {
    let o = mode_sense_10(s, pc, 0x08, 0, true)?;
    Ok((o.status == 0)
        .then(|| mode_page_of(&o.data, true).map(<[u8]>::to_vec))
        .flatten())
}

/// The mode parameter list of one page for MODE SELECT (10) (8-byte header) or (6) (4-byte header):
/// a header of zeros, no block descriptors, and the page with PS cleared.
pub(super) fn parameter_list(page: &[u8], ten: bool) -> Vec<u8> {
    let mut list = vec![0u8; if ten { 8 } else { 4 }];
    list.extend_from_slice(page);
    if let Some(b) = list.get_mut(if ten { 8 } else { 4 }) {
        *b &= 0x7f;
    }
    list
}

/// MODE SELECT (10) or (6) of `page` with PF set, and SP when `save`.
pub(super) fn select(s: &mut Port, page: &[u8], ten: bool, save: bool) -> Result<Outcome> {
    let list = parameter_list(page, ten);
    let flags = 0x10 | u8::from(save);
    if ten {
        let n = (list.len() as u16).to_be_bytes();
        s.command(&[0x55, flags, 0, 0, 0, 0, 0, n[0], n[1], 0], 0, Some(&list))
    } else {
        s.command(&[0x15, flags, 0, 0, list.len() as u8, 0], 0, Some(&list))
    }
}

/// `page` with WCE set to `on`.
pub(super) fn with_wce(page: &[u8], on: bool) -> Vec<u8> {
    let mut p = page.to_vec();
    if let Some(b) = p.get_mut(2) {
        if on {
            *b |= WCE;
        } else {
            *b &= !WCE;
        }
    }
    p
}

/// Whether the page has WCE set.
pub(super) fn wce(page: &[u8]) -> bool {
    page.get(2).is_some_and(|b| b & WCE != 0)
}

/// Sets WCE to `on` with MODE SELECT (10), not saved, and returns the current page from before, for
/// `restore`. Fails when the page cannot be read or the MODE SELECT is not GOOD.
pub(super) fn set_wce(s: &mut Port, on: bool) -> Result<Vec<u8>> {
    let before = caching_page(s, CURRENT)?.ok_or("no Caching mode page")?;
    if wce(&before) != on {
        let o = select(s, &with_wce(&before, on), true, false)?;
        if o.status != 0 {
            return Err(format!("MODE SELECT (10) answered {}", describe(&o)));
        }
    }
    Ok(before)
}

/// Writes `page` back as the current values with MODE SELECT (10).
pub(super) fn restore(s: &mut Port, page: &[u8]) -> Result<()> {
    let o = select(s, page, true, false)?;
    if o.status == 0 {
        Ok(())
    } else {
        Err(format!("MODE SELECT (10) answered {}", describe(&o)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_parameter_list_is_a_zero_header_and_the_page_without_ps() {
        let page = [0x88, 0x12, 0x04, 0, 0xff];
        assert_eq!(
            parameter_list(&page, true),
            vec![0, 0, 0, 0, 0, 0, 0, 0, 0x08, 0x12, 0x04, 0, 0xff]
        );
        assert_eq!(parameter_list(&page, false)[..5], [0, 0, 0, 0, 0x08]);
    }

    #[test]
    fn wce_is_byte_2_bit_2() {
        let page = [0x08, 0x12, 0x01];
        assert!(!wce(&page));
        let on = with_wce(&page, true);
        assert_eq!(on[2], 0x05);
        assert!(wce(&on));
        assert_eq!(with_wce(&on, false)[2], 0x01);
    }
}
