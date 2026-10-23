# CCTV recording workload

Linux:

```sh
spit -F /mnt/recordings/camera --cams 100,5,24 -c k64 --filesizemb 16 -t 7d
```

Windows PowerShell:

```powershell
spit -F F:\recordings\camera --cams 100,5,24 -c k64 --filesizemb 16 -t 7d
```

`--cams COUNT,MBPS,HOURS` starts independent synthetic recording streams.
MBPS means **decimal megabits per second per camera**. The example targets
500 Mbit/s (62.5 MB/s), retains 24 hours of recordings, and runs for seven days.
The files contain generated benchmark data.

Each camera writes sequentially with a fixed, reusable **64 KiB** buffer by
default. `-c k64` makes that explicit; a fixed `M` size also works.
`--filesizemb 16` means **16 MiB** and defaults to 16. File size must be a
multiple of block size. Files rotate when full: at 5 Mbit/s, one 16 MiB file
contains about 26.84 seconds. A final partial recording remains preallocated;
counters include only completed payload writes, not unused reserved space.

A separate allocation worker prepares one file ahead of each camera. Linux uses
`fallocate`; Windows uses `SetFileInformationByHandle(FileAllocationInfo)` and
sets EOF before sequential recording begins. Physical contiguity is not
guaranteed. If native reservation is unsupported, the background worker fills
the reservation with zeros and reports the fallback. macOS/BSD currently use
this fallback. Spit does not use privileged `SetFileValidData`.

References: [Windows allocation size](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_allocation_info),
[valid data length](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-setfilevaliddata),
and [Linux fallocate](https://man7.org/linux/man-pages/man2/fallocate.2.html).
This configurable synthetic workload does not reproduce proprietary Milestone
internals. OS caching/drivers may combine or split application writes into
different physical/SCSI requests.

One long-lived Tokio blocking worker owns each camera. `--memory` bounds the
fixed pools; stacks, OS caches and reserved disk space are additional.
Successful data-write loops audit `hot_path_allocations=0`; file rotation,
directory scanning and logs can allocate bounded metadata. Late cameras skip
missed pacing slots instead of producing catch-up bursts.

## Progress and logs

`-s` controls C-style progress, default **one second**. `-s 0.1` gives ten
samples per second; `-s 0` disables progress but keeps minute/hour summaries.
Logs live beside the recording prefix:

```text
cctv-test-2026-10-23_14-35-01Z.log
cctv-test-2026-10-23_14-35-01Z.min
cctv-test-2026-10-23_14-35-01Z.hour
```

Times are UTC; same-second runs receive a numeric suffix. Logs are created
exclusively. `.log` contains settings, progress and the final result. `.min`
and `.hour` are CSV with one header and one row per elapsed minute/hour:

```text
period,utc_start,elapsed_start_s,seconds,status,write_ops,write_bytes,avg_write_ms,max_write_ms,preallocated_files,avg_preallocation_ms,max_preallocation_ms,deleted_files
```

`avg_write_ms` is total completed application-write latency divided by write
count; `max_write_ms` is the slowest completed write in that period. These
measure `write_all`, exclude pacing/buffer generation and do not measure media
durability. Closing a recording flushes data separately. Preallocation has its
own latency columns. Hourly averages are weighted by write count. Empty periods
show zero samples and zero latency.

Every progress/minute/hour record is flushed from userspace. Each hourly row
is also synced to the filesystem, so `.hour` can be analysed during the run.
A completed **`-t 7d` produces 168 hourly data rows**, plus the header, and
10,080 minute rows. Final partial periods are labelled `partial`; exact hour
boundaries do not add empty rows. The accelerated seven-day aggregation test
does not imply a seven-day wall-clock soak.

`-t` accepts seconds or `ms`, `s`, `m`, `h`, `d`, `w` suffixes, including
`-t 1.5h`. CCTV defaults to running until Ctrl+C/SIGTERM. Timed progress/counts
stop at the deadline; pending filesystem calls and final flushes can delay exit.
Late writes are counted separately.

## Ownership and retention

`-F DIRECTORY/PREFIX` names recordings; parent directories are created if needed.
`PREFIX.cctv.owner` identifies the recording set, and a filesystem lock prevents
simultaneous runs on that prefix. Keep the ownership file to resume retention.

Closed names contain camera number, UTC completion time, run ID and sequence.
Reserved/active files end in `.pending`. Retention runs at startup and each
minute. It removes only expired regular files matching the exact prefix/name
and binary ownership header, skipping this run's pending files. Symlinks and
foreign contents are preserved. Old owned pending files from interrupted runs
can expire. Other files and all run logs are preserved. Avoid modifying
recordings externally while a run owns the prefix.

Allow about **5.4 TB** for 100 cameras at 5 Mbit/s with 24-hour retention,
plus active/reserved files, filesystem overhead and logs. At 16 MiB this means
roughly 322,000 recordings, replacing the original one-file-per-minute proposal.
