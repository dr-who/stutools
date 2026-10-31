use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
#[cfg(unix)]
use std::{
    process::{Child, Stdio},
    thread,
    time::{Duration, Instant},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

/// A fresh directory that is removed when the test ends.
struct Dir(PathBuf);

impl Dir {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "spit-fsfiller-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const EXTENSIONS: [&str; 5] = ["txt", "docx", "jpg", "bin", "mp4"];

/// Runs spit inside `dir`, because the fill writes its chart files to the working directory.
fn spit_in(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_spit"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}

/// Every regular file under `root`, with its size in bytes.
fn files_under(root: &Path) -> Vec<(PathBuf, u64)> {
    let mut found = Vec::new();
    if !root.exists() {
        return found;
    }
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if entry.file_type().unwrap().is_dir() {
                pending.push(path);
            } else {
                found.push((path.clone(), entry.metadata().unwrap().len()));
            }
        }
    }
    found
}

fn report_field(stdout: &str, field: &str) -> u64 {
    stdout
        .split_whitespace()
        .find_map(|w| w.strip_prefix(field))
        .unwrap_or_else(|| panic!("no {field} in {stdout}"))
        .parse()
        .unwrap()
}

#[test]
fn fills_two_levels_with_mixed_sizes_until_the_cap() {
    let dir = Dir::new();
    let root = dir.0.join("fs");
    let root_arg = root.to_str().unwrap();
    let out = spit_in(
        &dir.0,
        &[
            "--fsfiller",
            "-F",
            root_arg,
            "-G",
            "16MiB",
            "--fsfiller-dirs",
            "3,2",
        ],
    );
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout.contains("stopped: -G cap reached"), "{stdout}");

    for t in 0..3 {
        for s in 0..2 {
            assert!(root
                .join(format!("t{t:03}"))
                .join(format!("s{s:03}"))
                .is_dir());
        }
    }
    let files = files_under(&root);
    assert!(!files.is_empty());
    let total: u64 = files.iter().map(|(_, n)| n).sum();
    assert!(total <= 16 << 20, "wrote {total} bytes past the cap");
    assert_eq!(report_field(&stdout, "files="), files.len() as u64);
    for (path, size) in &files {
        let ext = path.extension().unwrap().to_str().unwrap();
        assert!(
            EXTENSIONS.contains(&ext),
            "{} has an unknown type",
            path.display()
        );
        assert!(
            *size >= 1024,
            "{} is {size} bytes, under the 1 KiB minimum",
            path.display()
        );
    }
    let mut sizes: Vec<u64> = files.iter().map(|(_, n)| *n).collect();
    sizes.sort();
    sizes.dedup();
    assert!(sizes.len() > 1, "only one file size was written: {sizes:?}");
}

#[test]
fn time_limit_stops_and_reports() {
    let dir = Dir::new();
    let root = dir.0.join("fs");
    let out = spit_in(
        &dir.0,
        &[
            "--fsfiller",
            "-F",
            root.to_str().unwrap(),
            "--fsfiller-dirs",
            "2,2",
            "-t",
            "1",
        ],
    );
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("stopped: -t time limit reached"),
        "{stdout}"
    );
    assert!(stdout.contains("directories=6"), "{stdout}");
}

#[test]
fn rejects_options_that_do_not_fit() {
    let dir = Dir::new();
    let out = spit_in(
        &dir.0,
        &["--fsfiller", "-f", "scratch.img", "-c", "rs0k4q4"],
    );
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--fsfiller takes one -F directory"));
    let out = spit_in(
        &dir.0,
        &["--fsfiller-dirs", "2,2", "-c", "rs0k4q4", "-f", "x"],
    );
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--fsfiller-dirs requires --fsfiller"));
    let out = spit_in(
        &dir.0,
        &["--ratelimitmbps", "100", "-c", "rs0k4q4", "-f", "x"],
    );
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--ratelimitmbps requires --fsfiller"));
    let out = spit_in(
        &dir.0,
        &["--writes", "buffered", "-c", "rs0k4q4", "-f", "x"],
    );
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--writes requires --fsfiller"));
}

#[test]
fn default_fill_writes_through_and_buffered_turns_it_off() {
    let dir = Dir::new();
    let root = dir.0.join("fs");
    let out = spit_in(
        &dir.0,
        &[
            "--fsfiller",
            "-F",
            root.to_str().unwrap(),
            "--fsfiller-dirs",
            "2,2",
            "-G",
            "8MiB",
        ],
    );
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout.contains("write_mode=writethrough"), "{stdout}");
    assert_eq!(
        report_field(&stdout, "files="),
        files_under(&root).len() as u64,
        "{stdout}"
    );

    let root = dir.0.join("buffered");
    let out = spit_in(
        &dir.0,
        &[
            "--fsfiller",
            "-F",
            root.to_str().unwrap(),
            "-G",
            "8MiB",
            "--writes",
            "buffered",
        ],
    );
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "{stdout}");
    assert!(stdout.contains("write_mode=buffered"), "{stdout}");
}

#[test]
fn target_mbit_limits_the_write_rate() {
    let dir = Dir::new();
    let root = dir.0.join("fs");
    let out = spit_in(
        &dir.0,
        &[
            "--fsfiller",
            "-F",
            root.to_str().unwrap(),
            "--fsfiller-dirs",
            "2,2",
            "-t",
            "3",
            "--ratelimitmbps",
            "100",
        ],
    );
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    // 100 Mbit/s is 11.92 MiB/s; the limit may be slightly under, never far over.
    let rate: f64 = stdout
        .split_whitespace()
        .find_map(|w| w.strip_prefix("MiB_per_s="))
        .unwrap()
        .parse()
        .unwrap();
    assert!(
        (9.0..13.0).contains(&rate),
        "rate {rate} MiB/s is not near 100 Mbit/s:\n{stdout}"
    );
    assert!(dir.0.join("fsfiller-over-time.dat").exists());
}

/// Waits for `child` to exit, killing it and failing the test after `limit`.
#[cfg(unix)]
fn wait_for_exit(mut child: Child, limit: Duration) -> Output {
    let start = Instant::now();
    loop {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        if start.elapsed() > limit {
            // Reap the child before the caller's cleanup removes the directory it writes into.
            let _ = child.kill();
            let _ = child.wait();
            panic!("spit did not exit within {limit:?} after Ctrl-C");
        }
        thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(unix)]
fn interrupt(child: &Child) {
    let status = Command::new("kill")
        .arg("-INT")
        .arg(child.id().to_string())
        .status()
        .unwrap();
    assert!(status.success());
}

#[cfg(unix)]
#[test]
fn ctrl_c_stops_fsfiller_and_still_reports() {
    let dir = Dir::new();
    let root = dir.0.join("fs");
    let child = Command::new(env!("CARGO_BIN_EXE_spit"))
        .current_dir(&dir.0)
        .args([
            "--fsfiller",
            "-F",
            root.to_str().unwrap(),
            "--fsfiller-dirs",
            "2,2",
            "-G",
            "64GiB",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // The fill runs at page-cache speed, so interrupt after a few dozen files, before much is written.
    let start = Instant::now();
    while files_under(&root).len() < 50 {
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "files did not appear"
        );
        thread::sleep(Duration::from_millis(5));
    }
    interrupt(&child);
    let out = wait_for_exit(child, Duration::from_secs(30));
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout.contains("stopped: Ctrl-C"), "{stdout}");
    assert!(report_field(&stdout, "files=") > 0, "{stdout}");
    // A partially written file is removed on interrupt, so the files on disk are exactly the completed ones.
    assert_eq!(
        files_under(&root).len() as u64,
        report_field(&stdout, "files="),
        "{stdout}"
    );
}

#[cfg(unix)]
#[test]
fn ctrl_c_in_command_mode_reports_partial_results() {
    let dir = Dir::new();
    let target = dir.0.join("target.img");
    fs::create_dir_all(&dir.0).unwrap();
    fs::write(&target, vec![0x55; 64 << 20]).unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_spit"))
        .current_dir(&dir.0)
        .args([
            "-f",
            target.to_str().unwrap(),
            "-c",
            "rs0k4q4",
            "-t",
            "-1",
            "-s",
            "0",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    thread::sleep(Duration::from_millis(500));
    interrupt(&child);
    let out = wait_for_exit(child, Duration::from_secs(30));
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(out.status.success(), "{stderr}");
    assert!(stderr.contains("interrupted by Ctrl-C"), "{stderr}");
    assert!(stdout.contains("read_ops="), "{stdout}");
    assert!(report_field(&stdout, "read_ops=") > 0, "{stdout}");
}

#[test]
fn default_help_is_short_and_points_to_full_help() {
    let out = spit_in(&std::env::temp_dir(), &["--help"]);
    assert!(out.status.success());
    let help = String::from_utf8_lossy(&out.stdout).into_owned();
    let lines = help.lines().count();
    assert!(lines <= 15, "default help is {lines} lines:\n{help}");
    assert!(help.contains("--help all"), "{help}");
    assert!(help.contains("--fsfiller"), "{help}");
    assert!(help.contains("--cctv"), "{help}");
    assert!(!help.contains("WORKLOAD TOKENS"), "{help}");

    let full = String::from_utf8_lossy(&spit_in(&std::env::temp_dir(), &["--help", "all"]).stdout)
        .into_owned();
    assert!(full.contains("WORKLOAD TOKENS"));
    assert!(full.contains("APPLICATION MODES"));
}

#[cfg(any(windows, target_os = "linux", target_os = "freebsd"))]
#[test]
fn direct_writes_fill_and_verify() {
    let dir = Dir::new();
    let root = dir.0.join("fs");
    let out = spit_in(
        &dir.0,
        &[
            "--fsfiller",
            "-F",
            root.to_str().unwrap(),
            "--fsfiller-dirs",
            "2,2",
            "-G",
            "8MiB",
            "--writes",
            "direct",
            "-v",
        ],
    );
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout.contains("write_mode=direct"), "{stdout}");
    assert_eq!(
        report_field(&stdout, "verified="),
        report_field(&stdout, "files="),
        "{stdout}"
    );
    assert_eq!(
        report_field(&stdout, "files="),
        files_under(&root).len() as u64,
        "{stdout}"
    );
}
