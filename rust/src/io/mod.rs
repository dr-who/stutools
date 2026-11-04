use crate::cli::Backend;

mod queue;
mod target;

pub use queue::Queue;
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub(crate) use target::is_direct_target;
pub use target::{inspect, open, open_slots, validate_target};
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
use std::{
    io,
    time::{Duration, Instant},
};

/// The backend a `--backend auto` run picks on this platform.
pub fn default_backend() -> Backend {
    Native::default_backend()
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
