//! Choosing which target path each request uses when a workload runs over several paths (iSCSI or NVMe-oF).
//! Each request names one path. The policy decides the order: round robin, reverse round robin,
//! random, or fastest (the path with the lowest recent average latency).

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, AtomicUsize, Ordering::Relaxed};

/// A path's tier, from its ALUA state. Requests go to the lowest tier that has a path in rotation: optimized
/// paths first, then non-optimized ones only when no optimized path is usable. A standby, unavailable,
/// transitioning or offline path is in tier 2 and never takes I/O.
pub const TIER_OPTIMIZED: u8 = 0;
pub const TIER_NON_OPTIMIZED: u8 = 1;
pub const TIER_STANDBY: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    RoundRobin,
    ReverseRoundRobin,
    Random,
    Fastest,
}

impl Policy {
    pub fn parse(text: &str) -> Result<Self, String> {
        match text {
            "rr" | "round-robin" => Ok(Self::RoundRobin),
            "rrr" | "reverse-round-robin" => Ok(Self::ReverseRoundRobin),
            "random" => Ok(Self::Random),
            "fastest" => Ok(Self::Fastest),
            other => Err(format!(
                "--policy takes rr, rrr, random or fastest, not {other}"
            )),
        }
    }
}

/// The paths of one workload and the state each policy needs: a request counter, a random seed, and
/// the recent latency of each path in nanoseconds (0 until a path has been measured).
pub struct Lanes {
    paths: usize,
    policy: Policy,
    next: AtomicUsize,
    seed: AtomicU64,
    latency_ns: Vec<AtomicU64>,
    /// Set when a path reports that it cannot serve I/O (standby, unavailable or transitioning).
    unavailable: Vec<AtomicBool>,
    /// Each path's tier, one of the `TIER_` values.
    tier: Vec<AtomicU8>,
}

impl Lanes {
    pub fn new(paths: usize, policy: Policy, seed: u64) -> Result<Self, String> {
        if paths == 0 {
            return Err("a workload needs at least one path".into());
        }
        Ok(Self {
            paths,
            policy,
            next: AtomicUsize::new(0),
            seed: AtomicU64::new(seed.max(1)),
            latency_ns: (0..paths).map(|_| AtomicU64::new(0)).collect(),
            unavailable: (0..paths).map(|_| AtomicBool::new(false)).collect(),
            tier: (0..paths).map(|_| AtomicU8::new(TIER_OPTIMIZED)).collect(),
        })
    }

    /// The path for the next request.
    pub fn pick(&self) -> usize {
        match self.policy {
            Policy::RoundRobin => self.next.fetch_add(1, Relaxed) % self.paths,
            Policy::ReverseRoundRobin => {
                self.paths - 1 - self.next.fetch_add(1, Relaxed) % self.paths
            }
            Policy::Random => self.random() as usize % self.paths,
            Policy::Fastest => self.fastest(),
        }
    }

    /// The path for the next request, skipping any path that cannot serve I/O. None when every path is
    /// unavailable.
    pub fn pick_available(&self) -> Option<usize> {
        let start = self.pick();
        (0..self.paths)
            .map(|k| (start + k) % self.paths)
            .find(|&i| self.is_usable(i))
    }

    /// Sets a path's tier, from its ALUA state when it was opened or reconnected.
    pub fn set_tier(&self, path: usize, tier: u8) {
        self.tier[path].store(tier, Relaxed);
    }

    /// The lowest tier with a path in rotation, if that tier can take I/O at all.
    fn best_tier(&self) -> Option<u8> {
        (0..self.paths)
            .filter(|&i| !self.unavailable[i].load(Relaxed))
            .map(|i| self.tier[i].load(Relaxed))
            .min()
            .filter(|&tier| tier < TIER_STANDBY)
    }

    /// How many paths can take I/O at the best tier: the paths a request can be sent to.
    pub fn usable_count(&self) -> usize {
        let best = self.best_tier();
        (0..self.paths)
            .filter(|&i| {
                !self.unavailable[i].load(Relaxed) && Some(self.tier[i].load(Relaxed)) == best
            })
            .count()
    }

    /// Whether `path` may take a request now: it is in rotation and in the best tier.
    pub fn is_usable(&self, path: usize) -> bool {
        !self.unavailable[path].load(Relaxed)
            && Some(self.tier[path].load(Relaxed)) == self.best_tier()
    }

    /// Like `pick_available`, but never `avoid` unless no other path is available. Verify reads use it to
    /// read a block back through a different path from the one that wrote it.
    pub fn pick_available_avoiding(&self, avoid: usize) -> Option<usize> {
        // Reads do not advance the rotation, so writes still alternate across the paths.
        let start = match self.policy {
            Policy::RoundRobin => self.next.load(Relaxed) % self.paths,
            Policy::ReverseRoundRobin => self.paths - 1 - self.next.load(Relaxed) % self.paths,
            Policy::Random | Policy::Fastest => self.pick(),
        };
        (0..self.paths)
            .map(|k| (start + k) % self.paths)
            .find(|&i| i != avoid && self.is_usable(i))
            .or_else(|| self.pick_available())
    }

    /// Takes `path` out of rotation after the target reported it unavailable.
    /// Puts a path back in rotation after it reconnected. Its latency is measured again.
    pub fn mark_available(&self, path: usize) {
        self.unavailable[path].store(false, Relaxed);
        self.latency_ns[path].store(0, Relaxed);
    }

    pub fn mark_unavailable(&self, path: usize) {
        self.unavailable[path].store(true, Relaxed);
        self.latency_ns[path].store(u64::MAX, Relaxed);
    }

    /// Records how long a request on `path` took. The average moves 1/8 towards each new sample.
    pub fn record(&self, path: usize, nanos: u64) {
        let slot = &self.latency_ns[path];
        let old = slot.load(Relaxed);
        let new = if old == 0 {
            nanos
        } else {
            old - old / 8 + nanos / 8
        };
        slot.store(new.max(1), Relaxed);
    }

    /// The path with the lowest average latency. Paths not measured yet go first, in round-robin order,
    /// so each path gets a sample before any one is preferred.
    fn fastest(&self) -> usize {
        if let Some(unmeasured) = (0..self.paths).find(|&i| self.latency_ns[i].load(Relaxed) == 0) {
            return unmeasured;
        }
        (0..self.paths)
            .min_by_key(|&i| self.latency_ns[i].load(Relaxed))
            .unwrap_or(0)
    }

    /// xorshift64, one step per call, shared by every worker.
    fn random(&self) -> u64 {
        let mut x = self.seed.load(Relaxed);
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.seed.store(x, Relaxed);
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_policy_names() {
        assert_eq!(Policy::parse("rr").unwrap(), Policy::RoundRobin);
        assert_eq!(Policy::parse("rrr").unwrap(), Policy::ReverseRoundRobin);
        assert_eq!(Policy::parse("random").unwrap(), Policy::Random);
        assert_eq!(Policy::parse("fastest").unwrap(), Policy::Fastest);
        assert!(Policy::parse("sticky").is_err());
    }

    #[test]
    fn round_robin_cycles_through_every_path_in_order() {
        let lanes = Lanes::new(3, Policy::RoundRobin, 1).unwrap();
        let picks: Vec<usize> = (0..6).map(|_| lanes.pick()).collect();
        assert_eq!(picks, vec![0, 1, 2, 0, 1, 2]);
    }

    #[test]
    fn reverse_round_robin_cycles_backwards() {
        let lanes = Lanes::new(3, Policy::ReverseRoundRobin, 1).unwrap();
        let picks: Vec<usize> = (0..6).map(|_| lanes.pick()).collect();
        assert_eq!(picks, vec![2, 1, 0, 2, 1, 0]);
    }

    #[test]
    fn random_stays_in_range_and_reaches_every_path() {
        let lanes = Lanes::new(4, Policy::Random, 42).unwrap();
        let mut seen = [0u32; 4];
        for _ in 0..4000 {
            seen[lanes.pick()] += 1;
        }
        assert!(seen.iter().all(|&n| n > 800), "uneven spread {seen:?}");
    }

    #[test]
    fn fastest_measures_every_path_first_then_prefers_the_lowest_latency() {
        let lanes = Lanes::new(3, Policy::Fastest, 1).unwrap();
        assert_eq!(lanes.pick(), 0);
        lanes.record(0, 900);
        assert_eq!(lanes.pick(), 1);
        lanes.record(1, 100);
        assert_eq!(lanes.pick(), 2);
        lanes.record(2, 500);
        assert_eq!(lanes.pick(), 1);
        for _ in 0..50 {
            lanes.record(1, 10_000);
        }
        assert_eq!(lanes.pick(), 2);
    }

    #[test]
    fn unavailable_paths_are_skipped_until_none_is_left() {
        let lanes = Lanes::new(3, Policy::RoundRobin, 1).unwrap();
        lanes.mark_unavailable(1);
        let picks: Vec<Option<usize>> = (0..4).map(|_| lanes.pick_available()).collect();
        assert!(picks.iter().all(|p| *p != Some(1)), "{picks:?}");
        lanes.mark_unavailable(0);
        lanes.mark_unavailable(2);
        assert_eq!(lanes.pick_available(), None);
    }

    #[test]
    fn zero_paths_is_an_error() {
        assert!(Lanes::new(0, Policy::RoundRobin, 1).is_err());
    }

    /// Lanes for `tiers`, one per path, with nothing unavailable.
    fn with_tiers(tiers: &[u8]) -> Lanes {
        let lanes = Lanes::new(tiers.len(), Policy::RoundRobin, 1).unwrap();
        for (path, &tier) in tiers.iter().enumerate() {
            lanes.set_tier(path, tier);
        }
        lanes
    }

    /// Every request goes to an optimized path while one is usable; the other paths get none.
    #[test]
    fn optimized_paths_take_everything_before_non_optimized() {
        let lanes = with_tiers(&[TIER_NON_OPTIMIZED, TIER_OPTIMIZED, TIER_OPTIMIZED]);
        for _ in 0..20 {
            let path = lanes.pick_available().unwrap();
            assert_ne!(path, 0, "a non-optimized path took a request");
        }
        assert_eq!(lanes.usable_count(), 2);
    }

    /// Standby paths never take I/O, even when they are the only ones left.
    #[test]
    fn standby_paths_never_take_io() {
        let lanes = with_tiers(&[TIER_OPTIMIZED, TIER_STANDBY, TIER_STANDBY]);
        for _ in 0..20 {
            assert_eq!(lanes.pick_available(), Some(0));
        }
        assert!(!lanes.is_usable(1) && !lanes.is_usable(2));
        let only_standby = with_tiers(&[TIER_STANDBY, TIER_STANDBY]);
        assert_eq!(only_standby.pick_available(), None);
        assert_eq!(only_standby.usable_count(), 0);
    }

    /// Non-optimized paths take I/O only when no optimized path is usable.
    #[test]
    fn non_optimized_is_the_fallback() {
        let lanes = with_tiers(&[TIER_NON_OPTIMIZED, TIER_STANDBY]);
        assert_eq!(lanes.pick_available(), Some(0));
        assert!(lanes.is_usable(0));
        assert!(!lanes.is_usable(1));
    }

    /// An unavailable optimized path hands its requests to the next usable path in its tier, and a
    /// non-optimized path only once the optimized ones are gone.
    #[test]
    fn unavailable_optimized_path_falls_through() {
        let lanes = with_tiers(&[TIER_OPTIMIZED, TIER_OPTIMIZED, TIER_NON_OPTIMIZED]);
        lanes.mark_unavailable(0);
        assert_eq!(lanes.pick_available(), Some(1));
        lanes.mark_unavailable(1);
        assert_eq!(lanes.pick_available(), Some(2));
    }

    /// A verify read avoids the path that wrote, but never leaves the tier.
    #[test]
    fn avoiding_stays_in_the_tier() {
        let lanes = with_tiers(&[TIER_OPTIMIZED, TIER_STANDBY, TIER_OPTIMIZED]);
        for _ in 0..10 {
            let path = lanes.pick_available_avoiding(0).unwrap();
            assert_eq!(path, 2);
        }
    }
}
