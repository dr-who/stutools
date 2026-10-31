#[path = "cctv_files.rs"]
pub(crate) mod files;
use crate::signals;
#[path = "cctv_stats.rs"]
mod stats;
use crate::cctv_plots;
use crate::{allocation, cli::Config, progress, rate, Result};
use stats::{Metrics, MetricsStore, Periods};
use std::{
    fs::{File, OpenOptions},
    io::{self, BufWriter, Write},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc, Arc, Condvar, Mutex, OnceLock,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub count: usize,
    pub mbps: f64,
    pub hours: f64,
    pub block: usize,
    pub file_size: u64,
    pub direct: bool,
    /// Last camera count of a COUNT..END/STEP sweep; equals `count` for a single run.
    pub count_end: usize,
    pub count_step: usize,
    /// Ratio between successive counts in a geometric range; 1 for linear ranges.
    pub count_ratio: f64,
    /// A START..END range writes a per-write latency file for each camera count.
    pub ranged: bool,
    /// Fraction of the combined camera rate the last 10 s must reach to keep running.
    pub floor: f64,
    /// Link capacity in Mbit/s, drawn on the rate chart and used to report how many cameras fit.
    pub link_mbps: Option<f64>,
    /// Seconds a generated frame may wait for its write before it is dropped.
    pub drop_after: f64,
}
/// Parses `N` or `START..END/STEP` (step defaults to 1) into (start, end, step).
/// Parses COUNT as N, START..END/STEP (linear) or START..END*RATIO (geometric, e.g. 10..200*sqrt2).
/// Returns (start, end, linear step, ratio); ratio is 1 for linear counts.
fn parse_cameras(text: &str) -> Result<(usize, usize, usize, f64)> {
    const USAGE: &str = "--cctv needs COUNT,MBPS,HOURS; COUNT is N, START..END/STEP (e.g. 10..100/10) or START..END*RATIO (e.g. 10..200*sqrt2)";
    let (text, ratio) = match text.split_once('*') {
        Some((span, ratio)) => {
            let ratio = match ratio {
                "sqrt2" => std::f64::consts::SQRT_2,
                other => other.parse::<f64>().map_err(|_| USAGE)?,
            };
            if !(ratio > 1. && ratio.is_finite()) {
                return Err(USAGE.into());
            }
            (span, ratio)
        }
        None => (text, 1.),
    };
    let (span, step) = match text.split_once('/') {
        Some((span, step)) => (span, step.parse::<usize>().map_err(|_| USAGE)?),
        None => (text, 1),
    };
    let (start, end) = match span.split_once("..") {
        Some((a, b)) => (
            a.parse::<usize>().map_err(|_| USAGE)?,
            b.parse::<usize>().map_err(|_| USAGE)?,
        ),
        None => {
            let n = span.parse::<usize>().map_err(|_| USAGE)?;
            (n, n)
        }
    };
    if !(1..=1024).contains(&start) || !(start..=1024).contains(&end) || step == 0 {
        return Err(USAGE.into());
    }
    Ok((start, end, step, ratio))
}
/// The camera counts a range runs: every linear step, or each count a ratio times the last.
/// The end is always included.
pub fn camera_counts(s: &Settings) -> Vec<usize> {
    if s.count_ratio == 1. {
        return (s.count..=s.count_end).step_by(s.count_step).collect();
    }
    let mut counts = Vec::new();
    let mut x = s.count as f64;
    while x <= s.count_end as f64 + 0.5 {
        let n = x.round() as usize;
        if counts.last() != Some(&n) {
            counts.push(n);
        }
        x *= s.count_ratio;
    }
    if counts.last() != Some(&s.count_end) {
        counts.push(s.count_end);
    }
    counts
}
impl Settings {
    pub fn parse(text: &str) -> Result<Self> {
        let mut parts = text.split(',');
        let cameras_text = parts.next().ok_or("--cctv needs COUNT,MBPS,HOURS")?;
        let (count, count_end, count_step, count_ratio) = parse_cameras(cameras_text)?;
        let mbps = parts
            .next()
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or("--cctv needs COUNT,MBPS,HOURS")?;
        let hours = parts
            .next()
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or("--cctv needs COUNT,MBPS,HOURS")?;
        if parts.next().is_some()
            || !mbps.is_finite()
            || !(0.000008..=8_000_000.).contains(&mbps)
            || !hours.is_finite()
            || hours <= 0.
            || hours > 1e9
        {
            return Err(
                "--cctv needs a finite positive Mbit/s rate and positive retention hours".into(),
            );
        }
        Ok(Self {
            count,
            mbps,
            hours,
            block: 65536,
            file_size: 16 * 1048576,
            direct: cfg!(any(windows, target_os = "linux")),
            count_end,
            count_step,
            count_ratio,
            ranged: cameras_text.contains(".."),
            floor: RATE_FLOOR,
            link_mbps: Some(1000.),
            drop_after: DROP_AFTER_S,
        })
    }
    fn pool_bytes(self) -> Option<u64> {
        (self.count as u64)
            .checked_mul(self.block as u64 + 2048)?
            .checked_add(65536 + 3 * 8192 + 4096)
    }
    pub fn validate(self, memory: u64) -> Result<()> {
        if self.block < 64
            || self.block > 0x7ffff000
            || self.file_size == 0
            || self.file_size > i64::MAX as u64
            || self.file_size % self.block as u64 != 0
        {
            return Err("CCTV requires a fixed block of at least 64 bytes and a positive --filesizemb size divisible by the block size".into());
        }
        if self.direct && (self.block % 4096 != 0 || self.file_size % 4096 != 0) {
            return Err("--cctv-io direct needs block and --filesizemb sizes that are multiples of 4096 bytes".into());
        }
        let required = self.pool_bytes().ok_or("CCTV memory overflow")?;
        if required > memory {
            return Err(crate::memory::error(
                required,
                memory,
                "CCTV fixed buffer pools",
            ));
        }
        Ok(())
    }
}
struct Slot {
    value: Mutex<Option<files::Prepared>>,
    need: AtomicBool,
    ready: Condvar,
}
struct Shared {
    namespace: Arc<files::Namespace>,
    slots: Box<[Slot]>,
    settings: Settings,
    duration: Option<f64>,
    start: OnceLock<Instant>,
    ready: AtomicUsize,
    stop: AtomicBool,
    metrics: Mutex<MetricsStore>,
    wake: Condvar,
    wake_lock: Mutex<()>,
    late: AtomicU64,
    generated: AtomicU64,
    dropped: AtomicU64,
    fallback: AtomicBool,
    progress: Option<Arc<progress::State>>,
    written: AtomicU64,
    closer: Mutex<Option<mpsc::Sender<Closing>>>,
    closing: AtomicUsize,
    close_error: Mutex<Option<String>>,
}
/// A recording that is full (or partly full at shutdown) and waiting to be synced and renamed.
struct Closing {
    file: files::Prepared,
    written: u64,
}
const CLOSERS: usize = 4;
/// The throughput check averages over this many seconds.
const RATE_WINDOW_S: f64 = 10.;
/// Fraction of the combined camera rate that must be achieved.
const RATE_FLOOR: f64 = 0.;
/// How long a generated frame may wait for its write before it is dropped.
const DROP_AFTER_S: f64 = 5.;
fn closer(shared: Arc<Shared>, queue: Arc<Mutex<mpsc::Receiver<Closing>>>) {
    loop {
        let next = queue.lock().unwrap().recv();
        let Ok(Closing { file, written }) = next else {
            break;
        };
        let before = Instant::now();
        let result = file.finish(written);
        let ns = before.elapsed().as_nanos().min(u64::MAX as u128) as u64;
        shared.closing.fetch_sub(1, Ordering::AcqRel);
        match result {
            Ok(()) => {
                if let Err(e) = shared.record(Metrics {
                    closes: 1,
                    close_ns: ns as u128,
                    max_close_ns: ns,
                    ..Metrics::default()
                }) {
                    shared
                        .close_error
                        .lock()
                        .unwrap()
                        .get_or_insert(e.to_string());
                    shared.stop.store(true, Ordering::Release);
                }
            }
            Err(e) => {
                shared
                    .close_error
                    .lock()
                    .unwrap()
                    .get_or_insert(format!("close recording: {e}"));
                shared.stop.store(true, Ordering::Release);
            }
        }
    }
}
/// Hands a finished recording to a closer thread so the camera keeps writing.
fn close(shared: &Shared, file: files::Prepared, written: u64) -> Result<()> {
    if written == 0 {
        return file.finish(0).map_err(|e| e.to_string());
    }
    let depth = shared.closing.fetch_add(1, Ordering::AcqRel) + 1;
    shared
        .record(Metrics {
            max_close_queue: depth as u64,
            ..Metrics::default()
        })
        .map_err(|e| e.to_string())?;
    let sender = shared.closer.lock().unwrap();
    sender
        .as_ref()
        .ok_or("CCTV closer threads stopped")?
        .send(Closing { file, written })
        .map_err(|_| "CCTV closer threads stopped".into())
}
struct StopOnDrop(Arc<Shared>);
impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.stop.store(true, Ordering::Release);
        self.0.wake.notify_all();
        for slot in &self.0.slots {
            slot.ready.notify_all();
        }
    }
}
impl Shared {
    fn stopping(&self) -> bool {
        self.stop.load(Ordering::Acquire) || signals::INTERRUPTED.load(Ordering::Relaxed)
    }
    fn elapsed(&self) -> f64 {
        self.start.get().map_or(0., |s| s.elapsed().as_secs_f64())
    }
    fn record(&self, m: Metrics) -> io::Result<bool> {
        let mut store = self.metrics.lock().unwrap();
        let elapsed = self.elapsed();
        if self.duration.is_some_and(|limit| elapsed >= limit) {
            self.late.fetch_add(m.writes, Ordering::Relaxed);
            return Ok(false);
        }
        store.record(elapsed, m)?;
        Ok(true)
    }
    fn take(&self, index: usize, rotation: bool) -> io::Result<Option<files::Prepared>> {
        let slot = &self.slots[index];
        let mut locked = slot.value.lock().unwrap();
        let mut waited = None::<Instant>;
        loop {
            if self.stopping() {
                return Ok(None);
            }
            if let Some(mut file) = locked.take() {
                file.used = true;
                slot.need.store(true, Ordering::Release);
                self.wake.notify_one();
                drop(locked);
                if rotation {
                    let ns = waited.map_or(0, |start| {
                        start.elapsed().as_nanos().min(u64::MAX as u128) as u64
                    });
                    self.record(Metrics {
                        rotations: 1,
                        preallocation_waits: u64::from(waited.is_some()),
                        preallocation_wait_ns: ns as u128,
                        max_preallocation_wait_ns: ns,
                        ..Metrics::default()
                    })?;
                }
                return Ok(Some(file));
            }
            waited.get_or_insert_with(Instant::now);
            locked = slot
                .ready
                .wait_timeout(locked, Duration::from_millis(100))
                .unwrap()
                .0;
        }
    }
}

struct Logs {
    log: File,
    minute: BufWriter<File>,
    hour: BufWriter<File>,
    paths: [PathBuf; 3],
}
impl Logs {
    fn open(ns: &files::Namespace, settings: Settings, duration: Option<f64>) -> io::Result<Self> {
        let date = files::utc(files::now()?, true)
            .replace('T', "_")
            .replace(':', "-");
        let base = format!("cctv-test-{date}-cameras={}", settings.count);
        for serial in 0..1000 {
            let name = if serial == 0 {
                base.clone()
            } else {
                format!("{base}-{serial:03}")
            };
            let paths = [
                ns.directory.join(format!("{name}.log")),
                ns.directory.join(format!("{name}.min")),
                ns.directory.join(format!("{name}.hour")),
            ];
            let mut log = match OpenOptions::new()
                .append(true)
                .create_new(true)
                .open(&paths[0])
            {
                Ok(f) => f,
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            };
            let mut minute = BufWriter::new(
                OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&paths[1])?,
            );
            let mut hour = BufWriter::new(
                OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&paths[2])?,
            );
            writeln!(log,"# spit CCTV run; prefix={:?}; cameras={}; Mbit_s_each={}; retention_hours={}; block_bytes={}; file_bytes={}; duration_seconds={duration:?}",ns.directory.join(&ns.prefix),settings.count,settings.mbps,settings.hours,settings.block,settings.file_size)?;
            for file in [&mut minute, &mut hour] {
                stats::write_header(file)?;
                file.flush()?;
            }
            hour.get_ref().sync_data()?;
            return Ok(Self {
                log,
                minute,
                hour,
                paths,
            });
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not choose a unique CCTV run log name",
        ))
    }
    fn advance(
        &mut self,
        p: &mut Periods,
        elapsed: f64,
        finish: bool,
        store: &Mutex<MetricsStore>,
        utc: u64,
    ) -> io::Result<()> {
        // Never hold the camera metrics lock during filesystem log writes.
        let mut snapshot = store.lock().unwrap().drain(elapsed, finish);
        if p.advance(
            elapsed,
            finish,
            &mut snapshot,
            utc,
            &mut self.minute,
            &mut self.hour,
        )? {
            self.hour.get_ref().sync_data()?;
        }
        Ok(())
    }
}

fn allocate(shared: Arc<Shared>) -> Result<()> {
    let zeroes = files::Aligned::new(65536);
    let mut sequence = vec![0u64; shared.settings.count];
    while !shared.stopping() {
        let mut work = false;
        for (index, slot) in shared.slots.iter().enumerate() {
            if shared.stopping() {
                break;
            }
            if !slot.need.swap(false, Ordering::AcqRel) {
                continue;
            }
            work = true;
            sequence[index] += 1;
            let before = Instant::now();
            let file = shared
                .namespace
                .prepare(
                    index + 1,
                    sequence[index],
                    shared.settings.file_size,
                    &zeroes,
                )
                .map_err(|e| {
                    format!(
                        "CCTV camera {} background preallocation for {}: {e}",
                        index + 1,
                        shared
                            .namespace
                            .directory
                            .join(&shared.namespace.prefix)
                            .display()
                    )
                })?;
            let ns = before.elapsed().as_nanos().min(u64::MAX as u128) as u64;
            if file.fallback && !shared.fallback.swap(true, Ordering::Relaxed) {
                eprintln!("*info* native preallocation is unavailable on this filesystem; CCTV allocator is using a background zero-fill fallback");
            }
            shared
                .record(Metrics {
                    allocations: 1,
                    allocation_ns: ns as u128,
                    max_allocation_ns: ns,
                    ..Metrics::default()
                })
                .map_err(|e| e.to_string())?;
            if shared.stopping() {
                break;
            }
            *slot.value.lock().unwrap() = Some(file);
            slot.ready.notify_one();
        }
        if !work {
            let locked = shared.wake_lock.lock().unwrap();
            let _ = shared
                .wake
                .wait_timeout(locked, Duration::from_millis(100))
                .unwrap();
        }
    }
    Ok(())
}

fn fill(buffer: &mut [u8], state: &mut u64) {
    for word in buffer.chunks_mut(8) {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        word.copy_from_slice(&state.to_le_bytes()[..word.len()]);
    }
}
fn camera(
    shared: Arc<Shared>,
    index: usize,
    mut buffer: files::Aligned,
    seed: u64,
) -> Result<(u64, Vec<(f64, u32)>)> {
    let mut current = shared.take(index, false).map_err(|e| e.to_string())?;
    if current.is_none() {
        return Ok((0, Vec::new()));
    }
    shared.ready.fetch_add(1, Ordering::Release);
    let mut startup = shared.wake_lock.lock().unwrap();
    while shared.start.get().is_none() && !shared.stopping() {
        startup = shared
            .wake
            .wait_timeout(startup, Duration::from_millis(100))
            .unwrap()
            .0;
    }
    drop(startup);
    let Some(&start) = shared.start.get() else {
        if let Some(file) = current.take() {
            file.finish(0).map_err(|e| e.to_string())?;
        }
        return Ok((0, Vec::new()));
    };
    let period = shared.settings.block as f64 / rate::bytes_per_second(shared.settings.mbps);
    let mut due = period * (1. + index as f64 / shared.settings.count as f64);
    // Frames generated but not yet written, as the times they were generated.
    let mut backlog = std::collections::VecDeque::new();
    let mut offset = 0u64;
    let (mut writes, mut bytes, mut allocations) = (0, 0, 0);
    let mut samples = Vec::new();
    let mut random = (seed ^ ((index as u64 + 1).wrapping_mul(0x9e3779b97f4a7c15))) | 1;
    let counter = shared.progress.as_ref().map(|s| s.counter(index));
    loop {
        let elapsed = start.elapsed().as_secs_f64();
        if shared.stopping() || shared.duration.is_some_and(|limit| elapsed >= limit) {
            break;
        }
        // The camera generates a frame every period whether or not earlier writes have finished.
        while due <= elapsed {
            backlog.push_back(due);
            shared.generated.fetch_add(1, Ordering::Relaxed);
            due += period;
        }
        // A frame still unwritten after the drop deadline is lost, as a full camera buffer would lose it.
        while backlog
            .front()
            .is_some_and(|&g| elapsed - g > shared.settings.drop_after)
        {
            backlog.pop_front();
            shared.dropped.fetch_add(1, Ordering::Relaxed);
        }
        let Some(generated_at) = backlog.pop_front() else {
            thread::sleep(Duration::from_secs_f64((due - elapsed).clamp(0., 0.05)));
            continue;
        };
        if current.is_none() {
            current = shared.take(index, true).map_err(|e| e.to_string())?;
            offset = 0;
            if current.is_none() {
                break;
            }
        }
        let file = current.as_mut().unwrap();
        if shared.stopping()
            || shared
                .duration
                .is_some_and(|limit| start.elapsed().as_secs_f64() >= limit)
        {
            break;
        }
        let guard = allocation::Guard::start();
        let generate = Instant::now();
        fill(&mut buffer, &mut random);
        let gen_ns = generate.elapsed().as_nanos().min(u64::MAX as u128) as u64;
        if offset == 0 {
            buffer[..64].copy_from_slice(&file.head);
        }
        if let Some(c) = counter {
            c.submitted(1);
        }
        let before = Instant::now();
        let result = file.file.as_mut().unwrap().write_all(&buffer);
        let completed = Instant::now();
        let latency = completed
            .duration_since(before)
            .as_nanos()
            .min(u64::MAX as u128) as u64;
        if shared.settings.ranged {
            // Runs from when the frame was generated, so time spent waiting behind earlier writes counts.
            let end_to_end = completed.duration_since(start).as_secs_f64() - generated_at;
            samples.push((
                generated_at,
                (end_to_end * 1e6).clamp(0., u32::MAX as f64) as u32,
            ));
        }
        if let Some(c) = counter {
            c.completed(1);
        }
        let measured = if result.is_ok() {
            shared.record(Metrics {
                writes: 1,
                bytes: buffer.len() as u64,
                gen_ns: gen_ns as u128,
                max_gen_ns: gen_ns,
                write_ns: latency as u128,
                max_write_ns: latency,
                max_write_tag: stats::WriteTag {
                    camera: index + 1,
                    sequence: file.sequence,
                    offset,
                    submitted_ns: before
                        .duration_since(start)
                        .as_nanos()
                        .min(u64::MAX as u128) as u64,
                    completed_ns: completed
                        .duration_since(start)
                        .as_nanos()
                        .min(u64::MAX as u128) as u64,
                },
                writes_over_200ms: u64::from(latency > 200_000_000),
                ..Metrics::default()
            })
        } else {
            Ok(false)
        };
        if matches!(measured, Ok(true)) {
            writes += 1;
            bytes += buffer.len() as u64;
            shared
                .written
                .fetch_add(buffer.len() as u64, Ordering::Relaxed);
            if let Some(c) = counter {
                c.publish(0, writes, 0, bytes);
            }
        }
        allocations += guard.count();
        drop(guard);
        result.map_err(|e| {
            format!(
                "camera {} sequential write at offset {offset}, length {}: {e}",
                index + 1,
                buffer.len()
            )
        })?;
        measured.map_err(|e| e.to_string())?;
        offset += buffer.len() as u64;
        if offset == shared.settings.file_size {
            close(&shared, current.take().unwrap(), offset)
                .map_err(|e| format!("camera {} close recording: {e}", index + 1))?;
            offset = 0;
        }
    }
    if let Some(file) = current.take() {
        close(&shared, file, offset)
            .map_err(|e| format!("camera {} close partial recording: {e}", index + 1))?;
    }
    if let Some(c) = counter {
        c.finish();
    }
    Ok((allocations, samples))
}

/// One line per measured write: seconds since the measured start, then latency in milliseconds.
fn write_latencies(path: &std::path::Path, samples: &[(f64, u32)]) -> io::Result<()> {
    let mut out = BufWriter::new(OpenOptions::new().write(true).create_new(true).open(path)?);
    for &(submitted, us) in samples {
        writeln!(out, "{submitted:.6} {:.3}", us as f64 / 1000.)?;
    }
    out.flush()
}
/// The measured writes of one camera count in a range, kept for the plot files.
struct Measured {
    dir: std::path::PathBuf,
    seconds: f64,
    samples: Vec<(f64, u32)>,
    /// Total Mbit over the run: (offered, achieved, dropped).
    loss: (f64, f64, f64),
}

pub fn run(config: Config) -> Result<()> {
    let base = config.cameras.unwrap();
    let counts = camera_counts(&base);
    let mut measured = Vec::new();
    // A failed count still leaves the plot files for the counts measured before it.
    let mut failure = None;
    for (index, count) in counts.iter().enumerate() {
        if counts.len() > 1 {
            println!(
                "=== sweep {}/{}: {count} cameras ===",
                index + 1,
                counts.len()
            );
        }
        let settings = Settings {
            count: *count,
            ..base
        };
        match run_inner(&config, settings) {
            Ok(Some(run)) => measured.push((*count, run)),
            Ok(None) => {}
            Err(e) => {
                failure = Some(format!("CCTV: {e}"));
                break;
            }
        }
        if signals::INTERRUPTED.load(Ordering::Relaxed) {
            break;
        }
    }
    let mut plotted = Ok(());
    if let Some((_, first)) = measured.first() {
        let runs: Vec<cctv_plots::Run> = measured
            .iter()
            .map(|(count, run)| cctv_plots::Run {
                count: *count,
                block_bytes: base.block,
                seconds: run.seconds,
                samples: &run.samples,
                loss: Some(run.loss),
            })
            .collect();
        let io_mode = if base.direct { "direct" } else { "pagecache" };
        plotted = cctv_plots::write_range(&first.dir, &runs, io_mode)
            .map_err(|e| format!("CCTV plot files: {e}"));
    }
    match failure {
        Some(e) => Err(e),
        None => plotted,
    }
}
fn run_inner(config: &Config, settings: Settings) -> Result<Option<Measured>> {
    settings.validate(config.memory)?;
    let _signals = signals::Guard::install().map_err(|e| e.to_string())?;
    let ns = files::Namespace::open(
        &config.prefixes[0],
        files::run_id().map_err(|e| e.to_string())?,
        settings.direct,
    )
    .map_err(|e| e.to_string())?;
    match files::volume_info(&ns.directory) {
        Ok(v) => println!(
            "filesystem source={} type={} size_GiB={:.1} free_GiB={:.1} used_pct={:.1} io={} directory={}",
            v.source,
            v.fs_type,
            v.total as f64 / 1073741824.,
            v.free as f64 / 1073741824.,
            100. * (1. - v.free as f64 / v.total.max(1) as f64),
            if settings.direct { "direct" } else { "pagecache" },
            ns.directory.display()
        ),
        Err(e) => println!("filesystem unavailable: {e}"),
    }
    let mut logs = Logs::open(&ns, settings, config.duration).map_err(|e| e.to_string())?;
    for path in &logs.paths {
        println!("cctv_log={}", path.display());
    }
    let reporter = if config.interval > 0. {
        Some(
            progress::Reporter::with_log(
                settings.count,
                config.interval,
                config.duration,
                Some(logs.log.try_clone().map_err(|e| e.to_string())?),
            )
            .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };
    let shared = Arc::new(Shared {
        namespace: ns,
        slots: (0..settings.count)
            .map(|_| Slot {
                value: Mutex::new(None),
                need: AtomicBool::new(true),
                ready: Condvar::new(),
            })
            .collect(),
        settings,
        duration: config.duration,
        start: OnceLock::new(),
        ready: AtomicUsize::new(0),
        stop: AtomicBool::new(false),
        metrics: Mutex::new(MetricsStore::default()),
        wake: Condvar::new(),
        wake_lock: Mutex::new(()),
        late: AtomicU64::new(0),
        generated: AtomicU64::new(0),
        dropped: AtomicU64::new(0),
        fallback: AtomicBool::new(false),
        progress: reporter.as_ref().map(|r| r.state.clone()),
        written: AtomicU64::new(0),
        closer: Mutex::new(None),
        closing: AtomicUsize::new(0),
        close_error: Mutex::new(None),
    });
    let (close_tx, close_rx) = mpsc::channel();
    *shared.closer.lock().unwrap() = Some(close_tx);
    let close_rx = Arc::new(Mutex::new(close_rx));
    let closers: Vec<_> = (0..CLOSERS)
        .map(|_| {
            let state = shared.clone();
            let queue = close_rx.clone();
            thread::spawn(move || closer(state, queue))
        })
        .collect();
    let deleted = shared
        .namespace
        .prune(
            files::now()
                .map_err(|e| e.to_string())?
                .saturating_sub((settings.hours * 3600.).ceil() as u64),
        )
        .map_err(|e| e.to_string())?;
    shared
        .record(Metrics {
            deleted,
            ..Metrics::default()
        })
        .map_err(|e| e.to_string())?;
    let buffers: Vec<files::Aligned> = (0..settings.count)
        .map(|_| files::Aligned::new(settings.block))
        .collect();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .max_blocking_threads(settings.count + 1)
        .build()
        .map_err(|e| e.to_string())?;
    let _shutdown = StopOnDrop(shared.clone());
    let enter = runtime.enter();
    let allocation_shared = shared.clone();
    let allocator = tokio::task::spawn_blocking(move || {
        let _shutdown = StopOnDrop(allocation_shared.clone());
        let result = allocate(allocation_shared.clone());
        if result.is_err() {
            allocation_shared.stop.store(true, Ordering::Release);
        }
        result
    });
    let mut cameras = Vec::with_capacity(settings.count);
    for (index, buffer) in buffers.into_iter().enumerate() {
        let state = shared.clone();
        let seed = config.seed;
        cameras.push(tokio::task::spawn_blocking(move || {
            let _shutdown = StopOnDrop(state.clone());
            let result = camera(state.clone(), index, buffer, seed);
            if result.is_err() {
                state.stop.store(true, Ordering::Release);
            }
            if let Some(s) = &state.progress {
                s.counter(index).finish();
            }
            result
        }));
    }
    drop(enter);
    eprintln!(
        "preallocating initial {}-byte files for {} cameras before the measured run",
        settings.file_size, settings.count
    );
    // Start with both the current file and the lookahead reserved for every camera.
    while !shared.stopping()
        && (shared.ready.load(Ordering::Acquire) < settings.count
            || shared
                .slots
                .iter()
                .any(|slot| slot.value.lock().unwrap().is_none()))
    {
        thread::sleep(Duration::from_millis(10));
    }
    let utc = files::now().map_err(|e| e.to_string())?;
    let start = Instant::now();
    let _ = shared.start.set(start);
    shared.wake.notify_all();
    if let Some(r) = &reporter {
        let _ = r.state.start.set(start);
    }
    println!("backend={} cameras={} Mbit_s_each={} target_Mbit_s={} block_bytes={} file_bytes={} fixed_pool_bytes={} memory_budget={}",if settings.direct { "direct" } else { "pagecache" },settings.count,settings.mbps,settings.mbps*settings.count as f64,settings.block,settings.file_size,settings.pool_bytes().unwrap(),config.memory);
    if let Some(link) = settings.link_mbps {
        let fits = (link * settings.floor / settings.mbps).floor() as usize;
        println!(
            "link_mbps={link} floor_percent={:.0} cameras_that_fit={fits}",
            settings.floor * 100.
        );
    }
    let mut periods = Periods::default();
    let mut next_minute = 60.;
    // Rolling throughput check: the cameras must write at their combined Mbit/s rate.
    let mut next_sample = 1.;
    let mut samples: Vec<(f64, u64)> = Vec::new();
    // One (elapsed seconds, achieved Mbit/s over the last second) point per second of the run.
    let mut rates: Vec<(f64, f64)> = Vec::new();
    let mut last_rate_point = (0f64, 0u64);
    let mut error = None;
    loop {
        let elapsed = shared.elapsed();
        let capped = config.duration.map_or(elapsed, |limit| elapsed.min(limit));
        if capped >= next_minute {
            if let Err(e) = logs.advance(&mut periods, capped, false, &shared.metrics, utc) {
                error = Some(format!("write minute/hour logs: {e}"));
                break;
            }
            let cutoff = match files::now() {
                Ok(now) => now.saturating_sub((settings.hours * 3600.).ceil() as u64),
                Err(e) => {
                    error = Some(e.to_string());
                    break;
                }
            };
            match shared.namespace.prune(cutoff).and_then(|deleted| {
                shared.record(Metrics {
                    deleted,
                    ..Metrics::default()
                })
            }) {
                Ok(_) => {}
                Err(e) => {
                    error = Some(format!("retention: {e}"));
                    break;
                }
            }
            next_minute = (capped / 60.).floor() * 60. + 60.;
        }
        if elapsed >= next_sample {
            next_sample = elapsed + 1.;
            let written = shared.written.load(Ordering::Relaxed);
            let (last_t, last_bytes) = last_rate_point;
            if elapsed > last_t {
                rates.push((
                    elapsed,
                    (written - last_bytes) as f64 * 8. / 1e6 / (elapsed - last_t),
                ));
            }
            last_rate_point = (elapsed, written);
            samples.push((elapsed, written));
            // Keep the newest sample at least RATE_WINDOW_S old as the baseline.
            while samples.len() > 2 && samples[1].0 <= elapsed - RATE_WINDOW_S {
                samples.remove(0);
            }
            let (t0, b0) = samples[0];
            let span = elapsed - t0;
            if span >= RATE_WINDOW_S {
                let achieved =
                    (shared.written.load(Ordering::Relaxed) - b0) as f64 * 8. / 1e6 / span;
                // The floor (75% by default) of the combined rate is the success threshold for continuing.
                let required = settings.count as f64 * settings.mbps * settings.floor;
                if achieved < required {
                    error = Some(format!(
                        "upstream is not fast enough: {achieved:.1} Mbit/s written over the last {span:.0} s, but {} cameras at {} Mbit/s each need {required:.1} Mbit/s ({:.0}%)",
                        settings.count, settings.mbps, settings.floor * 100.
                    ));
                    break;
                }
            }
        }
        if shared.stopping() || config.duration.is_some_and(|limit| elapsed >= limit) {
            break;
        }
        if shared
            .progress
            .as_ref()
            .is_some_and(|s| s.log_failed.load(Ordering::Acquire))
        {
            error = Some("write progress .log failed".into());
            break;
        }
        thread::sleep(Duration::from_secs_f64(
            (next_minute - elapsed)
                .min(config.duration.map_or(0.05, |d| (d - elapsed).max(0.)))
                .clamp(0.001, 0.05),
        ));
    }
    let elapsed = config
        .duration
        .map_or(shared.elapsed(), |limit| shared.elapsed().min(limit));
    shared.stop.store(true, Ordering::Release);
    shared.wake.notify_all();
    for slot in &shared.slots {
        slot.ready.notify_all();
    }
    let (allocations, latencies) = runtime.block_on(async {
        let mut count = 0;
        let mut latencies = Vec::new();
        for camera in cameras {
            match camera.await {
                Ok(Ok((n, mut samples))) => {
                    count += n;
                    latencies.append(&mut samples);
                }
                Ok(Err(e)) => {
                    eprintln!("*error* {e}");
                    error = Some(e)
                }
                Err(e) => error = Some(e.to_string()),
            }
        }
        match allocator.await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                eprintln!("*error* {e}");
                error = Some(e)
            }
            Err(e) => error = Some(e.to_string()),
        }
        (count, latencies)
    });
    // Per-second Mbit/s for the whole run, beside the logs (for the sustained-rate chart).
    let rate_text: String = rates
        .iter()
        .map(|(t, m)| format!("{t:.0} {m:.3}\n"))
        .collect();
    if let Err(e) = std::fs::write(logs.paths[0].with_extension("rate"), rate_text) {
        error = Some(format!("write rate samples: {e}"));
    }
    if let (Some(dir), Some(stem)) = (
        logs.paths[0].parent(),
        logs.paths[0].file_stem().and_then(|s| s.to_str()),
    ) {
        if let Err(e) = cctv_plots::write_envelope(dir, stem, &rates, 30., settings.link_mbps) {
            error = Some(format!("write rate envelope: {e}"));
        }
    }
    let mut measured = None;
    if settings.ranged {
        let path = logs.paths[0].with_extension("lat");
        if let Err(e) = write_latencies(&path, &latencies) {
            error = Some(format!("write latency samples: {e}"));
        }
        let dir = logs.paths[0]
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf();
        let frame_mbit = settings.block as f64 * 8. / 1e6;
        let loss = (
            shared.generated.load(Ordering::Relaxed) as f64 * frame_mbit,
            shared.written.load(Ordering::Relaxed) as f64 * 8. / 1e6,
            shared.dropped.load(Ordering::Relaxed) as f64 * frame_mbit,
        );
        measured = Some(Measured {
            dir,
            seconds: elapsed.max(1e-9),
            samples: latencies,
            loss,
        });
    }
    // Closers sync and rename every queued recording, including final partial files,
    // before the totals are read.
    drop(shared.closer.lock().unwrap().take());
    for handle in closers {
        let _ = handle.join();
    }
    if let Some(e) = shared.close_error.lock().unwrap().take() {
        error = Some(e);
    }
    drop(reporter);
    if shared
        .progress
        .as_ref()
        .is_some_and(|s| s.log_failed.load(Ordering::Acquire))
    {
        error = Some("write progress .log failed".into());
    }
    if let Err(e) = logs.advance(&mut periods, elapsed, true, &shared.metrics, utc) {
        error = Some(format!("flush final minute/hour logs: {e}"));
    }
    let total = shared.metrics.lock().unwrap().total;
    let seconds = elapsed.max(1e-9);
    let rotation_stats = format!(
        "file_rotations={} preallocation_waits={} max_preallocation_wait_ms={:.6}",
        total.rotations,
        total.preallocation_waits,
        total.max_preallocation_wait_ns as f64 / 1e6
    );
    let timing = format!("writes_over_200ms={} max_write_camera={} max_write_sequence={} max_write_offset={} max_write_submit_s={:.9} max_write_complete_s={:.9} stop_reason={}",total.writes_over_200ms,total.max_write_tag.camera,total.max_write_tag.sequence,total.max_write_tag.offset,total.max_write_tag.submitted_ns as f64/1e9,total.max_write_tag.completed_ns as f64/1e9,if error.is_some(){"error"}else if signals::INTERRUPTED.load(Ordering::Relaxed){"interrupted"}else{"deadline"});
    println!("read_ops=0 write_ops={} seconds={seconds:.6} read_MiB_s=0.000 write_MiB_s={:.3} IOPS={:.0} hot_path_allocations={allocations} verified_ops=0 avg_write_ms={:.6} max_write_ms={:.6} late_write_ops={}",total.writes,total.bytes as f64/1048576./seconds,total.writes as f64/seconds,if total.writes==0{0.}else{total.write_ns as f64/total.writes as f64/1e6},total.max_write_ns as f64/1e6,shared.late.load(Ordering::Relaxed));
    let offered = shared.generated.load(Ordering::Relaxed);
    let dropped = shared.dropped.load(Ordering::Relaxed);
    let dropped_pct = if offered == 0 {
        0.
    } else {
        100. * dropped as f64 / offered as f64
    };
    println!(
        "open_loop drop_after_s={:.1} offered_frames={offered} written_frames={} dropped_frames={dropped} dropped_pct={dropped_pct:.2} offered_Mbit_s={:.3} achieved_Mbit_s={:.3}",
        settings.drop_after,
        total.writes,
        offered as f64 * settings.block as f64 * 8. / seconds / 1e6,
        total.bytes as f64 * 8. / seconds / 1e6,
    );
    println!("{rotation_stats}");
    println!("{timing}");
    let pipeline = format!(
        "avg_gen_ms={:.6} max_gen_ms={:.6} closes={} avg_close_ms={:.6} max_close_ms={:.6} max_close_queue={}",
        if total.writes == 0 { 0. } else { total.gen_ns as f64 / total.writes as f64 / 1e6 },
        total.max_gen_ns as f64 / 1e6,
        total.closes,
        if total.closes == 0 { 0. } else { total.close_ns as f64 / total.closes as f64 / 1e6 },
        total.max_close_ns as f64 / 1e6,
        total.max_close_queue,
    );
    println!("{pipeline}");
    writeln!(logs.log, "# pipeline {pipeline}").map_err(|e| e.to_string())?;
    writeln!(logs.log,"# finished seconds={seconds:.6} write_ops={} write_bytes={} avg_write_ms={:.6} max_write_ms={:.6} hot_path_allocations={allocations} {rotation_stats} {timing} status={}",total.writes,total.bytes,if total.writes==0{0.}else{total.write_ns as f64/total.writes as f64/1e6},total.max_write_ns as f64/1e6,if error.is_some(){"error"}else{"ok"}).map_err(|e|e.to_string())?;
    if let Some(e) = error {
        Err(e)
    } else {
        Ok(measured)
    }
}

/// Rebuilds the plot files from the newest cctv-test-*-cameras=N.lat per count in `dir`,
/// reading each block size and I/O mode from the matching .log header.
pub fn replot(dir: &std::path::Path) -> Result<()> {
    let mut newest: std::collections::BTreeMap<usize, std::path::PathBuf> = Default::default();
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "lat"))
        .collect();
    names.sort();
    for path in names {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if let Some(count) = name
            .rsplit("cameras=")
            .next()
            .and_then(|t| t.strip_suffix(".lat"))
        {
            if let Ok(count) = count.parse::<usize>() {
                newest.insert(count, path);
            }
        }
    }
    if newest.is_empty() {
        return Err(format!(
            "no cctv-test-*-cameras=N.lat files in {}",
            dir.display()
        ));
    }
    let mut data = Vec::new();
    for (count, path) in &newest {
        let log = std::fs::read_to_string(path.with_extension("log")).unwrap_or_default();
        let header = |key: &str| {
            log.split(key)
                .nth(1)
                .and_then(|t| t.split(|c: char| !c.is_ascii_alphanumeric()).next())
                .map(str::to_string)
        };
        let block_bytes = header("block_bytes=")
            .and_then(|v| v.parse().ok())
            .unwrap_or(65536);
        let io_mode = header("backend=").unwrap_or_else(|| "direct".into());
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let samples: Vec<(f64, u32)> = text
            .lines()
            .filter_map(|line| {
                let mut f = line.split(' ');
                let at: f64 = f.next()?.parse().ok()?;
                let ms: f64 = f.next()?.parse().ok()?;
                Some((at, (ms * 1000.).round() as u32))
            })
            .collect();
        let seconds = samples.last().map_or(1e-9, |s| s.0.max(1e-9));
        data.push((*count, block_bytes, io_mode, seconds, samples));
    }
    let io_mode = data[0].2.clone();
    let runs: Vec<cctv_plots::Run> = data
        .iter()
        .map(
            |(count, block_bytes, _, seconds, samples)| cctv_plots::Run {
                count: *count,
                block_bytes: *block_bytes,
                seconds: *seconds,
                samples,
                loss: None,
            },
        )
        .collect();
    cctv_plots::write_range(dir, &runs, &io_mode).map_err(|e| format!("CCTV plot files: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hourly_rows_are_visible_without_closing_the_log() {
        let run = files::run_id().unwrap();
        let directory = std::env::temp_dir().join(format!("spit-cctv-flush-{run}"));
        let ns = files::Namespace::open(&directory.join("camera"), run, false).unwrap();
        let mut logs = Logs::open(&ns, Settings::parse("1,5,24").unwrap(), Some(604800.)).unwrap();
        let store = Mutex::new(MetricsStore::default());
        let mut periods = Periods::default();
        store
            .lock()
            .unwrap()
            .record(
                1.,
                Metrics {
                    writes: 2,
                    bytes: 131072,
                    write_ns: 6_000_000,
                    max_write_ns: 4_000_000,
                    ..Metrics::default()
                },
            )
            .unwrap();
        logs.advance(&mut periods, 3600., false, &store, 0).unwrap();
        let text = std::fs::read_to_string(&logs.paths[2]).unwrap();
        assert_eq!(text.lines().count(), 2);
        let fields: Vec<_> = text
            .lines()
            .nth(1)
            .unwrap()
            .split(',')
            .map(str::trim)
            .collect();
        assert_eq!(&fields[7..9], &["3.000000", "4.000000"]);
        logs.advance(&mut periods, 7200., false, &store, 0).unwrap();
        assert_eq!(
            std::fs::read_to_string(&logs.paths[2])
                .unwrap()
                .lines()
                .count(),
            3
        );
        // A final partial hour must be readable before the writer is closed too.
        logs.advance(&mut periods, 8100., true, &store, 0).unwrap();
        let text = std::fs::read_to_string(&logs.paths[2]).unwrap();
        assert_eq!(text.lines().count(), 4);
        let fields: Vec<_> = text
            .lines()
            .last()
            .unwrap()
            .split(',')
            .map(str::trim)
            .collect();
        assert_eq!(&fields[2..5], &["7200", "900.000000", "partial"]);
        drop(logs);
        drop(ns);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
