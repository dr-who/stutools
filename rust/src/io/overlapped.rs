// Windows asynchronous I/O: overlapped files on one completion port.
//! Windows overlapped I/O: a batch of requests is submitted at once, each with its own
//! OVERLAPPED, and the completion port reports them as they finish.
use super::{Failure, Request, StreamStats, LATENCY_CAPACITY};
use crate::cli::Backend;
use std::ffi::c_void;
use std::fs::File;
use std::fs::OpenOptions;
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
    fn ReadFile(file: *mut c_void, buf: *mut u8, count: u32, read: *mut u32, ov: *mut Ov) -> i32;
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
    pub fn new(
        file: &File,
        _backend: crate::cli::Backend,
        _depth: usize,
        slots: Vec<File>,
    ) -> io::Result<Self> {
        let files = if slots.is_empty() {
            vec![file.try_clone()?]
        } else {
            slots
        };
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
                let latency = now.duration_since(start).as_micros().min(u32::MAX as u128) as u32;
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
    pub fn run_batch<C>(
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

impl WinAio {
    /// Windows always submits overlapped; direct I/O is the default because it matches the C backend.
    pub fn default_backend() -> Backend {
        Backend::Direct
    }

    pub fn configure_open(
        o: &mut OpenOptions,
        _path: &std::path::Path,
        backend: Backend,
    ) -> io::Result<()> {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_FLAG_OVERLAPPED: every Windows backend submits requests asynchronously.
        let mut flags = 0x4000_0000;
        if backend == Backend::Direct {
            // FILE_FLAG_NO_BUFFERING: bypass the page cache. Buffers, offsets and lengths
            // must be sector-aligned, which the engine already checks for direct backends.
            flags |= 0x2000_0000;
        }
        o.custom_flags(flags);
        Ok(())
    }

    pub fn after_open(file: &File, path: &std::path::Path, _backend: Backend) -> io::Result<()> {
        super::windows::prepare_handle(path, file)
    }

    pub fn open_slots(
        path: &std::path::Path,
        write: bool,
        backend: Backend,
        depth: usize,
    ) -> io::Result<Vec<File>> {
        if depth == 0 {
            return Ok(Vec::new());
        }
        (0..depth)
            .map(|_| super::open(path, write, false, backend))
            .collect()
    }

    pub fn check_target(path: &std::path::Path, write: bool) -> crate::Result<()> {
        super::windows::validate_target(path, write)
    }
}

impl super::AsyncIo for WinAio {
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
        super::AsyncStream::stream(self, ctx, next, prepare, complete)
    }

    fn execute<C>(&mut self, ctx: super::StreamCtx<'_>, complete: C) -> Result<(), Failure>
    where
        C: FnMut(usize, Request),
    {
        self.run_batch(ctx.ptr, ctx.stride, ctx.requests, ctx.observer, complete)
    }

    fn take_stats(&mut self) -> Option<StreamStats> {
        Some(self.stats.take())
    }
}
