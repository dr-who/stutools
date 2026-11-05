//! SCSI block commands sent over an iSCSI session: the INQUIRY pages that identify a LUN, its capacity,
//! and its target port groups (ALUA). Each function returns the decoded fields, not raw bytes.

use super::session::{Outcome, Session};
use crate::Result;

const INQUIRY: u8 = 0x12;
const READ_CAPACITY_16: u8 = 0x9e;
const MAINTENANCE_IN: u8 = 0xa3;
const REPORT_TARGET_PORT_GROUPS: u8 = 0x0a;
const READ_CAPACITY_16_SERVICE: u8 = 0x10;
/// READ CAPACITY(16) with a 32-byte allocation: the length is the 32-bit field at bytes 10 to 13.
const READ_CAPACITY_16_CDB: [u8; 16] = [
    READ_CAPACITY_16,
    READ_CAPACITY_16_SERVICE,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    32,
    0,
    0,
];

/// Runs a command, retrying while the target reports UNIT ATTENTION (sense key 6), which it raises
/// once after a configuration or ALUA change.
fn command(s: &mut Session, cdb: &[u8], read: u32) -> Result<Outcome> {
    for _ in 0..5 {
        let o = s.command(cdb, read, None)?;
        let unit_attention = o.status == 0x02 && o.sense.get(2).is_some_and(|k| k & 0x0f == 0x06);
        if !unit_attention {
            return Ok(o);
        }
    }
    Err("target kept reporting UNIT ATTENTION".into())
}

/// Turns a SCSI status other than GOOD into an error that shows the sense data.
pub fn check_status(operation: &str, status: u8, sense: &[u8]) -> Result<()> {
    if status == 0 {
        return Ok(());
    }
    let sense: String = sense.iter().map(|b| format!("{b:02x}")).collect();
    Err(format!(
        "{operation}: SCSI status {status:#04x}, sense {}",
        if sense.is_empty() {
            "none".into()
        } else {
            sense
        }
    ))
}

/// Turns a SCSI status other than GOOD into an error that shows the sense data.
pub fn check(operation: &str, outcome: &Outcome) -> Result<()> {
    if outcome.status == 0 {
        return Ok(());
    }
    let sense: String = outcome.sense.iter().map(|b| format!("{b:02x}")).collect();
    Err(format!(
        "{operation}: SCSI status {:#04x}, sense {}",
        outcome.status,
        if sense.is_empty() {
            "none".into()
        } else {
            sense
        }
    ))
}

fn be16(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*d.get(at)?, *d.get(at + 1)?]))
}

pub fn be32(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

fn be64(d: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(d.get(at..at + 8)?.try_into().ok()?))
}

fn text(d: &[u8]) -> String {
    String::from_utf8_lossy(d).trim().to_string()
}

fn hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// Standard INQUIRY data: peripheral type, vendor, product and revision.
#[derive(Debug, Clone, PartialEq)]
pub struct Inquiry {
    pub peripheral: u8,
    pub vendor: String,
    pub product: String,
    pub revision: String,
}

pub fn inquiry(s: &mut Session) -> Result<Inquiry> {
    let o = command(s, &[INQUIRY, 0, 0, 0, 96, 0], 96)?;
    check("INQUIRY", &o)?;
    let d = &o.data;
    Ok(Inquiry {
        peripheral: d.first().copied().unwrap_or(0xff) & 0x1f,
        vendor: text(d.get(8..16).unwrap_or(&[])),
        product: text(d.get(16..32).unwrap_or(&[])),
        revision: text(d.get(32..36).unwrap_or(&[])),
    })
}

/// VPD page 0x80: the unit serial number.
pub fn serial(s: &mut Session) -> Result<String> {
    let o = command(s, &[INQUIRY, 0x01, 0x80, 0, 255, 0], 255)?;
    check("INQUIRY VPD 0x80", &o)?;
    let len = o.data.get(3).copied().unwrap_or(0) as usize;
    Ok(text(o.data.get(4..4 + len).unwrap_or(&[])))
}

/// One identification descriptor from VPD page 0x83. `kind` is the designator type (3 is NAA,
/// 1 is T10 vendor ID, 2 is EUI-64, 8 is SCSI name string).
#[derive(Debug, Clone, PartialEq)]
pub struct Designator {
    pub kind: u8,
    /// Who the identifier names: 0 the LUN, 1 the target port that answered, 2 the target device.
    pub association: u8,
    pub value: String,
}

/// VPD page 0x83: every identification descriptor the LUN reports.
pub fn designators(s: &mut Session) -> Result<Vec<Designator>> {
    let o = command(s, &[INQUIRY, 0x01, 0x83, 0, 255, 0], 255)?;
    check("INQUIRY VPD 0x83", &o)?;
    let d = &o.data;
    let total = be16(d, 2).unwrap_or(0) as usize;
    let mut out = Vec::new();
    let mut i = 4;
    while i + 4 <= d.len().min(4 + total) {
        let association = (d[i + 1] >> 4) & 0x3;
        let kind = d[i + 1] & 0x0f;
        let len = d[i + 3] as usize;
        let value = d.get(i + 4..i + 4 + len).map(hex).unwrap_or_default();
        out.push(Designator {
            kind,
            association,
            value,
        });
        i += 4 + len;
    }
    Ok(out)
}

/// READ CAPACITY(16): the last LBA and the logical block length in bytes.
pub fn capacity(s: &mut Session) -> Result<(u64, u32)> {
    let o = command(s, &READ_CAPACITY_16_CDB, 32)?;
    check("READ CAPACITY(16)", &o)?;
    let last = be64(&o.data, 0).ok_or("READ CAPACITY(16) data is short")?;
    let block = be32(&o.data, 8).ok_or("READ CAPACITY(16) data is short")?;
    Ok((last, block))
}

/// Builds a READ(10) or WRITE(10) CDB: a 32-bit LBA and a 16-bit count of blocks.
pub fn block_cdb(opcode: u8, lba: u32, blocks: u16) -> [u8; 10] {
    let l = lba.to_be_bytes();
    let b = blocks.to_be_bytes();
    [opcode, 0, l[0], l[1], l[2], l[3], 0, b[0], b[1], 0]
}

/// READ(10): `blocks` blocks from `lba`. `block_len` is the LUN's block size from READ CAPACITY.
/// The asymmetric access state names (SPC-6 table 298: Eh offline, Fh transitioning).
pub fn state_name(state: u8) -> &'static str {
    match state {
        0x0 => "active/optimized",
        0x1 => "active/non-optimized",
        0x2 => "standby",
        0x3 => "unavailable",
        0x4 => "LBA-dependent",
        0xe => "offline",
        0xf => "transitioning",
        _ => "other",
    }
}

/// A target port group (an ALUA group) with its access state and member port IDs.
#[derive(Debug, Clone, PartialEq)]
pub struct PortGroup {
    pub id: u16,
    pub state: u8,
    pub status: u8,
    pub ports: Vec<u16>,
}

/// REPORT TARGET PORT GROUPS: the ALUA groups this LUN is in, with their access states.
pub fn port_groups(s: &mut Session) -> Result<Vec<PortGroup>> {
    let cdb = [
        MAINTENANCE_IN,
        REPORT_TARGET_PORT_GROUPS,
        0,
        0,
        0,
        0,
        0,
        0,
        4,
        0,
        0,
        0,
    ];
    let o = command(s, &cdb, 1024)?;
    check("REPORT TARGET PORT GROUPS", &o)?;
    let d = &o.data;
    let total = be32(d, 0).unwrap_or(0) as usize;
    let mut groups = Vec::new();
    let mut i = 4;
    while i + 8 <= d.len().min(4 + total) {
        let count = d[i + 7] as usize;
        let ports = (0..count)
            .filter_map(|p| be16(d, i + 8 + 4 * p + 2))
            .collect();
        groups.push(PortGroup {
            id: be16(d, i + 2).unwrap_or(0),
            state: d[i] & 0x0f,
            status: d[i + 5],
            ports,
        });
        i += 8 + 4 * count;
    }
    Ok(groups)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read10_and_write10_cdbs_put_lba_and_count_big_endian() {
        assert_eq!(
            block_cdb(0x28, 0x0102_0304, 0x0506),
            [0x28, 0, 1, 2, 3, 4, 0, 5, 6, 0]
        );
        assert_eq!(block_cdb(0x2a, 7, 1)[0], 0x2a);
    }
}
