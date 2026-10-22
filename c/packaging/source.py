#!/usr/bin/env python3
"""Create a clean source tarball including current, uncommitted source changes."""
import gzip
import io
import os
from pathlib import Path
import subprocess
import tarfile

root = Path(__file__).resolve().parents[1]
out = root / 'dist'
out.mkdir(exist_ok=True)
version = (root / 'VERSION').read_text().strip()
revision = subprocess.check_output(['git', 'describe', '--tags', '--always', '--dirty'], cwd=root, text=True).strip() if ((root / '.git').exists() or (root.parent / '.git').exists()) else (root / 'BUILD_VERSION').read_text().strip()
epoch = int(os.environ.get('SOURCE_DATE_EPOCH',
    (root / 'SOURCE_DATE_EPOCH').read_text().strip() if (root / 'SOURCE_DATE_EPOCH').is_file()
    else subprocess.check_output(['git', 'log', '-1', '--format=%ct'], cwd=root, text=True).strip()
    if ((root / '.git').exists() or (root.parent / '.git').exists())
    else str(int((root / 'BUILD_VERSION').stat().st_mtime))))
if ((root / '.git').exists() or (root.parent / '.git').exists()):
    names = subprocess.check_output(['git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard'], cwd=root).decode().split('\0')
else:
    names = [str(p.relative_to(root)) for p in root.rglob('*') if p.is_file() and not any(x in ('build', 'dist', '.git') for x in p.relative_to(root).parts)]
archive = out / ('stutools-' + version + '.tar.gz')
with archive.open('wb') as raw, gzip.GzipFile(filename='', mode='wb', fileobj=raw, mtime=epoch) as gz, tarfile.open(fileobj=gz, mode='w') as tar:
    prefix = 'stutools-' + version
    for name in sorted(set(names)):
        path = root / name
        if not name or not path.is_file() or name in ('BUILD_VERSION', 'SOURCE_DATE_EPOCH') or name.startswith(('build/', 'dist/')):
            continue
        if path.read_bytes()[:4] in (b'\x7fELF',):
            continue
        info = tar.gettarinfo(str(path), prefix + '/' + name)
        info.uid = info.gid = 0
        info.uname = info.gname = 'root'
        info.mtime = epoch
        with path.open('rb') as f:
            tar.addfile(info, f)
    for name, value in [('BUILD_VERSION', revision), ('SOURCE_DATE_EPOCH', str(epoch))]:
        data = (value + '\n').encode()
        info = tarfile.TarInfo(prefix + '/' + name)
        info.size, info.mtime, info.mode = len(data), epoch, 0o644
        tar.addfile(info, io.BytesIO(data))
print(archive)
