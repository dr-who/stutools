// POSIX AIO for macOS and FreeBSD.
use super::{AsyncStream, Failure, Request, StreamCtx, StreamStats, LATENCY_CAPACITY};
use crate::cli::Backend;
use std::{
    ffi::c_void,
    fs::File,
    io,
    os::unix::io::{AsRawFd, RawFd},
    time::{Duration, Instant},
};
use std::{fs::OpenOptions, path::Path};

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
    pub fn new(
        file: &File,
        backend: crate::cli::Backend,
        depth: usize,
        _slots: Vec<File>,
    ) -> io::Result<Self> {
        if !matches!(
            backend,
            crate::cli::Backend::Pagecache | crate::cli::Backend::Direct
        ) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "POSIX AIO serves pagecache and direct I/O only",
            ));
        }
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
                        std::slice::from_raw_parts_mut(ctx.ptr.add(slot * ctx.stride), request.len)
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

impl PosixAio {
    pub fn default_backend() -> Backend {
        Backend::Pagecache
    }

    /// FreeBSD direct I/O is O_DIRECT, set at open. macOS uses F_NOCACHE, set after open (see below).
    #[cfg(target_os = "freebsd")]
    pub fn configure_open(o: &mut OpenOptions, path: &Path, backend: Backend) -> io::Result<()> {
        use std::os::unix::fs::OpenOptionsExt;
        if backend != Backend::Pagecache && super::is_direct_target(path) {
            o.custom_flags(libc::O_DIRECT);
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    pub fn configure_open(_o: &mut OpenOptions, _path: &Path, _backend: Backend) -> io::Result<()> {
        Ok(())
    }

    /// F_NOCACHE is macOS's direct I/O: bypass the unified buffer cache.
    #[cfg(target_os = "macos")]
    pub fn after_open(file: &File, _path: &Path, backend: Backend) -> io::Result<()> {
        use std::os::unix::io::AsRawFd;
        if backend == Backend::Direct
            && unsafe { libc::fcntl(file.as_raw_fd(), libc::F_NOCACHE, 1) } != 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    #[cfg(target_os = "freebsd")]
    pub fn after_open(_file: &File, _path: &Path, _backend: Backend) -> io::Result<()> {
        Ok(())
    }

    pub fn open_slots(
        _path: &Path,
        _write: bool,
        _backend: Backend,
        _depth: usize,
    ) -> io::Result<Vec<File>> {
        Ok(Vec::new())
    }

    pub fn check_target(_path: &Path, _write: bool) -> crate::Result<()> {
        Ok(())
    }
}

impl super::AsyncIo for PosixAio {
    fn stream<N, P, C>(
        &mut self,
        ctx: super::StreamCtx<'_>,
        next: N,
        prepare: P,
        complete: C,
    ) -> std::result::Result<(), Failure>
    where
        N: FnMut() -> Option<Request>,
        P: FnMut(Request, &mut [u8]),
        C: FnMut(Request),
    {
        super::AsyncStream::stream(self, ctx, next, prepare, complete)
    }

    fn execute<C>(
        &mut self,
        ctx: super::StreamCtx<'_>,
        complete: C,
    ) -> std::result::Result<(), Failure>
    where
        C: FnMut(usize, Request),
    {
        self.run_batch(ctx.ptr, ctx.stride, ctx.requests, ctx.observer, complete)
    }

    fn take_stats(&mut self) -> Option<StreamStats> {
        Some(self.stats.take())
    }
}
