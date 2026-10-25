use crate::{
    allocation,
    cli::{Backend, Config, Workload},
    io::{self, Failure, Queue, Request},
    pattern, Result,
};
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Barrier, OnceLock,
    },
    time::Instant,
};
struct Plan {
    record_latency: bool,
    file: File,
    /// Windows only: one file per queue slot.
    slots: Vec<File>,
    work: Workload,
    lo: u64,
    hi: u64,
    loaded: Arc<[Request]>,
    seed: u64,
    backend: Backend,
    timeout: f64,
    duration: Option<f64>,
    verify: bool,
    errors: u64,
    max_len: usize,
    position_capacity: usize,
}
#[derive(Default)]
struct Stats {
    reads: u64,
    writes: u64,
    read_bytes: u64,
    write_bytes: u64,
    seconds: f64,
    allocations: u64,
    submit_errors: u64,
    verified: u64,
    completed_reads: u64,
    completed_writes: u64,
    /// Per-operation latency in microseconds, measured from the start of its batch.
    latencies_us: Vec<u32>,
    /// Measured in-flight depth from streamed runs: maximum, time-weighted sum and time.
    inflight_max: u64,
    inflight_area_ns: u128,
    inflight_time_ns: u128,
}
impl Stats {
    fn complete(&mut self, request: Request, measured: bool) {
        if request.write {
            self.completed_writes += 1;
        } else {
            self.completed_reads += 1;
        }
        if measured {
            if request.write {
                self.writes += 1;
                self.write_bytes += request.len as u64;
            } else {
                self.reads += 1;
                self.read_bytes += request.len as u64;
            }
        }
    }
    fn publish(&self, counter: Option<&crate::progress::Counter>) {
        if let Some(counter) = counter {
            counter.publish(self.reads, self.writes, self.read_bytes, self.write_bytes);
        }
    }
}
fn work_duration(work: &Workload, fallback: Option<f64>) -> Option<f64> {
    if work.passes.is_some() || work.range_passes.is_some() {
        None
    } else {
        work.duration.or(fallback)
    }
}
struct Outcome {
    stats: Stats,
    error: Option<Failure>,
    retained: bool,
}
struct StartGate {
    ready: Barrier,
    failed: AtomicBool,
    start: OnceLock<Instant>,
    progress: Option<Arc<crate::progress::State>>,
}
impl StartGate {
    fn new(workers: usize) -> Self {
        Self {
            ready: Barrier::new(workers),
            failed: AtomicBool::new(false),
            start: OnceLock::new(),
            progress: None,
        }
    }
}
struct Target {
    path: PathBuf,
    jobs: Vec<(Workload, usize)>,
}
fn worker_range(
    w: &Workload,
    index: usize,
    target_lo: u64,
    target_hi: u64,
    imported: bool,
) -> Result<(u64, u64)> {
    let (lo, hi) = w.range.unwrap_or((target_lo, target_hi));
    if lo < target_lo || hi > target_hi || lo >= hi {
        return Err("workload range outside target range".into());
    }
    if imported || (w.shared_range && !w.split_range) {
        return Ok((lo, hi));
    }
    let blocks = (hi - lo) / w.block as u64;
    let first = blocks * index as u64 / w.workers as u64;
    let last = blocks * (index + 1) as u64 / w.workers as u64;
    Ok((lo + first * w.block as u64, lo + last * w.block as u64))
}
fn target_specs(c: &Config) -> Vec<Target> {
    let jobs = c
        .jobs
        .iter()
        .flat_map(|w| (0..w.workers).map(move |index| (w.clone(), index)))
        .collect::<Vec<_>>();
    let mut targets = c
        .files
        .iter()
        .map(|path| Target {
            path: path.clone(),
            jobs: jobs.clone(),
        })
        .collect::<Vec<_>>();
    let mut sequence = 0;
    for prefix in &c.prefixes {
        for job in &jobs {
            sequence += 1;
            let mut name = prefix.as_os_str().to_owned();
            name.push(format!(".{sequence:04}"));
            targets.push(Target {
                path: name.into(),
                jobs: vec![job.clone()],
            });
        }
    }
    targets
}
fn loaded(c: &Config) -> Result<Arc<[Request]>> {
    let Some(path) = &c.positions_file else {
        return Ok(Arc::from([]));
    };
    let file = File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > c.memory / 4 {
        return Err("positions input exceeds memory budget".into());
    }
    let mut records = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|e| e.to_string())?;
        let mut words = line.split_whitespace();
        let offset = words
            .next()
            .ok_or("missing position")?
            .parse::<u64>()
            .map_err(|_| "invalid position")?;
        let write = match words.next() {
            Some("R") => false,
            Some("W") => true,
            _ => return Err("positions require R or W".into()),
        };
        let len = words
            .next()
            .ok_or("missing length")?
            .parse::<usize>()
            .map_err(|_| "invalid length")?;
        if words.next().is_some()
            || len == 0
            || len > 64 * 1024 * 1024
            || offset > i64::MAX as u64
            || offset.checked_add(len as u64).is_none()
        {
            return Err("invalid position record".into());
        }
        records.push(Request {
            offset,
            len,
            write,
            ..Request::default()
        });
        if records.len() as u64 * std::mem::size_of::<Request>() as u64 > c.memory / 2 {
            return Err("positions exceed memory budget".into());
        }
    }
    if records.is_empty() {
        return Err("empty position file".into());
    }
    Ok(records.into())
}
/// Upper bound on latency samples kept per worker; later completions are not recorded.
const LATENCY_CAPACITY: usize = 2_000_000;

/// What a finished workload measured. Printed as the usual summary line as well.
#[allow(dead_code)]
pub struct Summary {
    pub reads: u64,
    pub writes: u64,
    pub read_bytes: u64,
    pub write_bytes: u64,
    pub seconds: f64,
    pub latencies_us: Vec<u32>,
}

pub fn run(c: Config) -> Result<Summary> {
    let records = loaded(&c)?;
    let specs = target_specs(&c);
    let max_loaded = records.iter().map(|r| r.len).max().unwrap_or(0);
    let worker_count = specs.iter().map(|t| t.jobs.len()).sum::<usize>();
    let pool_bytes = specs
        .iter()
        .flat_map(|t| &t.jobs)
        .try_fold(0u64, |total, (w, _)| {
            let stride = (w.max_block.max(max_loaded) as u64 + 4095) & !4095;
            // Include request descriptors, control blocks, pointer table and events.
            total.checked_add((stride + 128).checked_mul(w.depth as u64)?)
        })
        .and_then(|v| v.checked_add(records.len() as u64 * std::mem::size_of::<Request>() as u64))
        .ok_or("memory budget overflow")?;
    let minimum =
        pool_bytes.saturating_add(worker_count as u64 * std::mem::size_of::<Request>() as u64);
    if minimum > c.memory {
        return Err(crate::memory::error(
            minimum,
            c.memory,
            "I/O pools and minimum position tables",
        ));
    }
    // Validate every target and range before creating, resizing or writing any file.
    let mut targets = Vec::new();
    for target in specs {
        let path = &target.path;
        let write = if records.is_empty() {
            target
                .jobs
                .iter()
                .any(|(w, _)| w.read_probability < 1. || w.alternate)
        } else {
            records.iter().any(|r| r.write)
        } || c.precondition.is_some();
        io::validate_target(path, write)?;
        let existing = match File::open(path) {
            Ok(f) => Some(f),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(format!("open target for size/range validation {}: {e}; check that the path exists and your account has read access (raw Windows volumes require Administrator)", path.display())),
        };
        let info = existing
            .as_ref()
            .map(|f| io::inspect(path, f))
            .transpose()?;
        let actual = info.as_ref().and_then(|i| i.length);
        let regular = info.as_ref().is_none_or(|i| i.regular);
        let sector = info.as_ref().and_then(|i| i.sector);
        if existing.is_none() && !write {
            return Err(format!("read target does not exist: {}; -F is a file prefix and reads require initialized PREFIX.0001, PREFIX.0002, ... files. Create disposable test files first with the same -F PREFIX, -G SIZE and j worker count using -c ws1x1, then read with -c rs0. Writes overwrite data; choose a new test prefix", path.display()));
        }
        let (lo, hi) = c
            .range
            .or_else(|| actual.map(|n| (0, n)))
            .ok_or("new/unsized targets need -G")?;
        if lo >= hi {
            return Err("empty range".into());
        }
        if actual.is_some_and(|n| hi > n) && !(regular && write && c.recreate) {
            return Err(
                "refusing I/O: range exceeds target size; writable regular files require --recreate to enlarge"
                    .into(),
            );
        }
        if let Some(pre) = &c.precondition {
            let (start, end) = worker_range(pre, 0, lo, hi, false)?;
            if pre.workers != 1
                || pre.max_block as u64 > end - start
                || (c.backend != Backend::Pagecache
                    && (start % 512 != 0 || pre.block % 512 != 0 || pre.max_block % 512 != 0))
            {
                return Err("invalid preconditioning block/range".into());
            }
            let required = (((pre.max_block as u64 + 4095) & !4095) + 128)
                .checked_mul(pre.depth as u64)
                .ok_or("preconditioning memory overflow")?;
            if required + std::mem::size_of::<Request>() as u64 > c.memory {
                return Err("preconditioning exceeds memory budget".into());
            }
        }
        for (w, index) in &target.jobs {
            let (start, end) = worker_range(w, *index, lo, hi, !records.is_empty())?;
            if let Some(sector) = sector {
                if start % sector as u64 != 0
                    || w.block % sector != 0
                    || w.max_block % sector != 0
                    || records
                        .iter()
                        .any(|r| r.offset % sector as u64 != 0 || r.len % sector != 0)
                {
                    return Err(format!("{}: raw Windows I/O requires offsets and lengths in multiples of {sector} bytes; worker start={start}, block={}, maximum block={}; adjust -G, k/M or -S positions", path.display(), w.block, w.max_block));
                }
            }
            let count = if records.is_empty() {
                w.positions.unwrap_or((end - start) / w.block as u64)
            } else {
                records.len() as u64
            };
            let count = count
                .checked_mul(if w.verify { 2 } else { 1 })
                .ok_or("operation limit overflow")?;
            if w.passes.is_some_and(|n| n.checked_mul(count).is_none())
                || w.range_passes
                    .is_some_and(|n| n.checked_mul(end - start).is_none())
            {
                return Err("operation limit overflow".into());
            }
            if records.is_empty() && end - start < w.max_block as u64 {
                return Err("block is larger than worker range".into());
            }
            if c.backend != Backend::Pagecache
                && (start % 512 != 0 || (w.block % 512 != 0 || w.max_block % 512 != 0))
            {
                return Err("direct I/O requires sector-aligned offsets and lengths".into());
            }
            for r in records.iter() {
                if r.offset < start
                    || r.offset
                        .checked_add(r.len as u64)
                        .filter(|position_end| *position_end <= end)
                        .is_none()
                {
                    return Err("loaded position outside range".into());
                }
                if c.backend != Backend::Pagecache && (r.offset % 512 != 0 || r.len % 512 != 0) {
                    return Err("unaligned loaded position".into());
                }
            }
        }
        targets.push((target, lo, hi, existing.is_none(), actual, regular, write));
    }
    let table_capacity = ((c.memory - pool_bytes)
        / worker_count as u64
        / std::mem::size_of::<Request>() as u64) as usize;
    if table_capacity == 0 {
        return Err("no memory left for fixed position tables".into());
    }
    if records.len() > table_capacity
        || c.jobs
            .iter()
            .any(|w| w.positions.is_some_and(|n| n > table_capacity as u64))
    {
        return Err("requested positions exceed fixed memory budget".into());
    }
    let mut plans = Vec::new();
    let mut labels = Vec::new();
    for (target, lo, hi, create, actual, regular, write) in targets {
        let path = &target.path;
        let file = io::open(path, write, create, c.backend).map_err(|e| {
            format!(
                "open {} target {}: {e}",
                if write { "read/write" } else { "read-only" },
                path.display()
            )
        })?;
        if regular && (create || actual.is_some_and(|n| hi > n)) {
            file.set_len(hi).map_err(|e| {
                format!("set file length to {hi} bytes for {}: {e}", path.display())
            })?;
        }
        if let Some(pre) = &c.precondition {
            let (start, end) = worker_range(pre, 0, lo, hi, false)?;
            let plan = Plan {
                record_latency: c.latency_out.is_some(),
                file: file.try_clone().map_err(|e| e.to_string())?,
                slots: io::open_slots(path, write, c.backend, pre.depth)
                    .map_err(|e| e.to_string())?,
                work: pre.clone(),
                lo: start,
                hi: end,
                loaded: Arc::from([]),
                seed: pre.seed.unwrap_or(c.seed),
                backend: c.backend,
                timeout: c.timeout,
                duration: None,
                verify: c.verify,
                errors: c.errors,
                max_len: pre.max_block,
                position_capacity: ((c.memory
                    - ((((pre.max_block as u64 + 4095) & !4095) + 128) * pre.depth as u64))
                    / std::mem::size_of::<Request>() as u64)
                    .min((end - start) / pre.block as u64)
                    as usize,
            };
            let out = worker(plan, Arc::new(StartGate::new(1)), 0);
            if let Some(e) = out.error {
                return Err(format!("precondition {}: {e}", path.display()));
            }
        }
        for (w, index) in &target.jobs {
            let (start, end) = worker_range(w, *index, lo, hi, !records.is_empty())?;
            labels.push(path.clone());
            plans.push(Plan {
                record_latency: c.latency_out.is_some(),
                file: file.try_clone().map_err(|e| e.to_string())?,
                slots: io::open_slots(path, write, c.backend, w.depth)
                    .map_err(|e| e.to_string())?,
                work: w.clone(),
                lo: start,
                hi: end,
                loaded: records.clone(),
                seed: w.seed.unwrap_or(c.seed),
                backend: c.backend,
                timeout: c.timeout,
                duration: c.duration,
                verify: c.verify || w.verify,
                errors: c.errors,
                max_len: w.max_block.max(max_loaded),
                position_capacity: if records.is_empty() {
                    w.positions
                        .unwrap_or((end - start) / w.block as u64)
                        .min(table_capacity as u64) as usize
                } else {
                    records.len()
                },
            });
        }
    }
    let position_bytes = plans
        .iter()
        .map(|p| p.position_capacity as u64 * std::mem::size_of::<Request>() as u64)
        .sum::<u64>();
    println!(
        "backend={} workers={} fixed_pool_bytes={} memory_budget={}",
        c.backend.name(),
        plans.len(),
        pool_bytes + position_bytes,
        c.memory
    );
    println!(
        "using {:.3} GiB RAM for positions",
        position_bytes as f64 / 1073741824.
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .max_blocking_threads(plans.len())
        .build()
        .map_err(|e| e.to_string())?;
    let report_duration = plans.iter().try_fold(0f64, |longest, plan| {
        work_duration(&plan.work, plan.duration).map(|duration| longest.max(duration))
    });
    let reporter = if c.interval > 0. {
        Some(
            crate::progress::Reporter::new(plans.len(), c.interval, report_duration)
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };
    let mut gate = StartGate::new(plans.len());
    gate.progress = reporter.as_ref().map(|reporter| reporter.state.clone());
    let barrier = Arc::new(gate);
    let outcomes = runtime.block_on(async {
        let mut joins = Vec::with_capacity(plans.len());
        let mut outcomes = Vec::with_capacity(plans.len());
        for (index, p) in plans.into_iter().enumerate() {
            let barrier = barrier.clone();
            joins.push(tokio::task::spawn_blocking(move || {
                worker(p, barrier, index)
            }));
        }
        for task in joins {
            outcomes.push(task.await);
        }
        outcomes
    });
    drop(reporter);
    let mut total = Stats::default();
    let mut error = None;
    for (index, result) in outcomes.into_iter().enumerate() {
        let out = result.map_err(|e| e.to_string())?;
        let s = out.stats;
        total.reads += s.reads;
        total.writes += s.writes;
        total.read_bytes += s.read_bytes;
        total.write_bytes += s.write_bytes;
        total.seconds = total.seconds.max(s.seconds);
        total.allocations += s.allocations;
        total.submit_errors += s.submit_errors;
        total.verified += s.verified;
        total.latencies_us.extend(s.latencies_us);
        total.inflight_max = total.inflight_max.max(s.inflight_max);
        total.inflight_area_ns += s.inflight_area_ns;
        total.inflight_time_ns += s.inflight_time_ns;
        total.completed_reads += s.completed_reads;
        total.completed_writes += s.completed_writes;
        if let Some(e) = out.error {
            let context = format!(
                "worker {} target {}: {e}",
                index + 1,
                labels[index].display()
            );
            eprintln!("*error* {context}");
            error = Some(context);
        }
        if out.retained {
            eprintln!("*error* buffers retained until process exit");
            error = Some("I/O cleanup failed; buffers retained".into());
        }
    }
    let seconds = total.seconds.max(1e-9);
    let late =
        (total.completed_reads + total.completed_writes).saturating_sub(total.reads + total.writes);
    if late > 0 && total.reads + total.writes == 0 {
        eprintln!("*info* {late} I/O operations completed after the measured deadline and are excluded from throughput/IOPS; the OS/driver completed them during cleanup");
    }
    println!("read_ops={} write_ops={} seconds={:.6} read_MiB_s={:.3} write_MiB_s={:.3} IOPS={:.0} hot_path_allocations={} verified_ops={}",total.reads,total.writes,seconds,total.read_bytes as f64/1048576./seconds,total.write_bytes as f64/1048576./seconds,(total.reads+total.writes) as f64/seconds,total.allocations,total.verified);
    if total.inflight_time_ns > 0 {
        println!(
            "in_flight avg={:.2} max={} measured_seconds={:.3}",
            total.inflight_area_ns as f64 / total.inflight_time_ns as f64,
            total.inflight_max,
            total.inflight_time_ns as f64 / 1e9
        );
    }
    if total.reads + total.writes < 2 {
        println!("latency variance: n/a (insufficient samples)");
    }
    if total.writes == 0 {
        println!("write throughput: no samples");
    }
    if total.submit_errors > 0 {
        error = Some("submission errors: Input/output error".into());
    }
    if let Some(expected) = c.expected {
        if expected != (total.completed_reads, total.completed_writes) {
            error = Some(format!(
                "operation count mismatch: expected {expected:?}, got {:?}",
                (total.completed_reads, total.completed_writes)
            ));
        }
    }
    if let Some(e) = error {
        return Err(e);
    }
    if c.verify {
        eprintln!("verification: incorrect 0, ioerrors 0");
    }
    if total.allocations != 0 {
        return Err(format!("hot path allocated {} times", total.allocations));
    }
    if let Some(path) = &c.latency_out {
        let mut text = String::with_capacity(total.latencies_us.len() * 12);
        for us in &total.latencies_us {
            text.push_str(&format!("{:.3}\n", *us as f64 / 1000.));
        }
        std::fs::write(path, text).map_err(|e| format!("write latency {}: {e}", path.display()))?;
    }
    Ok(Summary {
        reads: total.reads,
        writes: total.writes,
        read_bytes: total.read_bytes,
        write_bytes: total.write_bytes,
        seconds,
        latencies_us: total.latencies_us,
    })
}
fn write_pattern(
    buffer: &mut [u8],
    request: Request,
    work: &Workload,
    seed: u64,
    reusable: bool,
    template: &[u8; 4096],
) {
    let seed = if work.unique {
        request.seed as u64
    } else {
        seed
    };
    if reusable {
        pattern::stamp(buffer, request.offset, seed);
    } else if work.unique {
        pattern::fill(buffer, request.offset, seed);
    } else {
        pattern::fill_cached(buffer, request.offset, seed, template);
    }
}
fn check_pattern(
    buffer: &[u8],
    request: Request,
    work: &Workload,
    seed: u64,
    template: &[u8; 4096],
) -> bool {
    if work.unique {
        pattern::matches(buffer, request.offset, request.seed as u64)
    } else {
        pattern::matches_cached(buffer, request.offset, seed, template)
    }
}
fn worker(p: Plan, barrier: Arc<StartGate>, worker_index: usize) -> Outcome {
    let mixed = if p.loaded.is_empty() {
        p.work.read_probability > 0. && p.work.read_probability < 1.
    } else {
        p.loaded.iter().any(|r| r.write) && p.loaded.iter().any(|r| !r.write)
    };
    let reusable = !p.work.alternate
        && !p.work.unique
        && p.work.block == p.work.max_block
        && p.loaded.is_empty()
        && p.work.read_probability == 0.
        && p.work.block % 4096 == 0
        && p.lo % 4096 == 0;
    let prepared: std::result::Result<_, Failure> = (|| {
        let mut queue = Queue::new(
            p.file,
            p.backend,
            p.work.depth,
            p.max_len,
            p.timeout,
            p.work.submit_batch,
            p.slots,
        )?;
        let generated = crate::positions::Positions::new(
            &p.work,
            p.lo,
            p.hi,
            p.seed,
            p.position_capacity,
            &p.loaded,
        )
        .map_err(std::io::Error::other)?;
        if reusable {
            for i in 0..p.work.depth {
                queue.requests[i].len = p.work.block;
                pattern::fill(queue.buffer(i), 0, p.seed);
            }
        }
        // Restore this fixed payload after a mixed-workload read overwrites a slot.
        let mut template = [0; 4096];
        pattern::fill(&mut template, 0, p.seed);
        Ok((queue, generated, template))
    })();
    if prepared.is_err() {
        barrier.failed.store(true, Ordering::Release);
    }
    barrier.ready.wait();
    let (mut q, mut generated, template) = match prepared {
        Err(error) => {
            return Outcome {
                stats: Stats::default(),
                error: Some(error),
                retained: false,
            }
        }
        Ok(resources) if barrier.failed.load(Ordering::Acquire) => {
            drop(resources);
            return Outcome {
                stats: Stats::default(),
                error: None,
                retained: false,
            };
        }
        Ok(resources) => resources,
    };
    let start = *barrier.start.get_or_init(Instant::now);
    let progress = barrier.progress.as_ref().map(|state| {
        state.start.get_or_init(|| start);
        state.counter(worker_index)
    });
    q.observer = barrier
        .progress
        .as_ref()
        .map(|state| (state.clone(), worker_index));
    let mut stats = Stats::default();
    if p.record_latency {
        stats.latencies_us = Vec::with_capacity(LATENCY_CAPACITY);
    }
    let guard = allocation::Guard::start();
    let mut error = None;
    let positions = generated.len();
    let limit = match (
        p.work.passes.and_then(|n| n.checked_mul(positions)),
        p.work.operation_limit,
    ) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
    let byte_limit = p.work.range_passes.map(|n| n * (p.hi - p.lo));
    let duration = work_duration(&p.work, p.duration);
    let mut index = 0u64;
    // On Linux loop devices, continuously refilling pure-direction io_uring
    // queues creates block-dispatch contention at high worker counts. Keep
    // those submissions in bounded batches; mixed I/O benefits from refill.
    // Verify, flush, alternate, rate and duty need the batch loop: they check, sync, switch or pace
    // whole batches. Everything else streams where the platform's backend can.
    let batch_only = p.verify
        || p.work.flush
        || p.work.alternate
        || p.work.rate.is_some()
        || p.work.duty.is_some();
    let platform_streams = (cfg!(target_os = "linux")
        && !p.record_latency
        && (p.backend == Backend::Libaio || (p.backend == Backend::Uring && mixed)))
        || cfg!(any(windows, target_os = "macos", target_os = "freebsd"));
    let streamed = platform_streams && !batch_only;
    #[cfg(any(
        target_os = "linux",
        windows,
        target_os = "macos",
        target_os = "freebsd"
    ))]
    if streamed {
        let mut queued_bytes = 0u64;
        let result = q.execute_stream(
            || {
                if limit.is_some_and(|n| index >= n)
                    || byte_limit.is_some_and(|n| queued_bytes >= n)
                    || duration.is_some_and(|s| start.elapsed().as_secs_f64() >= s)
                {
                    return None;
                }
                let request = generated.get(index);
                index += 1;
                queued_bytes += request.len as u64;
                Some(request)
            },
            |request, buffer| {
                if request.write {
                    write_pattern(buffer, request, &p.work, p.seed, reusable, &template);
                }
            },
            |request| {
                let measured =
                    duration.is_none_or(|seconds| start.elapsed().as_secs_f64() < seconds);
                stats.complete(request, measured);
                stats.publish(progress);
            },
            p.errors,
        );
        if let Err(e) = result {
            error = Some(e);
        }
        if let Some(port) = q.take_stream_stats() {
            stats.inflight_max = port.max_in_flight;
            stats.inflight_area_ns = port.area_ns;
            stats.inflight_time_ns = port.time_ns;
            if p.record_latency {
                stats.latencies_us.extend(port.latencies_us);
            }
        }
    }
    if !streamed {
        loop {
            if limit.is_some_and(|n| index >= n)
                || byte_limit.is_some_and(|n| stats.read_bytes + stats.write_bytes >= n)
                || duration.is_some_and(|s| start.elapsed().as_secs_f64() >= s)
            {
                break;
            }
            let elapsed = start.elapsed().as_secs_f64();
            let rate_wait = p
                .work
                .rate
                .map_or(0., |rate| index as f64 / rate - elapsed)
                .max(0.);
            let duty_wait = p.work.duty.map_or(0., |(on, off)| {
                let phase = elapsed % (on + off);
                if phase >= on {
                    on + off - phase
                } else {
                    0.
                }
            });
            let wait = rate_wait.max(duty_wait);
            if wait > 0. {
                let wait = duration.map_or(wait, |d| wait.min((d - elapsed).max(0.)));
                std::thread::sleep(std::time::Duration::from_secs_f64(wait.min(60.)));
                continue;
            }
            let effective_depth = if p.work.rate.is_some() {
                1
            } else {
                p.work.depth
            };
            let batch_limit = limit.map_or(effective_depth, |n| {
                (n - index).min(effective_depth as u64) as usize
            });
            let batch_limit = batch_limit.min(generated.phase_remaining(index) as usize);
            let mut count = 0;
            let mut batch_bytes = 0u64;
            for i in 0..batch_limit {
                if byte_limit
                    .is_some_and(|n| stats.read_bytes + stats.write_bytes + batch_bytes >= n)
                {
                    break;
                }
                let request = generated.get(index + i as u64);
                count += 1;
                batch_bytes += request.len as u64;
                q.requests[i] = request;
                if request.write {
                    write_pattern(q.buffer(i), request, &p.work, p.seed, reusable, &template);
                }
            }
            let batch_started = Instant::now();
            let result = q.execute(count, p.errors, |i, r| {
                if r.write && p.work.verify {
                    generated.written(index + i as u64);
                }
                let measured =
                    duration.is_none_or(|seconds| start.elapsed().as_secs_f64() < seconds);
                if measured
                    && p.record_latency
                    && stats.latencies_us.len() < stats.latencies_us.capacity()
                {
                    stats
                        .latencies_us
                        .push(batch_started.elapsed().as_micros() as u32);
                }
                stats.complete(r, measured);
                stats.publish(progress);
            });
            if let Err(e) = result {
                error = Some(e);
                break;
            }
            stats.publish(progress);
            if p.work.flush && q.requests[..count].iter().any(|r| r.write) {
                if let Err(e) = q.flush() {
                    error = Some(e.into());
                    break;
                }
            }
            for i in 0..count {
                let request = q.requests[i];
                if request.verify && !request.write {
                    if !check_pattern(q.buffer(i), request, &p.work, p.seed, &template) {
                        error = Some(Failure::Verification);
                        break;
                    }
                    generated.checked(index + i as u64);
                    stats.verified += 1;
                }
            }
            if error.is_some() {
                break;
            }
            if p.verify && !p.work.verify {
                // Reuse the pool for verification, compacting only writes into its front.
                let mut n = 0;
                for i in 0..count {
                    let r = q.requests[i];
                    if r.write {
                        q.requests[n] = Request { write: false, ..r };
                        n += 1;
                    }
                }
                if n > 0 {
                    if let Err(e) = q.execute(n, p.errors, |_, _| {}) {
                        error = Some(e);
                        break;
                    }
                    for i in 0..n {
                        let r = q.requests[i];
                        if !check_pattern(q.buffer(i), r, &p.work, p.seed, &template) {
                            error = Some(Failure::Verification);
                            break;
                        }
                        stats.verified += 1;
                    }
                }
            }
            if error.is_some() {
                break;
            }
            index += count as u64;
        }
    }
    let elapsed = start.elapsed().as_secs_f64();
    stats.seconds = duration.map_or(elapsed, |seconds| elapsed.min(seconds));
    // A time/byte/operation limit may end between a write and its scheduled
    // metadata read. Complete those checks outside the workload counters.
    if p.work.verify && error.is_none() {
        let mut cursor = 0;
        loop {
            let mut count = 0;
            while count < p.work.depth {
                let Some(request) = generated.take_pending_check(&mut cursor) else {
                    break;
                };
                q.requests[count] = request;
                count += 1;
            }
            if count == 0 {
                break;
            }
            if let Err(e) = q.execute(count, p.errors, |_, _| {}) {
                error = Some(e);
                break;
            }
            for i in 0..count {
                let request = q.requests[i];
                if !check_pattern(q.buffer(i), request, &p.work, p.seed, &template) {
                    error = Some(Failure::Verification);
                    break;
                }
                stats.verified += 1;
            }
            if error.is_some() {
                break;
            }
        }
    }
    stats.publish(progress);
    stats.allocations = guard.count();
    stats.submit_errors = q.submit_errors;
    drop(guard);
    if let Some(counter) = progress {
        counter.finish();
    }
    let retained = !q.close();
    Outcome {
        stats,
        error,
        retained,
    }
}
