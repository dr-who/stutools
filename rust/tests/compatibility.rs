// Run the C implementation's regression specifications without copying or
// weakening expectations. Source archives carry these under tests/compat/.
#[cfg(target_os = "linux")]
#[test]
fn c_regressions() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let tests = if root.join("../c/tests").is_dir() {
        root.join("../c/tests")
    } else {
        root.join("tests/compat")
    };
    for suite in ["test_spit_safety.py", "test_spit_deep.py"] {
        let out = std::process::Command::new("python3")
            .arg("-c")
            .arg(r#"import os,pathlib,subprocess,sys,tempfile
with tempfile.TemporaryDirectory(prefix='spit-libaio-compat-') as temp:
    wrapper=pathlib.Path(temp)/'spit'
    wrapper.write_text('#!/usr/bin/env python3\nimport os,sys\nos.execv('+repr(sys.argv[1])+', ['+repr(sys.argv[1])+', "--backend", "libaio", *sys.argv[1:]])\n')
    wrapper.chmod(0o755)
    sys.exit(subprocess.call([sys.executable,sys.argv[2],str(wrapper)]))
"#)
            .arg(env!("CARGO_BIN_EXE_spit"))
            .arg(tests.join(suite))
            .output()
            .expect("python3 is required for the shared C regression tests");
        assert!(
            out.status.success(),
            "{suite}: {}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
