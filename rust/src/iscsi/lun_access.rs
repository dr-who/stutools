//! How the SCSI sections reach the LUN under test: log in as a given initiator port (an ISID is one
//! I_T nexus) offering CRC32C digests or none, name the target, read VPD pages, read and write the
//! test block (the data cases work on the last two blocks and write back what was there), fill a
//! block with a pattern, and drain and show the unit attentions pending for a nexus.

use super::cases::sense;
use super::login;
use super::ops::CHECK_CONDITION;
use super::path::Path;
use super::scsi;
use super::session::{Outcome, Session};
use crate::Result;

/// Credentials for one target: the CHAP user and secret, when the target requires CHAP.
pub(super) type Creds<'a> = Option<(&'a str, &'a [u8])>;

pub(super) const DIGEST: &str = "CRC32C,None";

pub(super) const CHAP_OFFER: &str = "5,6,7,8";

/// The LUN a section tests and whom it logs in as: the path, the initiator name and the CHAP
/// credentials. Sections that open sessions of their own take this rather than the three apart.
#[derive(Clone, Copy)]
pub(super) struct Lun<'a> {
    pub(super) path: &'a Path,
    pub(super) initiator: &'a str,
    pub(super) creds: Creds<'a>,
}

/// Logs in to the target as the initiator port `isid` (one I_T nexus per ISID), offering CRC32C digests.
pub(super) fn open(path: &Path, initiator: &str, creds: Creds, isid: [u8; 6]) -> Result<Session> {
    open_with(path, initiator, creds, isid, DIGEST)
}

/// `open`, offering `digest` for HeaderDigest and DataDigest.
pub(super) fn open_with(
    path: &Path,
    initiator: &str,
    creds: Creds,
    isid: [u8; 6],
    digest: &str,
) -> Result<Session> {
    login::login_as(
        &path.portal,
        &path.target,
        initiator,
        isid,
        creds,
        digest,
        CHAP_OFFER,
    )
}

/// The short name of a target: the part after the last colon of its IQN.
pub(super) fn short_name(path: &Path) -> String {
    path.target
        .rsplit(':')
        .next()
        .unwrap_or(&path.target)
        .to_string()
}

/// INQUIRY of one VPD page with `len` bytes of allocation.
pub(super) fn vpd(s: &mut Session, page: u8, len: u16) -> Result<Outcome> {
    let l = len.to_be_bytes();
    s.command(&[0x12, 0x01, page, l[0], l[1], 0], len as u32, None)
}

/// The last LBA of the LUN and its block length. The data cases work on the last two blocks.
pub(super) fn last_block(s: &mut Session) -> Result<(u32, usize)> {
    let (last, block_len) = scsi::capacity(s)?;
    let lba = u32::try_from(last).map_err(|_| "LUN is larger than 32-bit LBAs")?;
    Ok((lba, block_len as usize))
}

/// READ(10) of one block, as an outcome.
pub(super) fn read_cmd(s: &mut Session, lba: u32, len: usize) -> Result<Outcome> {
    s.command(&scsi::block_cdb(0x28, lba, 1), len as u32, None)
}

/// WRITE(10) of one block, as an outcome.
pub(super) fn write_cmd(s: &mut Session, lba: u32, data: &[u8]) -> Result<Outcome> {
    s.command(&scsi::block_cdb(0x2a, lba, 1), 0, Some(data))
}

/// READ(10) of one block, which must succeed.
pub(super) fn read_block(s: &mut Session, lba: u32, len: usize) -> Result<Vec<u8>> {
    let o = read_cmd(s, lba, len)?;
    scsi::check_status("READ(10)", o.status, &o.sense)?;
    Ok(o.data)
}

/// WRITE(10) of one block, which must succeed.
pub(super) fn write_block(s: &mut Session, lba: u32, data: &[u8]) -> Result<()> {
    let o = write_cmd(s, lba, data)?;
    scsi::check_status("WRITE(10)", o.status, &o.sense)
}

/// TEST UNIT READY, as an outcome.
pub(super) fn test_unit_ready(s: &mut Session) -> Result<Outcome> {
    s.command(&[0x00, 0, 0, 0, 0, 0], 0, None)
}

/// Drains the unit attentions pending for this nexus with TEST UNIT READY (SPC-4 allows it under every
/// reservation type) and returns each one's additional sense code and qualifier, in order.
pub(super) fn unit_attentions(s: &mut Session) -> Vec<(u8, u8)> {
    let mut seen = Vec::new();
    for _ in 0..8 {
        match test_unit_ready(s) {
            Ok(o) if o.status == CHECK_CONDITION && sense(&o.sense).0 == 0x6 => {
                let (_, asc, ascq) = sense(&o.sense);
                seen.push((asc, ascq));
            }
            _ => break,
        }
    }
    seen
}

/// The unit attentions as text for the actual column.
pub(super) fn show_attentions(seen: &[(u8, u8)]) -> String {
    if seen.is_empty() {
        return "none".into();
    }
    let all: Vec<String> = seen
        .iter()
        .map(|(a, q)| format!("{a:02x}/{q:02x}"))
        .collect();
    all.join(", ")
}

/// A block of `len` bytes filled with `seed` mixed with each byte's offset, so two patterns never
/// match by accident.
pub(super) fn pattern(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|i| seed ^ (i as u8).wrapping_mul(31))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_differ_by_seed() {
        assert_ne!(pattern(512, 1), pattern(512, 2));
        assert_eq!(pattern(512, 7).len(), 512);
    }
}
