"""Regression specifications from the deeper review; several currently fail.

Run: python3 tests/test_spit_deep.py build/spit/spit [TestClass.test_name ...]
Uses temporary regular files only. The corruption cases simulate bad read data.
For ASan/UBSan, select the position-loading tests (no preload needed).
"""
import os
from pathlib import Path
import resource
import subprocess
import sys
import tempfile
import unittest

BINARY = Path(sys.argv.pop(1) if len(sys.argv) > 1 else 'build/spit/spit').resolve()


class DeepTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
        cls.artifacts = tempfile.TemporaryDirectory(prefix='spit-deep-shim-')
        cls.shim = Path(cls.artifacts.name) / 'faults.so'
        subprocess.run(['cc', '-shared', '-fPIC', str(Path(__file__).with_name('aio_faults.c')),
                        '-o', str(cls.shim), '-ldl'], check=True)

    @classmethod
    def tearDownClass(cls):
        cls.artifacts.cleanup()

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='spit-deep-test-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.target = self.root / 'data'
        self.original = b'A' * (1024 * 1024)
        self.target.write_bytes(self.original)

    def run_spit(self, *args, corrupt=False):
        env = dict(os.environ)
        if corrupt:
            env.update(LD_PRELOAD=str(self.shim), SPIT_AIO_FAULT='corrupt')
        try:
            return subprocess.run([str(BINARY), '-f', str(self.target), '-G', '1MiB', *args],
                                  cwd=self.root, env=env, stdout=subprocess.PIPE,
                                  stderr=subprocess.STDOUT, timeout=3)
        except subprocess.TimeoutExpired as error:
            self.fail('Job failed to terminate within 3 seconds: ' +
                      (error.stdout or b'')[-1000:].decode(errors='replace'))

    def load_positions(self, lines, command):
        positions = self.root / 'positions.txt'
        positions.write_text(lines)
        result = self.run_spit('-S', str(positions), '-c', command)
        self.assertEqual(result.returncode, 0, result.stdout[-3000:])
        self.assertEqual(self.target.read_bytes(), self.original)

    def test_loaded_positions_single_worker_control(self):
        self.load_positions('0 R 4096\n4096 R 4096\n', 'rs1k4q1P2X1')

    def test_loaded_positions_parallel_workers(self):
        self.load_positions('0 R 4096\n4096 R 4096\n', 'rs1k4q1j2P2X1')

    def test_loaded_positions_grow_allocation(self):
        # Initial P1 allocation cannot hold 300 imported records.
        self.load_positions('0 R 4096\n' * 300, 'rs1k4q1P1X1')

    def test_loaded_block_exactly_fills_range(self):
        self.load_positions('0 R 1048576\n', 'rs1k4q1P1X1')

    def test_single_position_verification_control(self):
        for command in ('wm1s1k4q1P1X1', 'wUs1k4q1P1X1'):
            with self.subTest(command=command):
                result = self.run_spit('-c', command)
                self.assertEqual(result.returncode, 0, result.stdout)

    def test_single_position_corruption_is_detected(self):
        for command in ('wm1s1k4q1P1X1', 'wUs1k4q1P1X1'):
            with self.subTest(command=command):
                result = self.run_spit('-c', command, corrupt=True)
                self.assertEqual(result.returncode, 1, result.stdout)
                self.assertIn(b'verification mismatch', result.stdout)

    def test_preconditioning_preserves_bytes_outside_range(self):
        result = self.run_spit('-G', '512KiB-1MiB', '-p', 'k4', '-c', 'rs1k4q1P1X1')
        self.assertEqual(result.returncode, 0, result.stdout)
        current = self.target.read_bytes()
        self.assertTrue(current[:524288] == self.original[:524288],
                        'Preconditioning overwrote bytes below the requested range')
        self.assertNotEqual(current[524288:], self.original[524288:])

    def test_result_check_control(self):
        result = self.run_spit('-c', 'rs1k4q1P1X1', '-K', '1,0')
        self.assertEqual(result.returncode, 0, result.stdout)

    def test_result_check_rejects_mismatch_in_release_build(self):
        result = self.run_spit('-c', 'rs1k4q1P1X1', '-K', '1234,0')
        self.assertGreater(result.returncode, 0, result.stdout)

    def test_nonfinite_duration_rejected(self):
        # X1 keeps the broken implementation finite while validating the parser.
        result = self.run_spit('-c', 'rs1k4q1P1X1', '-t', 'nan')
        self.assertGreater(result.returncode, 0, result.stdout)

    def test_negative_thread_count_rejected(self):
        result = self.run_spit('-c', 'rs1k4q1j-1P1X1')
        self.assertGreater(result.returncode, 0, result.stdout)


if __name__ == '__main__':
    unittest.main()
