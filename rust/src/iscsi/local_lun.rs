//! A local disk as `--compliance` tests it: one path device (`/dev/sdX`) reached through SG_IO, which
//! is one I_T nexus from this host's initiator port to one of the disk's target ports. Commands go
//! straight to the device, one at a time, with the kernel's SCSI timeout of SG_IO (60 s). Opening
//! fails as the device open does; a command fails only when SG_IO itself does, or the host adapter
//! reports an error (the command never reached the device). One SG_IO carries at most what the
//! device's queue allows: its maximum hardware transfer, and one page per scatter-gather segment.
//! The data buffer is page-aligned, so the kernel maps it rather than bouncing it through a copy
//! that needs one page more than the transfer.

use super::commands::Commands;
use super::session::Outcome;
use crate::Result;
use std::fs::File;

/// The alignment that lets the kernel map the buffer directly.
const PAGE: usize = 4096;

pub struct LocalLun {
    file: File,
    device: String,
    /// The most bytes one SG_IO carries here.
    max_transfer: u32,
}

/// The most bytes one SG_IO to `device` carries: the queue's max_hw_sectors_kb, and max_segments
/// pages. Unknown limits allow 1 MiB.
fn queue_limit(device: &str) -> u32 {
    let name = std::fs::canonicalize(device)
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default();
    let read = |attr: &str| {
        std::fs::read_to_string(format!("/sys/block/{name}/queue/{attr}"))
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
    };
    let page = 4096u64;
    let hw = read("max_hw_sectors_kb").map_or(1 << 20, |kb| kb * 1024);
    let segments = read("max_segments").map_or(u64::MAX, |n| n * page);
    hw.min(segments).min(u64::from(u32::MAX)) as u32
}

impl LocalLun {
    /// Opens `device` for reading and writing.
    pub fn open(device: &str) -> Result<LocalLun> {
        let file = File::options()
            .read(true)
            .write(true)
            .open(device)
            .map_err(|e| format!("opening {device}: {e}"))?;
        Ok(LocalLun {
            file,
            device: device.to_string(),
            max_transfer: queue_limit(device),
        })
    }

    /// LOGICAL UNIT RESET through the host adapter (SG_SCSI_RESET).
    pub fn reset(&self) -> Result<()> {
        crate::sg_io::reset_device(&self.file).map_err(|e| format!("{}: {e}", self.device))
    }

    /// The most bytes one command can move through SG_IO to this device.
    pub fn max_transfer(&self) -> u32 {
        self.max_transfer
    }
}

impl Commands for LocalLun {
    fn command(&mut self, cdb: &[u8], read: u32, write: Option<&[u8]>) -> Result<Outcome> {
        let (len, to_device) = match write {
            Some(w) => (w.len(), true),
            None => (read as usize, false),
        };
        let mut raw = vec![0u8; len + PAGE];
        let at = raw.as_ptr().align_offset(PAGE);
        let buffer = &mut raw[at..at + len];
        if let Some(w) = write {
            buffer.copy_from_slice(w);
        }
        let reply = crate::sg_io::send(&self.file, cdb, buffer, to_device)
            .map_err(|e| format!("{}: {e}", self.device))?;
        let mut data = buffer.to_vec();
        // A host status other than DID_OK means the command never reached the device.
        if reply.host_status != 0 {
            return Err(format!(
                "{}: host status {:#x}, driver status {:#x}",
                self.device, reply.host_status, reply.driver_status
            ));
        }
        let received = if to_device {
            0
        } else {
            data.len().saturating_sub(reply.resid.max(0) as usize)
        };
        if to_device {
            data.clear();
        }
        Ok(Outcome {
            status: reply.status,
            sense: reply.sense,
            data,
            received,
        })
    }
}
