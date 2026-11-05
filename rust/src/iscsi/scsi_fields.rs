//! Reading fields out of SCSI data: big-endian integers of 2, 4 and 8 bytes (zero past the end, so a
//! short answer reads as zeros rather than failing), and text with non-printable bytes shown as \xNN.

use super::scsi;

/// Text safe to print: non-printable bytes as \xNN (a NUL in a serial number would otherwise end up in
/// the report).
pub(super) fn printable(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| {
            if (0x20..0x7f).contains(&b) {
                (b as char).to_string()
            } else {
                format!("\\x{b:02x}")
            }
        })
        .collect()
}

pub(super) fn be16(d: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([
        d.get(at).copied().unwrap_or(0),
        d.get(at + 1).copied().unwrap_or(0),
    ])
}

pub(super) fn be32(d: &[u8], at: usize) -> u32 {
    scsi::be32(d, at).unwrap_or(0)
}

pub(super) fn be64(d: &[u8], at: usize) -> u64 {
    d.get(at..at + 8)
        .and_then(|b| b.try_into().ok())
        .map(u64::from_be_bytes)
        .unwrap_or(0)
}
