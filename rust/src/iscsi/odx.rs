//! The `odx` cases of `--compliance`: token-based copy (SBC-5 4.28, 5.12, 5.59; SPC-6 6.25), which
//! Windows uses as ODX (offloaded data transfer) when the Third-party Copy VPD page (8Fh) lists POPULATE
//! TOKEN and WRITE USING TOKEN. When it does, one block is copied from the second-to-last block to the
//! last one by token and read back; both blocks get their original data back. When it does not, POPULATE
//! TOKEN must be refused.

use super::cases::{expect, record, Want, REFUSED_SA};
use super::disk::Disk;
use super::lun_access::{pattern, read_block, write_block};
use super::scsi_fields::{be16, be32};
use super::seen::Seen;
use super::session::{Outcome, Session};
use crate::compliance::Case;
use crate::Result;

const SECTION: &str = "odx";
const LIST_POPULATE: u32 = 0x5350_0001;
const LIST_WRITE: u32 = 0x5350_0002;

/// The commands the Supported Commands third-party copy descriptor (0001h) lists: an operation code
/// and its service actions (SPC-6 7.7.18.4).
pub(super) fn supported_commands(page: &[u8]) -> Vec<(u8, Vec<u8>)> {
    let end = (4 + be16(page, 2) as usize).min(page.len());
    let mut i = 4;
    let mut out = Vec::new();
    while i + 4 <= end {
        let kind = be16(page, i);
        let len = be16(page, i + 2) as usize;
        if kind == 0x0001 {
            let list_len = page.get(i + 4).copied().unwrap_or(0) as usize;
            let mut j = i + 5;
            while j + 2 <= (i + 5 + list_len).min(end) {
                let op = page[j];
                let n = page[j + 1] as usize;
                out.push((op, page.get(j + 2..j + 2 + n).unwrap_or(&[]).to_vec()));
                j += 2 + n;
            }
        }
        i += 4 + len;
    }
    out
}

/// Whether the third-party copy page advertises ODX: POPULATE TOKEN (83h/10h), WRITE USING TOKEN
/// (83h/11h) and RECEIVE ROD TOKEN INFORMATION (84h/07h).
pub(super) fn advertises_tokens(page: &[u8]) -> bool {
    let list = supported_commands(page);
    let has = |op: u8, sa: u8| list.iter().any(|(o, s)| *o == op && s.contains(&sa));
    has(0x83, 0x10) && has(0x83, 0x11) && has(0x84, 0x07)
}

/// The POPULATE TOKEN parameter list for one block range (SBC-5 table 74): the data length, no RTV (the
/// default ROD type), then one 16-byte block device range descriptor.
fn populate_list(lba: u64, blocks: u32) -> Vec<u8> {
    let mut d = vec![0u8; 32];
    d[0..2].copy_from_slice(&30u16.to_be_bytes());
    d[14..16].copy_from_slice(&16u16.to_be_bytes());
    d[16..24].copy_from_slice(&lba.to_be_bytes());
    d[24..28].copy_from_slice(&blocks.to_be_bytes());
    d
}

/// The WRITE USING TOKEN parameter list (SBC-5 table 161): the token at 16, one range descriptor at 536.
fn write_list(token: &[u8], lba: u64, blocks: u32) -> Vec<u8> {
    let mut d = vec![0u8; 552];
    d[0..2].copy_from_slice(&550u16.to_be_bytes());
    d[16..16 + token.len().min(512)].copy_from_slice(&token[..token.len().min(512)]);
    d[534..536].copy_from_slice(&16u16.to_be_bytes());
    d[536..544].copy_from_slice(&lba.to_be_bytes());
    d[544..548].copy_from_slice(&blocks.to_be_bytes());
    d
}

fn token_cdb(sa: u8, list: u32, len: u32) -> [u8; 16] {
    let mut c = [0u8; 16];
    c[0] = 0x83;
    c[1] = sa;
    c[6..10].copy_from_slice(&list.to_be_bytes());
    c[10..14].copy_from_slice(&len.to_be_bytes());
    c
}

fn receive_rod_token_information(s: &mut Session, list: u32) -> Result<Outcome> {
    let mut c = [0u8; 16];
    c[0] = 0x84;
    c[1] = 0x07;
    c[2..6].copy_from_slice(&list.to_be_bytes());
    c[10..14].copy_from_slice(&1024u32.to_be_bytes());
    s.command(&c, 1024, None)
}

/// The copy operation status and the first ROD token of RECEIVE ROD TOKEN INFORMATION data (SPC-5
/// tables 236, 237): the sense data field's length is at byte 13; the descriptors length follows it.
pub(super) fn parse_token(d: &[u8]) -> (u8, Option<Vec<u8>>) {
    let status = d.get(5).copied().unwrap_or(0) & 0x7f;
    let x = 31 + d.get(13).copied().unwrap_or(0) as usize;
    let descriptors = be32(d, x + 1) as usize;
    let token = (descriptors >= 2 + 512)
        .then(|| d.get(x + 7..x + 7 + 512).map(|t| t.to_vec()))
        .flatten();
    (status, token)
}

/// Returns whether ODX is advertised and works.
pub(super) fn run(
    s: &mut Session,
    name: &str,
    disk: &Disk,
    cases: &mut Vec<Case>,
    seen: &mut Seen,
) -> bool {
    let label = |what: &str| format!("{name} {what}");
    let page = disk.page(0x8f);
    let tokens = page.is_some_and(advertises_tokens);
    super::cases::record_info(
        cases,
        SECTION,
        label("VPD 8Fh token copy"),
        match page {
            None => "no VPD 8Fh".to_string(),
            Some(p) => format!(
                "{:?}",
                supported_commands(p)
                    .iter()
                    .map(|(o, s)| format!("{o:02x}h/{s:02x?}"))
                    .collect::<Vec<_>>()
            ),
        },
    );
    let len = disk.block_len as usize;
    let last = disk.last_lba;
    let Ok(source) = u32::try_from(last.saturating_sub(1)) else {
        return false;
    };
    let dest = source + 1;
    if !tokens {
        // SPC-6 4.2.5.2: a service action the device server does not implement is refused.
        let populate = s.command(
            &token_cdb(0x10, LIST_POPULATE, 32),
            0,
            Some(&populate_list(source as u64, 1)),
        );
        let a = expect(
            cases,
            SECTION,
            label("POPULATE TOKEN refused when not advertised"),
            &REFUSED_SA,
            populate,
        );
        seen.answer("POPULATE TOKEN", SECTION, &a);
        // A zero token is no token the device server issued; refused like POPULATE TOKEN.
        let write = s.command(
            &token_cdb(0x11, LIST_WRITE, 552),
            0,
            Some(&write_list(&[0; 512], dest as u64, 1)),
        );
        let a = expect(
            cases,
            SECTION,
            label("WRITE USING TOKEN refused when not advertised"),
            &REFUSED_SA,
            write,
        );
        seen.answer("WRITE USING TOKEN", SECTION, &a);
        let info = receive_rod_token_information(s, LIST_POPULATE);
        let a = expect(
            cases,
            SECTION,
            label("RECEIVE ROD TOKEN INFORMATION refused when not advertised"),
            &REFUSED_SA,
            info,
        );
        seen.answer("RECEIVE ROD TOKEN INFORMATION", SECTION, &a);
        return false;
    }
    let (Ok(src_data), Ok(dst_data)) = (read_block(s, source, len), read_block(s, dest, len))
    else {
        record(
            cases,
            SECTION,
            label("read the test blocks"),
            "GOOD",
            "failed",
            false,
        );
        return false;
    };
    let p = pattern(len, 0x99);
    let mut ok = write_block(s, source, &p).is_ok();
    let populate = s.command(
        &token_cdb(0x10, LIST_POPULATE, 32),
        0,
        Some(&populate_list(source as u64, 1)),
    );
    let a = expect(
        cases,
        SECTION,
        label("POPULATE TOKEN of one block"),
        &[Want::Good],
        populate,
    );
    seen.answer("POPULATE TOKEN", SECTION, &a);
    ok &= a.0;
    let info = receive_rod_token_information(s, LIST_POPULATE);
    let a = expect(
        cases,
        SECTION,
        label("RECEIVE ROD TOKEN INFORMATION"),
        &[Want::Good],
        info,
    );
    seen.answer("RECEIVE ROD TOKEN INFORMATION", SECTION, &a);
    let (status, token) =
        a.1.filter(|o| o.status == 0)
            .map(|o| parse_token(&o.data))
            .unwrap_or((0, None));
    // SPC-6 6.23: COPY OPERATION STATUS 01h is completed without errors.
    ok &= record(
        cases,
        SECTION,
        label("token returned"),
        "status 01h, a 512-byte token",
        format!(
            "status {status:02x}h, token {}",
            if token.is_some() { "present" } else { "absent" }
        ),
        status == 1 && token.is_some(),
    );
    if let Some(token) = token {
        let write = s.command(
            &token_cdb(0x11, LIST_WRITE, 552),
            0,
            Some(&write_list(&token, dest as u64, 1)),
        );
        let a = expect(
            cases,
            SECTION,
            label("WRITE USING TOKEN of one block"),
            &[Want::Good],
            write,
        );
        seen.answer("WRITE USING TOKEN", SECTION, &a);
        ok &= a.0;
        let back = read_block(s, dest, len);
        ok &= record(
            cases,
            SECTION,
            label("copied block reads back"),
            "the source data",
            if matches!(&back, Ok(d) if *d == p) {
                "the source data"
            } else {
                "other"
            },
            matches!(&back, Ok(d) if *d == p),
        );
    }
    if write_block(s, source, &src_data).is_err() || write_block(s, dest, &dst_data).is_err() {
        record(
            cases,
            SECTION,
            label("restore the test blocks"),
            "written back",
            "failed",
            false,
        );
        ok = false;
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_commands_descriptor_lists_operation_codes_and_service_actions() {
        // 83h: 00h 10h 11h; 84h: 03h 07h.
        let body = [7 + 2, 0x83, 3, 0x00, 0x10, 0x11, 0x84, 2, 0x03, 0x07, 0, 0];
        let mut page = vec![0, 0x8f, 0, 0];
        page.extend_from_slice(&[0x00, 0x01, 0, body.len() as u8]);
        page.extend_from_slice(&body);
        let n = (page.len() - 4) as u16;
        page[2..4].copy_from_slice(&n.to_be_bytes());
        let list = supported_commands(&page);
        assert_eq!(list[0], (0x83, vec![0x00, 0x10, 0x11]));
        assert_eq!(list[1], (0x84, vec![0x03, 0x07]));
        assert!(advertises_tokens(&page));
    }

    #[test]
    fn parameter_lists_have_their_fixed_lengths() {
        let p = populate_list(7, 1);
        assert_eq!(be16(&p, 0), 30);
        assert_eq!(be16(&p, 14), 16);
        let w = write_list(&[0xab; 512], 9, 1);
        assert_eq!(w.len(), 552);
        assert_eq!(be16(&w, 0), 550);
        assert_eq!(w[16], 0xab);
        assert_eq!(be16(&w, 534), 16);
    }

    #[test]
    fn token_is_read_after_the_sense_data() {
        let mut d = vec![0u8; 32];
        d[5] = 0x01;
        d[13] = 0;
        d.extend_from_slice(&514u32.to_be_bytes());
        d.extend_from_slice(&[0, 0]);
        d.extend_from_slice(&[0x5a; 512]);
        let (status, token) = parse_token(&d);
        assert_eq!(status, 1);
        assert_eq!(token.unwrap()[0], 0x5a);
    }
}
