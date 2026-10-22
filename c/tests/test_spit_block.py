"""Opt-in Linux loop-device tests; requires permission to manage loop devices.

python3 tests/test_spit_block.py [path/to/spit]
Only a freshly created temporary backing file is attached and written.
"""
import pathlib
import subprocess
import sys
import tempfile

binary = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else 'build/spit/spit').resolve()
with tempfile.TemporaryDirectory(prefix='spit-block-regression-') as temp:
    root = pathlib.Path(temp)
    backing = root / 'disk.img'
    original = b'A' * (32 * 1024 * 1024)
    backing.write_bytes(original)
    dev = subprocess.check_output(['losetup', '--find', '--show', str(backing)], text=True).strip()
    try:
        cases = [
            ('64MiB-128MiB', False),
            ('32MiB-64MiB', False),
            ('16MiB-64MiB', False),
            ('8MiB-16MiB', True),
        ]
        for interval, valid in cases:
            with open(dev, 'wb', buffering=0) as target:
                target.write(original)
            result = subprocess.run([str(binary), '-f', dev, '-G', interval,
                                     '-c', 'ws1k4q8x1', '-R', '1', '-v'],
                                    cwd=root, capture_output=True, timeout=15)
            with open(dev, 'rb', buffering=0) as target:
                contents = target.read()
            if valid:
                assert result.returncode == 0, result.stderr.decode()
                lo, hi = 8*1024*1024, 16*1024*1024
                assert contents[:lo] == original[:lo]
                assert contents[hi:] == original[hi:]
                assert contents[lo:hi] != original[lo:hi]
                assert b'incorrect 0, ioerrors 0' in result.stderr
            else:
                assert result.returncode != 0, result.stderr.decode()
                assert b'refusing I/O' in result.stderr
                assert contents == original, 'Rejected range modified the device'
            print('PASS', interval, flush=True)
        result = subprocess.run([str(binary), '-f', dev, '-c', 'rs1k4q1P1X1'],
                                cwd=root, capture_output=True, timeout=15)
        assert result.returncode == 0, result.stderr.decode()
        with open(dev, 'rb', buffering=0) as target:
            assert target.read() == contents
        print('PASS default-size read preserves device', flush=True)
    finally:
        subprocess.run(['losetup', '-d', dev], check=True)
        print('Detached', dev, flush=True)
