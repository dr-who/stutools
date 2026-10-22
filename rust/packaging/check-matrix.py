#!/usr/bin/env python3
"""Build/install in disposable distro containers. Requires host Podman access."""
import concurrent.futures,hashlib,json,pathlib,re,subprocess,sys
root=pathlib.Path(__file__).resolve().parents[1]
toolchain=pathlib.Path(subprocess.check_output(['rustup','which','rustc'],text=True).strip()).parents[1]
images={'debian12':'docker.io/library/debian:12','mint22':'docker.io/linuxmintd/mint22-amd64:latest','rocky9':'docker.io/library/rockylinux:9','alma9':'docker.io/library/almalinux:9','fedora':'registry.fedoraproject.org/fedora:latest','rhel9-ubi':'registry.access.redhat.com/ubi9/ubi:latest'}
version=re.search(r'^version = "([^"]+)"', (root/'Cargo.toml').read_text(), re.M)[1]
archive=root/'dist'/f'stutools-{version}.tar.gz'
source_sha256=hashlib.sha256(archive.read_bytes()).hexdigest()
if len(sys.argv)>1:images={k:images[k] for k in sys.argv[1:]}
def run(item):
 name,image=item;out=root/'dist'/name;out.mkdir(exist_ok=True)
 cmd=['podman','run','--rm','--cgroups=disabled','--user','0','--security-opt','label=disable','--security-opt','seccomp=unconfined','-e',f'STUTOOLS_SOURCE_ARCHIVE={archive.name}','-v',str(toolchain)+':/opt/rust:ro','-v',str(root/'dist')+':/input:ro','-v',str(root/'packaging')+':/scripts:ro','-v',str(out)+':/artifacts',image,'sh','/scripts/container-check.sh']
 with (out/'build.log').open('w') as log:r=subprocess.run(cmd,stdout=log,stderr=subprocess.STDOUT)
 print(name,r.returncode,flush=True)
 return {'distro':name,'image':image,'version':version,'source_sha256':source_sha256,'exit':r.returncode,'log':str(out/'build.log')}
with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:results=list(pool.map(run,images.items()))
report=root/'dist/matrix-results.json'
previous=json.loads(report.read_text()) if report.exists() else []
merged={r['distro']:r for r in previous if r.get('source_sha256')==source_sha256}
merged.update({r['distro']:r for r in results})
report.write_text(json.dumps(list(merged.values()),indent=2)+'\n')
sys.exit(any(r['exit'] for r in results))
