//! Closes a file and reports the error. `File` drop ignores close errors, so a write that fails
//! only at close would otherwise look successful.
use std::{fs::File, io};

/// Closes `file`, returning the OS error if the close fails.
#[cfg(unix)]
pub fn close(file: File) -> io::Result<()> {
    use std::os::unix::io::IntoRawFd;
    unsafe extern "C" {
        fn close(fd: i32) -> i32;
    }
    let fd = file.into_raw_fd();
    // SAFETY: fd was just released from `file`, so this is the only close of it.
    if unsafe { close(fd) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Closes `file`, returning the OS error if the close fails.
#[cfg(windows)]
pub fn close(file: File) -> io::Result<()> {
    use std::os::windows::io::IntoRawHandle;
    unsafe extern "system" {
        fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
    }
    let handle = file.into_raw_handle();
    // SAFETY: handle was just released from `file`, so this is the only close of it.
    if unsafe { CloseHandle(handle) } != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
