"""Verify build-time Git version refresh in an isolated temporary repository."""
import pathlib
import subprocess
import tempfile

source = pathlib.Path(__file__).resolve().parents[1]
script = source / 'spit' / 'GenerateVersion.cmake'
with tempfile.TemporaryDirectory(prefix='spit-version-test-') as temp:
    root = pathlib.Path(temp)
    def run(*args):
        return subprocess.check_output(args, cwd=root, text=True, stderr=subprocess.STDOUT)
    run('git', 'init', '-q')
    run('git', 'config', 'user.name', 'Version Test')
    run('git', 'config', 'user.email', 'version-test@example.invalid')
    (root / 'CMakeLists.txt').write_text(f'''cmake_minimum_required(VERSION 3.20)
project(version_test NONE)
add_custom_target(version ALL COMMAND ${{CMAKE_COMMAND}}
    -DSOURCE_DIR=${{CMAKE_CURRENT_SOURCE_DIR}}
    -DOUTPUT_FILE=${{CMAKE_CURRENT_BINARY_DIR}}/spit-version.h
    -P "{script}" VERBATIM)
''')
    (root / '.gitignore').write_text('build/\n')
    run('git', 'add', '.')
    run('git', 'commit', '-qm', 'Initial test fixture')
    run('cmake', '-S', '.', '-B', 'build')
    run('cmake', '--build', 'build')
    header = root / 'build' / 'spit-version.h'
    first = header.read_text()
    stamp = header.stat().st_mtime_ns
    run('cmake', '--build', 'build')
    assert header.stat().st_mtime_ns == stamp, 'Unchanged version rewrote header'
    run('git', 'commit', '--allow-empty', '-qm', 'Advance fixture revision')
    run('cmake', '--build', 'build')
    assert header.read_text() != first, 'Build did not refresh version after commit'
    assert run('git', 'rev-parse', '--short', 'HEAD').strip() in header.read_text()
    with (root / '.gitignore').open('a') as f:
        f.write('ignored\n')
    run('cmake', '--build', 'build')
    assert '-dirty' in header.read_text(), 'Modified source not identified'
    print('PASS version refresh, stable timestamps, and dirty marker')
