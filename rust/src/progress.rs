use std::{
    io::{self, Write},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, OnceLock,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
#[repr(align(64))]
#[derive(Default)]
pub struct Counter {
    reads: AtomicU64,
    writes: AtomicU64,
    read_bytes: AtomicU64,
    write_bytes: AtomicU64,
}
impl Counter {
    pub fn publish(&self, reads: u64, writes: u64, read_bytes: u64, write_bytes: u64) {
        self.reads.store(reads, Ordering::Relaxed);
        self.writes.store(writes, Ordering::Relaxed);
        self.read_bytes.store(read_bytes, Ordering::Relaxed);
        self.write_bytes.store(write_bytes, Ordering::Relaxed);
    }
}
pub struct State {
    pub start: OnceLock<Instant>,
    counters: Box<[Counter]>,
    stop: AtomicBool,
}
impl State {
    pub fn counter(&self, index: usize) -> &Counter {
        &self.counters[index]
    }
}
pub struct Reporter {
    pub state: Arc<State>,
    thread: Option<JoinHandle<()>>,
}
impl Reporter {
    pub fn new(workers: usize, interval: f64) -> io::Result<Self> {
        let state = Arc::new(State {
            start: OnceLock::new(),
            counters: (0..workers).map(|_| Counter::default()).collect(),
            stop: AtomicBool::new(false),
        });
        let shared = state.clone();
        let thread = thread::Builder::new().name("spit-progress".into()).spawn(move || {
            let stderr = io::stderr();
            let period = Duration::from_secs_f64(interval);
            let mut previous = (0u64, 0u64, 0u64, 0u64);
            let mut last = 0.;
            while !shared.stop.load(Ordering::Acquire) {
                let Some(start) = shared.start.get() else {
                    thread::park_timeout(Duration::from_millis(10));
                    continue;
                };
                let wait = (last + interval - start.elapsed().as_secs_f64()).max(0.);
                thread::park_timeout(Duration::from_secs_f64(wait).min(period));
                if shared.stop.load(Ordering::Acquire) { break; }
                let elapsed = start.elapsed().as_secs_f64();
                if elapsed < last + interval { continue; }
                let mut total = (0u64, 0u64, 0u64, 0u64);
                for counter in &shared.counters {
                    total.0 += counter.reads.load(Ordering::Relaxed);
                    total.1 += counter.writes.load(Ordering::Relaxed);
                    total.2 += counter.read_bytes.load(Ordering::Relaxed);
                    total.3 += counter.write_bytes.load(Ordering::Relaxed);
                }
                let seconds = (elapsed - last).max(1e-9);
                let operations = total.0.saturating_sub(previous.0) + total.1.saturating_sub(previous.1);
                if writeln!(stderr.lock(), "progress seconds={elapsed:.3} read_MiB_s={:.3} write_MiB_s={:.3} IOPS={:.0} read_ops={} write_ops={}", total.2.saturating_sub(previous.2) as f64 / 1048576. / seconds, total.3.saturating_sub(previous.3) as f64 / 1048576. / seconds, operations as f64 / seconds, total.0, total.1).is_err() { break; }
                previous = total;
                last = elapsed;
            }
        })?;
        Ok(Self {
            state,
            thread: Some(thread),
        })
    }
}
impl Drop for Reporter {
    fn drop(&mut self) {
        self.state.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
            let _ = thread.join();
        }
    }
}
