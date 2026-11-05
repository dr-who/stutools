//! The queue: one worker's buffers and in-flight requests, run on the platform backend.

use super::{AsyncIo, Failure, Native, Next, Request, StreamCtx, StreamStats};
use crate::cli::Backend;
use std::{
    alloc::{alloc_zeroed, dealloc, Layout},
    fs::File,
    io,
    ptr::NonNull,
    time::Duration,
};

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
pub struct Queue {
    native: Option<Native>,
    arena: Option<Arena>,
    pub requests: Box<[Request]>,
    stride: usize,
    /// The file the native backend submits to; None for an iSCSI connection.
    file: Option<File>,
    /// Set for a path workload: the queue's requests run through the pool instead.
    connection: Option<crate::paths::MultiPool>,
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
            file: Some(file),
            connection: None,
            observer: None,
            timeout: Duration::from_secs_f64(timeout),
            submit_errors: 0,
        })
    }
    /// A queue whose requests run over an iSCSI connection, with the same buffers as a file queue.
    pub fn over_connection(
        mut connection: crate::paths::MultiPool,
        depth: usize,
        max_len: usize,
        timeout: f64,
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
        connection.reserve(depth);
        Ok(Self {
            native: None,
            arena: Some(arena),
            requests: vec![Request::default(); depth].into_boxed_slice(),
            stride,
            submit_batch: depth,
            file: None,
            connection: Some(connection),
            observer: None,
            timeout: Duration::from_secs_f64(timeout),
            submit_errors: 0,
        })
    }
    /// The start of the arena: slot `i`'s buffer begins `i * stride()` bytes in. Valid while the queue lives.
    pub fn arena_base(&self) -> *mut u8 {
        self.arena.as_ref().unwrap().ptr.as_ptr()
    }
    /// The number of endpoints the queue's requests run over: 1 for a file or a single connection.
    pub fn endpoints(&self) -> usize {
        self.connection.as_ref().map_or(1, |c| c.endpoints())
    }

    /// Lets write-verify read-backs run while their writes are in flight. Only a connection uses it.
    pub fn set_race(&mut self, on: bool) {
        if let Some(connection) = self.connection.as_mut() {
            connection.set_race(on);
        }
    }
    pub fn stride(&self) -> usize {
        self.stride
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
        match &self.file {
            Some(file) => file.sync_data(),
            None => Ok(()),
        }
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
        N: FnMut() -> Next,
        P: FnMut(Request, &mut [u8]),
        C: FnMut(usize, Request),
    {
        let fd = self.file.as_ref().map_or(-1, raw_fd);
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
        match (self.connection.as_mut(), self.native.as_mut()) {
            (Some(connection), _) => AsyncIo::stream(connection, ctx, next, prepare, complete),
            (None, Some(native)) => AsyncIo::stream(native, ctx, next, prepare, complete),
            (None, None) => Err(Failure::from(io::Error::other("queue has no backend"))),
        }
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
        let fd = self.file.as_ref().map_or(-1, raw_fd);
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
        match (self.connection.as_mut(), self.native.as_mut()) {
            (Some(connection), _) => AsyncIo::execute(connection, ctx, complete),
            (None, Some(native)) => AsyncIo::execute(native, ctx, complete),
            (None, None) => Err(Failure::from(io::Error::other("queue has no backend"))),
        }
    }

    pub fn close(&mut self) -> bool {
        if self.connection.is_some() {
            return true;
        }
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
