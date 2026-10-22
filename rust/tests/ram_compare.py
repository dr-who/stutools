#!/usr/bin/env python3
"""Opt-in Linux RAM-backed loop comparison. Never accepts an existing device.
Run as root: python3 tests/ram_compare.py --c ../c/build/spit/spit --rust target/release/spit
"""
import argparse,json,math,pathlib,re,resource,subprocess,tempfile,time
p=argparse.ArgumentParser()
p.add_argument('--c',required=True);p.add_argument('--rust',required=True)
p.add_argument('--seconds',type=float,default=1);p.add_argument('--repeats',type=int,default=3)
p.add_argument('--workers',default='1,8,64');p.add_argument('--output',default='ram-results.json')
p.add_argument('--backends',default='libaio,io_uring,direct,buffered')
p.add_argument('--operations',default='r,w',help='comma-separated r, w or rw')
p.add_argument('--sequential',default='1',help='C/Rust s value; 0 is random')
p.add_argument('--queue',default='32',help='queue depth or submission-minimum/depth range, e.g. 1-32')
p.add_argument('--verify',action='store_true')
p.add_argument('--profile',help='perf record output for the Rust run; use one backend/workload')
a=p.parse_args();results=[]
if a.repeats<1 or not math.isfinite(a.seconds) or a.seconds<=0:p.error('positive repeats and finite positive seconds are required')
binaries={'c':str(pathlib.Path(a.c).resolve()),'rust':str(pathlib.Path(a.rust).resolve())}
with tempfile.TemporaryDirectory(prefix='spit-ram-',dir='/dev/shm') as temp:
 backing=pathlib.Path(temp)/'ram.img'
 with backing.open('wb') as f:
  for _ in range(256):f.write(b'\x55'*(1024*1024))
 dev=subprocess.check_output(['losetup','--find','--show',str(backing)],text=True).strip()
 try:
  for repeat in range(a.repeats):
   for workers in map(int,a.workers.split(',')):
    for op in a.operations.split(','):
     # Reverse ordering on alternate repeats to reduce order bias.
     variants=[('c','libaio')]+[('rust',backend) for backend in a.backends.split(',')]
     if repeat%2:variants.reverse()
     for implementation,backend in variants:
      cmd=[binaries[implementation],'-f',dev,'-G','256MiB','-c',f'{op}s{a.sequential}k4q{a.queue}j{workers}','-t',str(a.seconds),'-R','1']
      if a.verify:cmd+=['-v']
      if implementation=='rust':cmd+=['--backend',backend]
      if implementation=='rust' and a.profile:cmd=['perf','record','-q','-g','-o',a.profile,'--',*cmd]
      before=resource.getrusage(resource.RUSAGE_CHILDREN);start=time.monotonic()
      run=subprocess.run(cmd,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=60)
      wall=time.monotonic()-start;after=resource.getrusage(resource.RUSAGE_CHILDREN)
      output=run.stdout.decode(errors='replace')
      if implementation=='c':
       match=re.search(r'summary: read mean (\d+) MB/s \((\d+) IOPS\), write mean (\d+) MB/s \((\d+) IOPS\)',output)
       iops=int(match[2])+int(match[4]) if match else None
      else:
       match=re.search(r' IOPS=(\d+)',output);iops=int(match[1]) if match else None
      row=dict(implementation=implementation,backend=backend,workers=workers,operation=op,repeat=repeat,exit=run.returncode,iops=iops,wall_seconds=wall,process_cpu_percent=100*((after.ru_utime-before.ru_utime)+(after.ru_stime-before.ru_stime))/wall,output=output)
      results.append(row);print({k:v for k,v in row.items() if k!='output'},flush=True)
      pathlib.Path(a.output).write_text(json.dumps(results,indent=2)+'\n')
 finally:subprocess.run(['losetup','-d',dev],check=True)
failures=[r for r in results if r['exit']!=0 or r['iops'] is None or (r['implementation']=='rust' and 'hot_path_allocations=0' not in r['output'])]
if failures:raise SystemExit(f'{len(failures)} benchmark runs failed or lacked valid zero-allocation measurements; see {a.output}')
