#!/usr/bin/env python3
"""Create an offline-buildable Rust source archive with locked, vendored crates."""
import gzip,os,pathlib,shutil,subprocess,tarfile,tempfile,re
root=pathlib.Path(__file__).resolve().parents[1]
version=re.search(r'^version = "([^"]+)"', (root/'Cargo.toml').read_text(), re.M)[1]
out=root/'dist';out.mkdir(exist_ok=True)
epoch=os.environ.get('SOURCE_DATE_EPOCH')
if epoch is None:
 if (root/'SOURCE_DATE_EPOCH').is_file():epoch=(root/'SOURCE_DATE_EPOCH').read_text().strip()
 else:
  revision=subprocess.run(['git','log','-1','--format=%ct'],cwd=root,text=True,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL)
  epoch=revision.stdout.strip() if revision.returncode==0 else '0'
epoch=int(epoch)
if epoch<0:raise SystemExit('SOURCE_DATE_EPOCH must be nonnegative')
def metadata(info):
 info.uid=info.gid=0;info.uname=info.gname='root';info.mtime=epoch
 info.mode=0o755 if info.isdir() or info.mode&0o111 else 0o644
 info.pax_headers={}
 return info
with tempfile.TemporaryDirectory(prefix='stutools-source-') as temp:
 stage=pathlib.Path(temp)/f'stutools-{version}'
 shutil.copytree(root,stage,ignore=shutil.ignore_patterns('target','dist','.git','__pycache__'))
 license=root.parent/'LICENSE' if (root.parent/'LICENSE').is_file() else root/'LICENSE'
 shutil.copy2(license,stage/'LICENSE')
 (stage/'SOURCE_DATE_EPOCH').write_text(str(epoch)+'\n')
 compat=stage/'tests'/'compat';compat.mkdir(exist_ok=True)
 for name in ['aio_abi.h','aio_faults.c','test_spit_safety.py','test_spit_deep.py','test_spit_block.py']:
  source=root.parent/'c'/'tests'/name
  if source.is_file():shutil.copy2(source,compat/name)
  elif not (compat/name).is_file():raise SystemExit(f'Missing compatibility test {name}')
 if not (stage/'vendor').is_dir():
  config=subprocess.check_output(['cargo','vendor','--locked','--offline','--versioned-dirs',str(stage/'vendor')],cwd=root,text=True)
  (stage/'.cargo').mkdir(exist_ok=True)
  config_path=stage/'.cargo'/'config.toml'
  existing=config_path.read_text() if config_path.exists() else ''
  config_path.write_text(existing+'\n'+config.replace(str(stage/'vendor'),'vendor'))
 archive=out/f'stutools-{version}.tar.gz'
 with archive.open('wb') as raw,gzip.GzipFile(filename='',mode='wb',fileobj=raw,mtime=epoch) as gz,tarfile.open(fileobj=gz,mode='w') as tar:
  tar.add(stage,arcname=stage.name,filter=metadata)
 print(archive)
