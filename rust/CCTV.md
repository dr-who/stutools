# CCTV recording workload

Linux:

```sh
spit -F /mnt/recordings/camera --cctv 100,5,24 -c k64 --filesizemb 16 -t 7d
```

Windows PowerShell:

```powershell
spit -F E:\recordings\camera --cctv 100,5,24 -c k64 --filesizemb 16 -t 7d
```

`--cctv COUNT,MBPS,HOURS` starts independent synthetic recording streams.
MBPS means **decimal megabits per second per camera**. The example targets
500 Mbit/s (62.5 MB/s), retains 24 hours of recordings, and runs for seven days.
The files contain generated benchmark data.

For a three-day, 100-camera run with one day's retention:

```powershell
spit -F E:\recordings\camera --cctv 100,5,24 -c k64 --filesizemb 16 -t 3d -s 1
```

This requests 500 Mbit/s (62.5 MB/s). Each camera has its own long-lived worker
and open recording; the background allocator keeps a second open file ready.
Allow roughly 5.4 TB for 24 hours of payload, plus reservations and metadata.
The requested rate is a target; storage or CPU limits can reduce the achieved
rate, which is recorded in the progress and summary logs.

Each camera writes sequentially with a fixed, reusable **64 KiB** buffer by
default. `-c k64` makes that explicit; a fixed `M` size also works.
`--filesizemb 16` means **16 MiB** and defaults to 16. File size must be a
multiple of block size. Files rotate when full: at 5 Mbit/s, one 16 MiB file
fills in about 26.84 seconds. A final partial recording remains preallocated;
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
cctv-test-2026-10-23_14-35-01Z-cameras=100.log
cctv-test-2026-10-23_14-35-01Z-cameras=100.min
cctv-test-2026-10-23_14-35-01Z-cameras=100.hour
```

Times are UTC; same-second runs receive a numeric suffix. Logs are created
exclusively. `.log` contains settings, progress and the final result. `.min`
and `.hour` are CSV with space padding to align columns, one header and one row
per elapsed minute/hour. Trim surrounding whitespace from column names and
values when importing them into analysis tools:

```text
period,utc_start,elapsed_start_s,seconds,status,write_ops,write_bytes,avg_write_ms,max_write_ms,preallocated_files,avg_preallocation_ms,max_preallocation_ms,deleted_files,file_rotations,preallocation_waits,avg_preallocation_wait_ms,max_preallocation_wait_ms,max_write_camera,max_write_sequence,max_write_offset,max_write_submit_s,max_write_complete_s,writes_over_200ms,avg_gen_ms,max_gen_ms,closes,avg_close_ms,max_close_ms,max_close_queue\n
```

`avg_gen_ms` and `max_gen_ms` time buffer generation, which is excluded from write
latency. `closes`, `avg_close_ms` and `max_close_ms` count and time recording closes,
and `max_close_queue` is the close-queue depth. `avg_write_ms` is total completed
application-write latency divided by write count; `max_write_ms` is the slowest completed write in that period. These
measure `write_all`, exclude pacing/buffer generation and do not measure media
durability. CCTV uses synchronous `File::write_all` calls in Tokio blocking workers, not
Tokio asynchronous file writes. The elapsed time includes any OS blocking or
thread descheduling during that buffer-write call. Closing a recording flushes
data separately. Preallocation has its
own latency columns. Hourly averages are weighted by write count. Empty periods
show zero samples and zero latency.

`file_rotations` counts switches to the next recording after the initial file.
`preallocation_waits` counts switches where the allocation worker had not yet
made that file available; its average/max wait columns measure those delays.
The measured run starts only after every camera has its current and next file
reserved. Zero waits means each next file was ready when the camera requested it. This
does not promise zero filesystem write, flush or rename latency.

The slowest buffer write carries its camera, recording sequence and byte offset.
`max_write_submit_s` and `max_write_complete_s` are monotonic elapsed seconds
relative to the measured start, immediately before and after the synchronous
write call. Their difference in milliseconds equals `max_write_ms`. They are
application submission/return timestamps, not kernel IRP or durable-media
completion timestamps. `writes_over_200ms` counts completed buffer writes that
exceeded 200 ms; file allocation and closing flushes do not enter that count.

Every progress/minute/hour record is flushed from userspace. Each hourly row
is also synced to the filesystem, so `.hour` can be analysed during the run.
A completed **`-t 7d` produces 168 hourly data rows**, plus the header, and
10,080 minute rows. Final partial periods are labelled `partial`; exact hour
boundaries do not add empty rows. The accelerated seven-day aggregation test
does not imply a seven-day wall-clock soak.

For a 15-minute check, use `-t 15m`. A successful full run produces 15 complete
minute rows and **one partial-hour row covering 900 seconds**, including all
write and preallocation statistics. The partial row is flushed and synced
before the process exits.

`-t` accepts seconds or `ms`, `s`, `m`, `h`, `d`, `w` suffixes, including
`-t 1.5h`. CCTV defaults to running until Ctrl+C/SIGTERM. Timed progress/counts
stop at the deadline; pending filesystem calls and final flushes can delay exit.
Late writes are counted separately.

## Keep a Windows run alive after disconnecting

Use Windows Task Scheduler to run independently of an SSH or terminal session.
For example, in an elevated PowerShell session:

```powershell
$exe = (Get-Command spit).Source
$action = New-ScheduledTaskAction -Execute $exe -Argument '-F E:\recordings\camera --cctv 100,5,24 -c k64 --filesizemb 16 -t 15m -s 1'
$principal = New-ScheduledTaskPrincipal -UserId 'SYSTEM' -LogonType ServiceAccount -RunLevel Highest
$settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit (New-TimeSpan -Hours 2)
Register-ScheduledTask -TaskName 'spit-cctv-test' -Action $action -Principal $principal -Settings $settings
Start-ScheduledTask -TaskName 'spit-cctv-test'
Get-ScheduledTask -TaskName 'spit-cctv-test'
Get-Content E:\recordings\cctv-test-*.log -Tail 5
```

The task writes the normal CCTV logs even without an attached console. Choose
a fresh prefix and a unique task name for each independent recording set. For
`-t 7d`, raise the task's execution limit (for example, to eight days); otherwise
Task Scheduler could forcibly stop it before Spit flushes the final summary.
The machine must stay running and the volume must remain mounted. An SSH
disconnect does not stop this task; a reboot does. Windows user-logoff
notifications are ignored by the background recorder; shutdown and Ctrl+C
still request a clean stop. Forcefully ending the task
cannot guarantee a final partial-hour summary, so let `-t` finish normally.

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
roughly 322,000 recordings.
