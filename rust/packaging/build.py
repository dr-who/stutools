#!/usr/bin/env python3
"""Package an already built/tested binary. Source RPM builds use stutools.spec."""
import argparse,gzip,os,pathlib,platform,shutil,subprocess,sys,tarfile,tempfile,re,zipfile
root=pathlib.Path(__file__).resolve().parents[1]
version=re.search(r'^version = "([^"]+)"', (root/'Cargo.toml').read_text(), re.M)[1]
out=root/'dist';out.mkdir(exist_ok=True)
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('format',nargs='?',default='auto',choices=['auto','deb','rpm','pkg','tar','zip'])
parser.add_argument('--binary',type=pathlib.Path,help='package an already tested executable')
parser.add_argument('--target',choices=['macos-arm64','macos-x86_64','macos-universal','windows-x86_64','windows-arm64','linux-x86_64','linux-arm64'])
options=parser.parse_args()
kind=options.format
if options.target and kind not in ['tar','zip','pkg']:parser.error('--target requires tar, zip or pkg')
if kind=='auto':
 if sys.platform=='darwin':kind='pkg'
 elif sys.platform=='win32':kind='zip'
 else:
  release=pathlib.Path('/etc/os-release')
  ids=set()
  if release.exists():
   for line in release.read_text().splitlines():
    key,separator,value=line.partition('=')
    if separator and key in ['ID','ID_LIKE']:ids.update(value.strip('"').split())
  if ids.intersection(['rhel','fedora','centos','rocky','almalinux']):kind='rpm'
  elif ids.intersection(['debian','ubuntu','linuxmint']):kind='deb'
  elif shutil.which('dpkg-buildpackage'):kind='deb'
  elif shutil.which('rpmbuild'):kind='rpm'
  else:raise SystemExit('Install Debian or RPM package tools, or request tar/zip.')
binary=options.binary or root/'target'/'release'/('spit.exe' if sys.platform=='win32' else 'spit')
if not binary.is_file():raise SystemExit('Run cargo build --release --locked first.')
license=root.parent/'LICENSE' if (root.parent/'LICENSE').is_file() else root/'LICENSE'
with tempfile.TemporaryDirectory(prefix='stutools-package-') as temp:
 temp=pathlib.Path(temp)
 if kind=='deb':
  # Use debhelper to generate dependencies, checksums, docs and the man page.
  subprocess.run([sys.executable,str(root/'packaging/source.py')],check=True)
  archive=out/f'stutools-{version}.tar.gz'
  shutil.copy2(archive,temp/f'stutools_{version}.orig.tar.gz')
  subprocess.run(['tar','-xzf',str(archive),'-C',str(temp)],check=True)
  source=temp/f'stutools-{version}'
  suffix=os.environ.get('STUTOOLS_DEB_SUFFIX','')
  if suffix:
   if not re.fullmatch(r'[a-z0-9.]+',suffix):raise SystemExit('Invalid Debian revision suffix')
   changelog=source/'debian/changelog'
   changelog.write_text(changelog.read_text().replace(f'({version}-1)',f'({version}-1+{suffix})',1))
  command=['dpkg-buildpackage','-us','-uc','-b']
  if os.environ.get('STUTOOLS_EXTERNAL_RUST')=='1':
   for tool in ['rustc','cargo']:
    text=subprocess.check_output([tool,'--version'],text=True)
    match=re.search(r'\b(\d+)\.(\d+)\.(\d+)',text)
    if not match or tuple(map(int,match.groups()))<(1,85,0):raise SystemExit(f'{tool} >= 1.85 is required')
   command+=['-Ppkg.stutools.external-rust']
  subprocess.run(command,cwd=source,check=True)
  for path in temp.iterdir():
   if path.suffix in ['.deb','.changes','.buildinfo']:shutil.copy2(path,out/path.name)
 elif kind=='rpm':
  for name in ['BUILD','BUILDROOT','RPMS','SOURCES','SPECS','SRPMS']:(temp/name).mkdir()
  # The native binary path does not need to vendor/download crates again.
  source=temp/f'stutools-{version}';source.mkdir()
  shutil.copy2(license,source/'LICENSE');shutil.copy2(root/'README.md',source/'README.md');shutil.copy2(root/'THIRD_PARTY_LICENSES.txt',source/'THIRD_PARTY_LICENSES.txt')
  shutil.copy2(root/'packaging/spit.1',source/'spit.1')
  shutil.copy2(root/'INSTALL.md',source/'INSTALL.md')
  with tarfile.open(temp/'SOURCES'/f'stutools-{version}.tar.gz','w:gz') as tar:tar.add(source,arcname=source.name)
  shutil.copy2(binary,temp/'SOURCES/spit')
  subprocess.run(['rpmbuild','-bb','--define',f'_topdir {temp}','--define','prebuilt 1',str(root/'packaging/stutools.spec')],check=True)
  for rpm in (temp/'RPMS').rglob('*.rpm'):shutil.copy2(rpm,out/rpm.name)
 elif kind=='pkg':
  if sys.platform!='darwin' or not shutil.which('pkgbuild'):raise SystemExit('Build .pkg installers on macOS with pkgbuild.')
  payload=temp/'payload'
  for directory in ['usr/local/bin','usr/local/share/man/man1','usr/local/share/doc/stutools']:(payload/directory).mkdir(parents=True,exist_ok=True)
  shutil.copy2(binary,payload/'usr/local/bin/spit')
  (payload/'usr/local/bin/spit').chmod(0o755)
  shutil.copy2(root/'packaging/spit.1',payload/'usr/local/share/man/man1/spit.1')
  for path in [license,root/'README.md',root/'INSTALL.md',root/'THIRD_PARTY_LICENSES.txt']:shutil.copy2(path,payload/'usr/local/share/doc/stutools'/path.name)
  target=options.target or 'macos-'+{'aarch64':'arm64'}.get(platform.machine(),platform.machine())
  if not target.startswith('macos-'):parser.error('pkg requires a macOS target')
  package=out/f'spit-{version}-{target}.pkg'
  subprocess.run(['pkgbuild','--root',str(payload),'--identifier','org.stutools.spit','--version',version,'--install-location','/','--ownership','recommended',str(package)],check=True)
 elif kind in ['tar','zip']:
  system={'Darwin':'macos','Windows':'windows'}.get(platform.system(),platform.system().lower())
  arch={'AMD64':'x86_64','aarch64':'arm64'}.get(platform.machine(),platform.machine())
  if options.target:system,arch=options.target.split('-',1)
  name=f'spit-{version}-{system}-{arch}'
  if kind=='zip':
   with zipfile.ZipFile(out/f'{name}.zip','w',compression=zipfile.ZIP_DEFLATED) as z:
    for path in [binary,license,root/'README.md',root/'INSTALL.md',root/'THIRD_PARTY_LICENSES.txt',root/'packaging/spit.1']:z.write(path,arcname=f'{name}/{path.name}')
  else:
   with tarfile.open(out/f'{name}.tar.gz','w:gz') as tar:
    for path in [binary,license,root/'README.md',root/'INSTALL.md',root/'THIRD_PARTY_LICENSES.txt',root/'packaging/spit.1']:tar.add(path,arcname=f'{name}/{path.name}')
 else:raise SystemExit(f'Unknown format: {kind}')
print(out)
