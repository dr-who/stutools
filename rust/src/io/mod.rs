use crate::cli::Backend;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod overlapped;
#[cfg(any(target_os = "macos", target_os = "freebsd"))]
mod posix_aio;
#[cfg(not(any(
    target_os = "linux",
    windows,
    target_os = "macos",
    target_os = "freebsd"
)))]
mod unsupported;

// The one backend type each platform provides. The queue never names a platform.
#[cfg(target_os = "linux")]
pub use linux::Native;
#[cfg(windows)]
pub use overlapped::WinAio as Native;
#[cfg(any(target_os = "macos", target_os = "freebsd"))]
pub use posix_aio::PosixAio as Native;
#[cfg(not(any(
    target_os = "linux",
    windows,
    target_os = "macos",
    target_os = "freebsd"
)))]
pub use unsupported::Native;

#[cfg(windows)]
#[path = "../windows.rs"]
mod windows;
#[cfg(target_os = "linux")]
use std::os::unix::io::AsRawFd;
use std::{
    alloc::{alloc_zeroed, dealloc, Layout},
    fs::{File, OpenOptions},
    io,
    path::Path,
    ptr::NonNull,
    time::{Duration, Instant},
};

/// The backend a `--backend auto` run picks on this platform.
pub fn default_backend() -> Backend {
    Native::default_backend()
}

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
pub fn length(file: &File) -> io::Result<Option<u64>> {
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
struct Arena {
    ptr: NonNull<u8>,
    layout: Layout,
}
impl Arena {
    fn new(size: usize) -> io::Result<Self> {
        let layout = Layout::from_size_align(size, 4096)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "pool layout overflow"))?;
        let ptr = NonNull::new(unsafe { alloc_zeroed(layout) })
            .ok_or_else(|| io::Error::from(io::ErrorKind::OutOfMemory))?;
        // Touch every page before the timed loop, including read-only workloads.
        for n in (0..size).step_by(4096) {
            unsafe {
                ptr.as_ptr().add(n).write_volatile(0);
            }
        }
        Ok(Self { ptr, layout })
    }
}
impl Drop for Arena {
    fn drop(&mut self) {
        unsafe {
            dealloc(self.ptr.as_ptr(), self.layout);
        }
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Request {
    pub offset: u64,
    pub len: usize,
    pub write: bool,
    pub verify: bool,
    pub seed: u16,
    // Fits in descriptor padding: 0 free, 1 prepared, 2 owned by the kernel.
    pub state: u8,
}
#[derive(Debug)]
pub enum Failure {
    System(io::Error),
    #[allow(dead_code)] // Constructed by the overlapped and POSIX AIO backends.
    Operation {
        error: io::Error,
        write: bool,
        offset: u64,
        len: usize,
    },
    /// A transfer that finished with a negative errno or fewer bytes than the request asked for.
    Incomplete(i64, Request),
    #[cfg(target_os = "linux")]
    Timeout,
    Verification,
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::System(e) => write!(f, "{e}"),
            Self::Operation { error, write, offset, len } => {
                write!(f, "{} at byte offset {offset}, length {len} bytes: {error}", if *write { "write" } else { "read" })?;
                #[cfg(windows)]
                if error.raw_os_error() == Some(87) {
                    write!(f, "; Windows rejected an I/O request parameter (not a CLI option); raw-volume offsets, lengths and buffers must be sector-aligned")?;
                }
                Ok(())
            },
            Self::Incomplete(n, r) => {
                write!(f, "{} at byte offset {}, length {} bytes: ", if r.write { "write" } else { "read" }, r.offset, r.len)?;
                write!(f, "incomplete I/O: result {n}, expected {} bytes", r.len)?;
                if *n < 0 {
                    write!(f, ": {}", io::Error::from_raw_os_error((-n) as i32))?;
                }
                Ok(())
            }
            #[cfg(target_os = "linux")]
            Self::Timeout => write!(f, "I/O timed out: no completion within -T; increase -T SECONDS for a slow or heavily queued device"),
            Self::Verification => write!(f, "verification mismatch"),
        }
    }
}
impl From<io::Error> for Failure {
    fn from(e: io::Error) -> Self {
        Self::System(e)
    }
}

// These objects remain on the worker that created them. Kernel pointers reference
// stable, separately allocated pools, never movable Vec elements or stack data.
/// The streaming contract the asynchronous backends share: libaio and io_uring on Linux and
/// WinAio (overlapped I/O) on Windows. Each keeps the queue full, refilling a slot as soon as
/// its request completes.
/// What a streamed run needs from its queue. Each backend uses the fields it needs.
// Each backend reads the fields it needs; the rest are for other platforms.
#[allow(dead_code)]
pub struct StreamCtx<'a> {
    pub fd: i32,
    pub ptr: *mut u8,
    pub stride: usize,
    pub requests: &'a mut [Request],
    pub timeout: Duration,
    pub error_limit: u64,
    pub errors: &'a mut u64,
    pub submit_batch: usize,
    pub observer: Option<&'a crate::progress::Counter>,
}

/// Latencies kept per run, the same bound the engine uses for its own samples.
#[allow(dead_code)] // Only the measuring backends (Windows, macOS, FreeBSD) keep latencies.
const LATENCY_CAPACITY: usize = 2_000_000;

/// What a streamed run measured: the in-flight depth over time and each request's latency.
#[derive(Default)]
#[allow(dead_code)] // Only the measuring backends (Windows, macOS, FreeBSD) fill these.
pub struct StreamStats {
    pub max_in_flight: u64,
    /// Sum over time of the in-flight count, in count x nanoseconds.
    pub area_ns: u128,
    pub time_ns: u128,
    pub latencies_us: Vec<u32>,
    in_flight: u64,
    last_change: Option<Instant>,
}
#[allow(dead_code)] // Only the measuring backends (Windows, macOS, FreeBSD) use these.
impl StreamStats {
    /// Reserves room for every latency up front, so a run itself never grows the vector.
    pub fn new() -> Self {
        Self {
            latencies_us: Vec::with_capacity(LATENCY_CAPACITY),
            ..Self::default()
        }
    }

    /// Used by the Windows trace line; elsewhere it is only measured.
    pub fn in_flight(&self) -> u64 {
        self.in_flight
    }

    /// Moves the in-flight count by one, first adding the time spent at the previous count.
    pub fn change(&mut self, now: Instant, submitted: bool) {
        if let Some(last) = self.last_change {
            let dt = now.duration_since(last).as_nanos();
            self.area_ns += dt * self.in_flight as u128;
            self.time_ns += dt;
        }
        self.last_change = Some(now);
        if submitted {
            self.in_flight += 1;
            self.max_in_flight = self.max_in_flight.max(self.in_flight);
        } else {
            self.in_flight -= 1;
        }
    }

    /// Returns what was measured and clears it for the next run.
    pub fn take(&mut self) -> StreamStats {
        std::mem::take(self)
    }
}

/// The streaming contract every asynchronous backend implements, so Linux and Windows run through
/// the same code: libaio and io_uring on Linux, WinAio (overlapped I/O) on Windows. Each keeps the
/// queue full, refilling a slot as soon as its request completes.
pub trait AsyncStream {
    fn stream<N, P, C>(
        &mut self,
        ctx: StreamCtx<'_>,
        next: N,
        prepare: P,
        complete: C,
    ) -> std::result::Result<(), Failure>
    where
        N: FnMut() -> Option<Request>,
        P: FnMut(Request, &mut [u8]),
        C: FnMut(Request);
}

/// The asynchronous backend a queue runs on. Each platform module provides `Native`, which implements
/// this trait, so the queue itself never matches on the platform.
pub trait AsyncIo: Sized {
    /// Keeps the queue full: refills each slot as soon as its request completes, until `next` runs out.
    fn stream<N, P, C>(
        &mut self,
        ctx: StreamCtx<'_>,
        next: N,
        prepare: P,
        complete: C,
    ) -> std::result::Result<(), Failure>
    where
        N: FnMut() -> Option<Request>,
        P: FnMut(Request, &mut [u8]),
        C: FnMut(Request);

    /// Runs one batch: `ctx.requests` occupy slots from 0, and the call returns once every one has completed.
    fn execute<C>(&mut self, ctx: StreamCtx<'_>, complete: C) -> std::result::Result<(), Failure>
    where
        C: FnMut(usize, Request);

    /// The in-flight depth and latency measured by the last streamed run, when the backend measures them.
    fn take_stats(&mut self) -> Option<StreamStats> {
        None
    }

    /// Releases the backend. Returns false when the kernel may still use the queue's buffers.
    fn close(self) -> bool {
        true
    }
}

pub struct Queue {
    native: Option<Native>,
    arena: Option<Arena>,
    pub requests: Box<[Request]>,
    stride: usize,
    file: File,
    pub observer: Option<(std::sync::Arc<crate::progress::State>, usize)>,
    timeout: Duration,
    submit_batch: usize,
    pub submit_errors: u64,
}

impl Queue {
    pub fn new(
        file: File,
        backend: Backend,
        depth: usize,
        max_len: usize,
        timeout: f64,
        submit_batch: Option<usize>,
        slots: Vec<File>,
    ) -> io::Result<Self> {
        let stride = max_len
            .checked_add(4095)
            .ok_or(io::ErrorKind::InvalidInput)?
            & !4095;
        let arena = Arena::new(
            stride
                .checked_mul(depth)
                .ok_or(io::ErrorKind::InvalidInput)?,
        )?;
        let native = Native::new(&file, backend, depth, slots)?;
        Ok(Self {
            native: Some(native),
            arena: Some(arena),
            requests: vec![Request::default(); depth].into_boxed_slice(),
            stride,
            // Async backends can submit the whole fixed batch in one syscall.
            // qMIN-MAX explicitly limits each submission group to MIN.
            submit_batch: submit_batch.unwrap_or(depth),
            file,
            observer: None,
            timeout: Duration::from_secs_f64(timeout),
            submit_errors: 0,
        })
    }
    pub fn buffer(&mut self, index: usize) -> &mut [u8] {
        assert!(index < self.requests.len());
        let len = self.requests[index].len;
        assert!(len <= self.stride);
        unsafe {
            std::slice::from_raw_parts_mut(
                self.arena
                    .as_ref()
                    .unwrap()
                    .ptr
                    .as_ptr()
                    .add(index * self.stride),
                len,
            )
        }
    }
    pub fn flush(&self) -> io::Result<()> {
        self.file.sync_data()
    }
    /// The in-flight depth and per-request latency measured by the last streamed run.
    pub(crate) fn take_stream_stats(&mut self) -> Option<StreamStats> {
        self.native.as_mut()?.take_stats()
    }

    /// Streams requests through the platform's asynchronous backend. See `AsyncIo::stream`.
    pub fn execute_stream<N, P, C>(
        &mut self,
        next: N,
        prepare: P,
        complete: C,
        error_limit: u64,
    ) -> std::result::Result<(), Failure>
    where
        N: FnMut() -> Option<Request>,
        P: FnMut(Request, &mut [u8]),
        C: FnMut(Request),
    {
        let fd = raw_fd(&self.file);
        let ptr = self.arena.as_ref().unwrap().ptr.as_ptr();
        let observer = self
            .observer
            .as_ref()
            .map(|(state, index)| state.counter(*index));
        let ctx = StreamCtx {
            fd,
            ptr,
            stride: self.stride,
            requests: &mut self.requests,
            timeout: self.timeout,
            error_limit,
            errors: &mut self.submit_errors,
            submit_batch: self.submit_batch,
            observer,
        };
        AsyncIo::stream(self.native.as_mut().unwrap(), ctx, next, prepare, complete)
    }

    /// Runs the first `count` requests as one batch. See `AsyncIo::execute`.
    pub fn execute<C>(
        &mut self,
        count: usize,
        error_limit: u64,
        complete: C,
    ) -> std::result::Result<(), Failure>
    where
        C: FnMut(usize, Request),
    {
        assert!(count <= self.requests.len());
        let fd = raw_fd(&self.file);
        let ptr = self.arena.as_ref().unwrap().ptr.as_ptr();
        let observer = self
            .observer
            .as_ref()
            .map(|(state, index)| state.counter(*index));
        let ctx = StreamCtx {
            fd,
            ptr,
            stride: self.stride,
            requests: &mut self.requests[..count],
            timeout: self.timeout,
            error_limit,
            errors: &mut self.submit_errors,
            submit_batch: self.submit_batch,
            observer,
        };
        AsyncIo::execute(self.native.as_mut().unwrap(), ctx, complete)
    }

    pub fn close(&mut self) -> bool {
        let Some(native) = self.native.take() else {
            return true;
        };
        if native.close() {
            return true;
        }
        // The kernel may still use the buffers, so they are kept until process exit.
        if let Some(arena) = self.arena.take() {
            std::mem::forget(arena);
        }
        false
    }
}
impl Drop for Queue {
    fn drop(&mut self) {
        if !self.close() {
            eprintln!("*error* io_destroy failed; buffers retained until process exit");
        }
    }
}

#[cfg(unix)]
fn raw_fd(file: &File) -> i32 {
    std::os::unix::io::AsRawFd::as_raw_fd(file)
}
#[cfg(not(unix))]
fn raw_fd(_file: &File) -> i32 {
    -1
}
