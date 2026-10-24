//! gnuplot inputs written after a CCTV camera range: the latency-vs-cameras sweep (one dot per
//! write on y1, achieved Mbit/s on y2) and 0.1 ms latency histograms for all writes, the lowest
//! camera count and the highest. Each `.gnu` renders its `.dat` files with `gnuplot <file>`.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::Path;

/// Run label (host and remote endpoint) appended to every chart title; set once by `--label`.
pub static LABEL: std::sync::OnceLock<String> = std::sync::OnceLock::new();
/// Added to each CCTV chart file name ("-direct" or "-pagecache") so both I/O modes can be kept.
pub static SUFFIX: std::sync::OnceLock<String> = std::sync::OnceLock::new();

fn sfx() -> String {
    SUFFIX.get().cloned().unwrap_or_default()
}

/// The target and test details, on a line of their own under the chart title. Backslashes are
/// escaped because gnuplot reads them inside double quotes.
pub fn label_suffix() -> String {
    LABEL.get().map(|l| format!("\\n{}", l.replace('\\', "\\\\"))).unwrap_or_default()
}

/// One camera count's measured writes.
pub struct Run<'a> {
    pub count: usize,
    pub block_bytes: usize,
    pub seconds: f64,
    /// (seconds since the measured start, latency in microseconds), as written to the .lat file.
    pub samples: &'a [(f64, u32)],
    /// Total Mbit over the run: (offered, achieved, dropped). None when the run did not record it.
    pub loss: Option<(f64, f64, f64)>,
}

const HIST_BIN_MS: f64 = 0.1;
const LINE_MS: f64 = 200.;

/// Writes every plot input for a camera range into `dir`.
pub fn write_range(dir: &Path, runs: &[Run], io_mode: &str) -> io::Result<()> {
    if runs.is_empty() {
        return Ok(());
    }
    write_sweep(dir, runs, io_mode)?;
    let low = runs.iter().min_by_key(|r| r.count).unwrap();
    let high = runs.iter().max_by_key(|r| r.count).unwrap();
    let mut picks = vec![low];
    if high.count != low.count {
        picks.push(high);
    }
    for run in picks {
        let ms: Vec<f64> = run.samples.iter().map(|&(_, us)| us as f64 / 1000.).collect();
        write_histogram(dir, &run.count.to_string(), &format!("{} cameras, {io_mode}", run.count), &ms)?;
    }
    Ok(())
}

fn create(path: &Path) -> io::Result<BufWriter<File>> {
    Ok(BufWriter::new(File::create(path)?))
}

/// Forward slashes keep gnuplot string literals valid on Windows paths.
fn gnu_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn block_label(bytes: usize) -> String {
    if bytes % 1048576 == 0 {
        format!("{} MiB", bytes / 1048576)
    } else {
        format!("{} KiB", bytes / 1024)
    }
}

fn write_sweep(dir: &Path, runs: &[Run], io_mode: &str) -> io::Result<()> {
    let dots_path = dir.join(format!("cctv-allranges{}.dat", sfx()));
    let mut dots = create(&dots_path)?;
    for run in runs {
        for &(_, us) in run.samples {
            writeln!(dots, "{} {:.3}", run.count, us as f64 / 1000.)?;
        }
    }
    dots.flush()?;

    let rates_path = dir.join(format!("cctv-allranges-mbps{}.dat", sfx()));
    let mut rates = create(&rates_path)?;
    for run in runs {
        let mbit = run.samples.len() as f64 * run.block_bytes as f64 * 8. / run.seconds / 1e6;
        writeln!(rates, "{} {:.1}", run.count, mbit)?;
    }
    rates.flush()?;

    let mut counts: Vec<usize> = runs.iter().map(|r| r.count).collect();
    counts.sort_unstable();
    counts.dedup();
    let tics = counts.iter().map(usize::to_string).collect::<Vec<_>>().join(", ");
    let block = block_label(runs[0].block_bytes);
    let png = gnu_path(&dir.join(format!("cctv-allranges{}.png", sfx())));
    // With loss recorded, offered, achieved and dropped Mbit/s per camera count go on y2.
    let loss_path = dir.join(format!("cctv-allranges-loss{}.dat", sfx()));
    let (loss_def, loss_plot) = if runs.iter().all(|r| r.loss.is_some()) {
        let mut loss = create(&loss_path)?;
        for run in runs {
            let (offered, achieved, dropped) = run.loss.unwrap();
            let seconds = run.seconds.max(1e-9);
            writeln!(loss, "{} {:.3} {:.3} {:.3}", run.count, offered / seconds, achieved / seconds, dropped / seconds)?;
        }
        loss.flush()?;
        (
            format!("LOSS = \"{}\"\n", gnu_path(&loss_path)),
            format!(
                ", \\\n     LOSS using 1:2 axes x1y2 with lines dt 2 lw 4 lc rgb \"#7f7f7f\" title \"offered Mbit/s\", \\\n     LOSS using 1:4 axes x1y2 with linespoints lw 6 pt 7 ps 3 lc rgb \"#d62728\" title \"dropped Mbit/s\""
            ),
        )
    } else {
        (String::new(), String::new())
    };
    let gnu = format!(
        r##"# Per-write CCTV latency against camera count, one dot per write, with achieved Mbit/s on y2.
# Each dot is moved by a random factor of about +/-4% of its camera count (log axis) so points at the same count do not stack.
# Render with:  gnuplot "{gnu}"   (writes the PNG named below)
set terminal pngcairo size 6400,3600 font "sans,48" background rgb "white"
set output "{png}"
set title "spit CCTV write latency vs cameras ({io_mode}, {block} writes, q1 per camera, one dot per write){label_suffix}"
set xlabel "cameras"
set logscale x 2
set ylabel "write latency (ms)"
set y2label "achieved Mbit/s"
set logscale y
set grid ytics
set key outside bottom center horizontal
set xtics ({tics})
set y2tics
set arrow from graph 0, first {LINE_MS} to graph 1, first {LINE_MS} nohead dt 2 lw 3 lc rgb "#d62728"
DATA = "{dots}"
MBPS = "{rates}"
{loss_def}plot DATA using ($1*(1+0.08*(rand(0)-0.5))):2 axes x1y1 with points pt 7 ps 0.3 lc rgb "#1f77b4" title "write latency (ms)", \
     MBPS using 1:2 axes x1y2 with linespoints lw 6 pt 7 ps 3 lc rgb "#2ca02c" title "achieved Mbit/s"{loss_plot}
"##,
        label_suffix = label_suffix(),
        loss_def = loss_def,
        loss_plot = loss_plot,
        gnu = gnu_path(&dir.join(format!("cctv-allranges{}.gnu", sfx()))),
        dots = gnu_path(&dots_path),
        rates = gnu_path(&rates_path),
    );
    fs::write(dir.join(format!("cctv-allranges{}.gnu", sfx())), gnu)?;
    println!(
        "wrote {} and {}: render with gnuplot {}",
        dots_path.display(),
        rates_path.display(),
        dir.join(format!("cctv-allranges{}.gnu", sfx())).display()
    );
    Ok(())
}

/// Nearest-rank percentile of an ascending slice.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    let rank = ((sorted.len() as f64 * p).ceil() as usize).clamp(1, sorted.len());
    sorted[rank - 1]
}

fn write_histogram(dir: &Path, tag: &str, label: &str, values: &[f64]) -> io::Result<()> {
    if values.is_empty() {
        return Ok(());
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let n = sorted.len();
    let mean = sorted.iter().sum::<f64>() / n as f64;
    let p999 = percentile(&sorted, 0.999);
    let p9999 = percentile(&sorted, 0.9999);
    let (low, high) = (sorted[0], sorted[n - 1]);

    let mut bins: BTreeMap<i64, u64> = BTreeMap::new();
    for v in &sorted {
        *bins.entry((v / HIST_BIN_MS + 0.5).floor() as i64).or_insert(0) += 1;
    }
    // The axis runs from the first to the last bin holding more than one write, so a single
    // outlier at either end does not stretch the chart. Falls back to min and max if no bin qualifies.
    let busy: Vec<f64> = bins.iter().filter(|(_, count)| **count > 1).map(|(bin, _)| *bin as f64 * HIST_BIN_MS).collect();
    let (low, high) = match (busy.first(), busy.last()) {
        (Some(first), Some(last)) => ((first - HIST_BIN_MS).max(HIST_BIN_MS), last + HIST_BIN_MS),
        _ => (low, high),
    };
    let dat_path = dir.join(format!("cctv-hist-{tag}{}.dat", sfx()));
    let mut out = create(&dat_path)?;
    let mut cum = 0u64;
    for (bin, count) in &bins {
        cum += count;
        writeln!(
            out,
            "{:.1} {} {:.6}",
            *bin as f64 * HIST_BIN_MS,
            count,
            100. * cum as f64 / n as f64
        )?;
    }
    out.flush()?;

    let png = gnu_path(&dir.join(format!("cctv-hist-{tag}{}.png", sfx())));
    let gnu = format!(
        r##"# Histogram of per-write CCTV latency, {label}, 0.1 ms bins.
# Log-log axes; y2 is the cumulative distribution; vertical lines mark the average response time, p99.9 and p99.99.
# Render with:  gnuplot "{gnu}"   (writes the PNG named below)
set terminal pngcairo size 6400,3600 font "sans,48" background rgb "white"
set output "{png}"
set title "spit CCTV write latency histogram, {label} ({n} writes){label_suffix}"
set xlabel "write latency (ms), 0.1 ms bins"
set ylabel "writes per 0.1 ms bin"
set y2label "cumulative distribution (%)"
set logscale x
set logscale y
set xrange [{low_x:.2}:{high:.2}]
set y2range [0:100]
set y2tics 0,10,100
set ytics format "%.0f"
set grid
set key outside bottom center horizontal
set arrow from {mean:.2}, graph 0 to {mean:.2}, graph 1 nohead dt 2 lw 4 lc rgb "#2ca02c"
set label "average response time {mean:.2} ms" at {mean:.2}, graph 0.97 right rotate by 90 offset -1,0 front tc rgb "#2ca02c"
set arrow from {p999:.2}, graph 0 to {p999:.2}, graph 1 nohead dt 2 lw 4 lc rgb "#d62728"
set label "p99.9 {p999:.2} ms" at {p999:.2}, graph 0.97 right rotate by 90 offset -1,0 front tc rgb "#d62728"
set arrow from {p9999:.2}, graph 0 to {p9999:.2}, graph 1 nohead dt 2 lw 4 lc rgb "#9467bd"
set label "p99.99 {p9999:.2} ms" at {p9999:.2}, graph 0.6 right rotate by 90 offset -1,0 front tc rgb "#9467bd"
set boxwidth {bin:.2} * 0.9
set style fill solid 1.0 noborder
plot "{dat}" using 1:2 with boxes lc rgb "#1f77b4" title "writes per bin", \
     "{dat}" using 1:3 axes x1y2 with lines lw 6 lc rgb "#ff7f0e" title "cumulative %", \
     1/0 with lines dt 2 lw 4 lc rgb "#2ca02c" title "average {mean:.2} ms", \
     1/0 with lines dt 2 lw 4 lc rgb "#d62728" title "p99.9 {p999:.2} ms", \
     1/0 with lines dt 2 lw 4 lc rgb "#9467bd" title "p99.99 {p9999:.2} ms"
"##,
        label_suffix = label_suffix(),
        gnu = gnu_path(&dir.join(format!("cctv-hist-{tag}{}.gnu", sfx()))),
        dat = gnu_path(&dat_path),
        low_x = low,
        bin = HIST_BIN_MS,
    );
    fs::write(dir.join(format!("cctv-hist-{tag}{}.gnu", sfx())), gnu)?;
    println!(
        "wrote {} ({n} writes, mean {mean:.2} ms, p99.9 {p999:.2} ms, p99.99 {p9999:.2} ms)",
        dat_path.display()
    );
    Ok(())
}

/// Rolling mean and +/-1, 2, 3 standard deviations of the per-second rate over `window_s` seconds.
/// Writes `<stem>.env.dat` (t mean sd1 sd2 sd3) and `<stem>.env.gnu`, which renders it with the raw rate points.
pub fn write_envelope(dir: &Path, stem: &str, rates: &[(f64, f64)], window_s: f64, link_mbps: Option<f64>) -> io::Result<()> {
    if rates.is_empty() {
        return Ok(());
    }
    let dat_path = dir.join(format!("{stem}.env.dat"));
    let mut out = create(&dat_path)?;
    let (mut start, mut sum, mut sum2) = (0usize, 0f64, 0f64);
    for (i, &(t, v)) in rates.iter().enumerate() {
        sum += v;
        sum2 += v * v;
        while rates[start].0 <= t - window_s {
            let old = rates[start].1;
            sum -= old;
            sum2 -= old * old;
            start += 1;
        }
        let n = (i + 1 - start) as f64;
        let mean = sum / n;
        let sd = (sum2 / n - mean * mean).max(0.0).sqrt();
        writeln!(out, "{t:.0} {mean:.3} {sd:.3} {:.3} {:.3}", 2.0 * sd, 3.0 * sd)?;
    }
    out.flush()?;

    let png = gnu_path(&dir.join(format!("{stem}.env.png")));
    let rate = gnu_path(&dir.join(format!("{stem}.rate")));
    let link = match link_mbps {
        Some(mbps) => format!(
            "set arrow from graph 0, first {mbps} to graph 1, first {mbps} nohead dt 2 lw 4 lc rgb \"#d62728\"\nset label \"link {mbps} Mbit/s\" at graph 0.01, first {mbps} offset 0,1 tc rgb \"#d62728\"\n"
        ),
        None => String::new(),
    };
    let gnu = format!(
        r##"# Sustained write rate with a rolling {window_s:.0} s mean and +/-1, 2, 3 SD bands ({stem}).
# Render with:  gnuplot "{gnu}"   (writes the PNG named below)
set terminal pngcairo size 6400,3600 font "sans,48" background rgb "white"
set output "{png}"
set title "spit sustained write rate, rolling {window_s:.0} s mean and +/-1, 2, 3 SD{label_suffix}"
set xlabel "time (s)"
set ylabel "achieved Mbit/s"
set grid
set key top right
set yrange [0:*]
{link}plot "{dat}" using 1:($2-$5):($2+$5) with filledcurves fc rgb "#d6e8f7" title "+/-3 SD", \
     "{dat}" using 1:($2-$4):($2+$4) with filledcurves fc rgb "#9ecae1" title "+/-2 SD", \
     "{dat}" using 1:($2-$3):($2+$3) with filledcurves fc rgb "#3182bd" title "+/-1 SD", \
     "{dat}" using 1:2 with lines lw 6 lc rgb "#08306b" title "rolling mean", \
     "{rate}" using 1:2 with points pt 7 ps 1.2 lc rgb "#2ca02c" title "per second"
"##,
        label_suffix = label_suffix(),
        gnu = gnu_path(&dir.join(format!("{stem}.env.gnu"))),
        dat = gnu_path(&dat_path),
    );
    fs::write(dir.join(format!("{stem}.env.gnu")), gnu)?;
    Ok(())
}
