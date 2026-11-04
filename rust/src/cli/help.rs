//! The text printed by -h and --help all.

use super::Backend;

/// The short overview printed by -h and by a bare run.
pub(super) fn help() {
    println!("spit {} - Stu's I/O tester", env!("CARGO_PKG_VERSION"));
    println!(
        "Default backend: {} (--backend auto selects {} on this system)",
        Backend::Direct.name(),
        crate::io::default_backend().name()
    );
    println!(
        r#"
Command mode (-c workloads against a target, repeat -c to run several):
  spit -f scratch.img -c rs0k4q32j4 -t 5
  spit -F /mnt/test/f -G 1GiB -c ws1k4q32 -c rs0k4q32 -t 10

App modes (each prints its report when it ends, or on Ctrl-C):
  spit -F /mnt/cctv --cctv 100,5,24  # 100 cameras at 5 Mbit/s, kept 24 h
  spit -F /mnt/fs --fsfiller -G 50GiB  # fill a file system with mixed file sizes

More help:
  spit --help all      every option, workload token and backend
  spit --version       print the version"#
    );
}
pub(super) fn help_all() {
    println!(
        "Default backend: {} (--backend auto selects {} on this system)",
        Backend::Direct.name(),
        crate::io::default_backend().name()
    );
    println!(
        "Default --memory on this system: {:.3} GiB (automatic)",
        crate::memory::default_budget() as f64 / 1073741824.
    );
    println!(
        r#"spit {} (Rust) - Stu's powerful I/O tester

USAGE
  spit -f TARGET -c WORKLOAD [options]

START HERE (writes overwrite the selected range)
  spit -f scratch.img -G 16MiB -c ws1k4q32j4x1 -v
  spit -f scratch.img -c rs0k4q32j4 -t 5
  spit -f scratch.img -c rws0k4q32j8 -t 10 --backend libaio

  ws1k4q32j4x1 = writes, sequential, 4 KiB, depth 32, 4 workers,
                  one worker-range worth of bytes per worker.

TARGETS AND RUN LIMITS
  -f FILE         file/device; repeat for several targets
  -F PREFIX       one file per worker: PREFIX.0001, PREFIX.0002, ...
                  initialize new test files with -c ws1x1 before reading
  -G SIZE         bytes [0,SIZE); -G LOW-HIGH selects [LOW,HIGH)
                  bare numbers are decimal GB; KiB/MiB/GiB are exact
  -b BYTES        size in bytes (equivalent to -G with a byte suffix)
  -c WORKLOAD     workload string; repeat to run workloads together
  -t SECONDS      measured time limit (default 10); -1 unlimited; x/X override it
                  suffixes ms/s/m/h/d/w accepted, e.g. -t 7d
                  progress/counts stop at this boundary; queued I/O drains silently
  -s SECONDS      live progress interval (default 1); -s 0 disables progress
                  C layout: time / workers / in-flight, read/write MB/s and IOPS,
                  average bytes per I/O, cumulative GiB
  --recreate      permit enlargement of an existing writable regular file

WORKLOAD TOKENS (whitespace allowed inside a quoted workload)
  r / w / rw      reads / writes / 50:50 mix; rrw means 2:1 reads:writes
  pPROB           read probability from 0 to 1 (p0.7 = 70% reads)
  s0 / s1 / s-1  random / sequential / reverse; s2 interleaves 2 stripes
  kN / MN         block size in KiB / MiB (default k4)
  kMIN-MAX        variable blocks in MIN-KiB steps; kMIN:MAX powers of 2
  qN / qMIN-MAX   queue depth / submission-group limit and depth (default q64)
  jN / JN         workers splitting the range / workers sharing the range
  Pn / P-n        first n positions / n random samples with replacement
  P+n / P.n       n spaced positions / alternating low-high positions
  Xn / xn / Yn    n table passes / n range-byte passes / n operations
  O / = / n / N   one table pass / alternate actions / shuffle / shift per pass
  z / F           start at zero / flush completed write batches
  m[COUNT]        metadata write/read checks; optional queue-depth cap
  u / U           per-position seeds / seeds plus checked read pairs (depth 1)
  SIOPS / TSEC    per-worker IOPS limit / duration (not the global -T timeout)
  RSEED           per-workload seed, overriding global -R
  WON:OFF         active/idle seconds, e.g. W1:0.5
  GLOW-HIGH       per-workload GiB range; bLOW-HIGH is a byte range
                  bare G uses the target range; G_ also splits J workers

APPLICATION MODES (instead of -c)
  --cctv COUNT,MBPS,HOURS -F PREFIX   open-loop camera simulation; see [cctv] above
  --fsfiller -F DIR           fill DIR with directories and files of 1 KiB to 16 MiB
                  --fsfiller-dirs TOP,SECOND  directory counts (default 100,100)
                  --fsfiller-threads N  parallel writers (default 150, 1 to 256)
                  --ratelimitmbps N  limit the fill to N Mbit/s, the same unit as --cctv
                  --cantstopthis  retry failed writes, including a full disk, with exponential back-off up to 10 min
                  -v  read each file back after writing and check it
                  --synconclose  flush each file to the device before closing it
                  --writes MODE  buffered, writethrough (default: each write reaches the device before it returns) or direct (bypasses the cache, like O_DIRECT)
                  -G SIZE caps the bytes written; -t SECONDS sets a time limit
                  runs until the file system is full, then reports; Ctrl-C reports early

VERIFICATION, RESOURCES AND DIAGNOSTICS
  -v              read back each write; verification reads are not workload ops
  -R SEED         repeatable random seed (default 1)
  -S FILE         positions: byte-offset R|W byte-length, one per line
  -p WORKLOAD     precondition first (one worker; defaults to ws1x1)
  -K READ,WRITE   require these completed workload counts, including drained I/O
  -E ERRORS       libaio submission-error stop threshold (default 1; 0 continues)
                  any submission error still makes the final exit nonzero
  -T SECONDS      async completion-stall timeout (default 3600, as in spitc; synchronous calls
                  and kernel queue destruction cannot guarantee this bound)
  --backend NAME  auto | io_uring | libaio | direct | pagecache
  --memory SIZE   buffer/table budget (automatic default shown above); excludes stacks,
                  runtime, kernel queues and OS page cache
  -h, --help      short help; --help all for everything; --version prints version

Linux auto probes io_uring queue creation, then uses libaio if unavailable.
Linux administrator enable command:
  sudo sh -c 'echo 0 > /proc/sys/kernel/io_uring_disabled'
macOS/Windows use page-cache positional I/O unless direct is requested. Explicit backends never fall back.
Windows: -f E:\ is a filesystem root; use -F E:\spit-test for file tests.
  spit -F E:\spit-test -G 1MiB -c ws1x1 -v
  spit -F E:\spit-test -G 1MiB -c rs0 -t 10
Administrator raw volume READS: spit -f \\.\C: -c rs0 -G 1GiB
Raw Windows writes are unsupported. Virtual filesystems may have no raw device.
-G limits the range; it never shrinks an existing file. If an I/O stalls beyond
-t, its late completion is excluded from IOPS; the driver must finish before exit.
Synchronous backends do one I/O at a time per worker; q sets reusable batch size.
Pools allocate at startup; successful worker I/O loops report hot_path_allocations=0.
Summary: completed reads/writes, elapsed seconds, MiB/s, IOPS and verified_ops.
Timed summaries count only completions observed within the measured window;
draining, verification cleanup and resource teardown do not extend that window.
Metadata checks pending at a limit finish before success, outside workload counts.
[cctv]
  --cctv COUNT,MBPS,HOURS  camera count (N or START..END/STEP, e.g. 10..100/10),
                           decimal Mbit/s EACH, retention hours
  --info DEVICE           describe a block device (identity, queue and transfer limits,
                          iSCSI negotiated values where the OS exposes them); no I/O
  --cctv-io MODE           direct (default on Linux/Windows) or pagecache
  --link-mbps RATE         link capacity in Mbit/s; drawn on the rate chart
  --cctv-floor-percent PERCENT     stop when the last 10 s average falls below PERCENT of the camera
                           rate (default 0, off); e.g. --cctv-floor-percent 75
  -F DIR/PREFIX           one independent sequential recording stream per camera
  -c k64                  fixed write block size (default 64 KiB; M sizes accepted)
  --filesizemb 16         preallocated recording file size in MiB (default 16)
                          files rotate when full; background worker prepares ahead
  -t 7d                   run for seven days (CCTV default: until Ctrl+C)
  -s SECONDS              normal progress interval (default 1; 0 disables it)
  spit -F recordings/camera --cctv 100,5,24 -c k64 --filesizemb 16 -t 7d
  Logs: cctv-test-UTC-DATETIME-cameras=N.log, .min and .hour, beside the recording prefix.
  A camera range writes one set of logs per count.
  .min/.hour are flushed CSV, with average/max write ms for each minute/hour.
  A completed seven-day run has 168 hourly data rows; final partial periods are labelled.
  Retention deletes only this prefix's owned recording files; logs are preserved.
  CCTV uses direct (no page cache) filesystem I/O on Linux and Windows by default;
  --cctv-io pagecache selects the page cache. Other systems use pagecache I/O.
C data/spitchecker formats and some advanced C modes are not supported; use spitc
for those workflows. See README.md and man spit. Invalid/incomplete I/O fails.

"Premature optimization is the root of all evil." - Donald Knuth
"There is no Rule 6." - Rob Pike
Bring back Pluto as a planet."#,
        env!("CARGO_PKG_VERSION")
    );
}
