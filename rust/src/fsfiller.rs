//! `--fsfiller`: fills a directory tree the way a file server does. Files sit in a top-level and a
//! secondary directory. Each file type has its own size distribution, so the sizes form one peak per type.
//! The run ends when the file system
//! is full, the -G cap or -t limit is reached, or Ctrl-C; the report is printed in every case.
mod journal;

use crate::{
    cctv::files::Aligned, cctv::files::ALIGN, checked_close, pattern, rate, signals, Result,
};
use journal::Journal;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering::Relaxed},
        Mutex,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Debug)]
pub struct Settings {
    pub root: PathBuf,
    pub top: usize,
    pub second: usize,
    pub cap: Option<u64>,
    pub duration: Option<f64>,
    pub ratelimit_mbps: Option<f64>,
    pub threads: usize,
    /// Retry failed writes with exponential back-off instead of stopping.
    pub cant_stop: bool,
    /// Read each file back after writing it and check the contents.
    pub verify: bool,
    /// Flush each file to the device before it is closed.
    pub sync_on_close: bool,
    /// How each write reaches the device.
    pub write_mode: WriteMode,
}

/// How each file's writes reach the device.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WriteMode {
    /// Ordinary buffered writes; the file system writes them back later.
    Buffered,
    /// Each write reaches the device before it returns.
    WriteThrough,
    /// Bypasses the file system cache, like O_DIRECT.
    Direct,
}

impl WriteMode {
    pub fn name(self) -> &'static str {
        match self {
            WriteMode::Buffered => "buffered",
            WriteMode::WriteThrough => "writethrough",
            WriteMode::Direct => "direct",
        }
    }
}

/// One colour per file type, in the order of TYPES, so each suffix keeps its colour in the size chart.
const COLOURS: [&str; 5] = ["#1b9e77", "#d95f02", "#7570b3", "#e7298a", "#66a61e"];

/// File types as (extension, mean KiB, s.d. KiB). Each type gets an equal share of the files.
const TYPES: [(&str, f64, f64); 5] = [
    ("txt", 4., 1.),
    ("docx", 48., 12.),
    ("jpg", 256., 64.),
    ("bin", 1024., 256.),
    ("mp4", 8192., 2048.),
];
/// The size histogram has 300 bins spaced evenly on a log scale from 100 bytes to 64 MiB, each about 4.6%
/// wider than the last. Linear bins would put the 4 KiB type in one or two bins.
const BINS: usize = 300;
const LOW_BYTES: f64 = 100.;
const HIGH_BYTES: f64 = 64. * 1048576.;

/// The bin a size falls in; sizes outside the range go to the first or last bin.
fn bin_of(bytes: u64) -> usize {
    let position = (bytes.max(1) as f64 / LOW_BYTES).ln() / (HIGH_BYTES / LOW_BYTES).ln();
    ((position * BINS as f64).floor().max(0.) as usize).min(BINS - 1)
}

/// The smallest size in bin `bin`.
fn bin_edge(bin: usize) -> f64 {
    LOW_BYTES * (HIGH_BYTES / LOW_BYTES).powf(bin as f64 / BINS as f64)
}

/// A uniform value in (0, 1] from a 64-bit hash (splitmix64 finaliser).
fn unit(x: u64) -> f64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    ((z >> 11) + 1) as f64 / (1u64 << 53) as f64
}

/// The file's type and size, both fixed by its index: the type cycles, the size is a normal draw.
fn file_kind_and_size(index: usize) -> (usize, u64) {
    let kind = index % TYPES.len();
    let (_, mean, sd) = TYPES[kind];
    let (u1, u2) = (
        unit(index as u64),
        unit(index as u64 ^ 0xD1B5_4A32_D192_ED03),
    );
    let normal = (-2. * u1.ln()).sqrt() * (2. * std::f64::consts::PI * u2).cos();
    let kib = (mean + sd * normal).max(1.);
    (kind, (kib * 1024.).round() as u64)
}

/// Sizes written so far, per file type: count, sum and sum of squares (for mean and s.d.), and a histogram.
struct Sizes {
    count: [u64; TYPES.len()],
    sum: [f64; TYPES.len()],
    sum2: [f64; TYPES.len()],
    hist: Vec<[u64; TYPES.len()]>,
}

impl Sizes {
    fn new() -> Self {
        Self {
            count: [0; TYPES.len()],
            sum: [0.; TYPES.len()],
            sum2: [0.; TYPES.len()],
            hist: vec![[0; TYPES.len()]; BINS],
        }
    }

    fn add(&mut self, kind: usize, bytes: u64) {
        let kib = bytes as f64 / 1024.;
        self.count[kind] += 1;
        self.sum[kind] += kib;
        self.sum2[kind] += kib * kib;
        self.hist[bin_of(bytes)][kind] += 1;
    }

    /// Mean and population s.d. of the type's sizes in KiB.
    fn mean_sd(&self, kind: usize) -> (f64, f64) {
        let n = self.count[kind].max(1) as f64;
        let mean = self.sum[kind] / n;
        (mean, (self.sum2[kind] / n - mean * mean).max(0.).sqrt())
    }
}
const CHUNK: usize = 1 << 20;

/// First and longest wait between retries under --cantstopthis; the wait doubles each time.
const RETRY_MIN: Duration = Duration::from_millis(100);
const RETRY_MAX: Duration = Duration::from_secs(600);

#[derive(Clone, Copy, Debug, PartialEq)]
enum Stop {
    Full,
    Cap,
    Time,
    Interrupted,
}

impl Stop {
    fn describe(self) -> &'static str {
        match self {
            Self::Full => "file system full",
            Self::Cap => "-G cap reached",
            Self::Time => "-t time limit reached",
            Self::Interrupted => "Ctrl-C",
        }
    }
}

struct Shared {
    start: Instant,
    /// Bytes per second across all workers; None runs at full speed.
    rate: Option<f64>,
    done: AtomicBool,
    /// Set when no new file may start, such as at the -G cap. Files already started still finish.
    no_new: AtomicBool,
    reason: Mutex<Option<Stop>>,
    error: Mutex<Option<String>>,
    next: AtomicUsize,
    reserved: AtomicU64,
    /// Bytes written by every chunk, including a partial file that a stop removes. This is the I/O
    /// the report and chart show.
    written: AtomicU64,
    /// Bytes booked against the rate limit so far. Each chunk reserves its bytes here before sleeping,
    /// so workers queue up in order instead of all passing the same check.
    paced: AtomicU64,
    files: AtomicU64,
    retries: AtomicU64,
    verified: AtomicU64,
    /// When the first write of the current outage failed; None while writes succeed.
    outage: Mutex<Option<Instant>>,
    journal: Journal,
    sync_on_close: bool,
    write_mode: WriteMode,
    sizes: Mutex<Sizes>,
}

impl Shared {
    fn new(s: &Settings, journal: Journal) -> Self {
        Self {
            start: Instant::now(),
            journal,
            sync_on_close: s.sync_on_close,
            write_mode: s.write_mode,
            rate: s.ratelimit_mbps.map(rate::bytes_per_second),
            done: AtomicBool::new(false),
            no_new: AtomicBool::new(false),
            reason: Mutex::new(None),
            error: Mutex::new(None),
            next: AtomicUsize::new(0),
            reserved: AtomicU64::new(0),
            written: AtomicU64::new(0),
            paced: AtomicU64::new(0),
            files: AtomicU64::new(0),
            retries: AtomicU64::new(0),
            verified: AtomicU64::new(0),
            outage: Mutex::new(None),
            sizes: Mutex::new(Sizes::new()),
        }
    }

    /// Records why the run stopped. The first reason wins; later ones are ignored.
    /// Stops new files from starting, keeping the reason. Files already started finish, so a cap
    /// stop leaves no partial files behind.
    fn stop_new(&self, stop: Stop) {
        let mut reason = self.reason.lock().unwrap();
        if reason.is_none() {
            *reason = Some(stop);
        }
        self.no_new.store(true, Relaxed);
    }

    fn finish(&self, stop: Stop) {
        let mut reason = self.reason.lock().unwrap();
        if reason.is_none() {
            *reason = Some(stop);
        }
        self.done.store(true, Relaxed);
    }

    fn fail(&self, error: String) {
        *self.error.lock().unwrap() = Some(error);
        self.done.store(true, Relaxed);
    }
}

/// Directories are `t000/s000`, one level under the root for each of the two levels.
fn file_path(s: &Settings, index: usize) -> PathBuf {
    s.root
        .join(format!("t{:03}", index % s.top))
        .join(format!("s{:03}", (index / s.top) % s.second))
        .join(format!("f{index:08}.{}", TYPES[index % TYPES.len()].0))
}

fn create_tree(s: &Settings) -> io::Result<()> {
    fs::create_dir_all(&s.root)?;
    for t in 0..s.top {
        let top = s.root.join(format!("t{t:03}"));
        fs::create_dir_all(&top)?;
        for second in 0..s.second {
            fs::create_dir_all(top.join(format!("s{second:03}")))?;
        }
    }
    Ok(())
}

fn is_full(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::StorageFull || matches!(e.raw_os_error(), Some(28 | 39 | 112))
}

/// Creates the file, truncating any old one. The write mode sets how each write reaches the device.
fn create(path: &Path, mode: WriteMode) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    match mode {
        WriteMode::Buffered => {}
        WriteMode::WriteThrough => {
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt;
                options.custom_flags(0x8000_0000); // FILE_FLAG_WRITE_THROUGH
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.custom_flags(libc::O_SYNC);
            }
        }
        WriteMode::Direct => {
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt;
                options.custom_flags(0x2000_0000); // FILE_FLAG_NO_BUFFERING
            }
            #[cfg(any(target_os = "linux", target_os = "freebsd"))]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.custom_flags(libc::O_DIRECT);
            }
        }
    }
    options.open(path)
}

/// Writes one file. Returns false when the run stopped first, after removing the partial file.
/// Seed for the file at `path`, from its path bytes (FNV-1a), so a verify regenerates the same bytes.
fn seed(path: &Path) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in path.to_string_lossy().as_bytes() {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

fn write_file(path: &Path, size: u64, shared: &Shared, data: &mut [u8]) -> io::Result<bool> {
    let mut file = create(path, shared.write_mode)?;
    let seed = seed(path);
    let mut left = size;
    while left > 0 {
        if shared.done.load(Relaxed) {
            drop(file);
            let _ = fs::remove_file(path);
            return Ok(false);
        }
        let n = left.min(CHUNK as u64) as usize;
        if let Some(rate) = shared.rate {
            pace(shared, rate, n as u64);
        }
        // Direct writes must be whole aligned blocks; the padding is cut off by set_len below.
        let padded = if shared.write_mode == WriteMode::Direct {
            n.next_multiple_of(ALIGN)
        } else {
            n
        };
        pattern::fill(&mut data[..padded], size - left, seed);
        if let Err(e) = file.write_all(&data[..padded]) {
            drop(file);
            let _ = fs::remove_file(path);
            return Err(e);
        }
        left -= n as u64;
        shared.written.fetch_add(n as u64, Relaxed);
    }
    if shared.write_mode == WriteMode::Direct {
        if let Err(e) = file.set_len(size) {
            drop(file);
            let _ = fs::remove_file(path);
            return Err(e);
        }
    }
    if shared.sync_on_close {
        if let Err(e) = file.sync_all() {
            drop(file);
            let _ = fs::remove_file(path);
            return Err(e);
        }
    }
    if let Err(e) = checked_close::close(file) {
        let _ = fs::remove_file(path);
        return Err(e);
    }
    Ok(true)
}

/// Reads a written file back and checks its size and contents against the pattern for its path.
fn verify_file(path: &Path, size: u64, buf: &mut [u8]) -> io::Result<()> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    if len != size {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("size {len}, expected {size}"),
        ));
    }
    let seed = seed(path);
    let mut left = size;
    while left > 0 {
        let n = left.min(buf.len() as u64) as usize;
        file.read_exact(&mut buf[..n])?;
        if !pattern::matches(&buf[..n], size - left, seed) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "contents differ from what was written",
            ));
        }
        left -= n as u64;
    }
    Ok(())
}

/// Re-verifies journaled files and logs how many are OK, bad or missing.
fn verify_entries(label: &str, entries: &[(PathBuf, u64)]) {
    if entries.is_empty() {
        return;
    }
    let mut buf = vec![0u8; CHUNK];
    let (mut ok, mut missing) = (0u64, 0u64);
    let mut bad = Vec::new();
    for (path, size) in entries {
        match verify_file(path, *size, &mut buf) {
            Ok(()) => ok += 1,
            Err(e) if e.kind() == io::ErrorKind::NotFound => missing += 1,
            Err(e) => bad.push(format!("{}: {e}", path.display())),
        }
    }
    eprintln!(
        "verified {} {label} files: ok={ok} bad={} missing={missing}",
        entries.len(),
        bad.len()
    );
    for line in bad.iter().take(10) {
        eprintln!("*warning* bad {line}");
    }
}

/// Logs a failed write and, for the first failure of an outage, that the device has disappeared.
fn failed(shared: &Shared, path: &Path, e: &io::Error, attempt: u64, delay: Duration) {
    if is_full(e) {
        eprintln!(
            "Waiting for more space: {} is full; retry {attempt} in {:.1} s",
            path.display(),
            delay.as_secs_f64()
        );
        return;
    }
    let mut outage = shared.outage.lock().unwrap();
    if outage.is_none() {
        *outage = Some(Instant::now());
        eprintln!("device disappeared");
    }
    eprintln!(
        "write failed {}: {e}; retry {attempt} in {:.1} s",
        path.display(),
        delay.as_secs_f64()
    );
}

/// Logs the end of an outage when a write succeeds again, and the retries that the file needed.
fn recovered(shared: &Shared, path: &Path, attempt: u64) {
    if let Some(start) = shared.outage.lock().unwrap().take() {
        eprintln!(
            "device reappeared after {:.1} s",
            start.elapsed().as_secs_f64()
        );
        verify_entries("previous", &shared.journal.recent());
    }
    eprintln!("retried {attempt} times, continuing {}", path.display());
}

/// Waits for `delay`, checking for a stop every sample. Returns false when the run stopped.
fn pause(shared: &Shared, delay: Duration) -> bool {
    let end = Instant::now() + delay;
    loop {
        if shared.done.load(Relaxed) {
            return false;
        }
        let now = Instant::now();
        if now >= end {
            return true;
        }
        thread::sleep(SAMPLE.min(end - now));
    }
}

/// Sleeps until the bytes already written, plus this chunk, fit under the target rate.
fn pace(shared: &Shared, rate: f64, n: u64) {
    let sent = shared.paced.fetch_add(n, Relaxed) + n;
    let due = sent as f64 / rate;
    let now = shared.start.elapsed().as_secs_f64();
    if due > now {
        thread::sleep(Duration::from_secs_f64(due - now));
    }
}

fn worker(s: &Settings, shared: &Shared) {
    let mut data = Aligned::new(CHUNK);
    while !shared.done.load(Relaxed) && !shared.no_new.load(Relaxed) {
        let index = shared.next.fetch_add(1, Relaxed);
        let (kind, size) = file_kind_and_size(index);
        if let Some(cap) = s.cap {
            if shared.reserved.fetch_add(size, Relaxed) + size > cap {
                shared.stop_new(Stop::Cap);
                return;
            }
        }
        let path = file_path(s, index);
        if s.verify {
            eprintln!("writing {} ({size} bytes)", path.display());
        }
        let mut delay = RETRY_MIN;
        let mut attempt = 0;
        loop {
            let result = write_file(&path, size, shared, &mut data).and_then(|written| {
                if written && s.verify {
                    verify_file(&path, size, &mut data)?;
                    shared.verified.fetch_add(1, Relaxed);
                }
                if written {
                    shared.journal.record(&path, size)?;
                }
                Ok(written)
            });
            match result {
                Ok(true) => {
                    shared.files.fetch_add(1, Relaxed);
                    shared.sizes.lock().unwrap().add(kind, size);
                    if attempt > 0 {
                        recovered(shared, &path, attempt);
                    }
                    break;
                }
                Ok(false) => return,
                Err(e) if is_full(&e) && !s.cant_stop => {
                    shared.finish(Stop::Full);
                    return;
                }
                Err(e) if !s.cant_stop => {
                    shared.fail(format!("{}: {e}", path.display()));
                    return;
                }
                Err(e) => {
                    attempt += 1;
                    shared.retries.fetch_add(1, Relaxed);
                    failed(shared, &path, &e, attempt, delay);
                    if !pause(shared, delay) {
                        return;
                    }
                    delay = (delay * 2).min(RETRY_MAX);
                }
            }
        }
    }
}

/// Prints one progress line a second and stops the run on Ctrl-C or the -t limit.
/// One 100 ms sample: when it ended, and the MiB written and files completed during it.
struct Sample {
    at: f64,
    mib: f64,
    files: f64,
}

const SAMPLE: Duration = Duration::from_millis(100);

fn monitor(
    s: &Settings,
    shared: &Shared,
    handles: &[thread::ScopedJoinHandle<'_, ()>],
) -> Vec<Sample> {
    let mut samples = Vec::new();
    let (mut last_written, mut last_files) = (0u64, 0u64);
    let mut printed = Instant::now();
    while !handles.iter().all(|h| h.is_finished()) {
        thread::sleep(SAMPLE);
        if signals::INTERRUPTED.load(Relaxed) {
            shared.finish(Stop::Interrupted);
        }
        let elapsed = shared.start.elapsed().as_secs_f64();
        if s.duration.is_some_and(|d| elapsed >= d) {
            shared.finish(Stop::Time);
        }
        let written = shared.written.load(Relaxed);
        let files = shared.files.load(Relaxed);
        samples.push(Sample {
            at: elapsed,
            mib: (written - last_written) as f64 / 1048576.,
            files: (files - last_files) as f64,
        });
        (last_written, last_files) = (written, files);
        if printed.elapsed() >= Duration::from_secs(1) {
            printed = Instant::now();
            let files = shared.files.load(Relaxed);
            let mib = shared.written.load(Relaxed) as f64 / 1048576.;
            eprintln!(
                "*info* [{elapsed:.1}s] files {files}, {mib:.0} MiB, {:.0} files/s, {:.0} MiB/s",
                files as f64 / elapsed,
                mib / elapsed
            );
        }
    }
    samples
}

/// Samples per moving-average window: 20 seconds of 100 ms samples.
const WINDOW: usize = 200;

/// One row: time, the log-space centre of MiB/s over the trailing 20 s, the +/-1, 2, 3 s.d. edges
/// (symmetric on a log axis), and the raw files/s of this sample.
struct Row {
    at: f64,
    v: [f64; 8],
}

/// Complete 1-second bins behind each window; a 20 s window averages 20 of them.
const BIN_WINDOW: usize = 20;

/// Mean and s.d. of the trailing 20 complete 1-second bins, for each 100 ms sample. The edges are
/// mean +/- k s.d., with the lower edge held at zero because a speed cannot be negative.
fn log_bands(samples: &[Sample]) -> Vec<Row> {
    let seconds = samples.last().map_or(0, |last| last.at as usize + 1);
    let mut mib = vec![0f64; seconds];
    for x in samples {
        mib[x.at as usize] += x.mib;
    }
    let scale = 1. / SAMPLE.as_secs_f64();
    samples
        .iter()
        .filter_map(|x| {
            let now = x.at as usize;
            if now == 0 {
                return None;
            }
            let window = &mib[now.saturating_sub(BIN_WINDOW)..now];
            let (centre, sd) = mean_sd(window.iter().copied());
            let edge = |k: f64, sign: f64| (centre + sign * k * sd).max(0.);
            Some(Row {
                at: x.at,
                v: [
                    centre,
                    edge(1., -1.),
                    edge(1., 1.),
                    edge(2., -1.),
                    edge(2., 1.),
                    edge(3., -1.),
                    edge(3., 1.),
                    x.files * scale,
                ],
            })
        })
        .collect()
}

fn mean_sd(values: impl Iterator<Item = f64> + Clone) -> (f64, f64) {
    let n = values.clone().count() as f64;
    let mean = values.clone().sum::<f64>() / n;
    let var = values.map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    (mean, var.sqrt())
}

const DATA: &str = "fsfiller-over-time.dat";
const SCRIPT: &str = "fsfiller-over-time.gnu";
const IMAGE: &str = "fsfiller-over-time.png";
const SIZES_DATA: &str = "fsfiller-sizes.dat";
const SIZES_SCRIPT: &str = "fsfiller-sizes.gnu";
const SIZES_IMAGE: &str = "fsfiller-sizes.png";

/// Writes the smoothed rows and a gnuplot script with log axes, then renders the PNG when gnuplot is installed.
fn write_chart(s: &Settings, rows: &[Row]) -> io::Result<()> {
    // The bands need complete 1 s bins, so a run under 2 s (20 samples) has nothing to draw.
    if rows.len() < 20 {
        println!("chart: skipped, run shorter than 2 s (no complete 1 s bins)");
        return Ok(());
    }
    let mut dat = String::from("# time_s centre_MiB_s lo1 hi1 lo2 hi2 lo3 hi3 files_s\n");
    for row in rows {
        let v = row.v;
        dat += &format!(
            "{:.1} {:.3} {:.3} {:.3} {:.3} {:.3} {:.3} {:.3} {:.3}\n",
            row.at, v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7]
        );
    }
    fs::write(DATA, dat)?;
    let mode = match s.ratelimit_mbps {
        Some(mbit) => format!("target {mbit} Mbit/s"),
        None => "max speed".to_string(),
    };
    let target_line = match s.ratelimit_mbps {
        Some(mbit) => format!(
            ", {} with lines dashtype 2 lc rgb 'black' axes x1y1 title 'target {mbit} Mbit/s'",
            mbit / 8. / 1.048576
        ),
        None => String::new(),
    };
    let script = format!(
        "set terminal pngcairo size 6400,3600 font 'sans,40' background rgb 'white'\n\
         set output '{IMAGE}'\n\
         set title \"spit fsfiller, write MiB/s rolling 20 s mean and +/-1, 2, 3 SD\\n{mode} (spit {})\"\n\
         set xlabel 'time (s)'\n\
         set ylabel 'write MiB/s, 20 s rolling mean'\n\
         set y2label 'files/s, 0.1 s samples'\n\
         set ytics nomirror\n\
         set y2tics\n\
         set grid ytics\n\
         set key top right\n\
         plot '{DATA}' using 1:6:7 with filledcurves fillcolor rgb '#2f6fb4' fillstyle solid 0.10 noborder axes x1y1 title 'write MiB/s +/- 3 s.d.', \\\n\
              '{DATA}' using 1:4:5 with filledcurves fillcolor rgb '#2f6fb4' fillstyle solid 0.22 noborder axes x1y1 title '+/- 2 s.d.', \\\n\
              '{DATA}' using 1:2:3 with filledcurves fillcolor rgb '#2f6fb4' fillstyle solid 0.45 noborder axes x1y1 title '+/- 1 s.d.', \\\n\
              '' using 1:2 with lines lc rgb '#0b2a5b' lw 4 axes x1y1 title 'rolling mean', \\\n\
              '' using 1:9 with points pt 7 ps 1.5 lc rgb '#2ca02c' axes x1y2 title 'files/s per sample'{target_line}\n",
        env!("CARGO_PKG_VERSION")
    );
    fs::write(SCRIPT, script)?;
    match std::process::Command::new("gnuplot").arg(SCRIPT).status() {
        Ok(status) if status.success() => println!("chart: {IMAGE} (data {DATA}, script {SCRIPT})"),
        Ok(status) => eprintln!("*warning* gnuplot exited with {status}; chart not rendered"),
        Err(_) => {
            eprintln!("*warning* gnuplot not found; run `gnuplot {SCRIPT}` to render {IMAGE}")
        }
    }
    Ok(())
}

/// Distribution of the 20 s moving averages (one per sample once the window is full), with the
/// +/-1, 2 and 3 s.d. bands as percentages of the mean and the share of windows that land inside each.
fn distribution(samples: &[Sample]) -> Vec<String> {
    let scale = 1. / SAMPLE.as_secs_f64();
    if samples.len() < WINDOW {
        return vec!["distribution: run shorter than the 20 s window".to_string()];
    }
    let windows: Vec<(f64, f64)> = samples
        .windows(WINDOW)
        .map(|w| {
            let (mib, _) = mean_sd(w.iter().map(|x| x.mib * scale));
            let (files, _) = mean_sd(w.iter().map(|x| x.files * scale));
            (mib, files)
        })
        .collect();
    let mut lines = vec![format!(
        "distribution of {} 20 s moving averages (0.1 s samples):",
        windows.len()
    )];
    for (name, pick) in [("MiB_per_s", 0usize), ("files_per_s", 1)] {
        let mut values: Vec<f64> = windows
            .iter()
            .map(|w| if pick == 0 { w.0 } else { w.1 })
            .collect();
        values.sort_by(f64::total_cmp);
        let (mean, sd) = mean_sd(values.iter().copied());
        let at = |p: f64| values[((values.len() - 1) as f64 * p).round() as usize];
        lines.push(format!(
            "  {name}: median={:.1} p5={:.1} p95={:.1} mean={mean:.1} s.d.={sd:.1}",
            at(0.5),
            at(0.05),
            at(0.95)
        ));
        for k in [1., 2., 3.] {
            let inside = values
                .iter()
                .filter(|v| (**v - mean).abs() <= k * sd)
                .count();
            lines.push(format!(
                "    +/-{k} s.d. = +/-{:.0}% of mean, [{:.1}, {:.1}], {:.1}% of windows inside",
                k * sd / mean.max(f64::MIN_POSITIVE) * 100.,
                (mean - k * sd).max(0.),
                mean + k * sd,
                inside as f64 * 100. / values.len() as f64
            ));
        }
    }
    lines
}

fn report(s: &Settings, shared: &Shared, samples: &[Sample]) {
    let elapsed = shared.start.elapsed().as_secs_f64().max(1e-9);
    let stop = shared
        .reason
        .lock()
        .unwrap()
        .map_or("file system full or error", Stop::describe);
    let files = shared.files.load(Relaxed);
    let mib = shared.written.load(Relaxed) as f64 / 1048576.;
    println!("fsfiller: stopped: {stop}");
    println!(
        "sync_on_close={} write_mode={}",
        s.sync_on_close,
        s.write_mode.name()
    );
    println!(
        "directories={} files={files} MiB={mib:.1} elapsed={elapsed:.3} files_per_s={:.0} MiB_per_s={:.1}",
        s.top + s.top * s.second,
        files as f64 / elapsed,
        mib / elapsed
    );
    if s.verify {
        println!("verified={}", shared.verified.load(Relaxed));
    }
    if s.cant_stop {
        println!("retries={}", shared.retries.load(Relaxed));
    }
    let sizes = shared.sizes.lock().unwrap();
    for (kind, (ext, _, _)) in TYPES.iter().enumerate() {
        let (mean, sd) = sizes.mean_sd(kind);
        println!(
            "type {ext}: files={} mean={mean:.1} KiB sd={sd:.1} KiB",
            sizes.count[kind]
        );
    }
    for line in distribution(samples) {
        println!("{line}");
    }
}

/// Writes the per-type size histogram (quarter-octave bins) and renders it with gnuplot.
fn write_sizes(shared: &Shared) -> io::Result<()> {
    let sizes = shared.sizes.lock().unwrap();
    let mut dat = String::from("# size_bytes_bin_start");
    for (ext, _, _) in TYPES {
        dat += &format!(" {ext}");
    }
    dat += "\n";
    for (bin, counts) in sizes.hist.iter().enumerate() {
        dat += &format!("{:.1}", bin_edge(bin));
        for count in counts {
            dat += &format!(" {count}");
        }
        dat += "\n";
    }
    fs::write(SIZES_DATA, dat)?;
    let plots: Vec<String> = TYPES
        .iter()
        .enumerate()
        .map(|(kind, (ext, _, _))| {
            format!(
                "'{SIZES_DATA}' using 1:{} with boxes fc rgb '{}' title '{ext}'",
                kind + 2,
                COLOURS[kind]
            )
        })
        .collect();
    let script = format!(
        "set terminal pngcairo size 6400,3600 font 'sans,40' background rgb 'white'\n\
         set output '{SIZES_IMAGE}'\n\
         set title \"spit fsfiller, file size distribution by type (spit {})\"\n\
         set xlabel 'file size (bytes, 300 log-spaced bins)'\n\
         set ylabel 'files'\n\
         set logscale x 10\n\
         set boxwidth 0.9 relative\n\
         set style fill transparent solid 0.5 noborder\n\
         set key top right\n\
         set grid\n\
         plot {}\n",
        env!("CARGO_PKG_VERSION"),
        plots.join(", \\\n     ")
    );
    fs::write(SIZES_SCRIPT, script)?;
    match std::process::Command::new("gnuplot")
        .arg(SIZES_SCRIPT)
        .status()
    {
        Ok(status) if status.success() => println!("sizes: {SIZES_IMAGE} (data {SIZES_DATA})"),
        Ok(status) => eprintln!("*warning* gnuplot exited with {status}; size chart not rendered"),
        Err(_) => eprintln!("*warning* gnuplot not found; run `gnuplot {SIZES_SCRIPT}`"),
    }
    Ok(())
}

pub fn run(s: Settings) -> Result<()> {
    let _signals = signals::Guard::install().map_err(|e| e.to_string())?;
    create_tree(&s).map_err(|e| format!("{}: {e}", s.root.display()))?;
    let previous = journal::read_tail(&s.root).map_err(|e| format!("{}: {e}", journal::FILE))?;
    verify_entries("restart", &previous);
    let journal = Journal::open().map_err(|e| format!("{}: {e}", journal::FILE))?;
    let shared = Shared::new(&s, journal);
    let samples = thread::scope(|scope| {
        let handles: Vec<_> = (0..s.threads)
            .map(|_| scope.spawn(|| worker(&s, &shared)))
            .collect();
        monitor(&s, &shared, &handles)
    });
    report(&s, &shared, &samples);
    write_chart(&s, &log_bands(&samples)).map_err(|e| e.to_string())?;
    write_sizes(&shared).map_err(|e| e.to_string())?;
    let error = shared.error.lock().unwrap().take();
    match error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
