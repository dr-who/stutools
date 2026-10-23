#[path = "cctv_files.rs"]
mod files;
#[path = "cctv_signals.rs"]
mod signals;
#[path = "cctv_stats.rs"]
mod stats;
use crate::{allocation, cli::Config, progress, Result};
use stats::{Metrics, MetricsStore, Periods};
use std::{
    fs::{File, OpenOptions},
    io::{self, BufWriter, Write},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        Arc, Condvar, Mutex, OnceLock,
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
}
impl Settings {
    pub fn parse(text: &str) -> Result<Self> {
        let mut parts = text.split(',');
        let count = parts
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or("--cctv needs COUNT,MBPS,HOURS")?;
        let mbps = parts
            .next()
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or("--cctv needs COUNT,MBPS,HOURS")?;
        let hours = parts
            .next()
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or("--cctv needs COUNT,MBPS,HOURS")?;
        if parts.next().is_some()
            || !(1..=1024).contains(&count)
            || !mbps.is_finite()
            || !(0.000008..=8_000_000.).contains(&mbps)
            || !hours.is_finite()
            || hours <= 0.
            || hours > 1e9
        {
            return Err("--cctv needs 1..1024 cameras, a finite positive Mbit/s rate and positive retention hours".into());
        }
        Ok(Self {
            count,
            mbps,
            hours,
            block: 65536,
            file_size: 16 * 1048576,
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
    fallback: AtomicBool,
    progress: Option<Arc<progress::State>>,
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
        for serial in 0..1000 {
            let name = if serial == 0 {
                format!("cctv-test-{date}")
            } else {
                format!("cctv-test-{date}-{serial:03}")
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
    let zeroes = [0u8; 65536];
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
fn camera(shared: Arc<Shared>, index: usize, mut buffer: Box<[u8]>, seed: u64) -> Result<u64> {
    let mut current = shared.take(index, false).map_err(|e| e.to_string())?;
    if current.is_none() {
        return Ok(0);
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
        return Ok(0);
    };
    let period = shared.settings.block as f64 / (shared.settings.mbps * 1e6 / 8.);
    let mut due = period * (1. + index as f64 / shared.settings.count as f64);
    let mut offset = 0u64;
    let (mut writes, mut bytes, mut allocations) = (0, 0, 0);
    let mut random = (seed ^ ((index as u64 + 1).wrapping_mul(0x9e3779b97f4a7c15))) | 1;
    let counter = shared.progress.as_ref().map(|s| s.counter(index));
    loop {
        let elapsed = start.elapsed().as_secs_f64();
        if shared.stopping() || shared.duration.is_some_and(|limit| elapsed >= limit) {
            break;
        }
        if elapsed < due {
            thread::sleep(Duration::from_secs_f64((due - elapsed).min(0.05)));
            continue;
        }
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
        fill(&mut buffer, &mut random);
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
        if let Some(c) = counter {
            c.completed(1);
        }
        let measured = if result.is_ok() {
            shared.record(Metrics {
                writes: 1,
                bytes: buffer.len() as u64,
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
            current
                .take()
                .unwrap()
                .finish(offset)
                .map_err(|e| format!("camera {} close recording: {e}", index + 1))?;
            offset = 0;
        }
        let actual = start.elapsed().as_secs_f64();
        due += period;
        // A blocked camera drops missed pacing slots instead of issuing a burst.
        if due < actual {
            due = actual + period;
        }
    }
    if let Some(file) = current.take() {
        file.finish(offset)
            .map_err(|e| format!("camera {} close partial recording: {e}", index + 1))?;
    }
    if let Some(c) = counter {
        c.finish();
    }
    Ok(allocations)
}

pub fn run(config: Config) -> Result<()> {
    run_inner(config).map_err(|e| format!("CCTV: {e}"))
}
fn run_inner(config: Config) -> Result<()> {
    let settings = config.cameras.unwrap();
    settings.validate(config.memory)?;
    let _signals = signals::Guard::install().map_err(|e| e.to_string())?;
    let ns = files::Namespace::open(
        &config.prefixes[0],
        files::run_id().map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
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
        fallback: AtomicBool::new(false),
        progress: reporter.as_ref().map(|r| r.state.clone()),
    });
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
    let buffers: Vec<Box<[u8]>> = (0..settings.count)
        .map(|_| vec![0; settings.block].into_boxed_slice())
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
    println!("backend=buffered cameras={} Mbit_s_each={} target_Mbit_s={} block_bytes={} file_bytes={} fixed_pool_bytes={} memory_budget={}",settings.count,settings.mbps,settings.mbps*settings.count as f64,settings.block,settings.file_size,settings.pool_bytes().unwrap(),config.memory);
    let mut periods = Periods::default();
    let mut next_minute = 60.;
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
    let allocations = runtime.block_on(async {
        let mut count = 0;
        for camera in cameras {
            match camera.await {
                Ok(Ok(n)) => count += n,
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
        count
    });
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
    println!("{rotation_stats}");
    println!("{timing}");
    writeln!(logs.log,"# finished seconds={seconds:.6} write_ops={} write_bytes={} avg_write_ms={:.6} max_write_ms={:.6} hot_path_allocations={allocations} {rotation_stats} {timing} status={}",total.writes,total.bytes,if total.writes==0{0.}else{total.write_ns as f64/total.writes as f64/1e6},total.max_write_ns as f64/1e6,if error.is_some(){"error"}else{"ok"}).map_err(|e|e.to_string())?;
    if let Some(e) = error {
        Err(e)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hourly_rows_are_visible_without_closing_the_log() {
        let run = files::run_id().unwrap();
        let directory = std::env::temp_dir().join(format!("spit-cctv-flush-{run}"));
        let ns = files::Namespace::open(&directory.join("camera"), run).unwrap();
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
