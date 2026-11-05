//! Per-path counts for a multi-path run, and the totals printed at the end. Every transport counts the
//! same way, so the report is one table whether the paths are iSCSI portals or NVMe-oF controllers.

use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

/// Counts for one path: operations, bytes, latency and failovers. Plain integers, updated per request.
#[derive(Debug, Default, Clone, Copy)]
pub struct PathStats {
    pub reads: u64,
    pub writes: u64,
    pub bytes: u64,
    pub nanos_total: u64,
    pub nanos_max: u64,
    pub failovers: u64,
    pub disconnects: u64,
    pub reconnects: u64,
    /// The most commands in flight on this path at once.
    pub inflight_max: u64,
    /// The path's ALUA tier when it was opened: 0 optimized, 1 non-optimized, 2 standby. None when the
    /// target does not report ALUA.
    pub alua: Option<u8>,
}

/// The name of an ALUA tier, as the totals report it.
fn alua_name(alua: Option<u8>) -> &'static str {
    match alua {
        None => "-",
        Some(0) => "optimized",
        Some(1) => "non-optimized",
        Some(_) => "standby",
    }
}

impl PathStats {
    pub fn record(&mut self, write: bool, bytes: u64, nanos: u64) {
        if write {
            self.writes += 1;
        } else {
            self.reads += 1;
        }
        self.bytes += bytes;
        self.nanos_total += nanos;
        self.nanos_max = self.nanos_max.max(nanos);
    }

    pub fn merge(&mut self, other: &PathStats) {
        self.reads += other.reads;
        self.writes += other.writes;
        self.bytes += other.bytes;
        self.nanos_total += other.nanos_total;
        self.nanos_max = self.nanos_max.max(other.nanos_max);
        self.failovers += other.failovers;
        self.disconnects += other.disconnects;
        self.reconnects += other.reconnects;
        self.inflight_max = self.inflight_max.max(other.inflight_max);
        self.alua = self.alua.or(other.alua);
    }

    /// True when the path carried no I/O and saw no failures.
    pub fn is_idle(&self) -> bool {
        self.reads + self.writes + self.failovers + self.disconnects == 0
    }
}

/// Totals for every pool in the run, keyed by path label. Each pool adds its counts when it closes,
/// which is outside the timed loop.
static TOTALS: std::sync::Mutex<Vec<(String, PathStats)>> = std::sync::Mutex::new(Vec::new());

/// Adds one path's counts to the run totals under its label.
pub fn record_totals(label: &str, stats: &PathStats) {
    let mut totals = TOTALS.lock().unwrap_or_else(|e| e.into_inner());
    match totals.iter_mut().find(|(l, _)| l == label) {
        Some((_, total)) => total.merge(stats),
        None => totals.push((label.to_string(), *stats)),
    }
}

/// Commands still to dump (`-d`). Shared by every pool in the run.
static DUMP_LEFT: AtomicU64 = AtomicU64::new(0);

/// Sets how many commands to dump (`-d N`).
pub fn set_dump(count: u64) {
    DUMP_LEFT.store(count, Relaxed);
}

/// Takes one dump slot if any remain.
pub fn take_dump() -> bool {
    DUMP_LEFT
        .fetch_update(Relaxed, Relaxed, |n| n.checked_sub(1))
        .is_ok()
}

/// Prints the per-path totals for the run: operations, bytes, latency and failovers. Prints nothing when
/// no path carried I/O.
pub fn print_totals() {
    let totals = TOTALS.lock().unwrap_or_else(|e| e.into_inner());
    if totals.is_empty() {
        return;
    }
    println!("per-path totals:");
    println!(
        "  {:<44}{:>10}{:>10}{:>10}{:>12}{:>12}{:>12}{:>11}{:>8}{:>12}{:>8}  ALUA",
        "path",
        "ops",
        "reads",
        "writes",
        "MiB",
        "avg ms",
        "max ms",
        "failovers",
        "drops",
        "reconnects",
        "max qd"
    );
    for (label, t) in totals.iter() {
        let ops = t.reads + t.writes;
        let avg = if ops == 0 {
            0.0
        } else {
            t.nanos_total as f64 / ops as f64 / 1e6
        };
        println!(
            "  {:<44}{:>10}{:>10}{:>10}{:>12.1}{:>12.3}{:>12.3}{:>11}{:>8}{:>12}{:>8}  {}",
            label,
            ops,
            t.reads,
            t.writes,
            t.bytes as f64 / 1_048_576.0,
            avg,
            t.nanos_max as f64 / 1e6,
            t.failovers,
            t.disconnects,
            t.reconnects,
            t.inflight_max,
            alua_name(t.alua)
        );
    }
}
