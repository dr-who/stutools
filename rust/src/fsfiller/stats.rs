//! Statistics for a fsfiller run: size bins and per-type sizes, the 100 ms samples, and the
//! moving-average bands and distribution that the report prints.
use super::TYPES;
use std::time::Duration;

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
pub(super) fn bin_edge(bin: usize) -> f64 {
    LOW_BYTES * (HIGH_BYTES / LOW_BYTES).powf(bin as f64 / BINS as f64)
}

/// Sizes written so far, per file type: count, sum and sum of squares (for mean and s.d.), and a histogram.
pub(super) struct Sizes {
    pub(super) count: [u64; TYPES.len()],
    pub(super) sum: [f64; TYPES.len()],
    pub(super) sum2: [f64; TYPES.len()],
    pub(super) hist: Vec<[u64; TYPES.len()]>,
}

impl Sizes {
    pub(super) fn new() -> Self {
        Self {
            count: [0; TYPES.len()],
            sum: [0.; TYPES.len()],
            sum2: [0.; TYPES.len()],
            hist: vec![[0; TYPES.len()]; BINS],
        }
    }

    pub(super) fn add(&mut self, kind: usize, bytes: u64) {
        let kib = bytes as f64 / 1024.;
        self.count[kind] += 1;
        self.sum[kind] += kib;
        self.sum2[kind] += kib * kib;
        self.hist[bin_of(bytes)][kind] += 1;
    }

    /// Mean and population s.d. of the type's sizes in KiB.
    pub(super) fn mean_sd(&self, kind: usize) -> (f64, f64) {
        let n = self.count[kind].max(1) as f64;
        let mean = self.sum[kind] / n;
        (mean, (self.sum2[kind] / n - mean * mean).max(0.).sqrt())
    }
}

/// One 100 ms sample: when it ended, and the MiB written and files completed during it.
pub(super) struct Sample {
    pub(super) at: f64,
    pub(super) mib: f64,
    pub(super) files: f64,
}

pub(super) const SAMPLE: Duration = Duration::from_millis(100);

/// Samples per moving-average window: 20 seconds of 100 ms samples.
const WINDOW: usize = 200;

/// One row: time, the log-space centre of MiB/s over the trailing 20 s, the +/-1, 2, 3 s.d. edges
/// (symmetric on a log axis), and the raw files/s of this sample.
pub(super) struct Row {
    pub(super) at: f64,
    pub(super) v: [f64; 8],
}

/// Complete 1-second bins behind each window; a 20 s window averages 20 of them.
const BIN_WINDOW: usize = 20;

/// Mean and s.d. of the trailing 20 complete 1-second bins, for each 100 ms sample. The edges are
/// mean +/- k s.d., with the lower edge held at zero because a speed cannot be negative.
pub(super) fn log_bands(samples: &[Sample]) -> Vec<Row> {
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

/// Distribution of the 20 s moving averages (one per sample once the window is full), with the
/// +/-1, 2 and 3 s.d. bands as percentages of the mean and the share of windows that land inside each.
pub(super) fn distribution(samples: &[Sample]) -> Vec<String> {
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
