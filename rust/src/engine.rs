use crate::iscsi::{connection::IscsiFabric, credentials, path as iscsi_path};
use crate::nvme::{controller::random_hostid, session::NvmeFabric};
use crate::paths::{open_paths, PathSpec, Policy};
use crate::{
    allocation,
    cli::{Backend, Config, Workload},
    io::{self, Failure, Next, Queue, Request},
    pattern, Result,
};
use std::{
    cell::RefCell,
    collections::VecDeque,
    fs::File,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Barrier, OnceLock,
    },
    time::{Duration, Instant},
};
/// What a worker runs over: the paths of a target (iSCSI, NVMe-oF or both), with the settings to open them.
#[derive(Clone)]
struct PathPlan {
    paths: Vec<PathSpec>,
    initiator: String,
    chap: Option<(String, Vec<u8>)>,
    hostid: [u8; 16],
    policy: Policy,
}

struct Plan {
    record_latency: bool,
    /// None for an iSCSI target, which has no file.
    file: Option<File>,
    /// Windows only: one file per queue slot.
    slots: Vec<File>,
    paths: Option<PathPlan>,
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
/// How many wrong offsets a worker keeps for the report.
const INCORRECT_SAMPLES: usize = 16;

#[derive(Default)]
struct Stats {
    /// Blocks whose contents did not match what was written.
    incorrect: u64,
    incorrect_offsets: [u64; INCORRECT_SAMPLES],
    incorrect_offsets_len: u8,
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
/// The next block for write-verify: in order when `sequential`, else a xorshift draw.
fn write_verify_index(work: &Workload, blocks: u64, count: &mut u64, state: &mut u64) -> u64 {
    let index = if work.sequential >= 1. {
        *count % blocks
    } else {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state % blocks
    };
    *count += 1;
    index
}

/// Counts a block whose contents are wrong and keeps the first few offsets. Keeps going, so every wrong
/// block is counted. Fixed-size storage, so a bad run does not allocate in the timed loop.
fn count_incorrect(stats: &mut Stats, offset: u64) {
    stats.incorrect += 1;
    let n = stats.incorrect_offsets_len as usize;
    if n < INCORRECT_SAMPLES {
        stats.incorrect_offsets[n] = offset;
        stats.incorrect_offsets_len += 1;
    }
}

/// Seconds to wait before the request at `index` so the rate and duty limits hold.
fn pacing_wait(work: &Workload, index: u64, elapsed: f64) -> f64 {
    let rate_wait = work
        .rate
        .map_or(0., |rate| index as f64 / rate - elapsed)
        .max(0.);
    let duty_wait = work.duty.map_or(0., |(on, off)| {
        let phase = elapsed % (on + off);
        if phase >= on {
            on + off - phase
        } else {
            0.
        }
    });
    rate_wait.max(duty_wait)
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
    /// The paths of this target; empty for a file target.
    paths: Vec<PathSpec>,
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
fn target_specs(c: &Config, paths: &[PathSpec]) -> Vec<Target> {
    if !paths.is_empty() {
        let jobs = c
            .jobs
            .iter()
            .flat_map(|w| (0..w.workers).map(move |index| (w.clone(), index)))
            .collect::<Vec<_>>();
        return vec![Target {
            path: PathBuf::from("paths"),
            jobs,
            paths: paths.to_vec(),
        }];
    }
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
            paths: Vec::new(),
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
                paths: Vec::new(),
            });
        }
    }
    targets
}
/// `s0v` with `-S FILE`: writes the positions file as N write/read pairs. Each pair writes a block and
/// then reads the same block back, so a replay verifies every write. N is the workload's position count
/// (`P`), or the range divided by the block size. Offsets are random for `s0` and sequential for `s1`.
fn write_verify_positions(c: &Config, path: &Path) -> Result<()> {
    let Some(w) = c.jobs.iter().find(|w| w.write_verify) else {
        return Ok(());
    };
    let (lo, hi) = c
        .range
        .ok_or("s0v with -S needs -G SIZE to set the range the pairs cover")?;
    let block = w.block as u64;
    let blocks = (hi - lo) / block;
    if blocks == 0 {
        return Err("range is smaller than one block".into());
    }
    let count = w.positions.unwrap_or(blocks);
    let sequential = w.sequential >= 1.;
    let mut state = c.seed.max(1);
    let mut out = std::io::BufWriter::new(
        File::create(path).map_err(|e| format!("{}: {e}", path.display()))?,
    );
    for i in 0..count {
        let index = if sequential {
            i % blocks
        } else {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state % blocks
        };
        let offset = lo + index * block;
        writeln!(out, "{offset} W {}", w.block).map_err(|e| e.to_string())?;
        writeln!(out, "{offset} R {}", w.block).map_err(|e| e.to_string())?;
    }
    out.flush().map_err(|e| e.to_string())
}

fn loaded(c: &Config) -> Result<Arc<[Request]>> {
    let Some(path) = &c.positions_file else {
        return Ok(Arc::from([]));
    };
    write_verify_positions(c, path)?;
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
    // Installed first, so Ctrl-C during setup still stops the run and prints the report.
    let _signals = crate::signals::Guard::install().map_err(|e| e.to_string())?;
    let records = loaded(&c)?;
    let mut paths = c
        .iscsi
        .iter()
        .map(|p| iscsi_path::parse_path(p).map(PathSpec::Iscsi))
        .collect::<Result<Vec<_>>>()?;
    for p in &c.nvmeof {
        paths.push(PathSpec::Nvmeof(crate::nvme::parse_path(p)?));
    }
    crate::paths::set_dump(c.dump);
    let specs = target_specs(&c, &paths);
    let budget = memory_budget(&c, &specs, &records)?;
    let checked = check_targets(&c, specs, &records)?;
    let table_capacity = table_capacity(&c, &budget, records.len())?;
    let (plans, labels) = build_plans(&c, checked, &records, table_capacity)?;
    let position_bytes = plans
        .iter()
        .map(|p| p.position_capacity as u64 * std::mem::size_of::<Request>() as u64)
        .sum::<u64>();
    println!(
        "backend={} workers={} fixed_pool_bytes={} memory_budget={}",
        c.backend.name(),
        plans.len(),
        budget.pool_bytes + position_bytes,
        c.memory
    );
    println!(
        "using {:.3} GiB RAM for positions",
        position_bytes as f64 / 1073741824.
    );
    let outcomes = execute_plans(plans, c.interval)?;
    let (total, error) = aggregate(outcomes, &labels)?;
    let summary = report(&c, total, error);
    crate::paths::print_totals();
    summary
}

/// Memory the fixed I/O pools need, before any position tables are sized.
struct Budget {
    pool_bytes: u64,
    worker_count: usize,
}

fn memory_budget(c: &Config, specs: &[Target], records: &[Request]) -> Result<Budget> {
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
    Ok(Budget {
        pool_bytes,
        worker_count,
    })
}

/// Validates every target and range before any file is created, resized or written.
fn check_targets(c: &Config, specs: Vec<Target>, records: &[Request]) -> Result<Vec<Checked>> {
    specs
        .into_iter()
        .map(|target| check_target(c, target, records))
        .collect()
}

/// A target that passed validation, with the byte range the run may use.
struct Checked {
    target: Target,
    lo: u64,
    hi: u64,
    create: bool,
    actual: Option<u64>,
    regular: bool,
    write: bool,
}

/// Sizes the paths of a target with a probe open, then checks the workload against the LUN.
fn check_paths(c: &Config, target: Target, records: &[Request]) -> Result<Checked> {
    let write = if records.is_empty() {
        target
            .jobs
            .iter()
            .any(|(w, _)| w.read_probability < 1. || w.alternate)
    } else {
        records.iter().any(|r| r.write)
    };
    if c.precondition.is_some() {
        return Err("preconditioning is not supported over iSCSI or NVMe-oF yet".into());
    }
    let initiator = c.iscsi_initiator.as_deref().unwrap_or("");
    let chap = credentials::chap_from_config(c)?;
    let chap_ref = chap.as_ref().map(|(u, s)| (u.as_str(), s.as_slice()));
    let hostid = random_hostid()?;
    let probe = open_paths(
        &target.paths,
        IscsiFabric::new(initiator, chap_ref),
        NvmeFabric::new(crate::nvme::DEFAULT_HOSTNQN, hostid),
        c.path_policy,
        c.seed,
    )?;
    let capacity = probe.capacity_bytes();
    let (lo, hi) = c.range.unwrap_or((0, capacity));
    if lo >= hi {
        return Err("empty range".into());
    }
    if hi > capacity {
        return Err(format!("range exceeds the LUN size of {capacity} bytes"));
    }
    for (w, index) in &target.jobs {
        check_job(
            &target.path,
            Some(probe.block_len() as usize),
            w,
            *index,
            lo,
            hi,
            records,
            c.backend,
        )?;
    }
    Ok(Checked {
        target,
        lo,
        hi,
        create: false,
        actual: Some(capacity),
        regular: true,
        write,
    })
}

fn check_target(c: &Config, target: Target, records: &[Request]) -> Result<Checked> {
    if !target.paths.is_empty() {
        return check_paths(c, target, records);
    }
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
        check_precondition(pre, lo, hi, c.memory, c.backend)?;
    }
    for (w, index) in &target.jobs {
        check_job(path, sector, w, *index, lo, hi, records, c.backend)?;
    }
    Ok(Checked {
        target,
        lo,
        hi,
        create: existing.is_none(),
        actual,
        regular,
        write,
    })
}

fn check_precondition(
    pre: &Workload,
    lo: u64,
    hi: u64,
    memory: u64,
    backend: Backend,
) -> Result<()> {
    let (start, end) = worker_range(pre, 0, lo, hi, false)?;
    if pre.workers != 1
        || pre.max_block as u64 > end - start
        || (backend != Backend::Pagecache
            && (start % 512 != 0 || pre.block % 512 != 0 || pre.max_block % 512 != 0))
    {
        return Err("invalid preconditioning block/range".into());
    }
    let required = (((pre.max_block as u64 + 4095) & !4095) + 128)
        .checked_mul(pre.depth as u64)
        .ok_or("preconditioning memory overflow")?;
    if required + std::mem::size_of::<Request>() as u64 > memory {
        return Err("preconditioning exceeds memory budget".into());
    }
    Ok(())
}

/// Checks one worker's range, alignment and operation limits against its target.
#[allow(clippy::too_many_arguments)]
fn check_job(
    path: &Path,
    sector: Option<usize>,
    w: &Workload,
    index: usize,
    lo: u64,
    hi: u64,
    records: &[Request],
    backend: Backend,
) -> Result<()> {
    let (start, end) = worker_range(w, index, lo, hi, !records.is_empty())?;
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
    if backend != Backend::Pagecache
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
        if backend != Backend::Pagecache && (r.offset % 512 != 0 || r.len % 512 != 0) {
            return Err("unaligned loaded position".into());
        }
    }
    Ok(())
}

/// Position tables get whatever memory the pools leave, split evenly between workers.
fn table_capacity(c: &Config, budget: &Budget, record_count: usize) -> Result<usize> {
    let table_capacity = ((c.memory - budget.pool_bytes)
        / budget.worker_count as u64
        / std::mem::size_of::<Request>() as u64) as usize;
    if table_capacity == 0 {
        return Err("no memory left for fixed position tables".into());
    }
    if record_count > table_capacity
        || c.jobs
            .iter()
            .any(|w| w.positions.is_some_and(|n| n > table_capacity as u64))
    {
        return Err("requested positions exceed fixed memory budget".into());
    }
    Ok(table_capacity)
}

/// Opens each validated target, runs any preconditioning, and makes one plan per worker.
fn build_plans(
    c: &Config,
    checked: Vec<Checked>,
    records: &Arc<[Request]>,
    table_capacity: usize,
) -> Result<(Vec<Plan>, Vec<PathBuf>)> {
    let max_loaded = records.iter().map(|r| r.len).max().unwrap_or(0);
    let mut plans = Vec::new();
    let mut labels = Vec::new();
    let iscsi_chap = credentials::chap_from_config(c)?;
    for Checked {
        target,
        lo,
        hi,
        create,
        actual,
        regular,
        write,
    } in checked
    {
        let path = &target.path;
        let hostid = random_hostid()?;
        let paths = (!target.paths.is_empty()).then(|| PathPlan {
            paths: target.paths.clone(),
            initiator: c.iscsi_initiator.clone().unwrap_or_default(),
            chap: iscsi_chap.clone(),
            hostid,
            policy: c.path_policy,
        });
        let file = match paths {
            Some(_) => None,
            None => {
                let file = open_target(c, path, write, create, actual, regular, hi)?;
                if let Some(pre) = &c.precondition {
                    run_precondition(c, pre, &file, path, write, lo, hi)?;
                }
                Some(file)
            }
        };
        for (w, index) in &target.jobs {
            let (start, end) = worker_range(w, *index, lo, hi, !records.is_empty())?;
            labels.push(path.clone());
            plans.push(Plan {
                record_latency: c.latency_out.is_some(),
                file: match &file {
                    Some(file) => Some(file.try_clone().map_err(|e| e.to_string())?),
                    None => None,
                },
                slots: match paths {
                    Some(_) => Vec::new(),
                    None => io::open_slots(path, write, c.backend, w.depth)
                        .map_err(|e| e.to_string())?,
                },
                paths: paths.clone(),
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
    Ok((plans, labels))
}

/// Opens the target read-only or read/write, and sizes it to the range the run may touch.
fn open_target(
    c: &Config,
    path: &Path,
    write: bool,
    create: bool,
    actual: Option<u64>,
    regular: bool,
    hi: u64,
) -> Result<File> {
    let file = io::open(path, write, create, c.backend).map_err(|e| {
        format!(
            "open {} target {}: {e}",
            if write { "read/write" } else { "read-only" },
            path.display()
        )
    })?;
    if regular && (create || actual.is_some_and(|n| hi > n)) {
        file.set_len(hi)
            .map_err(|e| format!("set file length to {hi} bytes for {}: {e}", path.display()))?;
    }
    Ok(file)
}

/// Runs the preconditioning workload over the target before the measured workers start.
fn run_precondition(
    c: &Config,
    pre: &Workload,
    file: &File,
    path: &Path,
    write: bool,
    lo: u64,
    hi: u64,
) -> Result<()> {
    let (start, end) = worker_range(pre, 0, lo, hi, false)?;
    let plan = Plan {
        record_latency: c.latency_out.is_some(),
        file: Some(file.try_clone().map_err(|e| e.to_string())?),
        paths: None,
        slots: io::open_slots(path, write, c.backend, pre.depth).map_err(|e| e.to_string())?,
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
            .min((end - start) / pre.block as u64) as usize,
    };
    let out = worker(plan, Arc::new(StartGate::new(1)), 0);
    if let Some(e) = out.error {
        return Err(format!("precondition {}: {e}", path.display()));
    }
    Ok(())
}

/// Runs every plan on its own blocking task, started together behind one gate.
fn execute_plans(
    plans: Vec<Plan>,
    interval: f64,
) -> Result<Vec<std::result::Result<Outcome, tokio::task::JoinError>>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .max_blocking_threads(plans.len())
        .build()
        .map_err(|e| e.to_string())?;
    let report_duration = plans.iter().try_fold(0f64, |longest, plan| {
        work_duration(&plan.work, plan.duration).map(|duration| longest.max(duration))
    });
    let reporter = if interval > 0. {
        Some(
            crate::progress::Reporter::new(plans.len(), interval, report_duration)
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
    Ok(outcomes)
}

/// Merges every worker's counters into one total. The first worker error is returned with the total.
fn aggregate(
    outcomes: Vec<std::result::Result<Outcome, tokio::task::JoinError>>,
    labels: &[PathBuf],
) -> Result<(Stats, Option<String>)> {
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
        total.incorrect += s.incorrect;
        for &offset in &s.incorrect_offsets[..s.incorrect_offsets_len as usize] {
            if (total.incorrect_offsets_len as usize) < INCORRECT_SAMPLES {
                total.incorrect_offsets[total.incorrect_offsets_len as usize] = offset;
                total.incorrect_offsets_len += 1;
            }
        }
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
    Ok((total, error))
}

/// Prints the summary lines, applies the pass/fail checks, and writes the latency file.
fn report(c: &Config, total: Stats, mut error: Option<String>) -> Result<Summary> {
    let seconds = total.seconds.max(1e-9);
    if crate::signals::INTERRUPTED.load(Ordering::Relaxed) {
        eprintln!("*info* interrupted by Ctrl-C; results cover the I/O completed so far");
    }
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
    if total.submit_errors > 0 && error.is_none() {
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
    if c.verify || c.jobs.iter().any(|w| w.verify || w.write_verify) {
        eprintln!(
            "verification: checked {}, incorrect {}",
            total.verified, total.incorrect
        );
        for offset in &total.incorrect_offsets[..total.incorrect_offsets_len as usize] {
            eprintln!("*error* verification mismatch: wrong contents at offset {offset}");
        }
        if total.incorrect > 0 {
            return Err(format!(
                "verification mismatch: {} blocks had wrong contents",
                total.incorrect
            ));
        }
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
    // A panic during setup must still reach the barrier, or every other worker waits forever.
    let prepared = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| Run::prepare(p)))
        .unwrap_or_else(|_| {
            Err(Failure::from(std::io::Error::other(
                "worker panicked during setup",
            )))
        });
    if prepared.is_err() {
        barrier.failed.store(true, Ordering::Release);
    }
    barrier.ready.wait();
    let mut run = match prepared {
        Err(error) => return Outcome::failed(error),
        Ok(run) if barrier.failed.load(Ordering::Acquire) => {
            drop(run);
            return Outcome::stopped();
        }
        Ok(run) => run,
    };
    run.begin(&barrier, worker_index);
    let guard = allocation::Guard::start();
    if run.streamed {
        run.stream();
    } else {
        run.batches();
    }
    run.finish(guard)
}

/// One worker's measured run: its queue and position generator, the limits it runs under, and
/// what it has measured so far.
struct Run {
    queue: Queue,
    generated: crate::positions::Positions,
    /// `v` workloads: the pairs' own generator state, and the read due after the last write.
    wv_count: u64,
    wv_state: u64,
    wv_pending: Option<Request>,
    /// The streamed run's in-flight and owed-read lists, created before the timed loop so it does not allocate.
    flight: Vec<Request>,
    owed: VecDeque<Request>,
    held: Vec<Request>,
    wv_lo: u64,
    wv_hi: u64,
    template: [u8; 4096],
    work: Workload,
    seed: u64,
    reusable: bool,
    verify: bool,
    errors: u64,
    record_latency: bool,
    duration: Option<f64>,
    limit: Option<u64>,
    byte_limit: Option<u64>,
    streamed: bool,
    start: Instant,
    progress: Option<(Arc<crate::progress::State>, usize)>,
    stats: Stats,
    error: Option<Failure>,
    index: u64,
}

/// One streamed run's source and completion handling. The backend calls `source` for each free slot and
/// `finish` for each completion; both reach this state through one `RefCell`.
struct Stream<'a> {
    generated: &'a mut crate::positions::Positions,
    stats: &'a mut Stats,
    work: &'a Workload,
    template: &'a [u8; 4096],
    seed: u64,
    /// Whether written blocks are read back: `-v`, or a `v` or `U` workload.
    read_back: bool,
    /// Whether read-backs are workload operations: a `v` or `U` workload's reads are, a `-v` read-back is not.
    counted_read_backs: bool,
    /// Whether a read-back may run while its write is still in flight: `V` across several endpoints does.
    race: bool,
    duration: Option<f64>,
    start: Instant,
    progress: &'a Option<(Arc<crate::progress::State>, usize)>,
    limit: Option<u64>,
    byte_limit: Option<u64>,
    /// The worker's range, for write-verify's own offsets.
    lo: u64,
    hi: u64,
    /// The next position in the plan.
    index: u64,
    wv_count: u64,
    wv_state: u64,
    /// Write-verify requests handed out, writes and read-backs both, which the operation limit counts.
    wv_ops: u64,
    /// Bytes handed to the backend that count toward the byte limit.
    queued_bytes: u64,
    /// Requests handed to the backend and not yet completed.
    outstanding: usize,
    /// Those requests, kept when writes are read back, so a new request can be checked against them.
    in_flight: Vec<Request>,
    /// Read-backs of completed writes, owed now.
    due: VecDeque<Request>,
    /// Requests taken from the source but not yet handed out, newest last: each overlaps something that
    /// must finish first, so it waits its turn.
    held: Vec<Request>,
    /// Slot `i`'s buffer begins `i * stride` bytes past `base`.
    base: *mut u8,
    stride: usize,
}

impl Stream<'_> {
    /// The next request for a free slot, or Wait while one overlaps another in flight, or Done once the
    /// run has stopped and nothing is owed.
    fn source(&mut self) -> Next {
        self.pace();
        let candidate = match self.held.pop() {
            Some(request) => Some(request),
            None => match self.due.pop_front() {
                Some(request) => Some(request),
                None if self.stopped() => None,
                None => self.plan_request(),
            },
        };
        let Some(mut request) = candidate else {
            return if self.outstanding == 0 {
                Next::Done
            } else {
                Next::Wait
            };
        };
        // A write goes after the owed read-back of its extent: that read is issued first, and the write waits.
        if request.write {
            if let Some(at) = self.due.iter().position(|d| self.gates(&request, d)) {
                self.held.push(request);
                request = self.due.remove(at).expect("the owed read is in the list");
            }
        }
        if (self.work.rate.is_some() && self.outstanding > 0) || self.overlaps(&request) {
            self.held.push(request);
            return Next::Wait;
        }
        // A `-v` read-back is verification, not an operation, so it is outside the limits.
        if !request.verify || self.counted_read_backs {
            self.queued_bytes += request.len as u64;
        }
        if self.work.write_verify {
            self.wv_ops += 1;
        }
        self.outstanding += 1;
        if self.read_back {
            self.in_flight.push(request);
        }
        // With more than one endpoint the read-back is owed as the write is issued, so it can race the write.
        if self.race && request.write && self.read_back {
            self.due.push_back(read_back_of(&request));
        }
        Next::Request(request)
    }

    /// Records one completion. A write that verifies is owed a read-back of the same extent. A read-back is
    /// checked against its slot's buffer and counted as verification, not as an operation.
    fn finish(&mut self, slot: usize, request: Request) {
        self.outstanding -= 1;
        if self.read_back {
            let at = self
                .in_flight
                .iter()
                .position(|f| same_io(f, &request))
                .expect("completion for a request that is not in flight");
            self.in_flight.swap_remove(at);
        }
        if request.verify && !request.write {
            // SAFETY: the backend has finished this slot's transfer and does not reuse its buffer until
            // this call returns, and the slot lies within the arena of `request.len` bytes.
            let buffer = unsafe {
                std::slice::from_raw_parts(self.base.add(slot * self.stride), request.len)
            };
            if !check_pattern(buffer, request, self.work, self.seed, self.template) {
                count_incorrect(self.stats, request.offset);
            }
            self.stats.verified += 1;
        } else if request.write && self.read_back && !self.race {
            self.due.push_back(read_back_of(&request));
        }
        // A read-back that completes after the limit is verification outside the workload's counts.
        let is_read_back = request.verify && !request.write;
        if !is_read_back || (self.counted_read_backs && !self.limit_reached()) {
            let measured = self
                .duration
                .is_none_or(|seconds| self.start.elapsed().as_secs_f64() < seconds);
            self.stats.complete(request, measured);
        }
        self.stats.publish(counter(self.progress));
    }

    /// True once the run has no more new requests to issue.
    fn stopped(&self) -> bool {
        crate::signals::INTERRUPTED.load(Ordering::Relaxed)
            || self.limit_reached()
            || self
                .duration
                .is_some_and(|s| self.start.elapsed().as_secs_f64() >= s)
    }

    /// True once the operation or byte limit is reached. Plan slots count whether they are reads, writes
    /// or verified checks, as the position plan lays them out; write-verify counts its own requests.
    fn limit_reached(&self) -> bool {
        self.limit.is_some_and(|n| self.ops() >= n)
            || self.byte_limit.is_some_and(|n| self.queued_bytes >= n)
    }

    /// The operations counted against the limit and the rate: plan slots, or write-verify requests.
    fn ops(&self) -> u64 {
        if self.work.write_verify {
            self.wv_ops
        } else {
            self.index
        }
    }

    /// The next plan request, or None once the limit is reached. Plan reads of verified writes are skipped:
    /// each write's read-back is owed on its completion instead. Skipped slots still count toward the limit.
    fn plan_request(&mut self) -> Option<Request> {
        if self.work.write_verify {
            let block = self.work.block as u64;
            let blocks = ((self.hi - self.lo) / block).max(1);
            let index =
                write_verify_index(self.work, blocks, &mut self.wv_count, &mut self.wv_state);
            return Some(Request {
                offset: self.lo + index * block,
                len: self.work.block,
                write: true,
                verify: false,
                seed: 0,
                state: 0,
            });
        }
        loop {
            if self.limit_reached() {
                return None;
            }
            let request = self.generated.get(self.index);
            self.index += 1;
            if !request.verify || request.write {
                return Some(request);
            }
        }
    }

    /// True when `request` overlaps a request that must finish first: any request in flight, or any owed
    /// read-back, where at least one of the two writes. Reads of different extents never wait.
    fn overlaps(&self, request: &Request) -> bool {
        self.read_back
            && (self.in_flight.iter().any(|f| self.gates(request, f))
                || self.due.iter().any(|d| self.gates(request, d)))
    }

    /// Whether `candidate` may not be in flight with `other`. They conflict when they overlap and one writes.
    /// Under write-verify across paths, a read-back may be issued while its own write is in flight. That is the
    /// only such pair: a write cannot start on an extent while an earlier write's read-back is still pending,
    /// so any read-back of the extent that overlaps an in-flight write is that write's own.
    fn gates(&self, candidate: &Request, other: &Request) -> bool {
        let own_read_back = self.race && candidate.verify && !candidate.write && other.write;
        conflict(candidate, other) && !own_read_back
    }

    /// Sleeps through rate and duty off-times before the next request.
    fn pace(&self) {
        loop {
            let elapsed = self.start.elapsed().as_secs_f64();
            let wait = pacing_wait(self.work, self.ops(), elapsed);
            let wait = self
                .duration
                .map_or(wait, |d| wait.min((d - elapsed).max(0.)));
            if wait <= 0. {
                return;
            }
            std::thread::sleep(Duration::from_secs_f64(wait.min(60.)));
        }
    }
}

/// The read-back a verified write is owed: the same extent, read and checked.
fn read_back_of(write: &Request) -> Request {
    Request {
        write: false,
        verify: true,
        ..*write
    }
}

/// Whether two requests move the same bytes the same way. The backend's `state` is not part of it.
fn same_io(a: &Request, b: &Request) -> bool {
    a.offset == b.offset && a.len == b.len && a.write == b.write && a.verify == b.verify
}

/// Whether two requests may not be in flight together: they overlap and at least one writes.
fn conflict(a: &Request, b: &Request) -> bool {
    (a.write || b.write) && a.offset < b.offset + b.len as u64 && b.offset < a.offset + a.len as u64
}

/// The progress counter a worker publishes to, if the run reports progress.
fn counter(
    progress: &Option<(Arc<crate::progress::State>, usize)>,
) -> Option<&crate::progress::Counter> {
    progress
        .as_ref()
        .map(|(state, index)| state.counter(*index))
}

impl Outcome {
    /// The worker could not open its queue or generator.
    fn failed(error: Failure) -> Self {
        Self {
            stats: Stats::default(),
            error: Some(error),
            retained: false,
        }
    }

    /// Another worker failed before the start gate opened, so this one measured nothing.
    fn stopped() -> Self {
        Self {
            stats: Stats::default(),
            error: None,
            retained: false,
        }
    }
}

impl Run {
    /// Builds the queue and position generator. This runs before the start gate, so a failure here
    /// tells the other workers to stop instead of waiting for them.
    fn prepare(p: Plan) -> std::result::Result<Self, Failure> {
        let reusable = !p.work.alternate
            && !p.work.unique
            && p.work.block == p.work.max_block
            && p.loaded.is_empty()
            && p.work.read_probability == 0.
            && p.work.block % 4096 == 0
            && p.lo % 4096 == 0;
        let mut queue = match &p.paths {
            Some(plan) => {
                let chap = plan.chap.as_ref().map(|(u, s)| (u.as_str(), s.as_slice()));
                let connection = open_paths(
                    &plan.paths,
                    IscsiFabric::new(&plan.initiator, chap),
                    NvmeFabric::new(crate::nvme::DEFAULT_HOSTNQN, plan.hostid),
                    plan.policy,
                    p.seed,
                )
                .map_err(|e| Failure::from(std::io::Error::other(e)))?;
                Queue::over_connection(connection, p.work.depth, p.max_len, p.timeout)?
            }
            None => Queue::new(
                p.file.ok_or_else(|| {
                    Failure::from(std::io::Error::other("file target without a file"))
                })?,
                p.backend,
                p.work.depth,
                p.max_len,
                p.timeout,
                p.work.submit_batch,
                p.slots,
            )?,
        };
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
        let limit = match (
            p.work.passes.and_then(|n| n.checked_mul(generated.len())),
            p.work.operation_limit,
        ) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        let byte_limit = p.work.range_passes.map(|n| n * (p.hi - p.lo));
        let duration = work_duration(&p.work, p.duration);
        // A flush is a barrier: every write before it must finish first. That is the only reason left for a
        // batch. Verify, alternate, rate and duty stream too: verification reads back each write as it
        // completes, and pacing waits inside the source.
        let batch_only = p.work.flush;
        let needs_stream = p.verify
            || p.work.verify
            || p.work.write_verify
            || p.work.alternate
            || p.work.rate.is_some()
            || p.work.duty.is_some();
        // On Linux loop devices, continuously refilling pure-direction io_uring queues creates
        // block-dispatch contention at high worker counts, so those stay in bounded batches.
        let platform_streams = (cfg!(target_os = "linux")
            && !p.record_latency
            && (p.backend == Backend::Libaio || p.backend == Backend::Uring))
            || cfg!(any(windows, target_os = "macos", target_os = "freebsd"));
        let (wv_lo, wv_hi) = (p.lo, p.hi);
        let depth = queue.requests.len();
        Ok(Self {
            wv_lo,
            wv_hi,
            wv_count: 0,
            wv_state: p.seed.max(1),
            wv_pending: None,
            flight: Vec::with_capacity(depth),
            owed: VecDeque::with_capacity(depth),
            held: Vec::with_capacity(depth),
            queue,
            generated,
            template,
            work: p.work,
            seed: p.seed,
            reusable,
            verify: p.verify,
            errors: p.errors,
            record_latency: p.record_latency,
            duration,
            limit,
            byte_limit,
            streamed: (platform_streams || needs_stream) && !batch_only,
            start: Instant::now(),
            progress: None,
            stats: Stats::default(),
            error: None,
            index: 0,
        })
    }

    /// Starts the measured run: the shared clock, progress reporting and latency storage.
    fn begin(&mut self, barrier: &StartGate, worker_index: usize) {
        self.start = *barrier.start.get_or_init(Instant::now);
        self.progress = barrier
            .progress
            .as_ref()
            .map(|state| (state.clone(), worker_index));
        if let Some(state) = &barrier.progress {
            state.start.get_or_init(|| self.start);
        }
        self.queue.observer = self.progress.clone();
        if self.record_latency {
            self.stats.latencies_us = Vec::with_capacity(LATENCY_CAPACITY);
        }
    }

    fn out_of_work(&self) -> bool {
        crate::signals::INTERRUPTED.load(Ordering::Relaxed)
            || self.limit.is_some_and(|n| self.index >= n)
            || self
                .byte_limit
                .is_some_and(|n| self.stats.read_bytes + self.stats.write_bytes >= n)
            || self
                .duration
                .is_some_and(|s| self.start.elapsed().as_secs_f64() >= s)
    }

    /// Keeps the queue full through the backend's own refill loop.
    /// Streams the run: the backend keeps the queue full, asking `source` for each free slot and reporting
    /// each completion to `finish`. Writes that verify are read back as soon as they complete.
    fn stream(&mut self) {
        let read_back = self.verify || self.work.verify || self.work.write_verify;
        // `V` lets a read-back overtake its write only across more than one endpoint; on one it waits, as `v` does.
        let race = read_back && self.work.write_verify_race && self.queue.endpoints() > 1;
        self.queue.set_race(race);
        let state = RefCell::new(Stream {
            generated: &mut self.generated,
            stats: &mut self.stats,
            work: &self.work,
            template: &self.template,
            seed: self.seed,
            read_back,
            counted_read_backs: self.work.verify || self.work.write_verify,
            race,
            duration: self.duration,
            start: self.start,
            progress: &self.progress,
            limit: self.limit,
            byte_limit: self.byte_limit,
            lo: self.wv_lo,
            hi: self.wv_hi,
            index: self.index,
            wv_count: self.wv_count,
            wv_state: self.wv_state,
            wv_ops: 0,
            queued_bytes: 0,
            outstanding: 0,
            in_flight: std::mem::take(&mut self.flight),
            due: std::mem::take(&mut self.owed),
            held: std::mem::take(&mut self.held),
            base: self.queue.arena_base(),
            stride: self.queue.stride(),
        });
        let result = self.queue.execute_stream(
            || state.borrow_mut().source(),
            |request, buffer| {
                if request.write {
                    write_pattern(
                        buffer,
                        request,
                        &self.work,
                        self.seed,
                        self.reusable,
                        &self.template,
                    );
                }
            },
            |slot, request| state.borrow_mut().finish(slot, request),
            self.errors,
        );
        let stream = state.into_inner();
        self.flight = stream.in_flight;
        self.owed = stream.due;
        self.held = stream.held;
        self.index = stream.index;
        self.wv_count = stream.wv_count;
        self.wv_state = stream.wv_state;
        if let Err(e) = result {
            self.error = Some(e);
        }
        if let Some(port) = self.queue.take_stream_stats() {
            self.stats.inflight_max = port.max_in_flight;
            self.stats.inflight_area_ns = port.area_ns;
            self.stats.inflight_time_ns = port.time_ns;
            if self.record_latency {
                self.stats.latencies_us.extend(port.latencies_us);
            }
        }
    }

    /// Submits one batch at a time and waits for it, honouring rate and duty pacing.
    fn batches(&mut self) {
        loop {
            if self.out_of_work() {
                break;
            }
            let elapsed = self.start.elapsed().as_secs_f64();
            let wait = self.pacing_wait(elapsed);
            if wait > 0. {
                let wait = self
                    .duration
                    .map_or(wait, |d| wait.min((d - elapsed).max(0.)));
                std::thread::sleep(std::time::Duration::from_secs_f64(wait.min(60.)));
                continue;
            }
            let effective_depth = if self.work.rate.is_some() {
                1
            } else {
                self.work.depth
            };
            let batch_limit = self.limit.map_or(effective_depth, |n| {
                (n - self.index).min(effective_depth as u64) as usize
            });
            let batch_limit = batch_limit.min(self.generated.phase_remaining(self.index) as usize);
            let count = self.fill_batch(batch_limit);
            let batch_started = Instant::now();
            let result = self.queue.execute(count, self.errors, |i, r| {
                // Write-verify writes its own offsets (fill_write_verify), so the position plan is not
                // written and must not be checked back.
                if r.write && self.work.verify && !self.work.write_verify {
                    self.generated.written(self.index + i as u64);
                }
                let measured = self
                    .duration
                    .is_none_or(|seconds| self.start.elapsed().as_secs_f64() < seconds);
                if measured
                    && self.record_latency
                    && self.stats.latencies_us.len() < self.stats.latencies_us.capacity()
                {
                    self.stats
                        .latencies_us
                        .push(batch_started.elapsed().as_micros() as u32);
                }
                self.stats.complete(r, measured);
                self.stats.publish(counter(&self.progress));
            });
            if let Err(e) = result {
                self.error = Some(e);
                break;
            }
            self.stats.publish(counter(&self.progress));
            if self.work.flush && self.queue.requests[..count].iter().any(|r| r.write) {
                if let Err(e) = self.queue.flush() {
                    self.error = Some(e.into());
                    break;
                }
            }
            self.verify_reads(count);
            if self.error.is_some() {
                break;
            }
            self.verify_compacted_writes(count);
            if self.error.is_some() {
                break;
            }
            self.index += count as u64;
        }
    }

    /// Seconds to sleep before the next batch so rate and duty limits are respected.
    fn pacing_wait(&self, elapsed: f64) -> f64 {
        pacing_wait(&self.work, self.index, elapsed)
    }

    /// Fills the first `limit` queue slots with the next requests and their payloads. Returns how many.
    /// `v` workloads: fills the batch with write, then the read of that block, alternating. Each read
    /// names the same offset as the write before it, and a batch deeper than one slot can overlap the two.
    fn fill_write_verify(&mut self, limit: usize) -> usize {
        let block = self.work.block as u64;
        let blocks = ((self.wv_hi - self.wv_lo) / block).max(1);
        let mut count = 0;
        while count < limit {
            if let Some(read) = self.wv_pending.take() {
                self.queue.requests[count] = read;
            } else {
                let index =
                    write_verify_index(&self.work, blocks, &mut self.wv_count, &mut self.wv_state);
                let request = Request {
                    offset: self.wv_lo + index * block,
                    len: self.work.block,
                    write: true,
                    verify: false,
                    seed: 0,
                    state: 0,
                };
                self.queue.requests[count] = request;
                write_pattern(
                    self.queue.buffer(count),
                    request,
                    &self.work,
                    self.seed,
                    self.reusable,
                    &self.template,
                );
                self.wv_pending = Some(Request {
                    write: false,
                    verify: true,
                    ..request
                });
            }
            count += 1;
        }
        count
    }

    fn fill_batch(&mut self, limit: usize) -> usize {
        if self.work.write_verify {
            return self.fill_write_verify(limit);
        }
        let mut count = 0;
        let mut batch_bytes = 0u64;
        for i in 0..limit {
            if self
                .byte_limit
                .is_some_and(|n| self.stats.read_bytes + self.stats.write_bytes + batch_bytes >= n)
            {
                break;
            }
            let request = self.generated.get(self.index + i as u64);
            count += 1;
            batch_bytes += request.len as u64;
            self.queue.requests[i] = request;
            if request.write {
                write_pattern(
                    self.queue.buffer(i),
                    request,
                    &self.work,
                    self.seed,
                    self.reusable,
                    &self.template,
                );
            }
        }
        count
    }

    /// Counts a block whose contents are wrong.
    fn record_incorrect(&mut self, offset: u64) {
        count_incorrect(&mut self.stats, offset);
    }

    fn verify_reads(&mut self, count: usize) {
        for i in 0..count {
            let request = self.queue.requests[i];
            if request.verify && !request.write {
                if !check_pattern(
                    self.queue.buffer(i),
                    request,
                    &self.work,
                    self.seed,
                    &self.template,
                ) {
                    self.record_incorrect(request.offset);
                }
                if !self.work.write_verify {
                    self.generated.checked(self.index + i as u64);
                }
                self.stats.verified += 1;
            }
        }
    }

    /// With `--verify` on a write-only workload, reads back the batch's writes using the same pool.
    fn verify_compacted_writes(&mut self, count: usize) {
        if !self.verify || self.work.verify {
            return;
        }
        // Reuse the pool for verification, compacting only writes into its front.
        let mut n = 0;
        for i in 0..count {
            let r = self.queue.requests[i];
            if r.write {
                self.queue.requests[n] = Request { write: false, ..r };
                n += 1;
            }
        }
        if n == 0 {
            return;
        }
        if let Err(e) = self.queue.execute(n, self.errors, |_, _| {}) {
            self.error = Some(e);
            return;
        }
        for i in 0..n {
            let r = self.queue.requests[i];
            if !check_pattern(
                self.queue.buffer(i),
                r,
                &self.work,
                self.seed,
                &self.template,
            ) {
                self.record_incorrect(r.offset);
            }
            self.stats.verified += 1;
        }
    }

    /// Reads back the writes whose metadata check was still pending when the limit ended the run.
    fn verify_pending(&mut self) {
        if self.work.write_verify {
            return;
        }
        let mut cursor = 0;
        loop {
            let mut count = 0;
            while count < self.work.depth {
                let Some(request) = self.generated.take_pending_check(&mut cursor) else {
                    break;
                };
                self.queue.requests[count] = request;
                count += 1;
            }
            if count == 0 {
                break;
            }
            if let Err(e) = self.queue.execute(count, self.errors, |_, _| {}) {
                self.error = Some(e);
                return;
            }
            for i in 0..count {
                let request = self.queue.requests[i];
                if !check_pattern(
                    self.queue.buffer(i),
                    request,
                    &self.work,
                    self.seed,
                    &self.template,
                ) {
                    self.record_incorrect(request.offset);
                }
                self.stats.verified += 1;
            }
        }
    }

    /// Closes the queue and returns what was measured. `guard` covers everything after the start
    /// gate, so the allocation count includes the pending checks.
    fn finish(mut self, guard: allocation::Guard) -> Outcome {
        let elapsed = self.start.elapsed().as_secs_f64();
        self.stats.seconds = self
            .duration
            .map_or(elapsed, |seconds| elapsed.min(seconds));
        // A time/byte/operation limit may end between a write and its scheduled
        // metadata read. Complete those checks outside the workload counters.
        if self.work.verify && self.error.is_none() {
            self.verify_pending();
        }
        self.stats.publish(counter(&self.progress));
        self.stats.allocations = guard.count();
        self.stats.submit_errors = self.queue.submit_errors;
        drop(guard);
        if let Some(counter) = counter(&self.progress) {
            counter.finish();
        }
        let retained = !self.queue.close();
        Outcome {
            stats: self.stats,
            error: self.error,
            retained,
        }
    }
}
