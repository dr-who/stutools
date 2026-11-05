//! The `disk` cases of `--compliance`: what an operating system's disk driver reads when it finds a
//! disk (Windows storport/classpnp/disk.sys, macOS IOSCSIBlockCommandsDevice, Linux sd): standard
//! INQUIRY and its VPD pages, READ CAPACITY (10) and (16), the mode pages (Caching, Control, Control
//! Extension, Informational Exceptions Control, Read-Write Error Recovery) through MODE SENSE (10) and
//! (6), a MODE SELECT that changes nothing, REPORT SUPPORTED OPERATION CODES, REPORT SUPPORTED TASK
//! MANAGEMENT FUNCTIONS, REQUEST SENSE in both sense formats, START STOP UNIT and LOG SENSE. Every
//! case cites the SPC-5 or SBC-5 clause it checks. What it reads is returned as `Disk`, for the
//! sections that judge data commands, provisioning, feature sets and Windows.

use super::cases::{
    be16, be32, be64, expect, record, refused, vpd, Seen, Want, REFUSED, REFUSED_SA,
};
use super::session::{Outcome, Session};
use crate::compliance::Case;
use crate::Result;

const SECTION: &str = "disk";

/// VPD pages read when page 00h lists them (SPC-6 7.7, SBC-5 6.6), Power Condition (8Ah) and Power
/// Consumption (8Dh) among them for the feature sets.
const PAGES: [u8; 11] = [
    0x80, 0x83, 0x86, 0x87, 0x8a, 0x8d, 0x8f, 0x92, 0xb0, 0xb1, 0xb2,
];

/// One command REPORT SUPPORTED OPERATION CODES lists (SPC-6 6.32.2, table 260).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Supported {
    pub opcode: u8,
    pub service_action: Option<u16>,
    pub cdb_len: u16,
}

/// What the disk cases read from one LUN.
#[derive(Debug, Default)]
pub(super) struct Disk {
    pub inquiry: Vec<u8>,
    /// VPD page 00h.
    pub pages: Vec<u8>,
    /// Each VPD page read, by code.
    pub vpd: Vec<(u8, Vec<u8>)>,
    pub rc16: Vec<u8>,
    pub last_lba: u64,
    pub block_len: u32,
    /// Mode pages from MODE SENSE (10) with current values, by (page, subpage): the page alone.
    pub mode: Vec<((u8, u8), Vec<u8>)>,
    /// The changeable values (PC 01b) of the same pages.
    pub changeable: Vec<((u8, u8), Vec<u8>)>,
    /// The DEVICE-SPECIFIC PARAMETER of the mode parameter header (SBC-5 table 231).
    pub device_specific: u8,
    /// The block descriptor length MODE SENSE (10) returned with DBD clear.
    pub block_descriptor_len: Option<u16>,
    /// REPORT SUPPORTED OPERATION CODES, all commands, when the device server supports it.
    pub rsoc: Option<Vec<Supported>>,
    /// The SUPPORT field for FORMAT UNIT (one_command, 001b), which is never sent.
    pub format_unit_support: Option<u8>,
    /// REPORT SUPPORTED TASK MANAGEMENT FUNCTIONS bytes 0 and 1.
    pub tmf_bits: Option<[u8; 2]>,
    /// The log pages LOG SENSE page 00h lists, and the page/subpage pairs of 00h/FFh.
    pub log_pages: Option<Vec<u8>>,
    pub log_subpages: Option<Vec<(u8, u8)>>,
    /// A VPD 83h designator of association 0 (the LUN) that names it uniquely: NAA, EUI-64 or SCSI
    /// name string.
    pub unique_id: bool,
    /// Every disk case passed.
    pub answered: bool,
}

impl Disk {
    pub(super) fn page(&self, code: u8) -> Option<&[u8]> {
        self.vpd
            .iter()
            .find(|(c, _)| *c == code)
            .map(|(_, d)| d.as_slice())
    }

    pub(super) fn mode_page(&self, page: u8, subpage: u8) -> Option<&[u8]> {
        self.mode
            .iter()
            .find(|(k, _)| *k == (page, subpage))
            .map(|(_, d)| d.as_slice())
    }

    /// Whether a mode page bit can be set: it is set now, or the changeable values mark it.
    pub(super) fn mode_bit(&self, page: u8, byte: usize, mask: u8) -> bool {
        let at = |list: &Vec<((u8, u8), Vec<u8>)>| {
            list.iter()
                .find(|(k, _)| *k == (page, 0))
                .and_then(|(_, d)| d.get(byte))
                .is_some_and(|b| b & mask != 0)
        };
        at(&self.mode) || at(&self.changeable)
    }

    /// TPGS in standard INQUIRY byte 5 (SPC-6 table 148): 1 implicit, 2 explicit, 3 both.
    pub(super) fn tpgs(&self) -> u8 {
        (self.inquiry.get(5).copied().unwrap_or(0) >> 4) & 0x3
    }

    /// READ CAPACITY (16) LBPME and LBPRZ (SBC-5 table 89).
    pub(super) fn lbpme(&self) -> bool {
        self.rc16.get(14).is_some_and(|b| b & 0x80 != 0)
    }

    pub(super) fn lbprz(&self) -> bool {
        self.rc16.get(14).is_some_and(|b| b & 0x40 != 0)
    }

    /// The Block Limits VPD page fields (SBC-5 table 269).
    pub(super) fn limits(&self) -> Option<Limits> {
        let d = self.page(0xb0)?;
        (d.len() >= 44).then(|| Limits {
            max_compare_and_write: d[5],
            optimal_granularity: be16(d, 6),
            max_transfer: be32(d, 8),
            optimal_transfer: be32(d, 12),
            max_unmap_lba: be32(d, 20),
            max_unmap_descriptors: be32(d, 24),
            optimal_unmap_granularity: be32(d, 28),
            max_write_same: be64(d, 36),
            max_atomic_transfer: if d.len() >= 48 { be32(d, 44) } else { 0 },
        })
    }

    /// The Logical Block Provisioning VPD page (SBC-5 table 281).
    pub(super) fn provisioning(&self) -> Option<Provisioning> {
        let d = self.page(0xb2)?;
        let b5 = d.get(5).copied().unwrap_or(0);
        Some(Provisioning {
            lbpu: b5 & 0x80 != 0,
            lbpws: b5 & 0x40 != 0,
            lbpws10: b5 & 0x20 != 0,
            lbprz: (b5 >> 2) & 0x7,
            anc_sup: b5 & 0x02 != 0,
            provisioning_type: d.get(6).copied().unwrap_or(0) & 0x7,
        })
    }

    /// The Extended INQUIRY Data VPD page bits the feature sets need (SPC-6 table 553).
    pub(super) fn extended(&self) -> Option<Extended> {
        let d = self.page(0x86)?;
        let at = |i: usize| d.get(i).copied().unwrap_or(0);
        Some(Extended {
            simpsup: at(5) & 0x01 != 0,
            v_sup: at(6) & 0x01 != 0,
            nv_sup: at(6) & 0x02 != 0,
            luiclr: at(7) & 0x01 != 0,
            r_sup: at(8) & 0x10 != 0,
        })
    }

    /// WCE and RCD of the current Caching mode page (SBC-5 table 239).
    pub(super) fn write_cache(&self) -> Option<bool> {
        self.mode_page(0x08, 0)
            .and_then(|p| p.get(2))
            .map(|b| b & 0x04 != 0)
    }

    /// DPOFUA in the DEVICE-SPECIFIC PARAMETER (SBC-5 table 231).
    pub(super) fn dpofua(&self) -> bool {
        self.device_specific & 0x10 != 0
    }

    /// Whether REPORT SUPPORTED OPERATION CODES lists this command, when it lists anything.
    pub(super) fn lists(&self, opcode: u8, service_action: Option<u16>) -> Option<bool> {
        self.rsoc.as_ref().map(|list| {
            list.iter().any(|c| {
                c.opcode == opcode
                    && (service_action.is_none() || c.service_action == service_action)
            })
        })
    }

    /// The feature set codes of VPD page 92h (SPC-6 7.7.14), when it was read.
    pub(super) fn feature_sets(&self) -> Option<Vec<u16>> {
        self.page(0x92).map(parse_feature_sets)
    }
}

/// The Block Limits fields the data cases need.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(super) struct Limits {
    pub max_compare_and_write: u8,
    pub optimal_granularity: u16,
    pub max_transfer: u32,
    pub optimal_transfer: u32,
    pub max_unmap_lba: u32,
    pub max_unmap_descriptors: u32,
    pub optimal_unmap_granularity: u32,
    pub max_write_same: u64,
    pub max_atomic_transfer: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(super) struct Provisioning {
    pub lbpu: bool,
    pub lbpws: bool,
    pub lbpws10: bool,
    /// The three-bit LBPRZ field.
    pub lbprz: u8,
    pub anc_sup: bool,
    pub provisioning_type: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(super) struct Extended {
    pub simpsup: bool,
    pub v_sup: bool,
    pub nv_sup: bool,
    pub luiclr: bool,
    pub r_sup: bool,
}

/// The feature set codes in VPD page 92h: two bytes each from byte 8 (SPC-6 table 572).
pub(super) fn parse_feature_sets(d: &[u8]) -> Vec<u16> {
    let end = (4 + be16(d, 2) as usize).min(d.len());
    d.get(8..end.max(8))
        .unwrap_or(&[])
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect()
}

/// One designation descriptor of VPD page 83h (SPC-6 table 525).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Designation {
    pub code_set: u8,
    pub association: u8,
    pub kind: u8,
    pub value: Vec<u8>,
}

/// The designation descriptors, and whether they exactly fill the page length.
pub(super) fn parse_designations(d: &[u8]) -> (Vec<Designation>, bool) {
    let end = 4 + be16(d, 2) as usize;
    let mut out = Vec::new();
    let mut i = 4;
    while i + 4 <= end.min(d.len()) {
        let len = d[i + 3] as usize;
        out.push(Designation {
            code_set: d[i] & 0x0f,
            association: (d[i + 1] >> 4) & 0x3,
            kind: d[i + 1] & 0x0f,
            value: d.get(i + 4..i + 4 + len).unwrap_or(&[]).to_vec(),
        });
        i += 4 + len;
    }
    (out, i == end && end <= d.len())
}

/// Whether a designator names the LUN uniquely, as multipath software and Windows clustering need:
/// association 0, and an EUI-64 (2h), NAA (3h) of a valid NAA type, or SCSI name string (8h)
/// (SPC-6 7.7.6).
pub(super) fn names_lun(d: &Designation) -> bool {
    d.association == 0
        && match d.kind {
            2 => matches!(d.value.len(), 8 | 12 | 16),
            3 => d.code_set == 1 && naa_ok(&d.value),
            8 => matches!(d.code_set, 3) && !d.value.is_empty(),
            _ => false,
        }
}

/// An NAA designator is 8 bytes for NAA 2h, 3h and 5h and 16 bytes for NAA 6h (SPC-6 7.7.6.6).
fn naa_ok(v: &[u8]) -> bool {
    match v.first().map(|b| b >> 4) {
        Some(2 | 3 | 5) => v.len() == 8,
        Some(6) => v.len() == 16,
        _ => false,
    }
}

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

fn mode_sense_10(
    s: &mut Session,
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

fn mode_sense_6(s: &mut Session, page: u8) -> Result<Outcome> {
    s.command(&[0x1a, 0x08, page, 0, 255, 0], 255, None)
}

/// The all_commands list of REPORT SUPPORTED OPERATION CODES (SPC-5 tables 259, 260). A descriptor
/// with CTDP set carries a 12-byte command timeouts descriptor.
pub(super) fn parse_rsoc(d: &[u8]) -> Vec<Supported> {
    let end = (4 + be32(d, 0) as usize).min(d.len());
    let mut out = Vec::new();
    let mut i = 4;
    while i + 8 <= end {
        let flags = d[i + 5];
        out.push(Supported {
            opcode: d[i],
            service_action: (flags & 0x01 != 0).then(|| be16(d, i + 2)),
            cdb_len: be16(d, i + 6),
        });
        i += if flags & 0x02 != 0 { 20 } else { 8 };
    }
    out
}

fn rsoc_all(s: &mut Session, rctd: bool) -> Result<Outcome> {
    let a = 65536u32.to_be_bytes();
    let cdb = [
        0xa3,
        0x0c,
        if rctd { 0x80 } else { 0 },
        0,
        0,
        0,
        a[0],
        a[1],
        a[2],
        a[3],
        0,
        0,
    ];
    s.command(&cdb, 65536, None)
}

/// REPORT SUPPORTED OPERATION CODES for one command (REPORTING OPTIONS 001b, or 010b with a service
/// action; SPC-6 table 257).
pub(super) fn rsoc_one(
    s: &mut Session,
    opcode: u8,
    service_action: Option<u16>,
) -> Result<Outcome> {
    let sa = service_action.unwrap_or(0).to_be_bytes();
    let options = if service_action.is_some() { 0x02 } else { 0x01 };
    let cdb = [
        0xa3, 0x0c, options, opcode, sa[0], sa[1], 0, 0, 0x01, 0, 0, 0,
    ];
    s.command(&cdb, 256, None)
}

/// Runs every disk case and returns what it read.
pub(super) fn run(s: &mut Session, name: &str, cases: &mut Vec<Case>, seen: &mut Seen) -> Disk {
    let first = cases.len();
    let mut disk = Disk::default();
    inquiry(s, name, cases, seen, &mut disk);
    capacity(s, name, cases, seen, &mut disk);
    mode_pages(s, name, cases, seen, &mut disk);
    operation_codes(s, name, cases, seen, &mut disk);
    others(s, name, cases, seen, &mut disk);
    disk.answered = cases[first..].iter().all(|c| c.passed);
    disk
}

fn label(name: &str, what: &str) -> String {
    format!("{name} {what}")
}

/// SPC-6 6.7 INQUIRY, 7.7 VPD pages.
fn inquiry(s: &mut Session, name: &str, cases: &mut Vec<Case>, seen: &mut Seen, disk: &mut Disk) {
    let full = s.command(&[0x12, 0, 0, 0, 96, 0], 96, None);
    let answer = expect(
        cases,
        SECTION,
        label(name, "standard INQUIRY"),
        &[Want::Good],
        full,
    );
    seen.answer("INQUIRY", SECTION, &answer);
    let d = answer.1.map(|o| o.data).unwrap_or_default();
    // SPC-6 6.7.2: qualifier 000b, type 00h (direct access), RESPONSE DATA FORMAT 2h, at least 36 bytes.
    let at = |i: usize| d.get(i).copied().unwrap_or(0);
    let format_ok = at(0) == 0 && at(3) & 0x0f == 2 && at(4) >= 31;
    record(
        cases,
        SECTION,
        label(
            name,
            "standard INQUIRY: direct access, RESPONSE DATA FORMAT 2h, 36+ bytes",
        ),
        "qualifier 0, type 00h, format 2h, ADDITIONAL LENGTH >= 31",
        format!(
            "byte 0 {:02x}h, format {:x}h, ADDITIONAL LENGTH {}",
            at(0),
            at(3) & 0x0f,
            at(4)
        ),
        format_ok,
    );
    // SPC-6 table 152: 05h SPC-3, 06h SPC-4, 07h SPC-5. Windows MPIO and clustering need SPC-3 or later.
    record(
        cases,
        SECTION,
        label(name, "standard INQUIRY VERSION"),
        "05h, 06h or 07h (SPC-3 or later)",
        format!("{:02x}h", at(2)),
        matches!(at(2), 5..=7),
    );
    // SPC-6 6.7.2: the CMDQUE bit shall be set to one.
    record(
        cases,
        SECTION,
        label(name, "standard INQUIRY CMDQUE"),
        "1",
        format!("{}", (at(7) >> 1) & 1),
        at(7) & 0x02 != 0,
    );
    // SPC-6 4.3.1: ASCII data fields hold 20h to 7Eh.
    let ascii = d
        .get(8..36)
        .is_some_and(|f| f.iter().all(|b| (0x20..=0x7e).contains(b)));
    record(
        cases,
        SECTION,
        label(name, "vendor, product and revision are printable ASCII"),
        "20h to 7Eh",
        super::cases::printable(d.get(8..36).unwrap_or(&[])),
        ascii,
    );
    // SPC-6 4.2.5.6: a shorter allocation length returns the start of the same data.
    let short = s.command(&[0x12, 0, 0, 0, 36, 0], 36, None);
    let same = matches!(&short, Ok(o) if o.status == 0 && d.len() >= 36 && o.data[..36] == d[..36]);
    record(
        cases,
        SECTION,
        label(name, "36-byte INQUIRY matches the start of the full one"),
        "GOOD, same 36 bytes",
        match &short {
            Ok(o) if o.status == 0 => {
                if same {
                    "same".into()
                } else {
                    "different".into()
                }
            }
            Ok(o) => super::cases::describe(o),
            Err(e) => e.clone(),
        },
        same,
    );
    disk.inquiry = d;

    // SPC-6 7.7.17: page 00h lists the supported pages in ascending order, 00h, 80h and 83h among them.
    let list = vpd(s, 0x00, 255);
    let (_, list) = expect(
        cases,
        SECTION,
        label(name, "VPD 00h supported pages"),
        &[Want::Good],
        list,
    );
    let raw = list.map(|o| o.data).unwrap_or_default();
    let n = raw.get(3).copied().unwrap_or(0) as usize;
    disk.pages = raw.get(4..4 + n).unwrap_or(&[]).to_vec();
    let sorted = disk.pages.windows(2).all(|w| w[0] < w[1]);
    record(
        cases,
        SECTION,
        label(name, "VPD 00h ascending, with 00h, 80h and 83h"),
        "ascending; 00h 80h 83h",
        hex_list(&disk.pages),
        sorted && [0x00, 0x80, 0x83].iter().all(|p| disk.pages.contains(p)),
    );
    for page in PAGES {
        if !disk.pages.contains(&page) {
            continue;
        }
        let o = vpd(s, page, 4096);
        let (passed, o) = expect(
            cases,
            SECTION,
            label(name, &format!("VPD {page:02x}h")),
            &[Want::Good],
            o,
        );
        if let Some(o) = o.filter(|o| o.status == 0) {
            let len = 4 + be16(&o.data, 2) as usize;
            let ok = o.data.get(1) == Some(&page);
            // SBC-5 6.6.2 and 6.6.4: Block Device Characteristics and Block Limits are 003Ch long;
            // SPC-6 7.7.7: Extended INQUIRY Data is 003Ch long.
            let fixed = match page {
                0xb0 | 0xb1 | 0x86 => len == 0x40,
                0xb2 => len >= 8,
                _ => true,
            };
            if !(ok && fixed) {
                record(
                    cases,
                    SECTION,
                    label(name, &format!("VPD {page:02x}h page code and length")),
                    "as the standard sets",
                    format!(
                        "page {:02x}h, length {}",
                        o.data.get(1).copied().unwrap_or(0),
                        len - 4
                    ),
                    false,
                );
            }
            let data = o.data[..len.min(o.data.len())].to_vec();
            disk.vpd.push((page, data));
        }
        let _ = passed;
    }
    // SPC-6 6.7.1: a VPD page the device server does not support is an invalid field in the CDB.
    let unlisted = [0xb1u8, 0xb2, 0xb0, 0x86, 0x87, 0x92, 0x8f]
        .into_iter()
        .find(|p| !disk.pages.contains(p))
        .unwrap_or(0xfe);
    if !disk.pages.contains(&unlisted) {
        expect(
            cases,
            SECTION,
            label(name, &format!("unlisted VPD {unlisted:02x}h refused")),
            &[Want::Ascq(0x5, 0x24, 0x00)],
            vpd(s, unlisted, 255),
        );
    }
    // SPC-6 7.7.19 and 4.3.1: the serial number is right-aligned ASCII, 20h to 7Eh (no NUL).
    if let Some(d) = disk.page(0x80) {
        let serial = d.get(4..).unwrap_or(&[]);
        record(
            cases,
            SECTION,
            label(name, "VPD 80h serial number is printable ASCII"),
            "20h to 7Eh",
            super::cases::printable(serial),
            !serial.is_empty() && serial.iter().all(|b| (0x20..=0x7e).contains(b)),
        );
    }
    // SPC-6 7.7.6: the descriptors fill the page; one names the LUN uniquely.
    if let Some(d) = disk.page(0x83) {
        let (list, fills) = parse_designations(d);
        record(
            cases,
            SECTION,
            label(name, "VPD 83h descriptors fill the page"),
            "exactly",
            if fills { "exactly" } else { "not" },
            fills,
        );
        disk.unique_id = list.iter().any(names_lun);
        record(
            cases,
            SECTION,
            label(
                name,
                "VPD 83h names the LUN uniquely (NAA, EUI-64 or SCSI name)",
            ),
            "an association 0 designator",
            list.iter()
                .filter(|d| d.association == 0)
                .map(|d| format!("type {:x}h", d.kind))
                .collect::<Vec<_>>()
                .join(", "),
            disk.unique_id,
        );
    }
    // SBC-5 6.6.4: OPTIMAL TRANSFER LENGTH is at most MAXIMUM TRANSFER LENGTH when both are reported.
    if let Some(l) = disk.limits() {
        record(
            cases,
            SECTION,
            label(name, "VPD B0h optimal transfer within the maximum"),
            "optimal <= maximum",
            format!("optimal {}, maximum {}", l.optimal_transfer, l.max_transfer),
            l.max_transfer == 0 || l.optimal_transfer <= l.max_transfer,
        );
    }
    // SBC-5 6.6.2: MEDIUM ROTATION RATE 0000h (not reported), 0001h (non-rotating), or 0401h to FFFEh.
    if let Some(d) = disk.page(0xb1) {
        let rate = be16(d, 4);
        record(
            cases,
            SECTION,
            label(name, "VPD B1h medium rotation rate"),
            "0000h, 0001h or 0401h to FFFEh",
            format!("{rate:04x}h"),
            matches!(rate, 0 | 1 | 0x401..=0xfffe),
        );
    }
}

/// SBC-5 5.20, 5.21: READ CAPACITY (10) and (16).
fn capacity(s: &mut Session, name: &str, cases: &mut Vec<Case>, seen: &mut Seen, disk: &mut Disk) {
    let cdb16 = [0x9e, 0x10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 32, 0, 0];
    let answer = expect(
        cases,
        SECTION,
        label(name, "READ CAPACITY (16)"),
        &[Want::Good],
        s.command(&cdb16, 32, None),
    );
    seen.answer("READ CAPACITY (16)", SECTION, &answer);
    disk.rc16 = answer.1.map(|o| o.data).unwrap_or_default();
    disk.last_lba = be64(&disk.rc16, 0);
    disk.block_len = be32(&disk.rc16, 8);
    let exponent = disk.rc16.get(13).copied().unwrap_or(0) & 0x0f;
    let lowest = be16(&disk.rc16, 14) & 0x3fff;
    record(
        cases,
        SECTION,
        label(name, "READ CAPACITY (16) block size and alignment"),
        "a power of two of 512 B or more; lowest aligned LBA below 2^exponent",
        format!(
            "{} B, exponent {exponent}, lowest aligned {lowest}",
            disk.block_len
        ),
        disk.block_len >= 512
            && disk.block_len.is_power_of_two()
            && (lowest as u32) < (1u32 << exponent),
    );
    let answer = expect(
        cases,
        SECTION,
        label(name, "READ CAPACITY (10)"),
        &[Want::Good],
        s.command(&[0x25, 0, 0, 0, 0, 0, 0, 0, 0, 0], 8, None),
    );
    seen.answer("READ CAPACITY (10)", SECTION, &answer);
    if let Some(o) = answer.1.filter(|o| o.status == 0) {
        // SBC-5 5.20.2: the last LBA, or FFFF_FFFFh when it does not fit, and the same block length.
        let want = disk.last_lba.min(0xffff_ffff) as u32;
        record(
            cases,
            SECTION,
            label(name, "READ CAPACITY (10) agrees with (16)"),
            format!("{want} and {} B", disk.block_len),
            format!("{} and {} B", be32(&o.data, 0), be32(&o.data, 4)),
            be32(&o.data, 0) == want && be32(&o.data, 4) == disk.block_len,
        );
    }
}

/// SPC-5 6.14, 6.15, 7.5; SBC-5 6.5: the mode pages, and MODE SELECT of the Caching page unchanged.
fn mode_pages(
    s: &mut Session,
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

/// SPC-6 6.32 REPORT SUPPORTED OPERATION CODES and 6.35 REPORT SUPPORTED TASK MANAGEMENT FUNCTIONS.
fn operation_codes(
    s: &mut Session,
    name: &str,
    cases: &mut Vec<Case>,
    seen: &mut Seen,
    disk: &mut Disk,
) {
    let mut wants = vec![Want::Good];
    wants.extend(REFUSED_SA);
    let answer = expect(
        cases,
        SECTION,
        label(name, "REPORT SUPPORTED OPERATION CODES, all, RCTD"),
        &wants,
        rsoc_all(s, true),
    );
    let rctd = matches!(&answer.1, Some(o) if o.status == 0);
    seen.fact("RSOC RCTD", rctd);
    let list = if rctd {
        seen.answer("REPORT SUPPORTED OPERATION CODES", SECTION, &answer);
        answer.1.map(|o| o.data)
    } else {
        let again = expect(
            cases,
            SECTION,
            label(name, "REPORT SUPPORTED OPERATION CODES, all"),
            &wants,
            rsoc_all(s, false),
        );
        seen.answer("REPORT SUPPORTED OPERATION CODES", SECTION, &again);
        again.1.filter(|o| o.status == 0).map(|o| o.data)
    };
    seen.fact("RSOC 000b", list.is_some());
    disk.rsoc = list.as_deref().map(parse_rsoc);
    if let Some(rsoc) = &disk.rsoc {
        // The commands this section has already seen answer GOOD must be listed.
        let required: [(u8, Option<u16>, &str); 6] = [
            (0x12, None, "INQUIRY"),
            (0x25, None, "READ CAPACITY (10)"),
            (0x9e, Some(0x10), "READ CAPACITY (16)"),
            (0x5a, None, "MODE SENSE (10)"),
            (0xa3, Some(0x0c), "REPORT SUPPORTED OPERATION CODES"),
            (0x00, None, "TEST UNIT READY"),
        ];
        let missing: Vec<&str> = required
            .iter()
            .filter(|(op, sa, _)| {
                !rsoc
                    .iter()
                    .any(|c| c.opcode == *op && (sa.is_none() || c.service_action == *sa))
            })
            .map(|(_, _, n)| *n)
            .collect();
        record(
            cases,
            SECTION,
            label(
                name,
                "REPORT SUPPORTED OPERATION CODES lists the discovery commands",
            ),
            "all listed",
            if missing.is_empty() {
                format!("{} commands listed", rsoc.len())
            } else {
                format!("missing {}", missing.join(", "))
            },
            missing.is_empty(),
        );
        // SPC-6 6.32.3: one_command for READ (10) reports SUPPORT 011b and a 10-byte CDB usage map.
        let one = rsoc_one(s, 0x28, None);
        let ok = matches!(&one, Ok(o) if o.status == 0 && o.data[1] & 0x07 == 0x03 && be16(&o.data, 2) == 10 && o.data[4] == 0x28);
        seen.fact("RSOC 001b", ok);
        record(
            cases,
            SECTION,
            label(name, "one_command (001b) for READ (10)"),
            "SUPPORT 011b, CDB SIZE 10",
            show_one(&one),
            ok,
        );
        // SPC-6 6.32.3: 010b names an operation code with service actions: READ CAPACITY (16).
        let both = rsoc_one(s, 0x9e, Some(0x10));
        let ok = matches!(&both, Ok(o) if o.status == 0 && o.data[1] & 0x07 == 0x03 && be16(&o.data, 2) == 16);
        seen.fact("RSOC 010b", ok);
        record(
            cases,
            SECTION,
            label(name, "one_command (010b) for READ CAPACITY (16)"),
            "SUPPORT 011b, CDB SIZE 16",
            show_one(&both),
            ok,
        );
        // FORMAT UNIT is mandatory (SBC-5 table 34) and destroys data, so only RSOC is asked about it.
        disk.format_unit_support = rsoc_one(s, 0x04, None)
            .ok()
            .filter(|o| o.status == 0)
            .map(|o| o.data[1] & 0x07);
    }
    let a = 16u32.to_be_bytes();
    let repd = s.command(
        &[0xa3, 0x0d, 0x80, 0, 0, 0, a[0], a[1], a[2], a[3], 0, 0],
        16,
        None,
    );
    let mut wants = vec![Want::Good];
    wants.extend(REFUSED_SA);
    let answer = expect(
        cases,
        SECTION,
        label(name, "REPORT SUPPORTED TASK MANAGEMENT FUNCTIONS, REPD"),
        &wants,
        repd,
    );
    seen.answer(
        "REPORT SUPPORTED TASK MANAGEMENT FUNCTIONS",
        SECTION,
        &answer,
    );
    if let Some(o) = answer.1.filter(|o| o.status == 0) {
        // SPC-6 table 267: with REPD the additional data length is 0Ch.
        seen.fact("RSTMF REPD", o.data.get(3) == Some(&0x0c));
        disk.tmf_bits = Some([o.data[0], o.data[1]]);
        record(
            cases,
            SECTION,
            label(name, "REPD answer is the extended format"),
            "ADDITIONAL DATA LENGTH 0Ch",
            format!("{:02x}h", o.data[3]),
            o.data.get(3) == Some(&0x0c),
        );
    } else {
        seen.fact("RSTMF REPD", false);
    }
}

fn show_one(r: &Result<Outcome>) -> String {
    match r {
        Ok(o) if o.status == 0 => format!(
            "SUPPORT {:03b}b, CDB SIZE {}",
            o.data[1] & 0x07,
            be16(&o.data, 2)
        ),
        Ok(o) => super::cases::describe(o),
        Err(e) => e.clone(),
    }
}

/// SPC-6 6.36 REQUEST SENSE, SBC-5 5.31 START STOP UNIT, SPC-6 6.9 LOG SENSE.
fn others(s: &mut Session, name: &str, cases: &mut Vec<Case>, seen: &mut Seen, disk: &mut Disk) {
    // SPC-6 6.36: with nothing to report, REQUEST SENSE returns sense data in the format DESC asks for.
    let fixed = s.command(&[0x03, 0, 0, 0, 252, 0], 252, None);
    let (passed, o) = expect(
        cases,
        SECTION,
        label(name, "REQUEST SENSE, fixed format (DESC 0)"),
        &[Want::Good],
        fixed,
    );
    let fixed_ok = passed
        && o.as_ref()
            .is_some_and(|o| matches!(o.data.first().map(|b| b & 0x7f), Some(0x70 | 0x71)));
    record(
        cases,
        SECTION,
        label(name, "REQUEST SENSE DESC 0 response code"),
        "70h or 71h",
        show_byte(&o, 0),
        fixed_ok,
    );
    seen.fact("REQUEST SENSE DESC 0", fixed_ok);
    let desc = s.command(&[0x03, 0x01, 0, 0, 252, 0], 252, None);
    let (passed, o) = expect(
        cases,
        SECTION,
        label(name, "REQUEST SENSE, descriptor format (DESC 1)"),
        &[Want::Good, Want::Ascq(0x5, 0x24, 0x00)],
        desc,
    );
    let desc_ok = o.as_ref().is_some_and(|o| {
        o.status == 0 && matches!(o.data.first().map(|b| b & 0x7f), Some(0x72 | 0x73))
    });
    if o.as_ref().is_some_and(|o| o.status == 0) {
        record(
            cases,
            SECTION,
            label(name, "REQUEST SENSE DESC 1 response code"),
            "72h or 73h",
            show_byte(&o, 0),
            desc_ok,
        );
    }
    seen.fact("REQUEST SENSE DESC 1", desc_ok);
    seen.note(
        "REQUEST SENSE",
        SECTION,
        true,
        fixed_ok && passed,
        "see disk",
    );

    // SBC-5 5.31: START with POWER CONDITION 0h and no load or eject.
    let start = s.command(&[0x1b, 0, 0, 0, 0x01, 0], 0, None);
    let mut wants = vec![Want::Good];
    wants.extend(REFUSED);
    let answer = expect(
        cases,
        SECTION,
        label(name, "START STOP UNIT, START"),
        &wants,
        start,
    );
    seen.answer("START STOP UNIT", SECTION, &answer);

    // SPC-6 6.9: LOG SENSE of page 00h, current cumulative values; then 00h/FFh, pages and subpages.
    let log = s.command(&[0x4d, 0, 0x40, 0, 0, 0, 0, 0x04, 0x00, 0], 1024, None);
    let answer = expect(
        cases,
        SECTION,
        label(name, "LOG SENSE supported pages"),
        &wants,
        log,
    );
    seen.answer("LOG SENSE", SECTION, &answer);
    if let Some(o) = answer.1.filter(|o| o.status == 0) {
        let n = be16(&o.data, 2) as usize;
        let list = o.data.get(4..4 + n).unwrap_or(&[]).to_vec();
        record(
            cases,
            SECTION,
            label(name, "LOG SENSE page 00h ascending"),
            "page code 00h, ascending",
            hex_list(&list),
            o.data[0] & 0x3f == 0 && list.windows(2).all(|w| w[0] < w[1]),
        );
        disk.log_pages = Some(list);
        let sub = s.command(&[0x4d, 0, 0x40, 0xff, 0, 0, 0, 0x04, 0x00, 0], 1024, None);
        if let Some(o) = sub.ok().filter(|o| o.status == 0) {
            let n = be16(&o.data, 2) as usize;
            disk.log_subpages = Some(
                o.data
                    .get(4..4 + n)
                    .unwrap_or(&[])
                    .chunks_exact(2)
                    .map(|c| (c[0] & 0x3f, c[1]))
                    .collect(),
            );
        }
    }
}

fn show_byte(o: &Option<Outcome>, at: usize) -> String {
    match o {
        Some(o) if o.status == 0 => format!("{:02x}h", o.data.get(at).copied().unwrap_or(0) & 0x7f),
        Some(o) => super::cases::describe(o),
        None => "no answer".into(),
    }
}

fn hex_list(v: &[u8]) -> String {
    v.iter()
        .map(|b| format!("{b:02x}h"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_sets_parse_from_byte_8() {
        let d = [0, 0x92, 0, 8, 0, 0, 0, 0, 0x00, 0x01, 0x01, 0x02];
        assert_eq!(parse_feature_sets(&d), vec![0x0001, 0x0102]);
        assert!(parse_feature_sets(&[0, 0x92, 0, 4, 0, 0, 0, 0]).is_empty());
    }

    #[test]
    fn designations_parse_and_name_the_lun() {
        let mut d = vec![0, 0x83, 0, 0];
        d.extend_from_slice(&[0x01, 0x03, 0, 16]);
        d.extend_from_slice(&[0x60; 16]);
        d.extend_from_slice(&[0x51, 0x94, 0, 4, 0, 0, 0, 1]);
        let len = (d.len() - 4) as u16;
        d[2..4].copy_from_slice(&len.to_be_bytes());
        let (list, fills) = parse_designations(&d);
        assert!(fills);
        assert_eq!(list.len(), 2);
        assert!(names_lun(&list[0]), "NAA 6h, 16 bytes");
        assert!(
            !names_lun(&list[1]),
            "a relative target port is not the LUN"
        );
        let short = Designation {
            code_set: 1,
            association: 0,
            kind: 3,
            value: vec![0x60; 8],
        };
        assert!(!names_lun(&short), "NAA 6h must be 16 bytes");
    }

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

    #[test]
    fn rsoc_descriptors_with_and_without_timeouts() {
        let mut d = vec![0, 0, 0, 28];
        d.extend_from_slice(&[0x28, 0, 0, 0, 0, 0x00, 0, 10]);
        d.extend_from_slice(&[0x9e, 0, 0, 0x10, 0, 0x03, 0, 16]);
        d.extend_from_slice(&[0; 12]);
        let list = parse_rsoc(&d);
        assert_eq!(list.len(), 2);
        assert_eq!(
            list[0],
            Supported {
                opcode: 0x28,
                service_action: None,
                cdb_len: 10
            }
        );
        assert_eq!(
            list[1],
            Supported {
                opcode: 0x9e,
                service_action: Some(0x10),
                cdb_len: 16
            }
        );
    }

    #[test]
    fn provisioning_reads_the_three_bit_lbprz() {
        let mut disk = Disk::default();
        disk.vpd
            .push((0xb2, vec![0, 0xb2, 0, 4, 0, 0xc4 | 0x02, 0x02, 0]));
        let p = disk.provisioning().unwrap();
        assert!(p.lbpu && p.lbpws && !p.lbpws10 && p.anc_sup);
        assert_eq!(p.lbprz, 1);
        assert_eq!(p.provisioning_type, 2);
    }
}
