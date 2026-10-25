use crate::cli::Backend;
#[cfg(windows)]
#[path = "windows.rs"]
mod windows;
#[cfg(not(target_os = "linux"))]
use std::time::{Duration, Instant};
use std::{
    alloc::{alloc_zeroed, dealloc, Layout},
    fs::{File, OpenOptions},
    io,
    ptr::NonNull,
};
#[cfg(target_os = "linux")]
use std::{
    os::unix::{fs::OpenOptionsExt, io::AsRawFd},
    time::{Duration, Instant},
};

// Probe actual queue creation, so seccomp/permissions are considered as well as
// the sysctl. Drop the empty ring immediately; no I/O is issued by this probe.
pub fn default_backend() -> Backend {
    #[cfg(target_os = "linux")]
    {
        if io_uring::IoUring::new(2).is_ok() {
            Backend::Uring
        } else {
            Backend::Libaio
        }
    }
    #[cfg(windows)]
    {
        Backend::Direct
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        Backend::Pagecache
    }
}

/// One handle per queue slot on Windows, so in-flight requests do not share a file object.
/// Other platforms use the single handle the caller already has.
pub fn open_slots(
    path: &std::path::Path,
    write: bool,
    backend: Backend,
    depth: usize,
) -> io::Result<Vec<File>> {
    if !cfg!(windows) || depth == 0 {
        return Ok(Vec::new());
    }
    (0..depth)
        .map(|_| open(path, write, false, backend))
        .collect()
}

pub fn open(
    path: &std::path::Path,
    write: bool,
    create: bool,
    backend: Backend,
) -> io::Result<File> {
    let mut o = OpenOptions::new();
    o.read(true).write(write).create_new(create);
    #[cfg(target_os = "freebsd")]
    use std::os::unix::fs::OpenOptionsExt;
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    if backend != Backend::Pagecache {
        use std::os::unix::fs::FileTypeExt;
        let direct = std::fs::metadata(path)
            .map(|m| m.is_file() || m.file_type().is_block_device())
            .unwrap_or(true);
        if direct {
            o.custom_flags(libc::O_DIRECT);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_FLAG_OVERLAPPED: every Windows backend submits requests asynchronously.
        let mut flags = 0x4000_0000;
        if backend == Backend::Direct {
            // FILE_FLAG_NO_BUFFERING: bypass the page cache. Buffers, offsets and lengths
            // must be sector-aligned, which the engine already checks for direct backends.
            flags |= 0x2000_0000;
        }
        o.custom_flags(flags);
    }
    #[cfg(not(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "freebsd",
        windows
    )))]
    if backend != Backend::Pagecache {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "this platform currently supports only the pagecache backend",
        ));
    }
    let file = o.open(path)?;
    #[cfg(target_os = "macos")]
    if backend == Backend::Direct {
        // F_NOCACHE is macOS's direct I/O: bypass the unified buffer cache.
        use std::os::unix::io::AsRawFd;
        if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_NOCACHE, 1) } != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    #[cfg(windows)]
    windows::prepare_handle(path, &file)?;
    Ok(file)
}

pub fn validate_target(path: &std::path::Path, write: bool) -> crate::Result<()> {
    #[cfg(windows)]
    windows::validate_target(path, write)?;
    #[cfg(not(windows))]
    let _ = write;
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

pub fn inspect(path: &std::path::Path, file: &File) -> crate::Result<TargetInfo> {
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
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub state: u8,
}
#[derive(Debug)]
pub enum Failure {
    System(io::Error),
    #[cfg_attr(target_os = "linux", allow(dead_code))]
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
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
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
#[cfg_attr(
    not(any(windows, target_os = "macos", target_os = "freebsd")),
    allow(dead_code)
)]
const LATENCY_CAPACITY: usize = 2_000_000;

/// What a streamed run measured: the in-flight depth over time and each request's latency.
#[cfg_attr(
    not(any(windows, target_os = "macos", target_os = "freebsd")),
    allow(dead_code)
)]
#[derive(Default)]
pub struct StreamStats {
    pub max_in_flight: u64,
    /// Sum over time of the in-flight count, in count x nanoseconds.
    pub area_ns: u128,
    pub time_ns: u128,
    pub latencies_us: Vec<u32>,
    in_flight: u64,
    last_change: Option<Instant>,
}

#[cfg_attr(
    not(any(windows, target_os = "macos", target_os = "freebsd")),
    allow(dead_code)
)]
impl StreamStats {
    /// Reserves room for every latency up front, so a run itself never grows the vector.
    pub fn new() -> Self {
        Self {
            latencies_us: Vec::with_capacity(LATENCY_CAPACITY),
            ..Self::default()
        }
    }

    /// Used by the Windows trace line; elsewhere it is only measured.
    #[cfg_attr(not(windows), allow(dead_code))]
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
#[cfg_attr(
    not(any(
        target_os = "linux",
        windows,
        target_os = "macos",
        target_os = "freebsd"
    )),
    allow(dead_code)
)]
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

pub struct Queue {
    state: Option<State>,
    arena: Option<Arena>,
    pub requests: Box<[Request]>,
    stride: usize,
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    submit_batch: usize,
    file: File,
    pub observer: Option<(std::sync::Arc<crate::progress::State>, usize)>,
    timeout: Duration,
    pub submit_errors: u64,
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pending: bool,
}
// One enum per worker; keep the queue inline instead of allocating another box.
#[allow(clippy::large_enum_variant)]
enum State {
    #[cfg(any(target_os = "macos", target_os = "freebsd"))]
    PosixAio(posix_aio::PosixAio),
    #[cfg(windows)]
    Overlapped(overlapped::WinAio),
    #[cfg(target_os = "linux")]
    Aio(linux::Aio),
    #[cfg(target_os = "linux")]
    Uring(io_uring::IoUring),
}
impl Queue {
    pub fn new(
        file: File,
        backend: Backend,
        depth: usize,
        max_len: usize,
        _timeout: f64,
        submit_batch: Option<usize>,
        slots: Vec<File>,
    ) -> io::Result<Self> {
        #[cfg(not(windows))]
        drop(slots);
        let stride = max_len
            .checked_add(4095)
            .ok_or(io::ErrorKind::InvalidInput)?
            & !4095;
        let arena = Arena::new(
            stride
                .checked_mul(depth)
                .ok_or(io::ErrorKind::InvalidInput)?,
        )?;
        let state = match backend {
            #[cfg(windows)]
            _ => State::Overlapped(overlapped::WinAio::new(if slots.is_empty() {
                vec![file.try_clone()?]
            } else {
                slots
            })?),
            // Linux pagecache and direct I/O both go through the async backends; direct sets O_DIRECT in open().
            #[cfg(target_os = "linux")]
            Backend::Pagecache | Backend::Direct => match default_backend() {
                Backend::Uring => State::Uring(io_uring::IoUring::new(depth as u32)?),
                _ => State::Aio(linux::Aio::new(depth)?),
            },
            #[cfg(any(target_os = "macos", target_os = "freebsd"))]
            Backend::Pagecache | Backend::Direct => {
                State::PosixAio(posix_aio::PosixAio::new(&file, depth)?)
            }
            #[cfg(target_os = "linux")]
            Backend::Libaio => State::Aio(linux::Aio::new(depth)?),
            #[cfg(target_os = "linux")]
            Backend::Uring => State::Uring(io_uring::IoUring::new(depth as u32)?),
            #[cfg(not(any(windows, target_os = "linux")))]
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "no asynchronous I/O backend on this platform: pagecache and direct need POSIX AIO (macOS, FreeBSD), libaio or io_uring (Linux), or overlapped I/O (Windows)",
                ))
            }
        };
        Ok(Self {
            state: Some(state),
            arena: Some(arena),
            requests: vec![Request::default(); depth].into_boxed_slice(),
            stride,
            // Async backends can submit the whole fixed batch in one syscall.
            // qMIN-MAX explicitly limits each submission group to MIN.
            submit_batch: submit_batch.unwrap_or(depth),
            file,
            observer: None,
            timeout: Duration::from_secs_f64(_timeout),
            submit_errors: 0,
            pending: false,
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
        match self.state.as_mut()? {
            #[cfg(windows)]
            State::Overlapped(port) => Some(port.take_stats()),
            #[cfg(any(target_os = "macos", target_os = "freebsd"))]
            State::PosixAio(aio) => Some(aio.take_stats()),
            #[cfg(not(any(windows, target_os = "macos", target_os = "freebsd")))]
            _ => None,
        }
    }

    /// Streams requests through the platform's asynchronous backend: libaio or io_uring on Linux, WinAio on
    /// Windows and POSIX AIO on macOS and FreeBSD, each implementing `AsyncStream`.
    #[cfg_attr(
        not(any(
            target_os = "linux",
            windows,
            target_os = "macos",
            target_os = "freebsd"
        )),
        allow(dead_code, unused_variables)
    )]
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
        #[cfg(unix)]
        let fd = std::os::unix::io::AsRawFd::as_raw_fd(&self.file);
        #[cfg(not(unix))]
        let fd = -1;
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
        match self.state.as_mut().unwrap() {
            #[cfg(target_os = "linux")]
            State::Aio(aio) => aio.stream(ctx, next, prepare, complete),
            #[cfg(target_os = "linux")]
            State::Uring(ring) => {
                self.pending = true;
                let result = ring.stream(ctx, next, prepare, complete);
                if result.is_ok() {
                    self.pending = false;
                }
                result
            }
            #[cfg(windows)]
            State::Overlapped(port) => port.stream(ctx, next, prepare, complete),
            #[cfg(any(target_os = "macos", target_os = "freebsd"))]
            State::PosixAio(aio) => aio.stream(ctx, next, prepare, complete),
            #[cfg(not(any(
                windows,
                target_os = "linux",
                target_os = "macos",
                target_os = "freebsd"
            )))]
            _ => unreachable!("no asynchronous backend exists on this platform"),
        }
    }

    pub fn execute<C>(
        &mut self,
        count: usize,
        _error_limit: u64,
        mut complete: C,
    ) -> std::result::Result<(), Failure>
    where
        C: FnMut(usize, Request),
    {
        assert!(count <= self.requests.len());
        let ptr = self.arena.as_ref().unwrap().ptr.as_ptr();
        let stride = self.stride;
        let observer = self
            .observer
            .as_ref()
            .map(|(state, index)| state.counter(*index));
        match self.state.as_mut().unwrap() {
            #[cfg(windows)]
            State::Overlapped(port) => {
                port.execute(
                    &self.file,
                    ptr,
                    stride,
                    &self.requests[..count],
                    observer,
                    &mut complete,
                )?;
            }
            #[cfg(any(target_os = "macos", target_os = "freebsd"))]
            State::PosixAio(aio) => {
                aio.execute(
                    ptr,
                    stride,
                    &self.requests[..count],
                    observer,
                    &mut complete,
                )?;
            }
            #[cfg(not(any(
                windows,
                target_os = "linux",
                target_os = "macos",
                target_os = "freebsd"
            )))]
            _ => unreachable!("no asynchronous backend exists on this platform"),
            #[cfg(target_os = "linux")]
            State::Aio(aio) => {
                aio.execute(
                    self.file.as_raw_fd(),
                    ptr,
                    stride,
                    &self.requests[..count],
                    self.timeout,
                    _error_limit,
                    &mut self.submit_errors,
                    self.submit_batch,
                    observer,
                    &mut complete,
                )?;
            }
            #[cfg(target_os = "linux")]
            State::Uring(ring) => {
                use io_uring::{opcode, types};
                self.pending = true;
                let mut direction = None;
                for (i, r) in self.requests[..count].iter().enumerate() {
                    // Keep the requested order, but do not ask the Linux block
                    // plug to dispatch both directions in a single submission.
                    if direction.is_some_and(|previous| previous != r.write) {
                        linux::uring_submit(ring, observer)?;
                    }
                    direction = Some(r.write);
                    let buf = unsafe { ptr.add(i * stride) };
                    let entry = if r.write {
                        opcode::Write::new(types::Fd(self.file.as_raw_fd()), buf, r.len as u32)
                            .offset(r.offset)
                            .build()
                    } else {
                        opcode::Read::new(types::Fd(self.file.as_raw_fd()), buf, r.len as u32)
                            .offset(r.offset)
                            .build()
                    }
                    .user_data(i as u64);
                    unsafe { ring.submission().push(&entry) }
                        .map_err(|_| io::Error::other("submission queue full"))?;
                    if (i + 1) % self.submit_batch == 0 {
                        linux::uring_submit(ring, observer)?;
                    }
                }
                let mut done = 0;
                let mut progress = Instant::now();
                let mut failure = None;
                // Nonblocking submission plus bounded polling gives every backend the same
                // stall timeout; a stalled device must not trap an uninterruptible wait.
                linux::uring_submit(ring, observer)?;
                while done < count {
                    let before = done;
                    for cqe in ring.completion() {
                        if let Some(counter) = observer {
                            counter.completed(1);
                        }
                        done += 1;
                        let index = cqe.user_data() as usize;
                        if index >= count {
                            return Err(io::Error::other("invalid completion id").into());
                        }
                        let expected = self.requests[index].len;
                        if cqe.result() as i64 != expected as i64 && failure.is_none() {
                            failure = Some(Failure::Incomplete(
                                cqe.result() as i64,
                                self.requests[index],
                            ));
                        }
                        if cqe.result() as i64 == expected as i64 {
                            complete(index, self.requests[index]);
                        }
                    }
                    if done > before {
                        progress = Instant::now();
                    } else {
                        if progress.elapsed() > self.timeout {
                            return Err(Failure::Timeout);
                        }
                        if ring.params().is_feature_ext_arg() {
                            let timeout = io_uring::types::Timespec::from(Duration::from_millis(1));
                            let args = io_uring::types::SubmitArgs::new().timespec(&timeout);
                            // Submit through the accounting path before waiting.
                            if !ring.submission().is_empty() {
                                linux::uring_submit(ring, observer)?;
                                continue;
                            }
                            if let Err(e) = ring.submitter().submit_with_args(1, &args) {
                                if !matches!(
                                    e.raw_os_error(),
                                    Some(libc::ETIME) | Some(libc::EINTR)
                                ) {
                                    return Err(e.into());
                                }
                            }
                        } else {
                            std::thread::sleep(Duration::from_micros(50));
                        }
                    }
                    // Retry SQEs not accepted by a partial submit.
                    if !ring.submission().is_empty() {
                        linux::uring_submit(ring, observer)?;
                    }
                }
                self.pending = false;
                if let Some(e) = failure {
                    return Err(e);
                }
            }
        }
        Ok(())
    }
    pub fn close(&mut self) -> bool {
        if let Some(state) = self.state.take() {
            match state {
                #[cfg(target_os = "linux")]
                State::Aio(mut aio) => {
                    if !aio.close() {
                        // Never free memory the kernel may still own, even when destruction fails.
                        std::mem::forget(aio);
                        if let Some(arena) = self.arena.take() {
                            std::mem::forget(arena);
                        }
                        return false;
                    }
                }
                // Close the ring before freeing the arena. On an incomplete/error
                // path retain the arena: cancellation may still reference buffers.
                #[cfg(target_os = "linux")]
                State::Uring(ring) => {
                    drop(ring);
                    if self.pending {
                        if let Some(arena) = self.arena.take() {
                            std::mem::forget(arena);
                        }
                        return false;
                    }
                }
                #[cfg(windows)]
                State::Overlapped(_) => {}
                #[cfg(any(target_os = "macos", target_os = "freebsd"))]
                State::PosixAio(_) => {}
            }
        }
        true
    }
}
impl Drop for Queue {
    fn drop(&mut self) {
        if !self.close() {
            eprintln!("*error* io_destroy failed; buffers retained until process exit");
        }
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    pub fn uring_submit(
        ring: &io_uring::IoUring,
        observer: Option<&crate::progress::Counter>,
    ) -> io::Result<usize> {
        let submitted = ring.submit()?;
        if let Some(counter) = observer {
            counter.submitted(submitted);
        }
        Ok(submitted)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn uring_stream<N, P, C>(
        ring: &mut io_uring::IoUring,
        fd: i32,
        ptr: *mut u8,
        stride: usize,
        requests: &mut [Request],
        timeout: Duration,
        submit_batch: usize,
        observer: Option<&crate::progress::Counter>,
        mut next: N,
        mut prepare: P,
        mut complete: C,
    ) -> std::result::Result<(), Failure>
    where
        N: FnMut() -> Option<Request>,
        P: FnMut(Request, &mut [u8]),
        C: FnMut(Request),
    {
        use io_uring::{opcode, types};
        let mut pending = 0;
        let mut finished = false;
        let mut progress = Instant::now();
        loop {
            let mut direction = None;
            let mut group = 0;
            if !finished {
                for (i, slot) in requests.iter_mut().enumerate() {
                    if slot.state != 0 {
                        continue;
                    }
                    let Some(mut request) = next() else {
                        finished = true;
                        break;
                    };
                    assert!(request.len <= stride);
                    let buffer =
                        unsafe { std::slice::from_raw_parts_mut(ptr.add(i * stride), request.len) };
                    prepare(request, buffer);
                    if direction.is_some_and(|previous| previous != request.write) {
                        if uring_submit(ring, observer)? > 0 {
                            progress = Instant::now();
                        }
                        group = 0;
                    }
                    direction = Some(request.write);
                    let entry = if request.write {
                        opcode::Write::new(types::Fd(fd), buffer.as_ptr(), request.len as u32)
                            .offset(request.offset)
                            .build()
                    } else {
                        opcode::Read::new(types::Fd(fd), buffer.as_mut_ptr(), request.len as u32)
                            .offset(request.offset)
                            .build()
                    }
                    .user_data(i as u64);
                    unsafe { ring.submission().push(&entry) }
                        .map_err(|_| io::Error::other("submission queue full"))?;
                    request.state = 2;
                    *slot = request;
                    pending += 1;
                    group += 1;
                    if group == submit_batch {
                        if uring_submit(ring, observer)? > 0 {
                            progress = Instant::now();
                        }
                        group = 0;
                    }
                }
            }
            let queued = !ring.submission().is_empty();
            if queued && uring_submit(ring, observer)? > 0 {
                progress = Instant::now();
            }
            if finished && pending == 0 {
                return Ok(());
            }
            let mut completed = 0;
            for cqe in ring.completion() {
                if let Some(counter) = observer {
                    counter.completed(1);
                }
                let slot = requests
                    .get_mut(cqe.user_data() as usize)
                    .ok_or_else(|| io::Error::other("invalid completion id"))?;
                if slot.state != 2 {
                    return Err(io::Error::other("completion for a free slot").into());
                }
                if cqe.result() as i64 != slot.len as i64 {
                    return Err(Failure::Incomplete(cqe.result() as i64, *slot));
                }
                let request = *slot;
                slot.state = 0;
                pending -= 1;
                completed += 1;
                complete(request);
            }
            if completed > 0 {
                progress = Instant::now();
                continue;
            }
            if progress.elapsed() > timeout {
                return Err(Failure::Timeout);
            }
            if ring.params().is_feature_ext_arg() {
                if !ring.submission().is_empty() {
                    if uring_submit(ring, observer)? > 0 {
                        progress = Instant::now();
                    }
                    continue;
                }
                let wait = types::Timespec::from(Duration::from_millis(1));
                let args = types::SubmitArgs::new().timespec(&wait);
                if let Err(e) = ring.submitter().submit_with_args(1, &args) {
                    if !matches!(e.raw_os_error(), Some(libc::ETIME) | Some(libc::EINTR)) {
                        return Err(e.into());
                    }
                }
            } else {
                std::thread::sleep(Duration::from_micros(50));
            }
        }
    }
    // Linux libaio ABI (64-bit little endian). Keep layouts covered by size tests.
    #[cfg(not(all(target_pointer_width = "64", target_endian = "little")))]
    compile_error!("libaio backend currently requires 64-bit little-endian Linux");
    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct Cb {
        data: u64,
        key: u32,
        flags: u32,
        opcode: u16,
        priority: i16,
        fd: i32,
        buf: u64,
        len: u64,
        offset: i64,
        reserved: u64,
        options: u32,
        eventfd: u32,
    }
    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct Event {
        data: u64,
        obj: u64,
        result: i64,
        result2: i64,
    }
    #[link(name = "aio")]
    extern "C" {
        fn io_setup(max: libc::c_int, ctx: *mut usize) -> libc::c_int;
        fn io_submit(ctx: usize, n: libc::c_long, cbs: *mut *mut Cb) -> libc::c_int;
        fn io_getevents(
            ctx: usize,
            min: libc::c_long,
            max: libc::c_long,
            events: *mut Event,
            timeout: *mut libc::timespec,
        ) -> libc::c_int;
        fn io_destroy(ctx: usize) -> libc::c_int;
    }
    pub struct Aio {
        ctx: usize,
        cbs: Box<[Cb]>,
        pointers: Box<[*mut Cb]>,
        events: Box<[Event]>,
    }
    impl Aio {
        pub fn new(depth: usize) -> io::Result<Self> {
            let mut ctx = 0;
            let ret = unsafe { io_setup(depth as i32, &mut ctx) };
            if ret < 0 {
                return Err(io::Error::from_raw_os_error(-ret));
            }
            Ok(Self {
                ctx,
                cbs: vec![Cb::default(); depth].into_boxed_slice(),
                pointers: vec![std::ptr::null_mut(); depth].into_boxed_slice(),
                events: vec![Event::default(); depth].into_boxed_slice(),
            })
        }
        #[allow(clippy::too_many_arguments)]
        pub fn execute<C>(
            &mut self,
            fd: i32,
            ptr: *mut u8,
            stride: usize,
            requests: &[Request],
            timeout: Duration,
            error_limit: u64,
            errors: &mut u64,
            submit_batch: usize,
            observer: Option<&crate::progress::Counter>,
            mut complete: C,
        ) -> std::result::Result<(), Failure>
        where
            C: FnMut(usize, Request),
        {
            for (i, r) in requests.iter().enumerate() {
                self.cbs[i] = Cb {
                    data: i as u64,
                    fd,
                    opcode: if r.write { 1 } else { 0 },
                    buf: unsafe { ptr.add(i * stride) } as u64,
                    len: r.len as u64,
                    offset: r.offset as i64,
                    ..Cb::default()
                };
                self.pointers[i] = &mut self.cbs[i];
            }
            let mut submitted = 0;
            let mut completed = 0;
            let mut progress = Instant::now();
            let mut failure = None;
            while completed < requests.len() {
                // Fill the available batch before reaping completions. Waiting
                // after each io_submit silently reduces qDEPTH to submit_batch.
                while submitted < requests.len() {
                    let direction = requests[submitted].write;
                    let same_direction = requests[submitted..]
                        .iter()
                        .take_while(|r| r.write == direction)
                        .count();
                    let ret = unsafe {
                        io_submit(
                            self.ctx,
                            same_direction.min(submit_batch) as libc::c_long,
                            self.pointers.as_mut_ptr().add(submitted),
                        )
                    };
                    if ret > 0 {
                        if let Some(counter) = observer {
                            counter.submitted(ret as usize);
                        }
                        submitted += ret as usize;
                        progress = Instant::now();
                    } else if ret == 0 || ret == -libc::EAGAIN || ret == -libc::EINTR {
                        break;
                    } else if ret < 0 && ret != -libc::EAGAIN && ret != -libc::EINTR {
                        *errors += 1;
                        if (error_limit != 0 && *errors >= error_limit) || *errors >= 1024 {
                            return Err(io::Error::from_raw_os_error(-ret).into());
                        }
                    }
                }
                if completed < submitted {
                    let mut wait = libc::timespec {
                        tv_sec: 0,
                        tv_nsec: 1_000_000,
                    };
                    let ret = unsafe {
                        io_getevents(
                            self.ctx,
                            1,
                            (submitted - completed) as libc::c_long,
                            self.events.as_mut_ptr(),
                            &mut wait,
                        )
                    };
                    if ret < 0 && ret != -libc::EINTR {
                        return Err(io::Error::from_raw_os_error(-ret).into());
                    }
                    if ret > 0 {
                        if let Some(counter) = observer {
                            counter.completed(ret as usize);
                        }
                        progress = Instant::now();
                        for e in &self.events[..ret as usize] {
                            completed += 1;
                            let r = requests
                                .get(e.data as usize)
                                .ok_or_else(|| io::Error::other("invalid completion id"))?;
                            if (e.result != r.len as i64 || e.result2 != 0) && failure.is_none() {
                                failure = Some(Failure::Incomplete(e.result, *r));
                            }
                            if e.result == r.len as i64 && e.result2 == 0 {
                                complete(e.data as usize, *r);
                            }
                        }
                    }
                }
                if progress.elapsed() > timeout {
                    return Err(Failure::Timeout);
                }
            }
            if let Some(e) = failure {
                return Err(e);
            }
            Ok(())
        }
        #[allow(clippy::too_many_arguments)]
        pub fn execute_stream<N, P, C>(
            &mut self,
            fd: i32,
            ptr: *mut u8,
            stride: usize,
            requests: &mut [Request],
            timeout: Duration,
            error_limit: u64,
            errors: &mut u64,
            submit_batch: usize,
            observer: Option<&crate::progress::Counter>,
            mut next: N,
            mut prepare: P,
            mut complete: C,
        ) -> std::result::Result<(), Failure>
        where
            N: FnMut() -> Option<Request>,
            P: FnMut(Request, &mut [u8]),
            C: FnMut(Request),
        {
            let mut ready = 0;
            let mut head = 0;
            let mut pending = 0;
            let mut finished = false;
            let mut progress = Instant::now();
            loop {
                if head == ready {
                    ready = 0;
                    head = 0;
                    if !finished {
                        for (i, slot) in requests.iter_mut().enumerate() {
                            if slot.state != 0 {
                                continue;
                            }
                            let Some(mut request) = next() else {
                                finished = true;
                                break;
                            };
                            assert!(request.len <= stride);
                            // A free slot has no kernel-owned buffer. Prepare it
                            // before publishing its stable control-block pointer.
                            let buffer = unsafe {
                                std::slice::from_raw_parts_mut(ptr.add(i * stride), request.len)
                            };
                            prepare(request, buffer);
                            request.state = 1;
                            *slot = request;
                            self.cbs[i] = Cb {
                                data: i as u64,
                                fd,
                                opcode: if request.write { 1 } else { 0 },
                                buf: unsafe { ptr.add(i * stride) } as u64,
                                len: request.len as u64,
                                offset: request.offset as i64,
                                ..Cb::default()
                            };
                            self.pointers[ready] = &mut self.cbs[i];
                            ready += 1;
                        }
                    }
                }
                while head < ready {
                    let direction = unsafe { (*self.pointers[head]).opcode };
                    let mut count = 1;
                    while head + count < ready
                        && count < submit_batch
                        && unsafe { (*self.pointers[head + count]).opcode } == direction
                    {
                        count += 1;
                    }
                    let ret = unsafe {
                        io_submit(
                            self.ctx,
                            count as libc::c_long,
                            self.pointers.as_mut_ptr().add(head),
                        )
                    };
                    if ret > 0 {
                        if let Some(counter) = observer {
                            counter.submitted(ret as usize);
                        }
                        for pointer in &self.pointers[head..head + ret as usize] {
                            let index = unsafe { (**pointer).data as usize };
                            requests[index].state = 2;
                        }
                        head += ret as usize;
                        pending += ret as usize;
                        progress = Instant::now();
                    } else if ret == 0 || ret == -libc::EAGAIN || ret == -libc::EINTR {
                        break;
                    } else {
                        *errors += 1;
                        if (error_limit != 0 && *errors >= error_limit) || *errors >= 1024 {
                            return Err(io::Error::from_raw_os_error(-ret).into());
                        }
                    }
                }
                if finished && pending == 0 && head == ready {
                    return Ok(());
                }
                if pending != 0 {
                    let mut wait = libc::timespec {
                        tv_sec: 0,
                        tv_nsec: 1_000_000,
                    };
                    let ret = unsafe {
                        io_getevents(
                            self.ctx,
                            1,
                            pending as libc::c_long,
                            self.events.as_mut_ptr(),
                            &mut wait,
                        )
                    };
                    if ret < 0 && ret != -libc::EINTR {
                        return Err(io::Error::from_raw_os_error(-ret).into());
                    }
                    if ret > 0 {
                        if let Some(counter) = observer {
                            counter.completed(ret as usize);
                        }
                        progress = Instant::now();
                        for event in &self.events[..ret as usize] {
                            let slot = requests
                                .get_mut(event.data as usize)
                                .ok_or_else(|| io::Error::other("invalid completion id"))?;
                            if slot.state != 2 {
                                return Err(io::Error::other("completion for a free slot").into());
                            }
                            if event.result != slot.len as i64 || event.result2 != 0 {
                                return Err(Failure::Incomplete(event.result, *slot));
                            }
                            let request = *slot;
                            slot.state = 0;
                            pending -= 1;
                            complete(request);
                        }
                    }
                } else if head < ready {
                    std::thread::sleep(Duration::from_micros(50));
                }
                if progress.elapsed() > timeout {
                    return Err(Failure::Timeout);
                }
            }
        }
        pub fn close(&mut self) -> bool {
            if self.ctx == 0 {
                return true;
            }
            let ret = unsafe { io_destroy(self.ctx) };
            if ret == 0 {
                self.ctx = 0;
                true
            } else {
                false
            }
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn abi() {
            assert_eq!(std::mem::size_of::<Cb>(), 64);
            assert_eq!(std::mem::size_of::<Event>(), 32);
            assert_eq!(std::mem::size_of::<Request>(), 24);
        }
    }
}

#[cfg(windows)]
mod overlapped {
    //! Windows overlapped I/O: a batch of requests is submitted at once, each with its own
    //! OVERLAPPED, and the completion port reports them as they finish.
    use super::{Failure, Request, StreamStats, LATENCY_CAPACITY};
    use std::ffi::c_void;
    use std::fs::File;
    use std::io;
    use std::os::windows::io::AsRawHandle;
    use std::time::Instant;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Ov {
        internal: usize,
        internal_high: usize,
        offset: u32,
        offset_high: u32,
        event: *mut c_void,
    }

    unsafe extern "system" {
        fn SetPriorityClass(process: *mut c_void, class: u32) -> i32;
        fn NtSetInformationProcess(
            process: *mut c_void,
            class: u32,
            info: *const c_void,
            length: u32,
        ) -> i32;
        fn GetCurrentProcess() -> *mut c_void;
        fn GetCurrentProcessId() -> u32;
        fn GetPriorityClass(process: *mut c_void) -> u32;
        fn GetProcessAffinityMask(
            process: *mut c_void,
            process_mask: *mut usize,
            system_mask: *mut usize,
        ) -> i32;
        fn ProcessIdToSessionId(pid: u32, session: *mut u32) -> i32;
        fn CreateIoCompletionPort(
            file: *mut c_void,
            existing: *mut c_void,
            key: usize,
            threads: u32,
        ) -> *mut c_void;
        fn ReadFile(
            file: *mut c_void,
            buf: *mut u8,
            count: u32,
            read: *mut u32,
            ov: *mut Ov,
        ) -> i32;
        fn WriteFile(
            file: *mut c_void,
            buf: *const u8,
            count: u32,
            written: *mut u32,
            ov: *mut Ov,
        ) -> i32;
        fn GetQueuedCompletionStatus(
            port: *mut c_void,
            bytes: *mut u32,
            key: *mut usize,
            ov: *mut *mut Ov,
            timeout: u32,
        ) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }

    const ERROR_IO_PENDING: i32 = 997;
    const INFINITE: u32 = 0xFFFF_FFFF;

    /// Windows asynchronous backend: overlapped I/O on one completion port, one file handle per slot.
    pub struct WinAio {
        /// One file per slot, so each in-flight request has its own file object. The completion
        /// port holds the handles; these keep the files open.
        #[allow(dead_code)]
        files: Vec<File>,
        handles: Vec<*mut c_void>,
        port: *mut c_void,
        ov: Box<[Ov]>,
        submitted_at: Vec<Option<Instant>>,
        /// SPIT_TRACE, read once when the queue is created: reading the environment allocates.
        trace: bool,
        /// The request each slot has in flight, kept here so a streamed run allocates nothing.
        inflight: Vec<Option<Request>>,
        stats: StreamStats,
    }

    impl WinAio {
        pub fn new(files: Vec<File>) -> io::Result<Self> {
            let mut port: *mut c_void = std::ptr::null_mut();
            let mut handles = Vec::with_capacity(files.len());
            for file in &files {
                let handle = file.as_raw_handle();
                // The first association creates the completion port; the others join it.
                let associated = unsafe { CreateIoCompletionPort(handle, port, 0, 0) };
                if associated.is_null() {
                    return Err(io::Error::last_os_error());
                }
                port = associated;
                handles.push(handle);
            }
            if handles.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "overlapped I/O needs at least one slot",
                ));
            }
            let depth = handles.len();
            let zero = Ov {
                internal: 0,
                internal_high: 0,
                offset: 0,
                offset_high: 0,
                event: std::ptr::null_mut(),
            };
            Ok(Self {
                files,
                handles,
                port,
                ov: vec![zero; depth].into_boxed_slice(),
                submitted_at: vec![None; depth],
                trace: std::env::var_os("SPIT_TRACE").is_some(),
                inflight: vec![None; depth],
                stats: StreamStats::new(),
            })
        }

        /// Starts one request on `slot`, using that slot's OVERLAPPED and file handle.
        fn submit(
            &mut self,
            slot: usize,
            ptr: *mut u8,
            stride: usize,
            r: Request,
        ) -> Result<(), Failure> {
            let ov = &mut self.ov[slot] as *mut Ov;
            unsafe {
                (*ov).internal = 0;
                (*ov).internal_high = 0;
                (*ov).offset = r.offset as u32;
                (*ov).offset_high = (r.offset >> 32) as u32;
                (*ov).event = std::ptr::null_mut();
            }
            let buf = unsafe { ptr.add(slot * stride) };
            let handle = self.handles[slot];
            let ok = unsafe {
                if r.write {
                    WriteFile(handle, buf, r.len as u32, std::ptr::null_mut(), ov)
                } else {
                    ReadFile(handle, buf, r.len as u32, std::ptr::null_mut(), ov)
                }
            };
            if ok == 0 && unsafe { get_last_error_code() } != ERROR_IO_PENDING {
                return Err(Failure::Operation {
                    error: io::Error::last_os_error(),
                    write: r.write,
                    offset: r.offset,
                    len: r.len,
                });
            }
            Ok(())
        }

        pub fn take_stats(&mut self) -> StreamStats {
            self.stats.take()
        }

        /// Keeps `slots` requests in flight until `next` runs out: each slot is refilled as soon as
        /// its write completes, so the queue depth stays full instead of waiting for a whole batch.
        pub fn execute_stream<N, P, C>(
            &mut self,
            ptr: *mut u8,
            stride: usize,
            mut next: N,
            mut prepare: P,
            mut complete: C,
            observer: Option<&crate::progress::Counter>,
        ) -> Result<(), Failure>
        where
            N: FnMut() -> Option<Request>,
            P: FnMut(Request, &mut [u8]),
            C: FnMut(Request),
        {
            let slots = self.ov.len();
            let trace = self.trace;
            let started = Instant::now();
            // Scheduled tasks can run at low I/O priority, which slows every completion. A benchmark
            // asks for normal priority so the device, not the scheduler, sets the pace.
            let io_priority: u32 = 2;
            let (class_ok, io_status) = unsafe {
                let process = GetCurrentProcess();
                (
                    SetPriorityClass(process, 0x20),
                    NtSetInformationProcess(
                        process,
                        33,
                        &io_priority as *const u32 as *const c_void,
                        4,
                    ),
                )
            };
            if trace {
                eprintln!(
                    "trace priority set_class_ok={} io_priority_status=0x{:x}",
                    class_ok, io_status
                );
            }
            if trace {
                let (mut session, mut mask, mut system) = (0u32, 0usize, 0usize);
                unsafe {
                    let process = GetCurrentProcess();
                    ProcessIdToSessionId(GetCurrentProcessId(), &mut session);
                    GetProcessAffinityMask(process, &mut mask, &mut system);
                    eprintln!(
                        "trace context pid={} session={} priority_class=0x{:x} affinity_mask=0x{:x} slots={}",
                        GetCurrentProcessId(),
                        session,
                        GetPriorityClass(process),
                        mask,
                        slots
                    );
                }
            }
            let mut inflight = std::mem::take(&mut self.inflight);
            let mut outstanding = 0usize;
            let mut exhausted = false;
            let mut failure: Option<Failure> = None;
            loop {
                if failure.is_none() && !exhausted {
                    for (slot, entry) in inflight.iter_mut().enumerate() {
                        if entry.is_some() {
                            continue;
                        }
                        let Some(request) = next() else {
                            exhausted = true;
                            break;
                        };
                        let buffer = unsafe {
                            std::slice::from_raw_parts_mut(ptr.add(slot * stride), request.len)
                        };
                        prepare(request, buffer);
                        let now = Instant::now();
                        if let Err(e) = self.submit(slot, ptr, stride, request) {
                            failure = Some(e);
                            break;
                        }
                        self.submitted_at[slot] = Some(now);
                        self.stats.change(now, true);
                        *entry = Some(request);
                        outstanding += 1;
                        if let Some(counter) = observer {
                            counter.submitted(1);
                        }
                    }
                }
                if outstanding == 0 {
                    break;
                }
                let mut bytes = 0u32;
                let mut key = 0usize;
                let mut done: *mut Ov = std::ptr::null_mut();
                let ok = unsafe {
                    GetQueuedCompletionStatus(self.port, &mut bytes, &mut key, &mut done, INFINITE)
                };
                if done.is_null() {
                    return Err(Failure::System(io::Error::last_os_error()));
                }
                let now = Instant::now();
                let slot = (done as usize - self.ov.as_ptr() as usize) / std::mem::size_of::<Ov>();
                let request = inflight[slot]
                    .take()
                    .expect("completion for a slot with nothing in flight");
                outstanding -= 1;
                self.stats.change(now, false);
                if let Some(counter) = observer {
                    counter.completed(1);
                }
                if let Some(start) = self.submitted_at[slot] {
                    let latency =
                        now.duration_since(start).as_micros().min(u32::MAX as u128) as u32;
                    if trace {
                        eprintln!(
                            "trace t_ms={} slot={} write={} offset={} latency_us={} in_flight={}",
                            started.elapsed().as_millis(),
                            slot,
                            request.write,
                            request.offset,
                            latency,
                            self.stats.in_flight()
                        );
                    }
                    if self.stats.latencies_us.len() < LATENCY_CAPACITY {
                        self.stats.latencies_us.push(latency);
                    }
                }
                if ok == 0 {
                    failure.get_or_insert(Failure::Operation {
                        error: io::Error::last_os_error(),
                        write: request.write,
                        offset: request.offset,
                        len: request.len,
                    });
                } else if bytes as usize != request.len {
                    failure.get_or_insert(Failure::Incomplete(bytes as i64, request));
                } else if failure.is_none() {
                    complete(request);
                }
            }
            self.inflight = inflight;
            match failure {
                Some(e) => Err(e),
                None => Ok(()),
            }
        }

        /// Batch submission: every request is in flight together and the batch waits for all of them.
        pub fn execute<C>(
            &mut self,
            _file: &File,
            ptr: *mut u8,
            stride: usize,
            requests: &[Request],
            observer: Option<&crate::progress::Counter>,
            mut complete: C,
        ) -> Result<(), Failure>
        where
            C: FnMut(usize, Request),
        {
            let mut submitted = 0usize;
            let mut failure = None;
            for (i, r) in requests.iter().enumerate() {
                if let Err(e) = self.submit(i, ptr, stride, *r) {
                    failure = Some(e);
                    break;
                }
                submitted += 1;
                if let Some(counter) = observer {
                    counter.submitted(1);
                }
            }
            // Reap everything that was submitted, even after a failure, so no buffer is in use by the kernel.
            for _ in 0..submitted {
                let mut bytes = 0u32;
                let mut key = 0usize;
                let mut done: *mut Ov = std::ptr::null_mut();
                let ok = unsafe {
                    GetQueuedCompletionStatus(self.port, &mut bytes, &mut key, &mut done, INFINITE)
                };
                if done.is_null() {
                    return Err(Failure::System(io::Error::last_os_error()));
                }
                let index = (done as usize - self.ov.as_ptr() as usize) / std::mem::size_of::<Ov>();
                let r = requests[index];
                if let Some(counter) = observer {
                    counter.completed(1);
                }
                if ok == 0 {
                    failure.get_or_insert(Failure::Operation {
                        error: io::Error::last_os_error(),
                        write: r.write,
                        offset: r.offset,
                        len: r.len,
                    });
                } else if bytes as usize != r.len {
                    failure.get_or_insert(Failure::Incomplete(bytes as i64, r));
                } else if failure.is_none() {
                    complete(index, r);
                }
            }
            match failure {
                Some(e) => Err(e),
                None => Ok(()),
            }
        }
    }

    impl super::AsyncStream for WinAio {
        fn stream<N, P, C>(
            &mut self,
            ctx: super::StreamCtx<'_>,
            next: N,
            prepare: P,
            complete: C,
        ) -> Result<(), Failure>
        where
            N: FnMut() -> Option<Request>,
            P: FnMut(Request, &mut [u8]),
            C: FnMut(Request),
        {
            self.execute_stream(ctx.ptr, ctx.stride, next, prepare, complete, ctx.observer)
        }
    }

    unsafe fn get_last_error_code() -> i32 {
        io::Error::last_os_error().raw_os_error().unwrap_or(0)
    }

    impl Drop for WinAio {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.port);
            }
        }
    }
}

/// POSIX AIO (macOS and FreeBSD). Each slot is one `aio_write` or `aio_read` the kernel holds, and this thread reaps
/// completions with `aio_suspend`, so the queue depth is not a count of threads.
#[cfg(any(target_os = "macos", target_os = "freebsd"))]
mod posix_aio {
    use super::{AsyncStream, Failure, Request, StreamCtx, StreamStats, LATENCY_CAPACITY};
    use std::{
        ffi::c_void,
        fs::File,
        io,
        os::unix::io::{AsRawFd, RawFd},
        time::{Duration, Instant},
    };

    /// The kernel caps the AIO requests a process can hold, and every worker shares that cap.
    enum Submit {
        Started,
        /// The kernel has no room (EAGAIN). The request was not started and can be retried.
        Full,
    }

    pub struct PosixAio {
        fd: RawFd,
        /// One control block per slot. Boxed, so each address stays fixed while its request is in flight.
        cbs: Box<[libc::aiocb]>,
        inflight: Vec<Option<Request>>,
        submitted_at: Vec<Option<Instant>>,
        /// A request the kernel had no room for, to be started before any new one.
        parked: Option<Request>,
        /// The control blocks still in flight, rebuilt for every `aio_suspend` without allocating.
        suspend: Vec<*const libc::aiocb>,
        stats: StreamStats,
    }

    impl PosixAio {
        pub fn new(file: &File, depth: usize) -> io::Result<Self> {
            Ok(Self {
                fd: file.as_raw_fd(),
                cbs: (0..depth).map(|_| unsafe { std::mem::zeroed() }).collect(),
                inflight: vec![None; depth],
                submitted_at: vec![None; depth],
                parked: None,
                suspend: Vec::with_capacity(depth),
                stats: StreamStats::new(),
            })
        }

        pub fn take_stats(&mut self) -> StreamStats {
            self.stats.take()
        }

        /// Hands one request to the kernel on `slot`, using that slot's control block.
        fn start(
            &mut self,
            slot: usize,
            ptr: *mut u8,
            stride: usize,
            request: Request,
            observer: Option<&crate::progress::Counter>,
        ) -> Result<Submit, Failure> {
            let cb = &mut self.cbs[slot];
            *cb = unsafe { std::mem::zeroed() };
            cb.aio_fildes = self.fd;
            cb.aio_buf = unsafe { ptr.add(slot * stride) } as *mut c_void;
            cb.aio_nbytes = request.len;
            cb.aio_offset = request.offset as libc::off_t;
            let rc = unsafe {
                if request.write {
                    libc::aio_write(cb)
                } else {
                    libc::aio_read(cb)
                }
            };
            if rc != 0 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() == Some(libc::EAGAIN) {
                    return Ok(Submit::Full);
                }
                return Err(Failure::System(error));
            }
            let now = Instant::now();
            self.submitted_at[slot] = Some(now);
            self.stats.change(now, true);
            self.inflight[slot] = Some(request);
            if let Some(counter) = observer {
                counter.submitted(1);
            }
            Ok(Submit::Started)
        }

        /// Blocks until at least one request in flight has finished.
        fn wait(&mut self) -> io::Result<()> {
            self.suspend.clear();
            for (slot, request) in self.inflight.iter().enumerate() {
                if request.is_some() {
                    self.suspend.push(&self.cbs[slot] as *const libc::aiocb);
                }
            }
            loop {
                let rc = unsafe {
                    libc::aio_suspend(
                        self.suspend.as_ptr(),
                        self.suspend.len() as libc::c_int,
                        std::ptr::null(),
                    )
                };
                if rc == 0 {
                    return Ok(());
                }
                let error = io::Error::last_os_error();
                if error.kind() != io::ErrorKind::Interrupted {
                    return Err(error);
                }
            }
        }

        /// Takes the result of `slot` if its request has finished, releasing its control block.
        fn finish(
            &mut self,
            slot: usize,
            observer: Option<&crate::progress::Counter>,
        ) -> Option<(Request, io::Result<usize>)> {
            let request = self.inflight[slot]?;
            let cb = &mut self.cbs[slot];
            if unsafe { libc::aio_error(cb as *const libc::aiocb) } == libc::EINPROGRESS {
                return None;
            }
            let now = Instant::now();
            self.inflight[slot] = None;
            // aio_return must follow aio_error once, so the kernel releases the request.
            let bytes = unsafe { libc::aio_return(cb as *mut libc::aiocb) };
            let error = unsafe { libc::aio_error(cb as *const libc::aiocb) };
            let result = if bytes < 0 {
                Err(io::Error::from_raw_os_error(error))
            } else {
                Ok(bytes as usize)
            };
            self.stats.change(now, false);
            if let Some(counter) = observer {
                counter.completed(1);
            }
            if let Some(start) = self.submitted_at[slot] {
                let latency = now.duration_since(start).as_micros().min(u32::MAX as u128) as u32;
                if self.stats.latencies_us.len() < LATENCY_CAPACITY {
                    self.stats.latencies_us.push(latency);
                }
            }
            Some((request, result))
        }

        /// Cancels every request still in flight and waits until the kernel no longer uses its buffer.
        fn drain(&mut self) {
            for slot in 0..self.cbs.len() {
                if self.inflight[slot].take().is_none() {
                    continue;
                }
                let cb = &self.cbs[slot] as *const libc::aiocb as *mut libc::aiocb;
                unsafe { libc::aio_cancel(self.fd, cb) };
                while unsafe { libc::aio_error(cb) } == libc::EINPROGRESS {
                    std::thread::sleep(Duration::from_millis(1));
                }
                unsafe { libc::aio_return(cb) };
            }
        }

        /// Batch submission: every request is in flight together and the batch waits for all of them.
        pub fn execute<C>(
            &mut self,
            ptr: *mut u8,
            stride: usize,
            requests: &[Request],
            observer: Option<&crate::progress::Counter>,
            mut complete: C,
        ) -> Result<(), Failure>
        where
            C: FnMut(usize, Request),
        {
            let mut outstanding = 0usize;
            let mut failure = None;
            for (slot, &request) in requests.iter().enumerate() {
                loop {
                    match self.start(slot, ptr, stride, request, observer) {
                        Ok(Submit::Started) => {
                            outstanding += 1;
                            break;
                        }
                        Ok(Submit::Full) if outstanding == 0 => {
                            // Another worker holds the kernel's requests; they finish on their own.
                            std::thread::sleep(Duration::from_millis(1));
                        }
                        Ok(Submit::Full) => {
                            if let Err(e) = self.wait() {
                                self.drain();
                                return Err(Failure::System(e));
                            }
                            outstanding -=
                                self.reap(requests.len(), observer, &mut failure, &mut complete);
                        }
                        Err(e) => {
                            self.drain();
                            return Err(e);
                        }
                    }
                }
            }
            while outstanding > 0 {
                if let Err(e) = self.wait() {
                    self.drain();
                    return Err(Failure::System(e));
                }
                outstanding -= self.reap(requests.len(), observer, &mut failure, &mut complete);
            }
            match failure {
                Some(e) => Err(e),
                None => Ok(()),
            }
        }

        /// Settles every finished request among the first `slots`, returning how many there were.
        fn reap<C>(
            &mut self,
            slots: usize,
            observer: Option<&crate::progress::Counter>,
            failure: &mut Option<Failure>,
            complete: &mut C,
        ) -> usize
        where
            C: FnMut(usize, Request),
        {
            let mut done = 0;
            for slot in 0..slots {
                if let Some((request, result)) = self.finish(slot, observer) {
                    done += 1;
                    match outcome(request, result) {
                        Ok(()) if failure.is_none() => complete(slot, request),
                        Ok(()) => {}
                        Err(e) => {
                            failure.get_or_insert(e);
                        }
                    }
                }
            }
            done
        }
    }

    /// Maps a finished request to success, a short transfer or an operation error.
    fn outcome(request: Request, result: io::Result<usize>) -> Result<(), Failure> {
        match result {
            Ok(n) if n == request.len => Ok(()),
            Ok(n) => Err(Failure::Incomplete(n as i64, request)),
            Err(error) => Err(Failure::Operation {
                error,
                write: request.write,
                offset: request.offset,
                len: request.len,
            }),
        }
    }

    impl AsyncStream for PosixAio {
        fn stream<N, P, C>(
            &mut self,
            ctx: StreamCtx<'_>,
            mut next: N,
            mut prepare: P,
            mut complete: C,
        ) -> std::result::Result<(), Failure>
        where
            N: FnMut() -> Option<Request>,
            P: FnMut(Request, &mut [u8]),
            C: FnMut(Request),
        {
            let slots = self.cbs.len();
            let mut outstanding = 0usize;
            let mut exhausted = false;
            let mut failure: Option<Failure> = None;
            loop {
                if failure.is_none() && !exhausted {
                    for slot in 0..slots {
                        if self.inflight[slot].is_some() {
                            continue;
                        }
                        let request = match self.parked.take().or_else(&mut next) {
                            Some(request) => request,
                            None => {
                                exhausted = true;
                                break;
                            }
                        };
                        let buffer = unsafe {
                            std::slice::from_raw_parts_mut(
                                ctx.ptr.add(slot * ctx.stride),
                                request.len,
                            )
                        };
                        prepare(request, buffer);
                        match self.start(slot, ctx.ptr, ctx.stride, request, ctx.observer) {
                            Ok(Submit::Started) => outstanding += 1,
                            Ok(Submit::Full) => {
                                self.parked = Some(request);
                                break;
                            }
                            Err(e) => {
                                failure = Some(e);
                                break;
                            }
                        }
                    }
                }
                if outstanding == 0 {
                    if self.parked.is_some() && failure.is_none() {
                        std::thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                    break;
                }
                if let Err(e) = self.wait() {
                    self.drain();
                    return Err(Failure::System(e));
                }
                for slot in 0..slots {
                    if let Some((request, result)) = self.finish(slot, ctx.observer) {
                        outstanding -= 1;
                        match outcome(request, result) {
                            Ok(()) if failure.is_none() => complete(request),
                            Ok(()) => {}
                            Err(e) => {
                                failure.get_or_insert(e);
                            }
                        }
                    }
                }
            }
            match failure {
                Some(e) => Err(e),
                None => Ok(()),
            }
        }
    }
}

#[cfg(target_os = "linux")]
impl AsyncStream for linux::Aio {
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
        C: FnMut(Request),
    {
        self.execute_stream(
            ctx.fd,
            ctx.ptr,
            ctx.stride,
            ctx.requests,
            ctx.timeout,
            ctx.error_limit,
            ctx.errors,
            ctx.submit_batch,
            ctx.observer,
            next,
            prepare,
            complete,
        )
    }
}

#[cfg(target_os = "linux")]
impl AsyncStream for io_uring::IoUring {
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
        C: FnMut(Request),
    {
        linux::uring_stream(
            self,
            ctx.fd,
            ctx.ptr,
            ctx.stride,
            ctx.requests,
            ctx.timeout,
            ctx.submit_batch,
            ctx.observer,
            next,
            prepare,
            complete,
        )
    }
}
