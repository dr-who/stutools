use crate::cli::Backend;
#[cfg(windows)]
#[path = "windows.rs"]
mod windows;
use std::{
    alloc::{alloc_zeroed, dealloc, Layout},
    fs::{File, OpenOptions},
    io,
    ptr::NonNull,
};
#[cfg(windows)]
use std::time::Duration;
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
    #[cfg(all(not(windows), not(target_os = "linux")))]
    {
        Backend::Pagecache
    }
}

/// One handle per queue slot on Windows, so in-flight requests do not share a file object.
/// Other platforms use the single handle the caller already has.
pub fn open_slots(path: &std::path::Path, write: bool, backend: Backend, depth: usize) -> io::Result<Vec<File>> {
    if !cfg!(windows) || depth == 0 {
        return Ok(Vec::new());
    }
    (0..depth).map(|_| open(path, write, false, backend)).collect()
}

pub fn open(
    path: &std::path::Path,
    write: bool,
    create: bool,
    backend: Backend,
) -> io::Result<File> {
    let mut o = OpenOptions::new();
    o.read(true).write(write).create_new(create);
    #[cfg(target_os = "linux")]
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
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
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
#[derive(Clone, Copy, Default)]
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
    Incomplete(i64, usize),
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
            Self::Incomplete(n, w) => write!(f, "incomplete I/O: result {n}, expected {w}"),
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
    pending: bool,
}
// One enum per worker; keep the queue inline instead of allocating another box.
#[allow(clippy::large_enum_variant)]
enum State {
    #[cfg(all(not(windows), not(target_os = "linux")))]
    Threads(threads::Pool),
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
            _ => State::Overlapped(overlapped::WinAio::new(if slots.is_empty() { vec![file.try_clone()?] } else { slots })?),
            // Linux pagecache and direct I/O both go through the async backends; direct sets O_DIRECT in open().
            #[cfg(target_os = "linux")]
            Backend::Pagecache | Backend::Direct => match default_backend() {
                Backend::Uring => State::Uring(io_uring::IoUring::new(depth as u32)?),
                _ => State::Aio(linux::Aio::new(depth)?),
            },
            #[cfg(all(not(windows), not(target_os = "linux")))]
            Backend::Pagecache | Backend::Direct => State::Threads(threads::Pool::new(&file, depth)?),
            #[cfg(target_os = "linux")]
            Backend::Libaio => State::Aio(linux::Aio::new(depth)?),
            #[cfg(target_os = "linux")]
            Backend::Uring => State::Uring(io_uring::IoUring::new(depth as u32)?),
            #[cfg(all(not(windows), not(target_os = "linux")))]
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "Linux backend requested on another OS",
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
    /// Windows only: the in-flight depth and per-request latency measured by the last streamed run.
    #[cfg(windows)]
    pub(crate) fn take_overlapped_stats(&mut self) -> Option<overlapped::Stats> {
        match self.state.as_mut()? {
            State::Overlapped(port) => Some(port.take_stats()),
            _ => None,
        }
    }

    /// Streams requests through the platform's asynchronous backend. Linux uses libaio or io_uring and
    /// Windows uses WinAio; each implements `AsyncStream`, so this is the same code on both.
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
            #[cfg(all(not(windows), not(target_os = "linux")))]
            State::Threads(_) => Err(io::Error::other("streaming requires an asynchronous backend").into()),
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
                port.execute(&self.file, ptr, stride, &self.requests[..count], observer, |i, r| complete(i, r))?;
            }
            #[cfg(all(not(windows), not(target_os = "linux")))]
            State::Threads(pool) => {
                pool.execute(ptr, stride, &self.requests[..count], observer, |i, r| complete(i, r))?;
            }
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
                            failure = Some(Failure::Incomplete(cqe.result() as i64, expected));
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
                #[cfg(all(not(windows), not(target_os = "linux")))]
                State::Threads(_) => {}
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
                    return Err(Failure::Incomplete(cqe.result() as i64, slot.len));
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
                                failure = Some(Failure::Incomplete(e.result, r.len));
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
                                return Err(Failure::Incomplete(event.result, slot.len));
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
    use super::{Failure, Request};
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
        fn NtSetInformationProcess(process: *mut c_void, class: u32, info: *const c_void, length: u32) -> i32;
        fn GetCurrentProcess() -> *mut c_void;
        fn GetCurrentProcessId() -> u32;
        fn GetPriorityClass(process: *mut c_void) -> u32;
        fn GetProcessAffinityMask(process: *mut c_void, process_mask: *mut usize, system_mask: *mut usize) -> i32;
        fn ProcessIdToSessionId(pid: u32, session: *mut u32) -> i32;
        fn CreateIoCompletionPort(file: *mut c_void, existing: *mut c_void, key: usize, threads: u32) -> *mut c_void;
        fn ReadFile(file: *mut c_void, buf: *mut u8, count: u32, read: *mut u32, ov: *mut Ov) -> i32;
        fn WriteFile(file: *mut c_void, buf: *const u8, count: u32, written: *mut u32, ov: *mut Ov) -> i32;
        fn GetQueuedCompletionStatus(port: *mut c_void, bytes: *mut u32, key: *mut usize, ov: *mut *mut Ov, timeout: u32) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }

    const ERROR_IO_PENDING: i32 = 997;
    const INFINITE: u32 = 0xFFFF_FFFF;

    /// Windows asynchronous backend: overlapped I/O on one completion port, one file handle per slot.
    /// Measured in-flight depth and per-request latency of a streamed run.
    #[derive(Default)]
    pub struct Stats {
        pub max_in_flight: u64,
        /// Sum over time of the in-flight count, in count x nanoseconds.
        pub area_ns: u128,
        pub time_ns: u128,
        pub latencies_us: Vec<u32>,
    }

    /// Latencies kept per run, the same bound the engine uses for its own samples.
    const LATENCY_CAPACITY: usize = 2_000_000;

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
        in_flight: u64,
        stats: Stats,
        last_change: Option<Instant>,
    }

    impl WinAio {
        pub fn new(files: Vec<File>) -> io::Result<Self> {
            let mut port: *mut c_void = std::ptr::null_mut();
            let mut handles = Vec::with_capacity(files.len());
            for file in &files {
                let handle = file.as_raw_handle() as *mut c_void;
                // The first association creates the completion port; the others join it.
                let associated = unsafe { CreateIoCompletionPort(handle, port, 0, 0) };
                if associated.is_null() {
                    return Err(io::Error::last_os_error());
                }
                port = associated;
                handles.push(handle);
            }
            if handles.is_empty() {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "overlapped I/O needs at least one slot"));
            }
            let depth = handles.len();
            let zero = Ov { internal: 0, internal_high: 0, offset: 0, offset_high: 0, event: std::ptr::null_mut() };
            Ok(Self {
                files,
                handles,
                port,
                ov: vec![zero; depth].into_boxed_slice(),
                submitted_at: vec![None; depth],
                trace: std::env::var_os("SPIT_TRACE").is_some(),
                inflight: vec![None; depth],
                in_flight: 0,
                // Reserved up front so the run itself never grows the vector.
                stats: Stats { latencies_us: Vec::with_capacity(LATENCY_CAPACITY), ..Stats::default() },
                last_change: None,
            })
        }

        /// Starts one request on `slot`, using that slot's OVERLAPPED and file handle.
        fn submit(&mut self, slot: usize, ptr: *mut u8, stride: usize, r: Request) -> Result<(), Failure> {
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
            if ok == 0 && unsafe { GetLastErrorCode() } != ERROR_IO_PENDING {
                return Err(Failure::Operation {
                    error: io::Error::last_os_error(),
                    write: r.write,
                    offset: r.offset,
                    len: r.len,
                });
            }
            Ok(())
        }

        /// Moves the in-flight count by one, first adding the time spent at the previous count.
        fn change(&mut self, now: Instant, submitted: bool) {
            if let Some(last) = self.last_change {
                let dt = now.duration_since(last).as_nanos();
                self.stats.area_ns += dt * self.in_flight as u128;
                self.stats.time_ns += dt;
            }
            self.last_change = Some(now);
            if submitted {
                self.in_flight += 1;
                self.stats.max_in_flight = self.stats.max_in_flight.max(self.in_flight);
            } else {
                self.in_flight -= 1;
            }
        }

        pub fn take_stats(&mut self) -> Stats {
            self.last_change = None;
            self.in_flight = 0;
            std::mem::take(&mut self.stats)
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
                    NtSetInformationProcess(process, 33, &io_priority as *const u32 as *const c_void, 4),
                )
            };
            if trace {
                eprintln!("trace priority set_class_ok={} io_priority_status=0x{:x}", class_ok, io_status);
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
                    for slot in 0..slots {
                        if inflight[slot].is_some() {
                            continue;
                        }
                        let Some(request) = next() else {
                            exhausted = true;
                            break;
                        };
                        let buffer = unsafe { std::slice::from_raw_parts_mut(ptr.add(slot * stride), request.len) };
                        prepare(request, buffer);
                        let now = Instant::now();
                        if let Err(e) = self.submit(slot, ptr, stride, request) {
                            failure = Some(e);
                            break;
                        }
                        self.submitted_at[slot] = Some(now);
                        self.change(now, true);
                        inflight[slot] = Some(request);
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
                let ok = unsafe { GetQueuedCompletionStatus(self.port, &mut bytes, &mut key, &mut done, INFINITE) };
                if done.is_null() {
                    return Err(Failure::System(io::Error::last_os_error()));
                }
                let now = Instant::now();
                let slot = (done as usize - self.ov.as_ptr() as usize) / std::mem::size_of::<Ov>();
                let request = inflight[slot].take().expect("completion for a slot with nothing in flight");
                outstanding -= 1;
                self.change(now, false);
                if let Some(counter) = observer {
                    counter.completed(1);
                }
                if let Some(start) = self.submitted_at[slot] {
                    let latency = now.duration_since(start).as_micros().min(u32::MAX as u128) as u32;
                    if trace {
                        eprintln!(
                            "trace t_ms={} slot={} write={} offset={} latency_us={} in_flight={}",
                            started.elapsed().as_millis(),
                            slot,
                            request.write,
                            request.offset,
                            latency,
                            self.in_flight
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
                    failure.get_or_insert(Failure::Incomplete(bytes as i64, request.len));
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
                let ok = unsafe { GetQueuedCompletionStatus(self.port, &mut bytes, &mut key, &mut done, INFINITE) };
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
                    failure.get_or_insert(Failure::Incomplete(bytes as i64, r.len));
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

    unsafe fn GetLastErrorCode() -> i32 {
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

/// macOS (and other non-Linux, non-Windows Unix) async I/O: one thread per queue slot, each doing
/// positional reads or writes, so up to `depth` requests are in flight at once.
#[cfg(all(not(windows), not(target_os = "linux")))]
mod threads {
    use super::{Failure, Request};
    use std::fs::File;
    use std::io;
    use std::os::unix::fs::FileExt;
    use std::sync::{mpsc, Arc};
    use std::thread::JoinHandle;

    struct Job {
        index: usize,
        ptr: usize,
        len: usize,
        offset: u64,
        write: bool,
    }

    pub struct Pool {
        jobs: Vec<mpsc::Sender<Job>>,
        done: mpsc::Receiver<(usize, io::Result<usize>)>,
        handles: Vec<JoinHandle<()>>,
    }

    impl Pool {
        pub fn new(file: &File, depth: usize) -> io::Result<Self> {
            let file = Arc::new(file.try_clone()?);
            let (done_tx, done) = mpsc::channel();
            let mut jobs = Vec::with_capacity(depth);
            let mut handles = Vec::with_capacity(depth);
            for _ in 0..depth {
                let (tx, rx) = mpsc::channel::<Job>();
                let file = file.clone();
                let done_tx = done_tx.clone();
                handles.push(std::thread::spawn(move || {
                    for job in rx {
                        let buf = unsafe { std::slice::from_raw_parts_mut(job.ptr as *mut u8, job.len) };
                        let result = if job.write {
                            file.write_at(buf, job.offset)
                        } else {
                            file.read_at(buf, job.offset)
                        };
                        if done_tx.send((job.index, result)).is_err() {
                            break;
                        }
                    }
                }));
                jobs.push(tx);
            }
            Ok(Self { jobs, done, handles })
        }

        /// Submits the batch to the slot threads and waits for every completion.
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
            for (i, r) in requests.iter().enumerate() {
                if let Some(counter) = observer {
                    counter.submitted(1);
                }
                let job = Job {
                    index: i,
                    ptr: ptr as usize + i * stride,
                    len: r.len,
                    offset: r.offset,
                    write: r.write,
                };
                self.jobs[i]
                    .send(job)
                    .map_err(|_| Failure::System(io::Error::other("I/O thread stopped")))?;
            }
            let mut failure = None;
            for _ in 0..requests.len() {
                let (index, result) = self
                    .done
                    .recv()
                    .map_err(|_| Failure::System(io::Error::other("I/O thread stopped")))?;
                let r = requests[index];
                if let Some(counter) = observer {
                    counter.completed(1);
                }
                match result {
                    Ok(n) if n == r.len => {
                        if failure.is_none() {
                            complete(index, r);
                        }
                    }
                    Ok(n) => {
                        failure.get_or_insert(Failure::Incomplete(n as i64, r.len));
                    }
                    Err(error) => {
                        failure.get_or_insert(Failure::Operation {
                            error,
                            write: r.write,
                            offset: r.offset,
                            len: r.len,
                        });
                    }
                }
            }
            match failure {
                Some(e) => Err(e),
                None => Ok(()),
            }
        }
    }

    impl Drop for Pool {
        fn drop(&mut self) {
            self.jobs.clear();
            for handle in self.handles.drain(..) {
                let _ = handle.join();
            }
        }
    }
}

#[cfg(target_os = "linux")]
impl AsyncStream for linux::Aio {
    fn stream<N, P, C>(&mut self, ctx: StreamCtx<'_>, next: N, prepare: P, complete: C) -> std::result::Result<(), Failure>
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
    fn stream<N, P, C>(&mut self, ctx: StreamCtx<'_>, next: N, prepare: P, complete: C) -> std::result::Result<(), Failure>
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
