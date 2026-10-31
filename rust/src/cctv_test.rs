//! `spit --cctv-test [short|long] -F PREFIX`: the standard test, with every chart's data and
//! gnuplot script written by spit. Short (the default) runs the CCTV camera range (charts 1, 3
//! and 4) and the block-size and queue-depth sweeps (charts 2 and 5). Long runs one CCTV count
//! and writes the rolling rate envelope (chart 6).
//!
//! Defaults: 1000 Mbit/s link and 5 Mbit/s per camera, so 200 cameras when no --cctv is given;
//! 30 s per camera count in the short test and 60 s in the long one (-t overrides both).

use crate::{cctv, cli, engine, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Block sizes in KiB for the block-size chart (chart 2).
const BLOCKS_KIB: [usize; 12] = [4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192];
/// Queue depths for the queue-depth chart (chart 5), powers of two.
const QUEUE_DEPTHS: [usize; 13] = [1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096];
/// Block sizes that get a queue-depth line on chart 5.
const QD_BLOCKS_KIB: [usize; 3] = [4, 64, 1024];
const SWEEP_SECONDS: &str = "10";
const SWEEP_RANGE: &str = "1GiB";
/// Enough for the largest chart 5 point (1 MiB blocks at queue depth 4096 need about 4 GiB of I/O pool).
const SWEEP_MEMORY: &str = "6GiB";
const CAMERA_MBPS: f64 = 5.;
const LINK_MBPS: f64 = 1000.;

pub fn run(mode: &str, mut args: Vec<String>) -> Result<()> {
    let mut label = String::new();
    if let Some(i) = args.iter().position(|a| a == "--label") {
        // Host and remote endpoint, written into every chart title.
        label = args.get(i + 1).cloned().ok_or("--label needs TEXT")?;
        args.drain(i..i + 2);
    }
    let prefix = value(&args, "-F").ok_or("--cctv-test needs -F PREFIX")?;
    // Every chart title says what was tested: the target path, the remote details and the test type.
    let version = format!("spit {}", env!("CARGO_PKG_VERSION"));
    let title = [
        version.as_str(),
        label.as_str(),
        &format!("{mode} test"),
        &format!("target {prefix}"),
    ]
    .iter()
    .filter(|s| !s.is_empty())
    .cloned()
    .collect::<Vec<_>>()
    .join(", ");
    let _ = crate::cctv_plots::LABEL.set(title);
    let dir = Path::new(&prefix)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let long = match mode {
        "short" | "range" => false,
        "long" => true,
        other => {
            return Err(format!(
                "--cctv-test takes short, range or long, not {other}"
            ))
        }
    };
    // Each chart file says which I/O mode made it, so direct and pagecache runs can sit side by side.
    let io = value(&args, "--cctv-io").unwrap_or_else(|| {
        if cfg!(any(windows, target_os = "linux")) {
            "direct"
        } else {
            "pagecache"
        }
        .into()
    });
    let _ = crate::cctv_plots::SUFFIX.set(format!("-{io}"));

    if !args.iter().any(|a| a == "--cctv") {
        let link = value(&args, "--link-mbps")
            .and_then(|v| v.parse().ok())
            .unwrap_or(LINK_MBPS);
        let fit = (link / CAMERA_MBPS).floor() as usize;
        let cameras = if long {
            format!("{fit},{CAMERA_MBPS},1")
        } else {
            format!("5..{fit}*sqrt2,{CAMERA_MBPS},1")
        };
        args.extend(["--cctv".into(), cameras]);
    }
    if !args.iter().any(|a| a == "-t") {
        args.extend(["-t".into(), if long { "60" } else { "30" }.into()]);
    }
    for (flag, value) in [("-c", "k64"), ("--filesizemb", "16"), ("-s", "0")] {
        if !args.iter().any(|a| a == flag) {
            args.extend([flag.into(), value.into()]);
        }
    }
    let config = cli::parse(args.clone())?.ok_or("nothing to run")?;
    cctv::run(config)?;
    interrupted()?;

    if mode == "short" {
        sweeps(&dir)?;
    }
    println!("cctv-test {mode} finished; files in {}", dir.display());
    Ok(())
}

/// Chart 5 (queue depth) and chart 2 (block size) for the short test, as their own .dat and .gnu files.
fn sweeps(dir: &Path) -> Result<()> {
    let file = dir.join("sweep-workload.dat");
    let file = file.to_string_lossy().to_string();

    for kib in QD_BLOCKS_KIB {
        let mut rows = String::new();
        for qd in QUEUE_DEPTHS {
            let s = point(
                &file,
                dir,
                &format!("ws1k{kib}q{qd}"),
                &format!("short-qd-{kib}k-q{qd}"),
            )?;
            if s.writes == 0 {
                // No completed operations: a missing value, not a rate of zero. Left out of the data.
                eprintln!("warning: {kib} KiB queue depth {qd} completed no writes; point left out of short-qd-{kib}k.dat");
                continue;
            }
            rows.push_str(&format!(
                "{qd} {} {} {} {}\n",
                mbit(s.write_bytes, s.seconds),
                iops(s.writes, s.seconds),
                mean_ms(&s.latencies_us),
                max_ms(&s.latencies_us)
            ));
        }
        fs::write(dir.join(format!("short-qd-{kib}k.dat")), rows).map_err(|e| e.to_string())?;
    }
    fs::write(dir.join("short-qd.gnu"), qd_gnu(dir)).map_err(|e| e.to_string())?;

    let mut write_rows = String::new();
    let mut read_rows = String::new();
    let mut write_ops = String::new();
    let mut read_ops = String::new();
    for kib in BLOCKS_KIB {
        let bytes = kib * 1024;
        let w = point(
            &file,
            dir,
            &format!("ws1k{kib}q1"),
            &format!("short-bs-write-{kib}k"),
        )?;
        if w.writes == 0 {
            eprintln!("warning: {kib} KiB write completed no writes; point left out of short-bs-write.dat");
        } else {
            write_rows.push_str(&format!(
                "{bytes} {} {} {}\n",
                mbit(w.write_bytes, w.seconds),
                mean_ms(&w.latencies_us),
                max_ms(&w.latencies_us)
            ));
        }
        for us in &w.latencies_us {
            write_ops.push_str(&format!("{bytes} {:.3}\n", *us as f64 / 1000.));
        }
        let r = point(
            &file,
            dir,
            &format!("rs1k{kib}q1"),
            &format!("short-bs-read-{kib}k"),
        )?;
        read_rows.push_str(&format!(
            "{bytes} {} {} {}\n",
            mbit(r.read_bytes, r.seconds),
            mean_ms(&r.latencies_us),
            max_ms(&r.latencies_us)
        ));
        for us in &r.latencies_us {
            read_ops.push_str(&format!("{bytes} {:.3}\n", *us as f64 / 1000.));
        }
    }
    fs::write(dir.join("short-bs-write.dat"), write_rows).map_err(|e| e.to_string())?;
    fs::write(dir.join("short-bs-read.dat"), read_rows).map_err(|e| e.to_string())?;
    fs::write(dir.join("short-bs-ops-write.dat"), write_ops).map_err(|e| e.to_string())?;
    fs::write(dir.join("short-bs-ops-read.dat"), read_ops).map_err(|e| e.to_string())?;
    fs::write(dir.join("short-bs.gnu"), bs_gnu(dir)).map_err(|e| e.to_string())?;
    Ok(())
}

/// Runs one sweep point through the normal engine. Latencies are recorded to a file beside the
/// results (kept as a record) and returned in memory for the .dat rows.
fn point(file: &str, dir: &Path, workload: &str, tag: &str) -> Result<engine::Summary> {
    interrupted()?;
    let latency = dir.join(format!("{tag}.lat"));
    let args: Vec<String> = [
        "-F",
        file,
        "-G",
        SWEEP_RANGE,
        "--backend",
        "direct",
        "-t",
        SWEEP_SECONDS,
        "-s",
        "0",
        "-c",
        workload,
        "--memory",
        SWEEP_MEMORY,
        "--latency-out",
    ]
    .iter()
    .map(|s| s.to_string())
    .chain([latency.to_string_lossy().to_string()])
    .collect();
    let config = cli::parse(args)?.ok_or("sweep point produced no configuration")?;
    match engine::run(config) {
        Ok(summary) => Ok(summary),
        Err(e) => {
            // One unrunnable point (for example one whose I/O pool exceeds the budget) must not
            // stop the rest of the sweep. It is reported and left out of the data, like a zero-op point.
            eprintln!("warning: {tag} skipped: {e}");
            Ok(engine::Summary {
                reads: 0,
                writes: 0,
                read_bytes: 0,
                write_bytes: 0,
                seconds: 0.,
                latencies_us: Vec::new(),
            })
        }
    }
}

/// A console stop (Ctrl+C, window close, shutdown) ends the test with an error, so the exit code
/// and the log both say so instead of the run just stopping.
fn interrupted() -> Result<()> {
    if crate::signals::INTERRUPTED.load(std::sync::atomic::Ordering::Relaxed) {
        let kind = crate::signals::SIGNAL.load(std::sync::atomic::Ordering::Relaxed);
        return Err(format!(
            "stopped by console signal {kind} (0 Ctrl+C, 1 Ctrl+Break, 2 window close, 6 shutdown)"
        ));
    }
    Ok(())
}

fn value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn mbit(bytes: u64, seconds: f64) -> String {
    format!("{:.3}", bytes as f64 * 8. / seconds.max(1e-9) / 1e6)
}

fn iops(ops: u64, seconds: f64) -> String {
    format!("{:.1}", ops as f64 / seconds.max(1e-9))
}

fn mean_ms(us: &[u32]) -> String {
    if us.is_empty() {
        return "0".into();
    }
    format!(
        "{:.4}",
        us.iter().map(|&u| u as f64).sum::<f64>() / us.len() as f64 / 1000.
    )
}

fn max_ms(us: &[u32]) -> String {
    format!(
        "{:.4}",
        us.iter().copied().max().unwrap_or(0) as f64 / 1000.
    )
}

fn path(dir: &Path, name: &str) -> String {
    dir.join(name).to_string_lossy().replace('\\', "/")
}

fn suffix_text() -> String {
    crate::cctv_plots::label_suffix()
}

fn qd_gnu(dir: &Path) -> String {
    let suffix = suffix_text();
    let out = path(dir, "short-qd.png");
    let mut plots = Vec::new();
    for (kib, title) in QD_BLOCKS_KIB.iter().zip(["4 KiB", "64 KiB", "1 MiB"]) {
        plots.push(format!(
            "\"{}\" using 1:2 with linespoints lw 6 pt 7 ps 3 title \"{title}\"",
            path(dir, &format!("short-qd-{kib}k.dat"))
        ));
    }
    let tics: Vec<String> = QUEUE_DEPTHS.iter().map(|q| q.to_string()).collect();
    format!(
        "set terminal pngcairo size 6400,3600 font \"sans,48\" background rgb \"white\"\n\
         set output \"{out}\"\n\
         set title \"spit queue depth vs throughput (direct, {SWEEP_SECONDS} s per point){suffix}\"\n\
         set xlabel \"queue depth (q)\"\n\
         set ylabel \"achieved Mbit/s\"\n\
         set logscale x 2\n\
         set xrange [0.8:5000]\n\
         set yrange [0:*]\n\
         set grid\n\
         set key top left\n\
         set xtics ({})\n\
         plot {}\n",
        tics.join(","),
        plots.join(", \\\n     ")
    )
}

fn bs_gnu(dir: &Path) -> String {
    let suffix = suffix_text();
    let out = path(dir, "short-bs.png");
    let tics: Vec<String> = BLOCKS_KIB
        .iter()
        .map(|&k| {
            let label = if k >= 1024 {
                format!("{}M", k / 1024)
            } else {
                format!("{k}K")
            };
            format!("\"{label}\" {}", k * 1024)
        })
        .collect();
    format!(
        "set terminal pngcairo size 6400,3600 font \"sans,48\" background rgb \"white\"\n\
         set output \"{out}\"\n\
         set title \"spit block-size sweep, queue depth 1: latency per operation (dots), Mbit/s (lines){suffix}\"\n\
         set xlabel \"block size\"\n\
         set ylabel \"latency per operation (ms)\"\n\
         set y2label \"achieved Mbit/s\"\n\
         set logscale x 2\n\
         set logscale y\n\
         set logscale y2 2\n\
         set grid\n\
         set key top left\n\
         set xtics ({})\n\
         set y2tics\n\
         plot \"{}\" using ($1*(1+0.3*(rand(0)-0.5))):2 axes x1y1 with points pt 7 ps 0.3 lc rgb \"#1f77b4\" title \"write latency\", \\\n\
              \"{}\" using ($1*(1+0.3*(rand(0)-0.5))):2 axes x1y1 with points pt 7 ps 0.3 lc rgb \"#ff7f0e\" title \"read latency\", \\\n\
              \"{}\" using 1:2 axes x1y2 with linespoints lw 6 pt 7 ps 3 lc rgb \"#1f77b4\" title \"write Mbit/s\", \\\n\
              \"{}\" using 1:2 axes x1y2 with linespoints lw 6 pt 7 ps 3 lc rgb \"#ff7f0e\" title \"read Mbit/s\"\n",
        tics.join(", "),
        path(dir, "short-bs-ops-write.dat"),
        path(dir, "short-bs-ops-read.dat"),
        path(dir, "short-bs-write.dat"),
        path(dir, "short-bs-read.dat"),
    )
}
