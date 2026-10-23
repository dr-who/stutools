use std::{
    io,
    sync::atomic::{AtomicBool, Ordering},
};
pub static INTERRUPTED: AtomicBool = AtomicBool::new(false);
#[cfg(unix)]
unsafe extern "C" {
    fn signal(number: i32, handler: usize) -> usize;
}
#[cfg(unix)]
extern "C" fn interrupt(_number: i32) {
    INTERRUPTED.store(true, Ordering::Relaxed);
}
#[cfg(windows)]
extern "system" fn interrupt(kind: u32) -> i32 {
    if matches!(kind, 0 | 1 | 2 | 5 | 6) {
        INTERRUPTED.store(true, Ordering::Relaxed);
        1
    } else {
        0
    }
}
#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn SetConsoleCtrlHandler(handler: Option<extern "system" fn(u32) -> i32>, add: i32) -> i32;
}
pub struct Guard {
    #[cfg(unix)]
    previous: [usize; 2],
    #[cfg(windows)]
    installed: bool,
}
impl Guard {
    pub fn install() -> io::Result<Self> {
        INTERRUPTED.store(false, Ordering::Relaxed);
        #[cfg(unix)]
        {
            let handler = interrupt as *const () as usize;
            let a = unsafe { signal(2, handler) };
            if a == usize::MAX {
                return Err(io::Error::last_os_error());
            }
            let b = unsafe { signal(15, handler) };
            if b == usize::MAX {
                unsafe { signal(2, a) };
                return Err(io::Error::last_os_error());
            }
            Ok(Self { previous: [a, b] })
        }
        #[cfg(windows)]
        {
            let installed = unsafe { SetConsoleCtrlHandler(Some(interrupt), 1) } != 0;
            if !installed {
                let e = io::Error::last_os_error();
                if e.raw_os_error() != Some(6) {
                    return Err(e);
                }
            }
            Ok(Self { installed })
        }
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        #[cfg(unix)]
        unsafe {
            signal(2, self.previous[0]);
            signal(15, self.previous[1]);
        }
        #[cfg(windows)]
        if self.installed {
            unsafe { SetConsoleCtrlHandler(Some(interrupt), 0) };
        }
    }
}
