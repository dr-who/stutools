#!/usr/bin/env python3
"""Temporarily enable host io_uring for tests; always restore its sysctl.
Requires explicit administrator approval. Does not install persistent settings.
"""
import os,pathlib,signal,subprocess,sys,tempfile
root=pathlib.Path(__file__).resolve().parents[2]
binary=root/'rust/target/release/spit'
key='kernel.io_uring_disabled'
original=subprocess.check_output(['sysctl','-n',key],text=True).strip()
def stop(*args):raise KeyboardInterrupt
signal.signal(signal.SIGTERM,stop)
try:
 subprocess.run(['sysctl','-w',key+'=0'],check=True)
 help_text=subprocess.check_output([str(binary),'--help'],text=True)
 assert 'Default backend on this system: io_uring (auto)' in help_text, help_text
 subprocess.run(['cargo','test','--release','--locked','--offline','--manifest-path',str(root/'rust/Cargo.toml')],check=True)
 with tempfile.TemporaryDirectory(prefix='spit-uring-test-') as temp:
  wrapper=pathlib.Path(temp)/'spit'
  wrapper.write_text('#!/usr/bin/env python3\nimport os,sys\nos.execv('+repr(str(binary))+', ['+repr(str(binary))+', "--backend", "io_uring", *sys.argv[1:]])\n')
  wrapper.chmod(0o755)
  methods=['test_read_preserves_existing_file','test_read_recreate_does_not_destroy','test_missing_read_target_not_created','test_write_resize_requires_opt_in','test_prefix_write_preserves_tail','test_new_write_target_and_verification','test_probability_read_only','test_eof_is_error','test_small_sample_diagnostics','test_metadata_without_corruption_succeeds','test_long_commands_rejected_before_writes']
  subprocess.run([sys.executable,str(root/'c/tests/test_spit_safety.py'),str(wrapper),*['SafetyTests.'+m for m in methods]],check=True)
  subprocess.run([sys.executable,str(root/'c/tests/test_spit_block.py'),str(wrapper)],check=True)
 subprocess.run([sys.executable,str(root/'rust/tests/ram_compare.py'),'--c',str(root/'c/build/spit/spit'),'--rust',str(binary),'--seconds','1','--repeats','3','--output','/tmp/spit-rust-all-backends.json',*sys.argv[1:]],check=True)
finally:
 subprocess.run(['sysctl','-w',key+'='+original],check=True)
 if original=='2':
  assert 'Default backend on this system: libaio (auto)' in subprocess.check_output([str(binary),'--help'],text=True)
