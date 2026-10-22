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
    in_flight: AtomicU64,
    finished: AtomicBool,
}
impl Counter {
    pub fn publish(&self, reads: u64, writes: u64, read_bytes: u64, write_bytes: u64) {
        self.reads.store(reads, Ordering::Relaxed);
        self.writes.store(writes, Ordering::Relaxed);
        self.read_bytes.store(read_bytes, Ordering::Relaxed);
        self.write_bytes.store(write_bytes, Ordering::Relaxed);
    }
    pub fn submitted(&self, count: usize) {
        // One worker writes each counter; the reporter only reads it.
        let pending = self.in_flight.load(Ordering::Relaxed);
        self.in_flight
            .store(pending + count as u64, Ordering::Relaxed);
    }
    pub fn completed(&self, count: usize) {
        let pending = self.in_flight.load(Ordering::Relaxed);
        self.in_flight
            .store(pending - count as u64, Ordering::Relaxed);
    }
    pub fn finish(&self) {
        self.in_flight.store(0, Ordering::Relaxed);
        self.finished.store(true, Ordering::Release);
    }
}

// Match C's commaPrint0dp, including its seven-column padding and truncation
// above 1000. Format directly to the reporter's output without heap strings.
struct Commas(f64);
impl std::fmt::Display for Commas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = self.0;
        let n = value as u64;
        if value >= 1_000_000_000. {
            write!(
                f,
                "{},{:03},{:03},{:03}",
                n / 1_000_000_000,
                n / 1_000_000 % 1000,
                n / 1000 % 1000,
                n % 1000
            )
        } else if value >= 1_000_000. {
            write!(
                f,
                "{},{:03},{:03}",
                n / 1_000_000,
                n / 1000 % 1000,
                n % 1000
            )
        } else if value >= 1000. {
            write!(f, "{:3},{:03}", n / 1000, n % 1000)
        } else {
            write!(f, "{value:7.0}")
        }
    }
}
#[derive(Clone, Copy, Default)]
struct Sample {
    reads: u64,
    writes: u64,
    read_bytes: u64,
    write_bytes: u64,
    workers: usize,
    in_flight: u64,
}
fn write_line(
    out: &mut impl Write,
    elapsed: f64,
    seconds: f64,
    total: Sample,
    previous: Sample,
) -> io::Result<()> {
    let reads = total.reads.saturating_sub(previous.reads);
    let writes = total.writes.saturating_sub(previous.writes);
    let read_bytes = total.read_bytes.saturating_sub(previous.read_bytes);
    let write_bytes = total.write_bytes.saturating_sub(previous.write_bytes);
    writeln!(out, "[{elapsed:2.2} / {} / {}] read {} MB/s ({} IOPS / {:.0}), write {} MB/s ({} IOPS / {:.0}), total {:.2} GiB",
        total.workers, total.in_flight,
        Commas(read_bytes as f64 / 1_000_000. / seconds), Commas(reads as f64 / seconds),
        if reads == 0 { 0. } else { read_bytes as f64 / reads as f64 },
        Commas(write_bytes as f64 / 1_000_000. / seconds), Commas(writes as f64 / seconds),
        if writes == 0 { 0. } else { write_bytes as f64 / writes as f64 },
        (total.read_bytes + total.write_bytes) as f64 / 1073741824.)
}

fn write_pending_notice(out: &mut impl Write, total: Sample, elapsed: f64) -> io::Result<()> {
    if total.reads == 0 && total.writes == 0 && total.in_flight > 0 {
        writeln!(out, "*info* no I/O completed within {elapsed:.2} seconds; {} requests are still pending in the OS/driver. Waiting for submitted I/O; synchronous calls cannot be stopped by -t/-T. For file tests, try a NEW prefix with -F PREFIX -G 1MiB -c ws1x1 first; -G limits the test range but does not shrink an existing file", total.in_flight)?;
    }
    Ok(())
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
    pub fn new(workers: usize, interval: f64, duration: Option<f64>) -> io::Result<Self> {
        let state = Arc::new(State {
            start: OnceLock::new(),
            counters: (0..workers).map(|_| Counter::default()).collect(),
            stop: AtomicBool::new(false),
        });
        let shared = state.clone();
        let thread = thread::Builder::new()
            .name("spit-progress".into())
            .spawn(move || {
                let stderr = io::stderr();
                let period = Duration::from_secs_f64(interval);
                let mut previous = Sample::default();
                let mut last = 0.;
                while !shared.stop.load(Ordering::Acquire) {
                    let Some(start) = shared.start.get() else {
                        thread::park_timeout(Duration::from_millis(10));
                        continue;
                    };
                    let next =
                        duration.map_or(last + interval, |limit| (last + interval).min(limit));
                    let wait = (next - start.elapsed().as_secs_f64()).max(0.);
                    thread::park_timeout(Duration::from_secs_f64(wait).min(period));
                    if shared.stop.load(Ordering::Acquire) {
                        break;
                    }
                    let actual = start.elapsed().as_secs_f64();
                    if actual < next {
                        continue;
                    }
                    let mut total = Sample::default();
                    for counter in &shared.counters {
                        total.reads += counter.reads.load(Ordering::Relaxed);
                        total.writes += counter.writes.load(Ordering::Relaxed);
                        total.read_bytes += counter.read_bytes.load(Ordering::Relaxed);
                        total.write_bytes += counter.write_bytes.load(Ordering::Relaxed);
                        total.in_flight += counter.in_flight.load(Ordering::Relaxed);
                        total.workers += usize::from(!counter.finished.load(Ordering::Acquire));
                    }
                    if total.workers == 0 {
                        break;
                    }
                    let elapsed = duration.map_or(actual, |limit| actual.min(limit));
                    let seconds = (elapsed - last).max(1e-9);
                    if write_line(&mut stderr.lock(), elapsed, seconds, total, previous).is_err() {
                        break;
                    }
                    previous = total;
                    last = elapsed;
                    if duration.is_some_and(|limit| actual >= limit) {
                        let _ = write_pending_notice(&mut stderr.lock(), total, elapsed);
                        break;
                    }
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
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zero_iops_with_pending_io_explains_the_wait() {
        let mut output = Vec::new();
        write_pending_notice(
            &mut output,
            Sample {
                in_flight: 1,
                ..Sample::default()
            },
            2.,
        )
        .unwrap();
        let notice = String::from_utf8(output).unwrap();
        assert!(
            notice.contains("no I/O completed within 2.00 seconds") && notice.contains("OS/driver")
        );
        assert!(notice.contains("NEW prefix") && notice.contains("does not shrink"));
        for sample in [
            Sample::default(),
            Sample {
                writes: 1,
                in_flight: 1,
                ..Sample::default()
            },
        ] {
            let mut output = Vec::new();
            write_pending_notice(&mut output, sample, 2.).unwrap();
            assert!(output.is_empty());
        }
    }
    #[test]
    fn matches_c_progress_layout_units_and_padding() {
        let mut output = Vec::new();
        write_line(
            &mut output,
            1.,
            1.,
            Sample {
                reads: 100_991,
                read_bytes: 413_659_136,
                workers: 1,
                in_flight: 6,
                ..Sample::default()
            },
            Sample::default(),
        )
        .unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "[1.00 / 1 / 6] read     414 MB/s (100,991 IOPS / 4096), write       0 MB/s (      0 IOPS / 0), total 0.39 GiB\n");
        assert_eq!(format!("{}", Commas(1234.9)), "  1,234");
        assert_eq!(format!("{}", Commas(1_234_567.9)), "1,234,567");
        assert_eq!(format!("{}", Commas(1_234_567_890.9)), "1,234,567,890");
    }
    #[test]
    fn fractional_intervals_use_deltas_and_keep_cumulative_total() {
        let previous = Sample {
            reads: 1_000_000,
            read_bytes: 4_096_000_000,
            ..Sample::default()
        };
        let total = Sample {
            reads: 1_100_000,
            writes: 10_000,
            read_bytes: 4_505_600_000,
            write_bytes: 655_360_000,
            workers: 32,
            in_flight: 2048,
        };
        let mut output = Vec::new();
        write_line(&mut output, 1.1, 0.1, total, previous).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "[1.10 / 32 / 2048] read   4,096 MB/s (1,000,000 IOPS / 4096), write   6,553 MB/s (100,000 IOPS / 65536), total 4.81 GiB\n");
    }
}
