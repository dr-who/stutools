use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
const MAGIC: &[u8; 8] = b"SPITCAM1";
/// Direct I/O needs buffers, offsets and lengths aligned to the device sector size.
const ALIGN: usize = 4096;

/// Zeroed buffer whose data starts on an `ALIGN` boundary.
pub struct Aligned {
    raw: Vec<u8>,
    start: usize,
    len: usize,
}
impl Aligned {
    pub fn new(len: usize) -> Self {
        let raw = vec![0; len + ALIGN];
        let start = raw.as_ptr().align_offset(ALIGN);
        Self { raw, start, len }
    }
}
impl Deref for Aligned {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.raw[self.start..self.start + self.len]
    }
}
impl DerefMut for Aligned {
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.raw[self.start..self.start + self.len]
    }
}

pub fn now() -> io::Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_secs())
}
pub fn run_id() -> io::Result<u128> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos()
        ^ ((std::process::id() as u128) << 96))
}
fn leap(y: u64) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}
pub fn utc(seconds: u64, iso: bool) -> String {
    let mut days = seconds / 86400;
    let mut year = 1970;
    while days >= if leap(year) { 366 } else { 365 } {
        days -= if leap(year) { 366 } else { 365 };
        year += 1;
    }
    let lengths = [
        31,
        if leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 0;
    while days >= lengths[month] {
        days -= lengths[month];
        month += 1;
    }
    let (hour, minute, second) = (seconds % 86400 / 3600, seconds % 3600 / 60, seconds % 60);
    if iso {
        format!(
            "{year:04}-{:02}-{:02}T{hour:02}:{minute:02}:{second:02}Z",
            month + 1,
            days + 1
        )
    } else {
        format!(
            "{year:04}{:02}{:02}T{hour:02}{minute:02}{second:02}Z",
            month + 1,
            days + 1
        )
    }
}
fn valid_stamp(s: &str) -> bool {
    if !s.is_ascii()
        || s.len() != 16
        || &s[8..9] != "T"
        || &s[15..] != "Z"
        || !s[..8]
            .bytes()
            .chain(s[9..15].bytes())
            .all(|b| b.is_ascii_digit())
    {
        return false;
    }
    let n = |a, b| s[a..b].parse::<u32>().unwrap();
    let y = n(0, 4);
    let m = n(4, 6);
    let d = n(6, 8);
    let lengths = [
        31,
        if leap(y as u64) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    y >= 1970
        && (1..=12).contains(&m)
        && d >= 1
        && d <= lengths[(m - 1) as usize]
        && n(9, 11) < 24
        && n(11, 13) < 60
        && n(13, 15) < 60
}
fn header(owner: u128, camera: usize, run: u128, sequence: u64) -> [u8; 64] {
    let mut h = [0; 64];
    h[..8].copy_from_slice(MAGIC);
    h[8..24].copy_from_slice(&owner.to_le_bytes());
    h[24..32].copy_from_slice(&(camera as u64).to_le_bytes());
    h[32..48].copy_from_slice(&run.to_le_bytes());
    h[48..56].copy_from_slice(&sequence.to_le_bytes());
    h
}

/// Volume facts printed at startup, so the target's size and free space are on record.
pub struct VolumeInfo {
    pub source: String,
    pub fs_type: String,
    pub total: u64,
    pub free: u64,
}

// The casts keep the statfs fields portable (their widths differ between Unix platforms). Only Windows and Linux
// report volume information; elsewhere the parameter goes unused.
#[allow(clippy::unnecessary_cast, unused_variables)]
pub fn volume_info(directory: &Path) -> io::Result<VolumeInfo> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let wide = |p: &Path| -> Vec<u16> { p.as_os_str().encode_wide().chain(Some(0)).collect() };
        let root = wide(directory.ancestors().last().unwrap_or(directory));
        let (mut caller_free, mut total, mut free) = (0u64, 0u64, 0u64);
        let (mut serial, mut max_len, mut flags) = (0u32, 0u32, 0u32);
        let mut label = [0u16; 261];
        let mut fs_name = [0u16; 261];
        unsafe extern "system" {
            fn GetDiskFreeSpaceExW(
                dir: *const u16,
                caller: *mut u64,
                total: *mut u64,
                free: *mut u64,
            ) -> i32;
            fn GetVolumeInformationW(
                root: *const u16,
                label: *mut u16,
                label_len: u32,
                serial: *mut u32,
                max_len: *mut u32,
                flags: *mut u32,
                fs: *mut u16,
                fs_len: u32,
            ) -> i32;
        }
        if unsafe {
            GetDiskFreeSpaceExW(
                wide(directory).as_ptr(),
                &mut caller_free,
                &mut total,
                &mut free,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        if unsafe {
            GetVolumeInformationW(
                root.as_ptr(),
                label.as_mut_ptr(),
                261,
                &mut serial,
                &mut max_len,
                &mut flags,
                fs_name.as_mut_ptr(),
                261,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let text = |b: &[u16]| {
            String::from_utf16_lossy(&b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())])
        };
        return Ok(VolumeInfo {
            source: text(&label),
            fs_type: text(&fs_name),
            total,
            free: caller_free,
        });
    }
    #[cfg(target_os = "linux")]
    {
        use std::{ffi::CString, os::unix::ffi::OsStrExt};
        let path = CString::new(directory.as_os_str().as_bytes()).map_err(io::Error::other)?;
        let mut vfs: libc::statvfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statvfs(path.as_ptr(), &mut vfs) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let mut fs: libc::statfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statfs(path.as_ptr(), &mut fs) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let fs_type = match fs.f_type as u64 {
            0xEF53 => "ext2/3/4",
            0x5846_5342 => "xfs",
            0x9123_683E => "btrfs",
            0x0102_1994 => "tmpfs",
            0x6969 => "nfs",
            0x794C_7630 => "overlayfs",
            _ => "other",
        };
        return Ok(VolumeInfo {
            source: mount_source(directory),
            fs_type: fs_type.into(),
            total: vfs.f_blocks as u64 * vfs.f_frsize as u64,
            free: vfs.f_bavail as u64 * vfs.f_frsize as u64,
        });
    }
    #[allow(unreachable_code)]
    Err(io::Error::other(
        "volume information is not available on this platform",
    ))
}

/// The device mounted at the longest mount point that contains `directory`.
#[cfg(target_os = "linux")]
fn mount_source(directory: &Path) -> String {
    let dir = fs::canonicalize(directory).unwrap_or_else(|_| directory.to_path_buf());
    let Ok(mounts) = fs::read_to_string("/proc/self/mounts") else {
        return "unknown".into();
    };
    mounts
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let (source, mount) = (fields.next()?, fields.next()?);
            dir.starts_with(mount)
                .then(|| (mount.len(), source.to_string()))
        })
        .max_by_key(|(len, _)| *len)
        .map_or_else(|| "unknown".into(), |(_, source)| source)
}

pub struct Namespace {
    pub directory: PathBuf,
    pub prefix: String,
    pub direct: bool,
    pub owner: u128,
    pub run: u128,
    _lock: File,
}
impl Namespace {
    pub fn open(prefix: &Path, run: u128, direct: bool) -> io::Result<Arc<Self>> {
        if direct && !cfg!(any(windows, target_os = "linux")) {
            return Err(io::Error::other(
                "--cctv-io direct is supported on Linux and Windows",
            ));
        }
        let name = prefix
            .file_name()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| io::Error::other("-F needs DIRECTORY/PREFIX, not a directory root"))?
            .to_owned();
        #[cfg(windows)]
        if prefix.to_string_lossy().starts_with(r"\\.\") {
            return Err(io::Error::other(
                "CCTV needs a filesystem file prefix, not a raw device",
            ));
        }
        let parent = prefix
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent)?;
        let directory = parent.canonicalize()?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(0);
        }
        let lock = options
            .open(directory.join(format!("{name}.cctv.lock")))
            .map_err(|e| {
                io::Error::new(
                    e.kind(),
                    format!("lock CCTV prefix {name}: {e}; another run may own this prefix"),
                )
            })?;
        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            unsafe extern "C" {
                fn flock(fd: i32, operation: i32) -> i32;
            }
            if unsafe { flock(lock.as_raw_fd(), 2 | 4) } != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "another CCTV run owns this prefix",
                ));
            }
        }
        let manifest = directory.join(format!("{name}.cctv.owner"));
        let owner = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&manifest)
        {
            Ok(mut f) => {
                writeln!(f, "SPIT-CCTV-OWNER-1 {run:032x}")?;
                f.sync_data()?;
                run
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                if !fs::symlink_metadata(&manifest)?.file_type().is_file() {
                    return Err(io::Error::other(
                        "CCTV ownership file must be a regular file",
                    ));
                }
                let mut bytes = [0u8; 128];
                let n = File::open(&manifest)?.read(&mut bytes)?;
                let value = std::str::from_utf8(&bytes[..n])
                    .ok()
                    .and_then(|s| s.trim_end().strip_prefix("SPIT-CCTV-OWNER-1 "))
                    .filter(|s| s.len() == 32)
                    .and_then(|s| u128::from_str_radix(s, 16).ok());
                value.ok_or_else(|| {
                    io::Error::other("invalid CCTV ownership marker; choose another prefix")
                })?
            }
            Err(e) => return Err(e),
        };
        Ok(Arc::new(Self {
            directory,
            prefix: name,
            direct,
            owner,
            run,
            _lock: lock,
        }))
    }
    fn name(&self, camera: usize, sequence: u64, stamp: u64, pending: bool) -> PathBuf {
        self.directory.join(format!(
            "{}.cam{camera:06}.{}.r{:032x}.s{sequence:010}.spitcam{}",
            self.prefix,
            utc(stamp, false),
            self.run,
            if pending { ".pending" } else { "" }
        ))
    }
    pub fn prepare(
        self: &Arc<Self>,
        camera: usize,
        sequence: u64,
        size: u64,
        fallback_buffer: &[u8],
    ) -> io::Result<Prepared> {
        let path = self.name(camera, sequence, now()?, true);
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        if self.direct {
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt;
                options.custom_flags(0x2000_0000); // FILE_FLAG_NO_BUFFERING
            }
            #[cfg(target_os = "linux")]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.custom_flags(libc::O_DIRECT);
            }
        }
        let file = options.open(&path)?;
        let mut prepared = Prepared {
            file: Some(file),
            path,
            namespace: self.clone(),
            camera,
            sequence,
            head: header(self.owner, camera, self.run, sequence),
            used: false,
            fallback: false,
        };
        let file = prepared.file.as_mut().unwrap();
        let native = reserve(file, size)?;
        if !native {
            file.seek(SeekFrom::Start(0))?;
            let mut left = size;
            while left > 0 {
                let n = (left as usize).min(fallback_buffer.len());
                file.write_all(&fallback_buffer[..n])?;
                left -= n as u64;
            }
            prepared.fallback = true;
        }
        file.set_len(size)?;
        file.seek(SeekFrom::Start(0))?;
        // Direct writes must be whole aligned blocks, so the camera writes the header
        // as part of its first block instead of here.
        if !self.direct {
            file.write_all(&prepared.head)?;
            file.seek(SeekFrom::Start(0))?;
        }
        Ok(prepared)
    }
    pub fn prune(&self, cutoff: u64) -> io::Result<u64> {
        let cutoff_seconds = cutoff;
        let cutoff = utc(cutoff, false);
        let prefix = format!("{}.cam", self.prefix);
        let mut deleted = 0;
        for entry in fs::read_dir(&self.directory)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let Some(rest) = name.strip_prefix(&prefix) else {
                continue;
            };
            // Fixed-size parsing without collecting directory entries or names.
            let mut fields = rest.split('.');
            let (Some(camera), Some(stamp), Some(run), Some(sequence), Some(extension)) = (
                fields.next(),
                fields.next(),
                fields.next(),
                fields.next(),
                fields.next(),
            ) else {
                continue;
            };
            if camera.len() != 6
                || !camera.bytes().all(|b| b.is_ascii_digit())
                || !valid_stamp(stamp)
                || stamp >= cutoff.as_str()
                || run.len() != 33
                || !run.starts_with('r')
                || sequence.len() != 11
                || !sequence.starts_with('s')
                || !sequence[1..].bytes().all(|b| b.is_ascii_digit())
                || extension != "spitcam"
            {
                continue;
            }
            let pending = match fields.next() {
                None => false,
                Some("pending") if fields.next().is_none() => true,
                _ => continue,
            };
            let (Ok(camera), Ok(run), Ok(sequence)) = (
                camera.parse::<usize>(),
                u128::from_str_radix(&run[1..], 16),
                sequence[1..].parse::<u64>(),
            ) else {
                continue;
            };
            if camera == 0
                || camera > 1024
                || (pending && run == self.run)
                || !entry.file_type()?.is_file()
            {
                continue;
            }
            // A crashed run's reservation may have received recent writes long
            // after it was prepared. Preserve those bytes for the full window.
            if pending
                && entry
                    .metadata()?
                    .modified()?
                    .duration_since(UNIX_EPOCH)
                    .map_or(true, |age| age.as_secs() >= cutoff_seconds)
            {
                continue;
            }
            let mut h = [0; 64];
            let path = entry.path();
            let mut file = match File::open(&path) {
                Ok(f) => f,
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e),
            };
            if file.read_exact(&mut h).is_err() || h != header(self.owner, camera, run, sequence) {
                continue;
            }
            drop(file);
            fs::remove_file(path)?;
            deleted += 1;
        }
        Ok(deleted)
    }
}
pub struct Prepared {
    pub file: Option<File>,
    path: PathBuf,
    namespace: Arc<Namespace>,
    camera: usize,
    pub sequence: u64,
    pub head: [u8; 64],
    pub used: bool,
    pub fallback: bool,
}
impl Prepared {
    pub fn finish(mut self, written: u64) -> io::Result<()> {
        let file = self.file.take().unwrap();
        if written == 0 {
            drop(file);
            fs::remove_file(&self.path)?;
            self.path = PathBuf::new();
            return Ok(());
        }
        file.sync_data()?;
        drop(file);
        let target = self
            .namespace
            .name(self.camera, self.sequence, now()?, false);
        if target.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "CCTV recording collision; refusing overwrite",
            ));
        }
        fs::rename(&self.path, target)?;
        self.path = PathBuf::new();
        Ok(())
    }
}
impl Drop for Prepared {
    fn drop(&mut self) {
        drop(self.file.take());
        if !self.used && !self.path.as_os_str().is_empty() {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn reserve(file: &mut File, size: u64) -> io::Result<bool> {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::io::AsRawFd;
        loop {
            if unsafe { libc::fallocate(file.as_raw_fd(), 0, 0, size as i64) } == 0 {
                return Ok(true);
            }
            let e = io::Error::last_os_error();
            if e.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            if matches!(
                e.raw_os_error(),
                Some(libc::EOPNOTSUPP) | Some(libc::ENOSYS) | Some(libc::EINVAL)
            ) {
                return Ok(false);
            }
            return Err(e);
        }
    }
    #[cfg(windows)]
    {
        use std::{ffi::c_void, os::windows::io::AsRawHandle};
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn SetFileInformationByHandle(
                handle: *mut c_void,
                class: i32,
                info: *const c_void,
                size: u32,
            ) -> i32;
        }
        let size = size as i64;
        if unsafe {
            SetFileInformationByHandle(file.as_raw_handle(), 5, (&size as *const i64).cast(), 8)
        } != 0
        {
            return Ok(true);
        }
        let e = io::Error::last_os_error();
        if matches!(e.raw_os_error(), Some(1 | 50 | 87)) {
            Ok(false)
        } else {
            Err(e)
        }
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = (file, size);
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn utc_dates_and_leap_days_are_portable() {
        assert_eq!(utc(0, false), "19700101T000000Z");
        assert_eq!(utc(951782400, true), "2000-02-29T00:00:00Z");
        assert!(valid_stamp("20240229T235959Z"));
        for bad in [
            "20230229T000000Z",
            "20260100T000000Z",
            "20260101T246000Z",
            "2026é101T00000Z",
        ] {
            assert!(!valid_stamp(bad));
        }
    }
    #[test]
    fn retention_requires_exact_prefix_header_and_expired_closed_file() {
        let dir = std::env::temp_dir().join(format!("spit-cctv-retention-{}", run_id().unwrap()));
        fs::create_dir(&dir).unwrap();
        let ns = Namespace::open(&dir.join("camera"), 1, false).unwrap();
        let bytes = [0; 65536];
        let mut p = ns.prepare(1, 1, 65536, &bytes).unwrap();
        p.used = true;
        let old = ns.name(1, 1, 1, false);
        drop(p.file.take());
        fs::rename(&p.path, &old).unwrap();
        p.path = PathBuf::new();
        let foreign = ns.name(2, 1, 1, false);
        fs::write(&foreign, [0; 64]).unwrap();
        let log = dir.join("cctv-test-old.hour");
        fs::write(&log, "old log").unwrap();
        let pending = ns.prepare(3, 1, 65536, &bytes).unwrap();
        assert_eq!(ns.prune(now().unwrap() + 10).unwrap(), 1);
        assert!(!old.exists());
        assert!(foreign.exists());
        assert!(pending.path.exists());
        assert!(log.exists());
        drop(pending);
        drop(p);
        drop(ns);
        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(any(target_os = "linux", windows))]
    #[test]
    fn prepared_files_have_real_allocation_not_only_a_sparse_length() {
        let run = run_id().unwrap();
        let dir = std::env::temp_dir().join(format!("spit-cctv-reserve-{run}"));
        let ns = Namespace::open(&dir.join("camera"), run, false).unwrap();
        let p = ns.prepare(1, 1, 1048576, &[0; 65536]).unwrap();
        let f = p.file.as_ref().unwrap();
        assert_eq!(f.metadata().unwrap().len(), 1048576);
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::MetadataExt;
            assert!(f.metadata().unwrap().blocks() * 512 >= 1048576);
        }
        #[cfg(windows)]
        {
            use std::{ffi::c_void, os::windows::io::AsRawHandle};
            #[link(name = "kernel32")]
            unsafe extern "system" {
                fn GetFileInformationByHandleEx(
                    handle: *mut c_void,
                    class: i32,
                    info: *mut c_void,
                    size: u32,
                ) -> i32;
            }
            let mut info = [0i64; 3];
            assert_ne!(
                unsafe {
                    GetFileInformationByHandleEx(f.as_raw_handle(), 1, info.as_mut_ptr().cast(), 24)
                },
                0,
                "{}",
                io::Error::last_os_error()
            );
            assert!(info[0] >= 1048576, "allocation={}", info[0]);
            assert_eq!(info[1], 1048576);
        }
        drop(p);
        drop(ns);
        fs::remove_dir_all(dir).unwrap();
    }
}
