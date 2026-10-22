#[cfg(target_os = "linux")]
use std::fs;
const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;

fn budget(available: Option<u64>) -> u64 {
    available.map_or(512 * MIB, |bytes| (bytes / 2).min(64 * GIB))
}
pub fn default_budget() -> u64 {
    budget(available())
}

#[cfg(target_os = "linux")]
fn available() -> Option<u64> {
    let memory = fs::read_to_string("/proc/meminfo").ok()?;
    let available = memory.lines().find_map(|line| {
        let value = line
            .strip_prefix("MemAvailable:")?
            .split_whitespace()
            .next()?;
        value.parse::<u64>().ok()?.checked_mul(1024)
    })?;
    let mut available = available;
    // Respect a finite cgroup-v2 limit at this group or any ancestor.
    let root = std::path::Path::new("/sys/fs/cgroup");
    if let Ok(groups) = fs::read_to_string("/proc/self/cgroup") {
        if let Some(group) = groups.lines().find_map(|line| line.strip_prefix("0::")) {
            let mut directory = root.join(group.trim_start_matches('/'));
            while directory.starts_with(root) {
                if let (Ok(limit), Ok(used)) = (
                    fs::read_to_string(directory.join("memory.max")),
                    fs::read_to_string(directory.join("memory.current")),
                ) {
                    if let (Ok(limit), Ok(used)) =
                        (limit.trim().parse::<u64>(), used.trim().parse::<u64>())
                    {
                        available = available.min(limit.saturating_sub(used));
                    }
                }
                if directory == root {
                    break;
                }
                directory.pop();
            }
        }
    }
    Some(available)
}
#[cfg(target_os = "macos")]
fn available() -> Option<u64> {
    use std::{
        ffi::c_void,
        os::raw::{c_char, c_int},
    };
    unsafe extern "C" {
        fn sysctlbyname(
            name: *const c_char,
            old: *mut c_void,
            len: *mut usize,
            new: *mut c_void,
            new_len: usize,
        ) -> c_int;
    }
    let mut bytes = 0u64;
    let mut size = std::mem::size_of::<u64>();
    let result = unsafe {
        sysctlbyname(
            c"hw.memsize".as_ptr(),
            (&mut bytes as *mut u64).cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    (result == 0 && size == 8).then_some(bytes)
}
#[cfg(windows)]
fn available() -> Option<u64> {
    #[repr(C)]
    struct Status {
        len: u32,
        load: u32,
        total: u64,
        free: u64,
        total_page: u64,
        free_page: u64,
        total_virtual: u64,
        free_virtual: u64,
        free_extended: u64,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        #[link_name = "GlobalMemoryStatusEx"]
        fn memory_status(status: *mut Status) -> i32;
    }
    let mut status = Status {
        len: std::mem::size_of::<Status>() as u32,
        load: 0,
        total: 0,
        free: 0,
        total_page: 0,
        free_page: 0,
        total_virtual: 0,
        free_virtual: 0,
        free_extended: 0,
    };
    (unsafe { memory_status(&mut status) } != 0).then_some(status.free.min(status.free_virtual))
}
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn available() -> Option<u64> {
    None
}

pub fn error(required: u64, permitted: u64, reason: &str) -> String {
    let (amount, unit) = if required >= GIB {
        (required.div_ceil(GIB), "GiB")
    } else {
        (required.div_ceil(MIB).max(1), "MiB")
    };
    format!("{reason} need {required} bytes; --memory permits {permitted}. To keep these block/queue/worker settings, try --memory {amount}{unit} (or reduce q, j or block size)")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn budget_scales_with_memory_and_has_a_finite_cap() {
        assert_eq!(budget(Some(256 * GIB)), 64 * GIB);
        assert_eq!(budget(Some(2 * GIB)), GIB);
        assert_eq!(budget(Some(64 * MIB)), 32 * MIB);
        assert_eq!(budget(None), 512 * MIB);
        assert!(budget(Some(256 * GIB)) > 8 * GIB + 32 * 64 * 128);
    }
    #[test]
    fn large_pool_error_includes_a_copyable_budget() {
        assert!(error(8 * GIB + 262912, 512 * MIB, "I/O pools").contains("try --memory 9GiB"));
    }
}
