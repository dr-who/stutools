//! What sends a SCSI command and waits for its answer: an iSCSI session, a local disk through
//! SG_IO (`local_lun`), or either (`port`). The helpers that read a device's identity and capacity
//! (`scsi`, `ops`) take any of them. A transport failure is an error; any SCSI status is an answer.

use super::session::{Outcome, Session};
use crate::Result;

pub trait Commands {
    /// Runs `cdb`, reading up to `read` bytes or writing `write`, and returns status, sense and data.
    fn command(&mut self, cdb: &[u8], read: u32, write: Option<&[u8]>) -> Result<Outcome>;
}

impl Commands for Session {
    fn command(&mut self, cdb: &[u8], read: u32, write: Option<&[u8]>) -> Result<Outcome> {
        Session::command(self, cdb, read, write)
    }
}
