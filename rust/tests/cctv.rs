use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "spit-cctv-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        Self(dir)
    }
    fn run(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_spit"))
            .arg("-F")
            .arg(self.0.join("recording"))
            .args(args)
            .output()
            .unwrap()
    }
    fn files(&self, extension: &str) -> Vec<PathBuf> {
        fs::read_dir(&self.0)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == extension))
            .collect()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn help_has_cctv_section_and_seven_day_example() {
    let out = Command::new(env!("CARGO_BIN_EXE_spit"))
        .arg("-h")
        .output()
        .unwrap();
    let help = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    for required in [
        "[cctv]",
        "--cctv",
        "--filesizemb",
        "-t 7d",
        ".hour",
        "64 KiB",
    ] {
        assert!(help.contains(required), "missing {required}: {help}");
    }
    assert!(!help.contains("--cams"));
}

#[test]
fn cameras_write_fixed_files_and_live_logs_without_hot_allocations() {
    let dir = Scratch::new();
    let out = dir.run(&[
        "--cctv",
        "2,8,24",
        "--filesizemb",
        "0.0625",
        "-c",
        "k64",
        "-t",
        "3s",
        "-s",
        "0.1",
    ]);
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("hot_path_allocations=0"));
    let summary = String::from_utf8_lossy(&out.stdout);
    for field in [
        "file_rotations=",
        "preallocation_waits=",
        "max_preallocation_wait_ms=",
    ] {
        assert!(summary.contains(field), "missing {field}: {summary}");
    }
    assert!(String::from_utf8_lossy(&out.stderr).contains("] read "));
    let files = dir.files("spitcam");
    assert!(files.len() >= 4, "expected rotation: {files:?}");
    for p in files {
        assert_eq!(fs::metadata(&p).unwrap().len(), 65536);
        assert!(p
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .contains(".cam00000"));
    }
    for ext in ["log", "min", "hour"] {
        let files = dir.files(ext);
        assert_eq!(files.len(), 1);
        let path = &files[0];
        assert!(path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("cctv-test-"));
        let text = fs::read_to_string(path).unwrap();
        if ext == "log" {
            assert!(text.contains("] read "));
        } else {
            let names: Vec<_> = text
                .lines()
                .next()
                .unwrap()
                .split(',')
                .map(str::trim)
                .collect();
            assert_eq!(&names[7..9], &["avg_write_ms", "max_write_ms"]);
            assert_eq!(text.lines().filter(|l| !l.starts_with('#')).count(), 2);
            assert!(text.contains("partial"));
            let data = text.lines().nth(1).unwrap();
            let values: Vec<_> = data.split(',').map(str::trim).collect();
            let submitted: f64 = values[20].parse().unwrap();
            let completed: f64 = values[21].parse().unwrap();
            let maximum_ms: f64 = values[8].parse().unwrap();
            assert!(submitted <= completed);
            assert!(((completed - submitted) * 1000. - maximum_ms).abs() < 0.000002);
            assert!(matches!(values[17], "1" | "2"));
            assert_eq!(values[19], "0");
            assert_eq!(
                text.lines()
                    .next()
                    .unwrap()
                    .match_indices(',')
                    .map(|(i, _)| i)
                    .collect::<Vec<_>>(),
                data.match_indices(',').map(|(i, _)| i).collect::<Vec<_>>()
            );
        }
    }
    assert!(!fs::read_dir(&dir.0).unwrap().any(|e| e
        .unwrap()
        .path()
        .extension()
        .is_some_and(|e| e == "pending")));
}

#[test]
fn invalid_camera_options_do_not_create_files() {
    for args in [
        vec!["--cctv", "0,5,24"],
        vec!["--cctv", "100,nan,24"],
        vec!["--cctv", "1,5,0"],
        vec!["--cctv", "1,5,24", "-c", "ws1"],
        vec!["--cctv", "1,5,24", "--filesizemb", "0.1", "-c", "k64"],
        vec!["--cctv", "1,5,24", "--memory", "1B"],
        vec!["--filesizemb", "16", "-c", "rs0"],
    ] {
        let dir = Scratch::new();
        let out = dir.run(&args);
        assert!(!out.status.success(), "{args:?}");
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 0, "{args:?}");
    }
}

#[test]
fn progress_can_be_disabled_and_restarts_preserve_logs_and_ownership() {
    let dir = Scratch::new();
    let mut marker = None;
    for _ in 0..2 {
        let out = dir.run(&[
            "--cctv",
            "1,8,24",
            "--filesizemb",
            "0.25",
            "-t",
            "0.2s",
            "-s",
            "0",
        ]);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(!String::from_utf8_lossy(&out.stderr).contains("] read "));
        let bytes = fs::read(dir.0.join("recording.cctv.owner")).unwrap();
        if let Some(old) = &marker {
            assert_eq!(old, &bytes)
        } else {
            marker = Some(bytes)
        }
    }
    for ext in ["log", "min", "hour"] {
        assert_eq!(dir.files(ext).len(), 2);
    }
    for path in dir.files("log") {
        assert!(!fs::read_to_string(path).unwrap().contains("] read "));
    }
}

#[test]
fn exclusive_prefix_lock_is_released_after_process_exit() {
    use std::{
        process::Stdio,
        time::{Duration, Instant},
    };
    let dir = Scratch::new();
    let mut child = Command::new(env!("CARGO_BIN_EXE_spit"))
        .arg("-F")
        .arg(dir.0.join("recording"))
        .args([
            "--cctv",
            "1,8,24",
            "--filesizemb",
            "0.25",
            "-t",
            "10s",
            "-s",
            "0",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !dir.0.join("recording.cctv.owner").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    let out = dir.run(&["--cctv", "1,8,24", "-t", "0.1s"]);
    let _ = child.kill();
    child.wait().unwrap();
    let error = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "second run must not share the prefix"
    );
    assert!(
        error.contains("prefix") && (error.contains("lock") || error.contains("owns")),
        "{error}"
    );
    let retry = dir.run(&[
        "--cctv",
        "1,8,24",
        "--filesizemb",
        "0.25",
        "-t",
        "0.1s",
        "-s",
        "0",
    ]);
    assert!(
        retry.status.success(),
        "{}",
        String::from_utf8_lossy(&retry.stderr)
    );
}

#[cfg(unix)]
#[test]
fn interrupt_finalizes_a_seven_day_run_without_waiting_seven_days() {
    use std::{
        process::Stdio,
        time::{Duration, Instant},
    };
    let dir = Scratch::new();
    let mut child = Command::new(env!("CARGO_BIN_EXE_spit"))
        .arg("-F")
        .arg(dir.0.join("recording"))
        .args([
            "--cctv",
            "1,8,24",
            "--filesizemb",
            "0.25",
            "-t",
            "7d",
            "-s",
            "0",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !dir.0.join("recording.cctv.owner").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    // Minimal containers need not ship a standalone kill executable.
    unsafe extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
    }
    assert_eq!(unsafe { kill(child.id() as i32, 2) }, 0);
    while child.try_wait().unwrap().is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    if child.try_wait().unwrap().is_none() {
        let _ = child.kill();
        panic!("CCTV did not handle SIGINT");
    }
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("write_ops="));
    assert!(!fs::read_dir(&dir.0).unwrap().any(|e| e
        .unwrap()
        .path()
        .extension()
        .is_some_and(|e| e == "pending")));
}
