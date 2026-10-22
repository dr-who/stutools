use std::{
    fs,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Target(std::path::PathBuf);
impl Target {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "spit-rust-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&p, vec![0x55; 1048576]).unwrap();
        Self(p)
    }
    fn run(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_spit"))
            .arg("-f")
            .arg(&self.0)
            .args(args)
            .output()
            .unwrap()
    }
}
impl Drop for Target {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
#[test]
fn buffered_multithread_fixed_pool() {
    let t = Target::new();
    let out = t.run(&[
        "--backend",
        "buffered",
        "-c",
        "ws1k4q8j4x1",
        "-v",
        "-K",
        "0,256",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("hot_path_allocations=0"));
    assert_ne!(fs::read(&t.0).unwrap(), vec![0x55; 1048576]);
}
#[test]
fn memory_limit_rejects_before_write() {
    let t = Target::new();
    let out = t.run(&[
        "--backend",
        "buffered",
        "-c",
        "ws1k4q128j64x1",
        "--memory",
        "1MiB",
    ]);
    assert!(!out.status.success());
    assert_eq!(fs::read(&t.0).unwrap(), vec![0x55; 1048576]);
}
#[test]
fn invalid_precondition_rejects_before_write() {
    let t = Target::new();
    let out = t.run(&[
        "--backend",
        "buffered",
        "-G",
        "1MiB",
        "-p",
        "M2",
        "-c",
        "rP1X1",
    ]);
    assert!(!out.status.success());
    assert_eq!(fs::read(&t.0).unwrap(), vec![0x55; 1048576]);
}
#[test]
fn pass_overflow_rejected() {
    let t = Target::new();
    let out = t.run(&["--backend", "buffered", "-c", "wP2X18446744073709551615"]);
    assert!(!out.status.success());
    assert_eq!(fs::read(&t.0).unwrap(), vec![0x55; 1048576]);
}
#[cfg(target_os = "linux")]
#[test]
fn direct_and_libaio_fixed_pools() {
    for backend in ["direct", "libaio"] {
        let t = Target::new();
        let out = t.run(&[
            "--backend",
            backend,
            "-c",
            "ws1k4q8j4x1",
            "-v",
            "-K",
            "0,256",
        ]);
        assert!(
            out.status.success(),
            "{backend}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(String::from_utf8_lossy(&out.stdout).contains("hot_path_allocations=0"));
    }
}

#[cfg(target_os = "linux")]
#[test]
fn async_streamed_writes_match_verified_buffered_data() {
    let mut backends = vec!["libaio"];
    let help = Command::new(env!("CARGO_BIN_EXE_spit"))
        .arg("--help")
        .output()
        .unwrap();
    if String::from_utf8_lossy(&help.stdout).contains("Default backend on this system: io_uring") {
        backends.push("io_uring");
    }
    for command in ["ws1k4q32j4x1", "ws0k4-16q32j4x1", "ws1k4q2-32j4x1"] {
        let expected = Target::new();
        let reference = expected.run(&["--backend", "buffered", "-c", command, "-R", "17", "-v"]);
        assert!(
            reference.status.success(),
            "{}",
            String::from_utf8_lossy(&reference.stderr)
        );
        for backend in &backends {
            let actual = Target::new();
            let streamed = actual.run(&["--backend", backend, "-c", command, "-R", "17"]);
            assert!(
                streamed.status.success(),
                "{}",
                String::from_utf8_lossy(&streamed.stderr)
            );
            assert!(String::from_utf8_lossy(&streamed.stdout).contains("hot_path_allocations=0"));
            let actual_data = fs::read(&actual.0).unwrap();
            let expected_data = fs::read(&expected.0).unwrap();
            let mismatch = actual_data
                .iter()
                .zip(&expected_data)
                .position(|(a, b)| a != b)
                .or_else(|| {
                    (actual_data.len() != expected_data.len())
                        .then_some(actual_data.len().min(expected_data.len()))
                });
            assert!(
                mismatch.is_none(),
                "{backend}: {command}: first differing byte {mismatch:?}"
            );
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn libaio_uses_requested_queue_depth() {
    let target = Target::new();
    let shim = Target::new();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = if root.join("../c/tests/aio_faults.c").is_file() {
        root.join("../c/tests/aio_faults.c")
    } else {
        root.join("tests/compat/aio_faults.c")
    };
    let compile = Command::new("cc")
        .args(["-shared", "-fPIC", "-O2"])
        .arg(source)
        .arg("-ldl")
        .arg("-o")
        .arg(&shim.0)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    for (command, submitted) in [("rs1k4q32P32X1", 32), ("rs1k4q2-32P32X1", 2)] {
        let out = Command::new(env!("CARGO_BIN_EXE_spit"))
            .arg("-f")
            .arg(&target.0)
            .args(["--backend", "libaio", "-c", command, "-K", "32,0"])
            .env("LD_PRELOAD", &shim.0)
            .env("SPIT_AIO_FAULT", "depth")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("MAX_PENDING=32"),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            String::from_utf8_lossy(&out.stderr).contains(&format!("MAX_SUBMIT={submitted}")),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(String::from_utf8_lossy(&out.stdout).contains("hot_path_allocations=0"));
    }
    let out = Command::new(env!("CARGO_BIN_EXE_spit"))
        .arg("-f")
        .arg(&target.0)
        .args(["--backend", "libaio", "-c", "rws0k4q32P32X1"])
        .env("LD_PRELOAD", &shim.0)
        .env("SPIT_AIO_FAULT", "depth")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("MAX_PENDING=32"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("MIXED_SUBMITS=0"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = Command::new(env!("CARGO_BIN_EXE_spit"))
        .arg("-f")
        .arg(&target.0)
        .args(["--backend", "libaio", "-c", "rs1k4q32P32X2", "-K", "64,0"])
        .env("LD_PRELOAD", &shim.0)
        .env("SPIT_AIO_FAULT", "refill")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("REFILLED_EARLY=1"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("hot_path_allocations=0"));
    let out = Command::new(env!("CARGO_BIN_EXE_spit"))
        .arg("-f")
        .arg(&target.0)
        .args(["--backend", "libaio", "-c", "ws1k4q8P8Y1m1", "-K", "0,1"])
        .env("LD_PRELOAD", &shim.0)
        .env("SPIT_AIO_FAULT", "corrupt")
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "the final pending metadata read must detect corruption"
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("verification mismatch"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let untouched = Target::new();
    let out = Command::new(env!("CARGO_BIN_EXE_spit"))
        .arg("-f")
        .arg(&untouched.0)
        .args(["--backend", "libaio", "-c", "ws1k4q8j4x1"])
        .env("LD_PRELOAD", &shim.0)
        .env("SPIT_AIO_FAULT", "setup_once")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(
        fs::read(&untouched.0)
            .unwrap()
            .iter()
            .all(|byte| *byte == 0x55),
        "a worker setup failure must abort every worker before writes"
    );
}

#[test]
fn help_reports_actual_default() {
    let help = Command::new(env!("CARGO_BIN_EXE_spit"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).unwrap();
    let default = help
        .lines()
        .find_map(|l| l.strip_prefix("Default backend on this system: "))
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap();
    let target = Target::new();
    let out = target.run(&["-c", "rs1k4q1P1X1"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains(&format!("backend={default} ")));
    let explicit = target.run(&["--backend", "buffered", "-c", "rs1k4q1P1X1"]);
    assert!(explicit.status.success());
    assert!(String::from_utf8_lossy(&explicit.stdout).contains("backend=buffered "));
}

#[test]
fn c_attached_options_and_zero_range() {
    let t = Target::new();
    let out = t.run(&[
        "--backend",
        "buffered",
        "-G0-1MiB",
        "-cws1k4q8P8X1",
        "-R1",
        "-K0,8",
        "-VV",
        "-v",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
#[test]
fn c_workload_whitespace() {
    let t = Target::new();
    let out = t.run(&[
        "--backend",
        "buffered",
        "-c",
        "r s1 k4 q8 P8 X1",
        "-K",
        "8,0",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn c_position_modes_and_variable_blocks() {
    for command in [
        "rs1k4P-10X2",
        "rs1k4P+10X2",
        "rs1k4P.10X2",
        "rs0k4-16P10X2",
        "rs0k4:16P10X2",
        "rs-1k4P10X2",
        "rs2k4P10X2",
        "rs1k4P10X2n",
        "rs1k4P10X2N",
    ] {
        let t = Target::new();
        let out = t.run(&["--backend", "buffered", "-c", command, "-K", "20,0"]);
        assert!(
            out.status.success(),
            "{command}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(String::from_utf8_lossy(&out.stdout).contains("hot_path_allocations=0"));
    }
}
#[test]
fn variable_writes_preserve_range() {
    for command in ["ws1k4-16q8P20X2", "ws0k4:64q8P20X2"] {
        let t = Target::new();
        let out = t.run(&[
            "--backend",
            "buffered",
            "-G",
            "256KiB-768KiB",
            "-c",
            command,
            "-v",
            "-K",
            "0,40",
        ]);
        assert!(
            out.status.success(),
            "{command}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let data = fs::read(&t.0).unwrap();
        assert!(data[..262144]
            .iter()
            .chain(data[786432..].iter())
            .all(|b| *b == 0x55));
    }
}

#[test]
fn c_queue_ranges_operation_limit_and_alternating_passes() {
    for (command, expected) in [
        ("rs1k4q2-8P20X1", "20,0"),
        ("rs1k4q1P20Y3", "3,0"),
        ("ws1k4q8P10X2=", "10,10"),
        ("rs1k4P5O", "5,0"),
    ] {
        let t = Target::new();
        let out = t.run(&["--backend", "buffered", "-c", command, "-K", expected]);
        assert!(
            out.status.success(),
            "{command}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn metadata_and_unique_modes_match_c_operation_counts() {
    for (command, expected) in [
        ("ws1k4q8P8X1m1", "8,8"),
        ("ws1k4q8P8X1m8", "8,8"),
        ("rs1k4q8P8X1m1", "16,0"),
        ("ws1k4q8P8X1U", "8,8"),
        ("rs1k4q8P8X1U", "16,0"),
        ("ws0k4-16q8P32X2m8nN", "64,64"),
        ("ws0k4-16q8P32X2UnN", "64,64"),
    ] {
        let target = Target::new();
        let out = target.run(&["--backend", "buffered", "-c", command, "-K", expected]);
        assert!(
            out.status.success(),
            "{command}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn metadata_finishes_verification_after_workload_limit() {
    for command in ["ws1k4q8P8Y1m1", "ws1k4q8P8Y1U"] {
        let target = Target::new();
        let out = target.run(&["--backend", "buffered", "-c", command, "-K", "0,1"]);
        assert!(
            out.status.success(),
            "{command}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(String::from_utf8_lossy(&out.stdout).contains("verified_ops=1"));
    }
}

#[test]
fn unique_mode_uses_a_different_seed_for_each_position() {
    let target = Target::new();
    let out = target.run(&[
        "--backend",
        "buffered",
        "-c",
        "ws1k4q8P8X1uz",
        "-R",
        "17",
        "-K",
        "0,8",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let data = fs::read(&target.0).unwrap();
    for index in 0..8usize {
        let address = index as u64 * 4096;
        let tag = address ^ (17 + index as u64).wrapping_mul(0x9e3779b97f4a7c15);
        assert_eq!(&data[index * 4096..index * 4096 + 8], &tag.to_le_bytes());
    }
    assert!(data[8 * 4096..].iter().all(|byte| *byte == 0x55));
}
#[test]
fn c_rate_limit_and_time_controls() {
    let t = Target::new();
    let start = std::time::Instant::now();
    let out = t.run(&["--backend", "buffered", "-c", "rs1k4q8P5X1S20"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(start.elapsed().as_secs_f64() >= 0.18);
    let out = t.run(&["--backend", "buffered", "-c", "rs1k4q1T0.05W0.01:0.01"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn prefix_creates_one_file_per_worker() {
    let t = Target::new();
    let out = Command::new(env!("CARGO_BIN_EXE_spit"))
        .arg("-F")
        .arg(&t.0)
        .args([
            "--backend",
            "buffered",
            "-G",
            "1MiB",
            "-c",
            "ws1k4q8j2x1",
            "-K",
            "0,256",
        ])
        .output()
        .unwrap();
    let a = fs::read(format!("{}.0001", t.0.display()));
    let b = fs::read(format!("{}.0002", t.0.display()));
    let _ = fs::remove_file(format!("{}.0001", t.0.display()));
    let _ = fs::remove_file(format!("{}.0002", t.0.display()));
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(a.len(), 1048576);
    assert_eq!(b.len(), 1048576);
    assert!(a[524288..].iter().all(|b| *b == 0));
    assert!(b[..524288].iter().all(|b| *b == 0));
}
#[test]
fn command_range_preserves_surroundings() {
    let t = Target::new();
    let out = t.run(&[
        "--backend",
        "buffered",
        "-c",
        "ws1k4q8b262144-786432x1",
        "-K",
        "0,128",
        "-v",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let data = fs::read(&t.0).unwrap();
    assert!(data[..262144]
        .iter()
        .chain(data[786432..].iter())
        .all(|b| *b == 0x55));
}

#[test]
fn gib_range_and_shared_worker_split_preserve_surroundings() {
    let target = Target::new();
    let out = target.run(&[
        "--backend",
        "buffered",
        "-c",
        "ws1k4q8J2G0.000244140625_0.000732421875x1",
        "-K",
        "0,128",
        "-v",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let data = fs::read(&target.0).unwrap();
    assert!(data[..262144]
        .iter()
        .chain(data[786432..].iter())
        .all(|b| *b == 0x55));
}

#[test]
fn bare_g_uses_the_target_range() {
    let target = Target::new();
    let out = target.run(&["--backend", "buffered", "-c", "ws1UG_j2P8X1", "-K", "16,16"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = target.run(&[
        "--backend",
        "buffered",
        "-p",
        "Gs1k4",
        "-c",
        "rs1k4P1X1",
        "-K",
        "1,0",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn missing_prefix_read_job_rejects_before_any_creation() {
    let target = Target::new();
    let out = Command::new(env!("CARGO_BIN_EXE_spit"))
        .arg("-F")
        .arg(&target.0)
        .args([
            "--backend",
            "buffered",
            "-G",
            "1MiB",
            "-c",
            "ws1k4x1",
            "-c",
            "rs1k4x1",
        ])
        .output()
        .unwrap();
    let first = std::path::PathBuf::from(format!("{}.0001", target.0.display()));
    let second = std::path::PathBuf::from(format!("{}.0002", target.0.display()));
    let created = first.exists() || second.exists();
    let _ = fs::remove_file(first);
    let _ = fs::remove_file(second);
    assert!(!out.status.success());
    assert!(
        !created,
        "preflight created a target before discovering the missing read file"
    );
}
