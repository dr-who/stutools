//! Renders the fsfiller charts with gnuplot: the write-speed chart over time and the file size
//! histogram. Each chart also writes its data and gnuplot script, so it can be redrawn by hand.
use super::{
    stats::{bin_edge, Row},
    Settings, Shared, TYPES,
};
use std::{fs, io, process::Command};

/// One colour per file type, in the order of TYPES, so each suffix keeps its colour in the size chart.
const COLOURS: [&str; 5] = ["#1b9e77", "#d95f02", "#7570b3", "#e7298a", "#66a61e"];

const DATA: &str = "fsfiller-over-time.dat";
const SCRIPT: &str = "fsfiller-over-time.gnu";
const IMAGE: &str = "fsfiller-over-time.png";
const SIZES_DATA: &str = "fsfiller-sizes.dat";
const SIZES_SCRIPT: &str = "fsfiller-sizes.gnu";
const SIZES_IMAGE: &str = "fsfiller-sizes.png";

/// Writes the smoothed rows and a gnuplot script with log axes, then renders the PNG when gnuplot is installed.
pub(super) fn write_chart(s: &Settings, rows: &[Row]) -> io::Result<()> {
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
    render("chart", SCRIPT, IMAGE, DATA);
    Ok(())
}

/// Writes the per-type size histogram (quarter-octave bins) and renders it with gnuplot.
pub(super) fn write_sizes(shared: &Shared) -> io::Result<()> {
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
    render("sizes", SIZES_SCRIPT, SIZES_IMAGE, SIZES_DATA);
    Ok(())
}

/// Runs gnuplot on `script` to draw `image`, reporting where the output went.
fn render(label: &str, script: &str, image: &str, data: &str) {
    match Command::new("gnuplot").arg(script).status() {
        Ok(status) if status.success() => {
            println!("{label}: {image} (data {data}, script {script})")
        }
        Ok(status) => eprintln!("*warning* gnuplot exited with {status}; {label} not rendered"),
        Err(_) => {
            eprintln!("*warning* gnuplot not found; run `gnuplot {script}` to render {image}")
        }
    }
}
