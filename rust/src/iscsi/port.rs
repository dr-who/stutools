//! One I_T nexus to the logical unit under test, as the SCSI sections of `--compliance` use it: an
//! iSCSI session, or a local disk's path device through SG_IO. Every section sends commands through
//! it alike; what only iSCSI has (task management, the negotiated digests, CmdSN) answers an error,
//! or nothing, on a local disk, and the sections that need it are not run there.

use super::commands::Commands;
use super::local_lun::LocalLun;
use super::login::Negotiated;
use super::session::{Outcome, Session, TaskAnswer};
use crate::Result;

pub enum Port {
    Iscsi(Session),
    Local(LocalLun),
}

impl Port {
    pub fn command(&mut self, cdb: &[u8], read: u32, write: Option<&[u8]>) -> Result<Outcome> {
        match self {
            Port::Iscsi(s) => s.command(cdb, read, write),
            Port::Local(l) => l.command(cdb, read, write),
        }
    }

    /// Ends the nexus: an iSCSI logout, or closing the device.
    pub fn logout(self) -> Result<()> {
        match self {
            Port::Iscsi(s) => s.logout(),
            Port::Local(_) => Ok(()),
        }
    }

    /// An iSCSI task management function (RFC 7143 11.5); a local disk has none here.
    pub fn task_management(
        &mut self,
        function: u8,
        referenced: u32,
        ref_cmd_sn: u32,
    ) -> Result<TaskAnswer> {
        match self {
            Port::Iscsi(s) => s.task_management(function, referenced, ref_cmd_sn),
            Port::Local(_) => Err("no task management on a local disk".into()),
        }
    }

    /// LOGICAL UNIT RESET of this nexus's logical unit (SAM-6 7.6): the iSCSI task management function,
    /// or SG_SCSI_RESET through the host adapter on a local disk. Fails unless the function completed.
    pub fn logical_unit_reset(&mut self) -> Result<()> {
        match self {
            Port::Iscsi(s) => match s.task_management(super::pdu::TMF_LOGICAL_UNIT_RESET, 0, 0)? {
                TaskAnswer::Response(0) => Ok(()),
                other => Err(format!("LOGICAL UNIT RESET answered {other:?}")),
            },
            Port::Local(l) => l.reset(),
        }
    }

    /// The CmdSN the next iSCSI command carries; 0 on a local disk.
    pub fn cmd_sn(&self) -> u32 {
        match self {
            Port::Iscsi(s) => s.cmd_sn(),
            Port::Local(_) => 0,
        }
    }

    /// The most bytes one command can move, when the transport limits it (SG_IO); iSCSI does not.
    pub fn max_transfer(&self) -> Option<u32> {
        match self {
            Port::Iscsi(_) => None,
            Port::Local(l) => Some(l.max_transfer()),
        }
    }

    /// What the iSCSI login negotiated.
    pub fn negotiated(&self) -> Option<&Negotiated> {
        match self {
            Port::Iscsi(s) => Some(&s.negotiated),
            Port::Local(_) => None,
        }
    }
}

impl Commands for Port {
    fn command(&mut self, cdb: &[u8], read: u32, write: Option<&[u8]>) -> Result<Outcome> {
        Port::command(self, cdb, read, write)
    }
}
