// Windows volumes are device handles, not ordinary files: file metadata APIs
// reject them with ERROR_INVALID_PARAMETER. Query geometry with disk IOCTLs.
// Keep this small and dependency-free; all calls here happen before measurement.
use std::{ffi::c_void, fs::File, io, os::windows::io::AsRawHandle, path::Path};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn DeviceIoControl(
        handle: *mut c_void,
        code: u32,
        input: *const c_void,
        input_len: u32,
        output: *mut c_void,
        output_len: u32,
        returned: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn GetVolumeInformationW(
        root: *const u16,
        name: *mut u16,
        name_len: u32,
        serial: *mut u32,
        max_component: *mut u32,
        flags: *mut u32,
        filesystem: *mut u16,
        filesystem_len: u32,
    ) -> i32;
}

fn drive(path: &Path) -> Option<char> {
    let s = path.as_os_str().to_str()?;
    let s = s.strip_prefix(r"\\.\").unwrap_or(s);
    let b = s.as_bytes();
    (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':')
        .then(|| (b[0] as char).to_ascii_uppercase())
}

pub fn is_device(path: &Path) -> bool {
    path.as_os_str()
        .to_str()
        .is_some_and(|s| s.starts_with(r"\\.\"))
}

fn filesystem(drive: char) -> Option<String> {
    let root: Vec<u16> = format!("{drive}:\\")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut fs = [0u16; 256];
    // Output pointers are optional apart from the requested filesystem name.
    let ok = unsafe {
        GetVolumeInformationW(
            root.as_ptr(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            fs.as_mut_ptr(),
            fs.len() as u32,
        )
    };
    (ok != 0).then(|| {
        String::from_utf16_lossy(&fs[..fs.iter().position(|n| *n == 0).unwrap_or(fs.len())])
    })
}

pub fn validate_target(path: &Path, write: bool) -> crate::Result<()> {
    let text = path.as_os_str().to_string_lossy();
    if is_device(path) {
        if write {
            return Err(format!("{}: raw Windows targets currently support read-only workloads (-c rs0); for filesystem writes use -F DRIVE:\\spit-test -G 1MiB -c ws1x1", path.display()));
        }
    } else if let Some(d) = drive(path) {
        if text.len() == 2 || (text.len() == 3 && (text.ends_with('\\') || text.ends_with('/'))) {
            let fs = filesystem(d).unwrap_or_else(|| "unknown".into());
            return Err(format!(
                "{} is a filesystem root ({fs}), not a test file. -F takes a file prefix: create disposable files with -F {d}:\\spit-test -G 1MiB -c ws1x1, then read with -F {d}:\\spit-test -G 1MiB -c rs0. For raw volume reads use -f \\\\.\\{d}: -c rs0 (Administrator required); the volume contains a filesystem. Virtual filesystems may not expose a raw block device", path.display()));
        }
    }
    Ok(())
}

fn ioctl(file: &File, code: u32, output: &mut [u8]) -> io::Result<usize> {
    let mut returned = 0;
    let ok = unsafe {
        DeviceIoControl(
            file.as_raw_handle(),
            code,
            std::ptr::null(),
            0,
            output.as_mut_ptr().cast(),
            output.len() as u32,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(returned as usize)
    }
}

pub fn inspect(path: &Path, file: &File) -> crate::Result<super::TargetInfo> {
    let describe = drive(path).and_then(filesystem);
    let hint = match (drive(path), &describe) {
        (Some(d), Some(fs)) => format!("{d}: has a {fs} filesystem; if it is virtual, it may not expose a raw block device. For a file test use -F {d}:\\spit-test -G 1MiB -c ws1x1, then -c rs0"),
        _ => "the target must expose a Windows disk/volume device; use -F PREFIX for filesystem file tests".into(),
    };
    let mut length = [0u8; 8];
    let n = ioctl(file, 0x7405c, &mut length).map_err(|e| format!(
        "query raw volume size (DeviceIoControl IOCTL_DISK_GET_LENGTH_INFO) for {}: {e}; {hint}", path.display()))?;
    let length = i64::from_le_bytes(length);
    if n != 8 || length <= 0 {
        return Err(format!(
            "query raw volume size for {}: invalid disk length {length} ({n} bytes returned)",
            path.display()
        ));
    }
    let mut geometry = [0u8; 24];
    let n = ioctl(file, 0x70000, &mut geometry).map_err(|e| format!(
        "query raw sector size (DeviceIoControl IOCTL_DISK_GET_DRIVE_GEOMETRY) for {}: {e}; {hint}", path.display()))?;
    let sector = u32::from_le_bytes(geometry[20..24].try_into().unwrap()) as usize;
    if n < 24 || sector == 0 || !sector.is_power_of_two() || sector > 4096 {
        return Err(format!("{}: unsupported raw sector size {sector}; fixed buffers support power-of-two sectors up to 4096 bytes", path.display()));
    }
    if let Some(fs) = describe {
        eprintln!("*info* {} contains a {fs} filesystem; raw benchmark is read-only (sector size {sector} bytes)", path.display());
    } else {
        eprintln!(
            "*info* {}: raw benchmark is read-only (sector size {sector} bytes)",
            path.display()
        );
    }
    Ok(super::TargetInfo {
        length: Some(length as u64),
        regular: false,
        sector: Some(sector),
    })
}

pub fn prepare_handle(path: &Path, file: &File) -> io::Result<()> {
    // This flag belongs to the measured handle, not the discarded probe handle.
    // It permits reading the final sectors without changing volume contents.
    if is_device(path) && drive(path).is_some() {
        ioctl(file, 0x90083, &mut []).map_err(|e| {
            io::Error::new(
                e.kind(),
                format!("enable reads of all volume sectors (FSCTL_ALLOW_EXTENDED_DASD_IO): {e}"),
            )
        })?;
    }
    Ok(())
}
