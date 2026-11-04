//! `--fsfiller`: fills a directory tree the way a file server does. Files sit in a top-level and a
//! secondary directory. Each file type has its own size distribution, so the sizes form one peak per type.
//! The run ends when the file system
//! is full, the -G cap or -t limit is reached, or Ctrl-C; the report is printed in every case.
mod chart;
mod journal;
mod stats;

use crate::{
    aligned::{AlignedBuf, DIRECT_ALIGN},
    checked_close, pattern, rate, signals, Result,
};
use chart::{write_chart, write_sizes};
use journal::Journal;
use stats::{distribution, log_bands, Sample, Sizes, SAMPLE};
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

/// File types as (extension, mean KiB, s.d. KiB). Each type gets an equal share of the files.
const TYPES: [(&str, f64, f64); 5] = [
    ("txt", 4., 1.),
    ("docx", 48., 12.),
    ("jpg", 256., 64.),
    ("bin", 1024., 256.),
    ("mp4", 8192., 2048.),
];
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

const CHUNK: usize = 1 << 20;

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

    /// Stops new files from starting, keeping the reason. Files already started finish, so a cap
    /// stop leaves no partial files behind.
    fn stop_new(&self, stop: Stop) {
        let mut reason = self.reason.lock().unwrap();
        if reason.is_none() {
            *reason = Some(stop);
        }
        self.no_new.store(true, Relaxed);
    }

    /// Records why the run stopped. The first reason wins; later ones are ignored.
    fn finish(&self, stop: Stop) {
        let mut reason = self.reason.lock().unwrap();
        if reason.is_none() {
            *reason = Some(stop);
        }
        self.done.store(true, Relaxed);
    }

    /// Records the first error that stopped the run; later ones are ignored, as with the stop reason.
    fn fail(&self, error: String) {
        let mut slot = self.error.lock().unwrap();
        if slot.is_none() {
            *slot = Some(error);
        }
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
    if e.kind() == io::ErrorKind::StorageFull {
        return true;
    }
    match e.raw_os_error() {
        #[cfg(unix)]
        Some(code) => code == libc::ENOSPC || code == libc::EDQUOT,
        // ERROR_HANDLE_DISK_FULL and ERROR_DISK_FULL.
        #[cfg(windows)]
        Some(code) => code == 39 || code == 112,
        _ => false,
    }
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

/// Seed for the file at `path`, from its path bytes (FNV-1a), so a verify regenerates the same bytes.
fn seed(path: &Path) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in path.to_string_lossy().as_bytes() {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// Writes one file. Returns false when the run stopped first. Any failure, or a stop, removes the
/// partial file.
fn write_file(path: &Path, size: u64, shared: &Shared, data: &mut [u8]) -> io::Result<bool> {
    let mut file = create(path, shared.write_mode)?;
    let outcome = write_contents(&mut file, size, seed(path), shared, data);
    let result = match outcome {
        Ok(true) => checked_close::close(file).map(|()| true),
        // Drop the handle before removing the file; Windows cannot remove an open file.
        other => {
            drop(file);
            other
        }
    };
    if !matches!(result, Ok(true)) {
        let _ = fs::remove_file(path);
    }
    result
}

/// Writes the pattern for `size` bytes into `file`. Returns false when the run stopped first.
fn write_contents(
    file: &mut File,
    size: u64,
    seed: u64,
    shared: &Shared,
    data: &mut [u8],
) -> io::Result<bool> {
    let mut left = size;
    while left > 0 {
        if shared.done.load(Relaxed) {
            return Ok(false);
        }
        let n = left.min(CHUNK as u64) as usize;
        if let Some(rate) = shared.rate {
            pace(shared, rate, n as u64);
        }
        // Direct writes must be whole aligned blocks; the padding is cut off by set_len below.
        let padded = if shared.write_mode == WriteMode::Direct {
            n.next_multiple_of(DIRECT_ALIGN)
        } else {
            n
        };
        pattern::fill(&mut data[..padded], size - left, seed);
        file.write_all(&data[..padded])?;
        left -= n as u64;
        shared.written.fetch_add(n as u64, Relaxed);
    }
    if shared.write_mode == WriteMode::Direct {
        file.set_len(size)?;
    }
    if shared.sync_on_close {
        file.sync_all()?;
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
    // Take the start time in its own statement, so the lock is released before verification runs.
    let outage = shared.outage.lock().unwrap().take();
    if let Some(start) = outage {
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
    let mut data = AlignedBuf::new(CHUNK);
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

pub fn run(s: Settings) -> Result<()> {
    let _signals = signals::Guard::install().map_err(|e| e.to_string())?;
    create_tree(&s).map_err(|e| format!("{}: {e}", s.root.display()))?;
    let previous = journal::read_tail(&s.root).map_err(|e| format!("{}: {e}", journal::FILE))?;
    verify_entries("restart", &previous);
    let journal = Journal::open(&s.root).map_err(|e| format!("{}: {e}", journal::FILE))?;
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn full_disk_errors_are_recognised_but_not_other_errnos() {
        assert!(is_full(&io::Error::from_raw_os_error(libc::ENOSPC)));
        assert!(is_full(&io::Error::from_raw_os_error(libc::EDQUOT)));
        // 39 is ENOTEMPTY on Linux; it must not be taken for a full disk.
        assert!(!is_full(&io::Error::from_raw_os_error(libc::ENOTEMPTY)));
        assert!(!is_full(&io::Error::from_raw_os_error(libc::EIO)));
    }
}
