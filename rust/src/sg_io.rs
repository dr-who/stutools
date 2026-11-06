//! Linux SG_IO: sends one SCSI command to a block or sg device and returns its status, sense data
//! and how much was not transferred. The caller owns the data buffer; nothing else is allocated but
//! the sense data. Fails with the ioctl's errno; a SCSI status other than GOOD is an answer, not an
//! error. Used by `--info` and by `--compliance` on a local disk.

use std::fs::File;
use std::os::unix::io::AsRawFd;

const SG_IO: libc::c_ulong = 0x2285;
const SG_DXFER_NONE: i32 = -1;
const SG_DXFER_TO_DEV: i32 = -2;
const SG_DXFER_FROM_DEV: i32 = -3;
/// Room for descriptor-format sense data (SPC-6 4.4.2: up to 252 bytes).
const SENSE_LEN: usize = 252;

#[repr(C)]
struct SgIoHdr {
    interface_id: i32,
    dxfer_direction: i32,
    cmd_len: u8,
    mx_sb_len: u8,
    iovec_count: u16,
    dxfer_len: u32,
    dxferp: *mut libc::c_void,
    cmdp: *mut u8,
    sbp: *mut u8,
    timeout: u32,
    flags: u32,
    pack_id: i32,
    usr_ptr: *mut libc::c_void,
    status: u8,
    masked_status: u8,
    msg_status: u8,
    sb_len_wr: u8,
    host_status: u16,
    driver_status: u16,
    resid: i32,
    duration: u32,
    info: u32,
}

pub struct Reply {
    pub status: u8,
    pub host_status: u16,
    pub driver_status: u16,
    pub sense: Vec<u8>,
    /// Bytes the device did not transfer.
    pub resid: i32,
}

/// Sends `cdb`. With `data` non-empty, the command transfers into it (`to_device` false)
/// or out of it (`to_device` true).
pub fn send(
    file: &File,
    cdb: &[u8],
    data: &mut [u8],
    to_device: bool,
) -> std::result::Result<Reply, String> {
    let mut sense = [0u8; SENSE_LEN];
    let direction = if data.is_empty() {
        SG_DXFER_NONE
    } else if to_device {
        SG_DXFER_TO_DEV
    } else {
        SG_DXFER_FROM_DEV
    };
    let mut hdr = SgIoHdr {
        interface_id: b'S' as i32,
        dxfer_direction: direction,
        cmd_len: cdb.len() as u8,
        mx_sb_len: SENSE_LEN as u8,
        iovec_count: 0,
        dxfer_len: data.len() as u32,
        dxferp: if data.is_empty() {
            std::ptr::null_mut()
        } else {
            data.as_mut_ptr() as *mut libc::c_void
        },
        cmdp: cdb.as_ptr() as *mut u8,
        sbp: sense.as_mut_ptr(),
        timeout: 60_000,
        flags: 0,
        pack_id: 0,
        usr_ptr: std::ptr::null_mut(),
        status: 0,
        masked_status: 0,
        msg_status: 0,
        sb_len_wr: 0,
        host_status: 0,
        driver_status: 0,
        resid: 0,
        duration: 0,
        info: 0,
    };
    let rc = unsafe { libc::ioctl(file.as_raw_fd(), SG_IO as _, &mut hdr as *mut SgIoHdr) };
    if rc < 0 {
        return Err(format!("SG_IO: {}", std::io::Error::last_os_error()));
    }
    Ok(Reply {
        status: hdr.status,
        host_status: hdr.host_status,
        driver_status: hdr.driver_status,
        sense: sense[..hdr.sb_len_wr as usize].to_vec(),
        resid: hdr.resid,
    })
}

const SG_SCSI_RESET: libc::c_ulong = 0x2284;
const SG_SCSI_RESET_DEVICE: libc::c_int = 1;

/// Asks the host adapter to reset the logical unit behind `file` (a LOGICAL UNIT RESET, SAM-6 7.8).
/// Needs CAP_SYS_ADMIN; fails with the ioctl's errno.
pub fn reset_device(file: &File) -> std::result::Result<(), String> {
    let mut how = SG_SCSI_RESET_DEVICE;
    let rc = unsafe {
        libc::ioctl(
            file.as_raw_fd(),
            SG_SCSI_RESET as _,
            &mut how as *mut libc::c_int,
        )
    };
    if rc < 0 {
        return Err(format!(
            "SG_SCSI_RESET: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}
