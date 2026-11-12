//! iSCSI as a path transport for the shared multi-path pool: `IscsiFabric` logs in to a portal and reads
//! the LUN's geometry, and a session sends each request as one READ(10) or WRITE(10).

use super::credentials::login_with_fallback;
use super::path::Path;
use super::scsi;
use super::session::Session;
use crate::paths::lanes;
use crate::paths::{Done, Fabric, Geometry, Io, Transport};
use crate::Result;

/// How the pool opens an iSCSI path: the initiator name, and CHAP where a target asks for it.
pub struct IscsiFabric {
    initiator: String,
    chap: Option<(String, Vec<u8>)>,
}

impl IscsiFabric {
    pub fn new(initiator: &str, chap: Option<(&str, &[u8])>) -> Self {
        Self {
            initiator: initiator.to_string(),
            chap: chap.map(|(u, s)| (u.to_string(), s.to_vec())),
        }
    }
}

impl Fabric for IscsiFabric {
    type Path = Path;
    type Session = Session;

    fn label(&self, path: &Path) -> String {
        format!("{} {}", path.portal, path.target)
    }

    fn open(&self, path: &Path) -> Result<(Session, Geometry)> {
        let chap = self.chap.as_ref().map(|(u, s)| (u.as_str(), s.as_slice()));
        let mut session = login_with_fallback(path, &self.initiator, chap, "CRC32C,None")?;
        session.access_tier = access_tier(&mut session)?;
        let (last, block_len) = scsi::capacity(&mut session)?;
        Ok((
            session,
            Geometry {
                block_len,
                capacity_bytes: (last + 1) * block_len as u64,
            },
        ))
    }
}

/// The lane tier of a portal, from the ALUA state of the port group it is in: optimized when the group is
/// active/optimized, non-optimized when it is active/non-optimized or LBA-dependent, and standby otherwise
/// (standby, unavailable, transitioning, offline). A target that does not report ALUA, or a portal in no
/// group, counts as optimized.
fn access_tier(session: &mut Session) -> Result<Option<u8>> {
    let designators = scsi::designators(session)?;
    let Ok(groups) = scsi::port_groups(session) else {
        return Ok(None);
    };
    let own = designators
        .iter()
        .find(|d| d.kind == 4 && d.association == 1)
        .and_then(|d| u16::from_str_radix(&d.value, 16).ok());
    let Some(group) = own.and_then(|port| groups.iter().find(|g| g.ports.contains(&port))) else {
        return Ok(None);
    };
    Ok(Some(match group.state {
        0x0 => lanes::TIER_OPTIMIZED,
        0x1 | 0x4 => lanes::TIER_NON_OPTIMIZED,
        _ => lanes::TIER_STANDBY,
    }))
}

impl Transport for Session {
    fn tier(&self) -> Option<u8> {
        self.access_tier
    }

    fn can_submit(&self) -> bool {
        Session::can_submit(self)
    }

    fn inflight(&self) -> usize {
        Session::inflight(self)
    }

    fn check(&self, io: Io) -> Result<()> {
        u32::try_from(io.lba).map_err(|_| "LBA beyond READ(10) range".to_string())?;
        u16::try_from(io.blocks).map_err(|_| "transfer beyond READ(10) range".to_string())?;
        Ok(())
    }

    unsafe fn submit(&mut self, tag: usize, io: Io, buf: *mut u8, len: usize) -> Result<()> {
        let cdb = scsi::block_cdb(
            if io.write { 0x2a } else { 0x28 },
            io.lba as u32,
            io.blocks as u16,
        );
        // SAFETY: the caller keeps `buf` valid until `poll` returns this tag.
        unsafe { Session::submit(self, &cdb, tag, buf, len, io.write) }
    }

    fn poll(&mut self) -> Result<Done> {
        Session::poll(self)
    }

    fn failure(&self, io: Io, done: &Done) -> String {
        let op = if io.write { "WRITE(10)" } else { "READ(10)" };
        scsi::check_status(op, done.status, &done.sense[..done.sense_len]).unwrap_err()
    }

    fn unavailable(&self, done: &Done) -> String {
        format!("ASC {:02x}/{:02x}", done.sense[12], done.sense[13])
    }
}
