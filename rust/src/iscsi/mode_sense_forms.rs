//! The MODE SENSE cases of the `disk` section that Windows' scan and the Microsoft SCSI compliance
//! tests make (SPC-6 6.13, 7.5.6, 7.5.8; SPC-5 6.14 for MODE SENSE (6), which SPC-6 made obsolete;
//! SBC-5 6.5.1): every page the device has (page code 3Fh) through MODE SENSE (6) and (10), with
//! block descriptors and without (DBD), for each page control (current, changeable, default and
//! saved values), and with LLBAA; the header arithmetic (MODE DATA LENGTH against the bytes returned,
//! BLOCK DESCRIPTOR LENGTH 0 or 8, or 16 with LONGLBA, MEDIUM TYPE 00h), the pages in ascending order
//! filling the length, each page read on its own equal to its place in the whole, and a short
//! allocation length returning the start of the answer. A MODE SENSE (6) refused as an unsupported
//! command skips its form. What it finds goes into `Disk` (the page list and whether saved values
//! answer) for the Windows cases.

use super::cases::{describe, expect, record, record_should, residual_not_reported, Want};
use super::disk::{hex_list, label, Disk, SECTION};
use super::port::Port;
use super::scsi_fields::be16;
use super::session::Outcome;
use crate::compliance::Case;
use crate::Result;

/// The allocation length asked for: room for every page of a disk.
const ALLOC_10: u16 = 4096;
const ALLOC_6: u8 = 255;

/// One mode page of an answer: its code, subpage (0 for the page_0 format), and the page itself.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Page {
    pub code: u8,
    pub subpage: u8,
    pub spf: bool,
    pub ps: bool,
    pub bytes: Vec<u8>,
}

/// A MODE SENSE answer taken apart (SPC-6 7.5.6; the 4-byte header of MODE SENSE (6) in SPC-5 7.5.6).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Parsed {
    /// MODE DATA LENGTH plus the bytes of the field itself: the whole answer the device has.
    pub total: usize,
    pub medium_type: u8,
    pub device_specific: u8,
    pub longlba: bool,
    pub bdl: usize,
    pub pages: Vec<Page>,
    /// The pages end exactly at `total` (when the data reaches that far).
    pub fills: bool,
}

/// Takes apart the `d` a MODE SENSE (10) (`ten`) or (6) returned, as far as it reaches.
pub(super) fn parse(d: &[u8], ten: bool) -> Option<Parsed> {
    let (header, total, medium_type, device_specific, longlba, bdl) = if ten {
        if d.len() < 8 {
            return None;
        }
        (
            8,
            be16(d, 0) as usize + 2,
            d[2],
            d[3],
            d[4] & 1 != 0,
            be16(d, 6) as usize,
        )
    } else {
        if d.len() < 4 {
            return None;
        }
        (4, d[0] as usize + 1, d[1], d[2], false, d[3] as usize)
    };
    let end = total.min(d.len());
    let mut pages = Vec::new();
    let mut i = header + bdl;
    let mut fills = i <= total;
    while i + 2 <= end {
        let b0 = d[i];
        let spf = b0 & 0x40 != 0;
        let len = if spf {
            if i + 4 > end {
                fills = false;
                break;
            }
            4 + be16(d, i + 2) as usize
        } else {
            2 + d[i + 1] as usize
        };
        if i + len > end {
            fills = false;
            break;
        }
        pages.push(Page {
            code: b0 & 0x3f,
            subpage: if spf { d[i + 1] } else { 0 },
            spf,
            ps: b0 & 0x80 != 0,
            bytes: d[i..i + len].to_vec(),
        });
        i += len;
    }
    if total <= d.len() {
        fills &= i == total;
    }
    Some(Parsed {
        total,
        medium_type,
        device_specific,
        longlba,
        bdl,
        pages,
        fills,
    })
}

/// Whether the page codes are in the order SPC-6 6.13.1 sets for page code 3Fh: ascending from 01h,
/// each once, with the vendor specific page 00h, if any, last.
pub(super) fn in_order(codes: &[u8]) -> bool {
    let key = |c: u8| if c == 0 { 0x40 } else { c };
    codes.windows(2).all(|w| key(w[0]) < key(w[1]))
}

/// One MODE SENSE form: (6) or (10).
#[derive(Clone, Copy)]
struct Form {
    ten: bool,
}

impl Form {
    fn name(self) -> &'static str {
        if self.ten {
            "MODE SENSE (10)"
        } else {
            "MODE SENSE (6)"
        }
    }

    /// The clause of the command: SPC-6 6.13, or SPC-5 6.14 for the form SPC-6 made obsolete.
    fn clause(self) -> &'static str {
        if self.ten {
            "SPC-6 6.13"
        } else {
            "SPC-5 6.14"
        }
    }

    /// The clause of the header: SPC-6 7.5.6, or SPC-5 7.5.6 for the 4-byte header of (6).
    fn header_clause(self) -> &'static str {
        if self.ten {
            "SPC-6 7.5.6"
        } else {
            "SPC-5 7.5.6"
        }
    }

    fn header_len(self) -> usize {
        if self.ten {
            8
        } else {
            4
        }
    }

    fn alloc(self) -> u32 {
        if self.ten {
            u32::from(ALLOC_10)
        } else {
            u32::from(ALLOC_6)
        }
    }

    fn send(
        self,
        s: &mut Port,
        dbd: bool,
        llbaa: bool,
        pc: u8,
        page: u8,
        alloc: u32,
    ) -> Result<Outcome> {
        let b1 = if dbd { 0x08 } else { 0 } | if llbaa { 0x10 } else { 0 };
        let b2 = (pc << 6) | page;
        if self.ten {
            let a = (alloc as u16).to_be_bytes();
            s.command(&[0x5a, b1, b2, 0, 0, 0, 0, a[0], a[1], 0], alloc, None)
        } else {
            s.command(&[0x1a, b1, b2, 0, alloc as u8, 0], alloc, None)
        }
    }
}

/// The data an answer returned: as many bytes as the device sent.
fn returned(o: &Outcome) -> &[u8] {
    &o.data[..o.received.min(o.data.len())]
}

const CONTROLS: [(u8, &str); 3] = [(1, "changeable"), (2, "default"), (3, "saved")];

/// Runs the MODE SENSE form cases on one LUN and notes in `disk` the pages it has and whether saved
/// values answer.
pub(super) fn run(s: &mut Port, name: &str, cases: &mut Vec<Case>, disk: &mut Disk) {
    for form in [Form { ten: true }, Form { ten: false }] {
        one_form(s, name, form, cases, disk);
    }
}

fn one_form(s: &mut Port, name: &str, form: Form, cases: &mut Vec<Case>, disk: &mut Disk) {
    let f = form.name();
    let cl = form.clause();
    let hcl = form.header_clause();
    let at = |what: &str| label(name, &format!("{cl} {f} {what}"));
    let mut wants = vec![Want::Good];
    if !form.ten {
        // Refused as an unsupported command: Windows falls back to MODE SENSE (10).
        wants.push(Want::Ascq(0x5, 0x20, 0x00));
    }
    let (_, all) = expect(
        cases,
        SECTION,
        at("all pages (3Fh), block descriptors allowed"),
        &wants,
        form.send(s, false, false, 0, 0x3f, form.alloc()),
    );
    let Some(all) = all.filter(|o| o.status == 0) else {
        return;
    };
    let data = returned(&all).to_vec();
    let Some(p) = parse(&data, form.ten) else {
        record(
            cases,
            SECTION,
            at("all pages: a mode parameter header"),
            format!("at least {} bytes ({hcl})", form.header_len()),
            format!("{} bytes", data.len()),
            false,
        );
        return;
    };
    // SPC-6 7.5.6, 4.2.5.6: the device returns MODE DATA LENGTH + n bytes, or the allocation length.
    let want = p.total.min(form.alloc() as usize);
    if residual_not_reported(all.received, p.total) {
        record(
            cases,
            SECTION,
            label(
                name,
                &format!("{hcl} {f} all pages: MODE DATA LENGTH counts the bytes returned"),
            ),
            format!(
                "bytes returned = MODE DATA LENGTH + {}",
                if form.ten { 2 } else { 1 }
            ),
            "not run: the transport reports the whole allocation as returned (no residual)",
            false,
        );
        return;
    }
    record(
        cases,
        SECTION,
        label(
            name,
            &format!("{hcl} {f} all pages: MODE DATA LENGTH counts the bytes returned"),
        ),
        format!(
            "bytes returned = MODE DATA LENGTH + {} ({want})",
            if form.ten { 2 } else { 1 }
        ),
        format!(
            "{} bytes, MODE DATA LENGTH {}",
            data.len(),
            p.total - if form.ten { 2 } else { 1 }
        ),
        data.len() == want,
    );
    // SBC-5 6.5.1: a direct access device has zero or one block descriptor, 8 bytes (16 with LONGLBA).
    let one = if p.longlba { 16 } else { 8 };
    record(
        cases,
        SECTION,
        label(
            name,
            &format!("SBC-5 6.5.1 {f} all pages: block descriptor length 0 or {one}"),
        ),
        format!("0 or {one}"),
        format!("{}", p.bdl),
        p.bdl == 0 || p.bdl == one,
    );
    // SBC-5 6.5.1: MEDIUM TYPE 00h.
    record(
        cases,
        SECTION,
        label(name, &format!("SBC-5 6.5.1 {f} MEDIUM TYPE 00h")),
        "00h",
        format!("{:02x}h", p.medium_type),
        p.medium_type == 0,
    );
    if form.ten {
        // SPC-6 6.13.1: with LLBAA zero, LONGLBA is zero.
        record(
            cases,
            SECTION,
            label(name, "SPC-6 6.13 MODE SENSE (10) LLBAA 0: LONGLBA 0"),
            "0",
            format!("{}", u8::from(p.longlba)),
            !p.longlba,
        );
    }
    // SPC-6 7.5.8: each page's PAGE LENGTH, so the pages end where MODE DATA LENGTH does.
    record(
        cases,
        SECTION,
        label(
            name,
            &format!("SPC-6 7.5.8 {f} all pages: the pages fill MODE DATA LENGTH"),
        ),
        "page lengths add up to MODE DATA LENGTH",
        if p.fills {
            format!("{} pages, exactly", p.pages.len())
        } else {
            format!(
                "{} pages, then a page past the end or bytes left over",
                p.pages.len()
            )
        },
        p.fills,
    );
    let codes: Vec<u8> = p.pages.iter().map(|g| g.code).collect();
    // SPC-6 6.13.1: page code 3Fh returns the pages in ascending order from 01h, 00h last; subpage 00h
    // asks for the page_0 format only (table 170).
    record(
        cases,
        SECTION,
        at("all pages in ascending order, page 00h last"),
        "ascending, each once, 00h last",
        hex_list(&codes),
        in_order(&codes),
    );
    let sub_pages: Vec<String> = p
        .pages
        .iter()
        .filter(|g| g.spf)
        .map(|g| format!("{:02x}h/{:02x}h", g.code, g.subpage))
        .collect();
    record(
        cases,
        SECTION,
        at("all pages (3Fh/00h): page_0 format only"),
        "no subpages",
        if sub_pages.is_empty() {
            "none".to_string()
        } else {
            sub_pages.join(" ")
        },
        sub_pages.is_empty(),
    );
    if form.ten {
        disk.mode_list = p.pages.iter().map(|g| (g.code, g.subpage)).collect();
    }

    // DBD: no block descriptors (SPC-6 6.13.1), and nothing else changes.
    let dbd = form.send(s, true, false, 0, 0x3f, form.alloc());
    let (_, dbd) = expect(cases, SECTION, at("all pages with DBD"), &[Want::Good], dbd);
    if let Some(q) = dbd
        .filter(|o| o.status == 0)
        .and_then(|o| parse(returned(&o), form.ten))
    {
        record(
            cases,
            SECTION,
            at("DBD 1: no block descriptors"),
            "BLOCK DESCRIPTOR LENGTH 0",
            format!("{}", q.bdl),
            q.bdl == 0,
        );
        let same = q.pages == p.pages && q.total + p.bdl == p.total;
        record(
            cases,
            SECTION,
            at("DBD 1: the same pages, shorter by the block descriptors"),
            format!("the same pages; MODE DATA LENGTH {} less", p.bdl),
            if same {
                "the same".to_string()
            } else if q.pages != p.pages {
                "different pages".to_string()
            } else {
                format!("length {} with DBD, {} without", q.total, p.total)
            },
            same,
        );
    }

    // Each page on its own, without block descriptors: the same bytes as in the whole.
    for g in p.pages.iter().filter(|g| g.code != 0 && !g.spf) {
        let o = form.send(s, true, false, 0, g.code, form.alloc());
        let what = format!("page {:02x}h alone: the same as in all pages", g.code);
        let (actual, passed) = match &o {
            Ok(o) if o.status == 0 => match parse(returned(o), form.ten) {
                Some(q) if q.pages.first() == Some(g) => ("the same".to_string(), true),
                Some(q) if q.pages.is_empty() => ("no page".to_string(), false),
                Some(_) => ("different".to_string(), false),
                None => ("no header".to_string(), false),
            },
            Ok(o) => (describe(o), false),
            Err(e) => (e.clone(), false),
        };
        record(
            cases,
            SECTION,
            at(&what),
            "GOOD, the same bytes",
            actual,
            passed,
        );
    }

    // The other page controls (SPC-6 6.13.3 to 6.13.5): changeable values may be refused when the logical
    // unit has none; saved values may be refused as SAVING PARAMETERS NOT SUPPORTED, unless a page says
    // it is saveable (PS, SPC-6 7.5.8).
    let saveable = p.pages.iter().any(|g| g.ps);
    for (pc, kind) in CONTROLS {
        let mut wants = vec![Want::Good];
        match pc {
            1 => wants.push(Want::Ascq(0x5, 0x24, 0x00)),
            3 if !saveable => wants.push(Want::Ascq(0x5, 0x39, 0x00)),
            _ => {}
        }
        let what = if pc == 3 && saveable {
            format!("all pages, {kind} values (a page has PS 1)")
        } else {
            format!("all pages, {kind} values")
        };
        let (_, o) = expect(
            cases,
            SECTION,
            at(&what),
            &wants,
            form.send(s, true, false, pc, 0x3f, form.alloc()),
        );
        if pc == 3 && form.ten {
            disk.saved_values = Some(o.as_ref().is_some_and(|o| o.status == 0));
        }
        let Some(q) = o
            .filter(|o| o.status == 0)
            .and_then(|o| parse(returned(&o), form.ten))
        else {
            continue;
        };
        // SPC-6 6.13.1: PS, SPF, the page and subpage codes and PAGE LENGTH should be as for PC 00b.
        let shape = |pages: &[Page]| -> Vec<(u8, u8, usize, bool)> {
            pages
                .iter()
                .map(|g| (g.code, g.subpage, g.bytes.len(), g.ps))
                .collect()
        };
        let same = shape(&q.pages) == shape(&p.pages);
        record_should(
            cases,
            SECTION,
            at(&format!(
                "{kind} values: the same pages and page lengths as the current"
            )),
            "the same page codes, PS and PAGE LENGTH",
            if same {
                "the same".to_string()
            } else {
                hex_list(&q.pages.iter().map(|g| g.code).collect::<Vec<_>>())
            },
            same,
        );
    }

    // SPC-6 4.2.5.6: a short allocation length returns the start of the same answer.
    let short = form.header_len() as u32;
    let o = form.send(s, false, false, 0, 0x3f, short);
    let (actual, passed) = match &o {
        Ok(o) if o.status == 0 => {
            let r = returned(o);
            let ok = r.len() == short as usize && data.get(..r.len()) == Some(r);
            (
                format!("{} bytes{}", r.len(), if ok { ", the same" } else { "" }),
                ok,
            )
        }
        Ok(o) => (describe(o), false),
        Err(e) => (e.clone(), false),
    };
    record(
        cases,
        SECTION,
        label(
            name,
            &format!("SPC-6 4.2.5.6 {f} all pages at allocation {short}"),
        ),
        format!("GOOD, the first {short} bytes"),
        actual,
        passed,
    );

    if form.ten {
        // SPC-6 6.13.1: with LLBAA set the device server should return 16-byte descriptors (LONGLBA);
        // SPC-6 7.5.6: with LONGLBA the descriptors are 16 bytes each.
        let o = form.send(s, false, true, 0, 0x3f, form.alloc());
        let (_, o) = expect(cases, SECTION, at("all pages with LLBAA"), &[Want::Good], o);
        if let Some(q) = o
            .filter(|o| o.status == 0)
            .and_then(|o| parse(returned(&o), true))
        {
            record(
                cases,
                SECTION,
                at("LLBAA 1: descriptors 16 bytes each with LONGLBA"),
                "LONGLBA 0 and 0 or 8 bytes, or LONGLBA 1 and a multiple of 16",
                format!("LONGLBA {}, {} bytes", u8::from(q.longlba), q.bdl),
                if q.longlba {
                    q.bdl % 16 == 0
                } else {
                    matches!(q.bdl, 0 | 8)
                },
            );
            if q.bdl > 0 {
                record_should(
                    cases,
                    SECTION,
                    at("LLBAA 1: long LBA block descriptor"),
                    "LONGLBA 1, 16 bytes",
                    format!("LONGLBA {}, {} bytes", u8::from(q.longlba), q.bdl),
                    q.longlba && q.bdl == 16,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answer10(bdl: usize, pages: &[&[u8]]) -> Vec<u8> {
        let mut d = vec![0u8; 8];
        d[6..8].copy_from_slice(&(bdl as u16).to_be_bytes());
        d.extend(std::iter::repeat_n(0u8, bdl));
        for p in pages {
            d.extend_from_slice(p);
        }
        let mdl = (d.len() - 2) as u16;
        d[0..2].copy_from_slice(&mdl.to_be_bytes());
        d
    }

    #[test]
    fn an_answer_parses_into_header_descriptors_and_pages() {
        let caching: &[u8] = &[0x88, 0x02, 0x04, 0x00];
        let control: &[u8] = &[0x0a, 0x02, 0x00, 0x00];
        let d = answer10(8, &[caching, control]);
        let p = parse(&d, true).unwrap();
        assert_eq!((p.total, p.bdl, p.pages.len()), (d.len(), 8, 2));
        assert!(p.fills && p.pages[0].ps && !p.pages[1].ps);
        assert_eq!(p.pages[1].code, 0x0a);
        let mut short = d.clone();
        short[1] -= 1;
        assert!(
            !parse(&short, true).unwrap().fills,
            "a page past the length"
        );
        let mut long = d.clone();
        long[1] += 3;
        assert!(
            parse(&long, true).unwrap().fills,
            "cut data is the length case's"
        );
        let six = [7u8, 0, 0x10, 0, 0x08, 0x02, 0x04, 0];
        let q = parse(&six, false).unwrap();
        assert_eq!((q.total, q.device_specific, q.pages[0].code), (8, 0x10, 8));
        assert!(q.fills);
        assert!(parse(&[0, 0], true).is_none());
    }

    #[test]
    fn a_subpage_is_read_in_the_sub_page_format() {
        let d = answer10(0, &[&[0x4a, 0x01, 0x00, 0x02, 1, 2]]);
        let p = parse(&d, true).unwrap();
        assert!(p.fills && p.pages[0].spf);
        assert_eq!(
            (p.pages[0].code, p.pages[0].subpage, p.pages[0].bytes.len()),
            (0x0a, 1, 6)
        );
    }

    #[test]
    fn page_order_is_ascending_from_01h_with_00h_last() {
        assert!(in_order(&[0x01, 0x08, 0x0a, 0x1c, 0x00]));
        assert!(in_order(&[]));
        assert!(!in_order(&[0x08, 0x01]));
        assert!(!in_order(&[0x00, 0x08]));
        assert!(!in_order(&[0x08, 0x08]));
    }
}
