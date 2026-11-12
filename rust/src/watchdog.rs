//! A progress watchdog for the long runs of `--compliance`. Each SCSI command beats it and prints a dot, so
//! a run shows it is alive. When nothing beats for the limit, the run stops with the error below rather
//! than waiting on a target that has stopped answering. Nothing is printed until `start` is called, so the
//! other commands' output is unchanged.

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The longest a run may go without a beat (seconds).
pub const LIMIT: Duration = Duration::from_secs(15);

static ENABLED: AtomicBool = AtomicBool::new(false);
/// Whether each command prints a dot (`--verbose`).
static VERBOSE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static LAST: Mutex<Option<Instant>> = Mutex::new(None);
static LAST_STEP: Mutex<String> = Mutex::new(String::new());

/// Whether the watchdog has been stalled since its last beat: no beat at all, or the last one more than
/// `limit` before `now`. A run that has not begun yet is not stalled.
pub fn stalled(last: Option<Instant>, limit: Duration, now: Instant) -> bool {
    last.is_some_and(|t| now.saturating_duration_since(t) > limit)
}

/// Records progress and prints a dot, once the watchdog is started. `step` names the command beginning,
/// so a stop can say what was in flight.
pub fn beat(step: &[u8]) {
    if !ENABLED.load(Relaxed) {
        return;
    }
    if let Ok(mut last) = LAST.lock() {
        *last = Some(Instant::now());
    }
    if let Ok(mut current) = LAST_STEP.lock() {
        *current = format!("{:02x?}", &step[..step.len().min(16)]);
    }
    if VERBOSE.load(Relaxed) {
        eprint!(".");
        let _ = std::io::stderr().flush();
    }
}

/// Starts watching: from now on each beat prints a dot, and the process exits if the run goes `LIMIT`
/// without one.
pub fn start(verbose: bool) {
    VERBOSE.store(verbose, Relaxed);
    ENABLED.store(true, Relaxed);
    beat(&[]);
    std::thread::spawn(|| loop {
        std::thread::sleep(Duration::from_secs(1));
        let last = LAST.lock().ok().and_then(|l| *l);
        if stalled(last, LIMIT, Instant::now()) {
            let step = LAST_STEP.lock().map(|s| s.clone()).unwrap_or_default();
            eprintln!(
                "\n*error* no progress for {} s: the run stopped; the last command started was CDB {step}",
                LIMIT.as_secs()
            );
            std::process::exit(2);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stalled_only_after_the_limit_with_a_beat() {
        let now = Instant::now();
        assert!(!stalled(None, LIMIT, now + Duration::from_secs(60)));
        assert!(!stalled(Some(now), LIMIT, now + Duration::from_secs(14)));
        assert!(stalled(Some(now), LIMIT, now + Duration::from_secs(16)));
    }
}
