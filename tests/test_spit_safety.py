"""Run with python3 tests/test_spit_safety.py [path/to/spit]. Uses only temporary files."""
import os
import resource
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

BINARY = Path(sys.argv.pop(1) if len(sys.argv) > 1 else 'build/spit/spit').resolve()

class SafetyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
        cls.artifacts = tempfile.TemporaryDirectory(prefix='spit-faults-')
        cls.shim = Path(cls.artifacts.name) / 'faults.so'
        subprocess.run(['cc', '-shared', '-fPIC', str(Path(__file__).with_name('aio_faults.c')),
                        '-o', str(cls.shim), '-ldl'], check=True)

    @classmethod
    def tearDownClass(cls):
        cls.artifacts.cleanup()

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='spit-safety-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.target = self.root / 'target'
        self.original = b'A' * (1024 * 1024)
        self.target.write_bytes(self.original)

    def run_spit(self, *args, fault=None, timeout=15):
        env = os.environ.copy()
        if fault:
            env.update(LD_PRELOAD=str(self.shim), SPIT_AIO_FAULT=fault)
        return subprocess.run([str(BINARY), *args], cwd=self.root, env=env,
                              stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=timeout)

    def test_read_preserves_existing_file(self):
        for size in ([], ['-G', '512KiB'], ['-G', '1MiB'], ['-G', '2MiB']):
            with self.subTest(size=size):
                r = self.run_spit('-f', str(self.target), '-c', 'rs1k4q1P1X1', *size)
                self.assertEqual(r.returncode, 1 if size == ['-G', '2MiB'] else 0, r.stdout)
                self.assertEqual(self.target.read_bytes(), self.original)

    def test_read_recreate_does_not_destroy(self):
        r = self.run_spit('--recreate', '-f', str(self.target), '-G', '2MiB', '-c', 'rP1X1')
        self.assertNotEqual(r.returncode, 0, r.stdout)
        self.assertEqual(self.target.read_bytes(), self.original)

    def test_missing_read_target_not_created(self):
        target = self.root / 'missing'
        r = self.run_spit('-f', str(target), '-G', '1MiB', '-c', 'rP1X1')
        self.assertNotEqual(r.returncode, 0, r.stdout)
        self.assertFalse(target.exists())

    def test_write_resize_requires_opt_in(self):
        args = ['-f', str(self.target), '-G', '2MiB', '-c', 'ws1k4q1P1X1']
        r = self.run_spit(*args)
        self.assertNotEqual(r.returncode, 0, r.stdout)
        self.assertEqual(self.target.read_bytes(), self.original)
        r = self.run_spit('--recreate', *args)
        self.assertEqual(r.returncode, 0, r.stdout)
        self.assertEqual(self.target.stat().st_size, 2*1024*1024)

    def test_prefix_write_preserves_tail(self):
        target = self.root / 'prefix.0001'
        target.write_bytes(self.original)
        r = self.run_spit('-F', str(self.root / 'prefix'), '-G', '512KiB', '-c', 'ws1k4q1P1X1')
        self.assertEqual(r.returncode, 0, r.stdout)
        self.assertEqual(target.stat().st_size, len(self.original))
        self.assertNotEqual(target.read_bytes(), self.original)
        self.assertEqual(target.read_bytes()[512*1024:], self.original[512*1024:])

    def test_new_write_target_and_verification(self):
        target = self.root / 'new'
        r = self.run_spit('-f', str(target), '-G', '1MiB', '-c', 'ws1k4q1P8X1', '-v')
        self.assertEqual(r.returncode, 0, r.stdout)
        self.assertEqual(target.stat().st_size, len(self.original))

    def test_probability_read_only(self):
        r = self.run_spit('--recreate', '-f', str(self.target), '-G', '2MiB', '-c', 'p1P1X1')
        self.assertNotEqual(r.returncode, 0, r.stdout)
        self.assertEqual(self.target.read_bytes(), self.original)

    def test_short_and_failed_io_in_both_completion_paths(self):
        for fault in ('short', 'error'):
            for action in ('r', 'w'):
                for limit in (['-t', '0.02'], []):
                    with self.subTest(fault=fault, action=action, limit=limit):
                        cmd = action + 's1k4q1' + ('' if limit else 'P1X1')
                        r = self.run_spit('-f', str(self.target), '-G', '1MiB', '-c', cmd, *limit, fault=fault)
                        self.assertNotEqual(r.returncode, 0, r.stdout)
                        self.assertIn(b'incomplete', r.stdout)

    def test_eof_is_error(self):
        r = self.run_spit('-f', '/dev/null', '-G', '1MiB', '-c', 'rs1k4q1P1X1')
        self.assertNotEqual(r.returncode, 0, r.stdout)
        self.assertIn(b'result 0', r.stdout)

    def test_small_sample_diagnostics(self):
        r = self.run_spit('-f', str(self.target), '-G', '1MiB', '-c', 'rs1k4q1P1X1')
        self.assertEqual(r.returncode, 0, r.stdout)
        self.assertNotRegex(r.stdout, rb'\bnan\b')
        self.assertIn(b'n/a (insufficient samples)', r.stdout)
        self.assertIn(b'write throughput: no samples', r.stdout)
        self.assertRegex(r.stdout, rb'using [0-9]{1,2}\.[0-9]{3} GiB RAM for positions')

    def test_metadata_corruption_fails(self):
        for fault in ('corrupt', 'corrupt_last'):
            with self.subTest(fault=fault):
                r = self.run_spit('-f', str(self.target), '-G', '1MiB',
                                  '-c', 'wm1s1k4q1P2X1', fault=fault)
                self.assertEqual(r.returncode, 1, r.stdout)
                self.assertIn(b'verification mismatch', r.stdout)

    def test_metadata_without_corruption_succeeds(self):
        r = self.run_spit('-f', str(self.target), '-G', '1MiB', '-c', 'wm1s1k4q1P2X1')
        self.assertEqual(r.returncode, 0, r.stdout)

    def test_long_commands_rejected_before_writes(self):
        for commands in (['r'*5000], ['r'*4094], ['r'*1500]*3):
            with self.subTest(lengths=[len(c) for c in commands]):
                args = [arg for c in commands for arg in ('-c', c)]
                r = self.run_spit('-f', str(self.target), '-G', '1MiB', '-t', '0.02',
                                  *args, timeout=3)
                self.assertEqual(r.returncode, 1, r.stdout)
                self.assertIn(b'command too long', r.stdout)
                self.assertEqual(self.target.read_bytes(), self.original)

    def test_failed_submission_retries_without_hanging(self):
        for action in ('r', 'w'):
            with self.subTest(action=action):
                try:
                    r = self.run_spit('-f', str(self.target), '-G', '1MiB',
                                      '-c', action+'s1k4q1P1X1', '-E', '2',
                                      fault='submit_once', timeout=3)
                except subprocess.TimeoutExpired:
                    self.fail('One failed submission left the job stuck')
                self.assertEqual(r.returncode, 1, r.stdout)
                self.assertIn(b'Input/output error', r.stdout)
                self.assertNotIn(b'inflight!', r.stdout)
                if action == 'r':
                    self.assertEqual(self.target.read_bytes(), self.original)
                else:
                    self.assertNotEqual(self.target.read_bytes(), self.original)

    def test_pending_io_cleanup(self):
        for fault in ('delay', 'destroy_fail'):
            with self.subTest(fault=fault):
                r = self.run_spit('-f', str(self.target), '-G', '1MiB', '-c', 'rs1k4q1P1X1', fault=fault)
                self.assertNotEqual(r.returncode, 0, r.stdout)
                self.assertNotIn(b'FREED_PENDING_IO', r.stdout)
                self.assertIn(b'timed out', r.stdout)
                if fault == 'destroy_fail':
                    self.assertIn(b'buffers retained', r.stdout)

if __name__ == '__main__':
    unittest.main()
