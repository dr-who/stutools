//! Opening and describing the target: the file or device, its size and sector size, and the
//! per-slot handles some backends need.

#[cfg(windows)]
use super::windows;
use super::Native;
use crate::cli::Backend;
#[cfg(target_os = "linux")]
use std::os::unix::io::AsRawFd;
use std::{
    fs::{File, OpenOptions},
    io,
    path::Path,
};

/// One handle per queue slot on Windows, so in-flight requests do not share a file object.
/// Other platforms use the single handle the caller already has.
pub fn open_slots(
    path: &Path,
    write: bool,
    backend: Backend,
    depth: usize,
) -> io::Result<Vec<File>> {
    Native::open_slots(path, write, backend, depth)
}

pub fn open(path: &Path, write: bool, create: bool, backend: Backend) -> io::Result<File> {
    let mut o = OpenOptions::new();
    o.read(true).write(write).create_new(create);
    Native::configure_open(&mut o, path, backend)?;
    let file = o.open(path)?;
    Native::after_open(&file, path, backend)?;
    Ok(file)
}

/// True when direct I/O applies to `path`: regular files and block devices.
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub(crate) fn is_direct_target(path: &Path) -> bool {
    use std::os::unix::fs::FileTypeExt;
    std::fs::metadata(path)
        .map(|m| m.is_file() || m.file_type().is_block_device())
        .unwrap_or(true)
}

pub fn validate_target(path: &Path, write: bool) -> crate::Result<()> {
    Native::check_target(path, write)?;
    if path.is_dir() {
        return Err(format!(
            "{} is a directory, not an I/O file; -F takes a file prefix, e.g. -F \"{}\" -G 1MiB -c ws1x1, then repeat with -c rs0 to read it",
            path.display(), path.join("spit-test").display()
        ));
    }
    Ok(())
}

pub struct TargetInfo {
    pub length: Option<u64>,
    pub regular: bool,
    pub sector: Option<usize>,
}

pub fn inspect(path: &Path, file: &File) -> crate::Result<TargetInfo> {
    #[cfg(windows)]
    if windows::is_device(path) {
        return windows::inspect(path, file);
    }
    let regular = file
        .metadata()
        .map_err(|e| format!("query file metadata for {}: {e}", path.display()))?
        .is_file();
    let length =
        length(file).map_err(|e| format!("query target size for {}: {e}", path.display()))?;
    Ok(TargetInfo {
        length,
        regular,
        sector: None,
    })
}
fn length(file: &File) -> io::Result<Option<u64>> {
    let meta = file.metadata()?;
    if meta.is_file() {
        return Ok(Some(meta.len()));
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::FileTypeExt;
        if meta.file_type().is_block_device() {
            let mut size = 0u64;
            // BLKGETSIZE64 is _IOR(0x12, 114, size_t); the direction bits
            // differ on PowerPC/MIPS. Let libc encode the target's ABI.
            let r =
                unsafe { libc::ioctl(file.as_raw_fd(), libc::_IOR::<usize>(0x12, 114), &mut size) };
            if r < 0 {
                return Err(io::Error::last_os_error());
            }
            return Ok(Some(size));
        }
    }
    Ok(None)
}
