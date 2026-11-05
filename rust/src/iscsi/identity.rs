//! What identifies the LUN behind one path, read after login: standard INQUIRY, VPD 80h and 83h, READ
//! CAPACITY (16), the answering port and REPORT TARGET PORT GROUPS (`collect`). Whether two paths agree is
//! judged on the identity table's rows (`report::identity_differences`).

use super::credentials::login_with_fallback;
use super::login::Negotiated;
use super::path::Path;
use super::scsi::{self, Designator, Inquiry, PortGroup};
use crate::Result;

/// Everything read from one path after a successful login.
#[derive(Debug, Clone)]
pub struct Collected {
    pub login: Negotiated,
    pub inquiry: Inquiry,
    pub serial: String,
    pub designators: Vec<Designator>,
    pub last_lba: u64,
    pub block_len: u32,
    pub groups: Vec<PortGroup>,
    /// The relative target port that answered this path, from its identifier in VPD 0x83.
    pub own_port: Option<u16>,
}

pub fn collect(path: &Path, initiator: &str, chap: Option<(&str, &[u8])>) -> Result<Collected> {
    let mut session = login_with_fallback(path, initiator, chap, "CRC32C,None")?;
    let negotiated = session.negotiated.clone();
    let inquiry = scsi::inquiry(&mut session)?;
    let serial = scsi::serial(&mut session)?;
    let designators = scsi::designators(&mut session)?;
    let (last_lba, block_len) = scsi::capacity(&mut session)?;
    let groups = scsi::port_groups(&mut session)?;
    session.logout()?;
    let own_port = designators
        .iter()
        .find(|d| d.kind == 4 && d.association == 1)
        .and_then(|d| u16::from_str_radix(&d.value, 16).ok());
    Ok(Collected {
        login: negotiated,
        inquiry,
        serial,
        designators,
        last_lba,
        block_len,
        groups,
        own_port,
    })
}
