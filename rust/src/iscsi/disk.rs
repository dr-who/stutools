//! The `disk` cases of `--compliance`: what an operating system's disk driver reads when it finds a
//! disk (Windows storport/classpnp/disk.sys, macOS IOSCSIBlockCommandsDevice, Linux sd): standard
//! INQUIRY and its VPD pages, READ CAPACITY (10) and (16), the mode pages (Caching, Control, Control
//! Extension, Informational Exceptions Control, Read-Write Error Recovery) through MODE SENSE (10) and
//! (6), a MODE SELECT that changes nothing, REPORT SUPPORTED OPERATION CODES, REPORT SUPPORTED TASK
//! MANAGEMENT FUNCTIONS, REQUEST SENSE in both sense formats, START STOP UNIT and LOG SENSE. Every
//! case cites the SPC-5 or SBC-5 clause it checks. What it reads is returned as `Disk`, for the
//! sections that judge data commands, provisioning, feature sets and Windows.

use super::capacity_cases::capacity;
use super::inquiry_cases::inquiry;
use super::mode_page_cases::mode_pages;
use super::operation_code_cases::operation_codes;
use super::operation_codes::Supported;
use super::scsi_fields::{be16, be32, be64};
use super::seen::Seen;
use super::session::Session;
use super::unit_state_cases;
use super::vpd_pages::parse_feature_sets;
use crate::compliance::{Case, ScsiLevel};

pub(super) const SECTION: &str = "disk";

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

/// Runs every disk case and returns what it read.
pub(super) fn run(
    s: &mut Session,
    name: &str,
    levels: &[ScsiLevel],
    cases: &mut Vec<Case>,
    seen: &mut Seen,
) -> Disk {
    let _first = cases.len();
    let mut disk = Disk::default();
    inquiry(s, name, levels, cases, seen, &mut disk);
    capacity(s, name, cases, seen, &mut disk);
    mode_pages(s, name, cases, seen, &mut disk);
    operation_codes(s, name, cases, seen, &mut disk);
    unit_state_cases::run(s, name, cases, seen, &mut disk);
    disk
}

pub(super) fn label(name: &str, what: &str) -> String {
    format!("{name} {what}")
}

pub(super) fn hex_list(v: &[u8]) -> String {
    v.iter()
        .map(|b| format!("{b:02x}h"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

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
