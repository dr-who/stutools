# Rust spit

CCTV mode simulates paced camera streams with background file preallocation,
retention and flushed minute/hour latency logs. For example:

```sh
spit -F recordings/camera --cams 100,5,24 -c k64 --filesizemb 16 -t 7d
```

See [CCTV.md](CCTV.md) for Linux/Windows examples and the exact recording,
preallocation, logging and deletion rules.

The Rust implementation of Stu's powerful I/O tester. It supports the common C
workload syntax; advanced compatibility limits are listed below. This version
uses Tokio to coordinate one long-lived blocking worker per workload
thread. It does not create a task, future, buffer or allocation for each I/O.

Build with Rust 1.85 or newer:

```sh
./install-deps.sh              # Linux linker, libaio headers, packaging tools
cargo build --release --locked
cargo test --locked
./target/release/spit --help
```

On macOS, install Xcode Command Line Tools and Rust. On Windows, install Rust
with the MSVC C++ build tools. Both build natively with `cargo build --release`;
the output is `target/release/spit` or `target/release/spit.exe`. Native tests use
disposable regular files. Linux also supports block devices.

Example (writes destroy data within the selected range):

```sh
./target/release/spit -f scratch.img -G 256MiB -c ws1k4q32j8x1 -v
./target/release/spit -f scratch.img -c rs0k4q32j8 -t 10 --backend libaio
```

## Backends and memory

| `--backend` | Platform | Operation |
|---|---|---|
| `libaio` | Linux, automatic fallback | Direct I/O, one kernel AIO context per worker |
| `io_uring` | Linux, preferred automatic default | Direct I/O, one submission/completion ring per worker |
| `direct` | Linux | Synchronous positional I/O with O_DIRECT |
| `buffered` (`bufferedio`) | Linux, macOS, Windows | Synchronous positional I/O through the OS page cache |

A synchronous backend performs one operation at a time per worker; `q` controls
its reusable batch size, not concurrent kernel requests. Use `j` to increase
concurrency. By default (or with `--backend auto`), Linux probes io_uring queue creation and
selects io_uring when available, otherwise libaio. `--help` shows the detected
default. Explicit backend requests never fall back to another implementation.
The current io_uring implementation uses ordinary read/write SQEs with stable
buffers; registered buffers, SQPOLL, IOCP and macOS asynchronous I/O remain future
work. Linux currently requires a 64-bit little-endian target.

`--memory SIZE` bounds user I/O buffer pools and request/position tables.
The automatic default is half available RAM, capped at 64 GiB (half physical
RAM on macOS), with a 512 MiB fallback if detection fails. Linux cgroup-v2
limits reduce the available amount. Help shows the detected default; an
insufficient-budget error suggests a copyable `--memory` value. Large blocks
multiply by queue depth and worker count: 4 MiB x q64 x 32 needs over 8 GiB.
It is **not** a whole-process RSS cap: thread stacks, Tokio, parsed arguments,
kernel queues, and OS page cache are additional memory. Pools are allocated and
touched at startup and reused. An allocator counter checks each successful
worker's timed loop and reports `hot_path_allocations=0`. Argument parsing,
startup, error reporting and shutdown can allocate.

Queues drain before reuse. On failed destruction or unresolved io_uring requests,
buffers are retained until process exit instead of freeing memory still owned
by the kernel. Submission retries have a finite hard limit. Completion errors
stop the worker. `-T` bounds completion stalls in asynchronous backends (default
3600 seconds, matching C; use `-T 30` for a shorter limit); synchronous
system calls and kernel queue destruction cannot guarantee a bounded wait.

## Compatibility status

Live progress uses C's layout: `[seconds / workers / in-flight] read ... MB/s
(... IOPS / bytes-per-I/O), write ..., total ... GiB`. Rates use decimal MB/s;
the cumulative total uses GiB. The default interval is one second; `-s1` and
`-s 0.1` select explicit intervals, and `-s 0` disables progress.
For timed workloads, output and performance counters stop at the duration limit.
The summary divides in-window completions by the measured duration, excluding
late completions and cleanup time. Outstanding I/O drains silently before exit.
`-K` still checks all completed workload operations, including drained requests.

Implemented: `-f`, per-worker `-F` target suffixes, `-G`, `-c`, `-t`, live `-s` progress, `-R`, `-v`,
`--recreate`, simple `-S` position files, `-p` range-aware preconditioning,
`-K` operation counts, submission-error threshold `-E` and asynchronous timeout
`-T`. Workloads support r/w ratios, pPROB, fractional/reverse/striped s values,
fixed or variable k/M sizes, qDEPTH and qMIN-MAX, j/J workers, positive P,
sampled/spaced/alternating P modes, x/X, Y operation limits, O single passes,
alternating read/write passes, n/N pass changes, S rate limits, T duration,
W duty cycles, per-job G/b ranges, z, metadata m/mCOUNT, unique seeds u/U and F flushing.
Workload `RSEED` overrides global `-R`. Progress defaults to one second;
`-s 0` disables it. Per-worker counters are allocated before I/O and published
without worker heap allocations; the reporter is separate from I/O workers.
qDEPTH allows asynchronous backends to batch submissions up to the depth;
qMIN-MAX explicitly limits submission groups to MIN and queue depth to MAX.
Libaio refills freed queue slots continuously. io_uring uses bounded batches for
pure read/write workloads and continuous refill for mixed workloads; this avoids
a measured Linux block-dispatch contention regression with many RAM-device workers.

This is not full C syntax or output compatibility. NUMA policy, position output,
coverage/GC/copy modes, changing rate schedules, benchmark logs and latency
histograms still need implementation. Unsupported tokens fail explicitly.
Metadata mode schedules a second read phase; U schedules write/read pairs at
queue depth one. Unique seeds use the C convention of 16-bit per-position
values. These reads are generated from the existing table without allocating
a duplicate. Any checks left pending at a workload limit are completed outside
the workload counters and reported in `verified_ops`. Metadata verification
cannot be combined with alternating-action `=` mode. `-v` verifies ordinary
writes immediately rather than in a final pass. Fixed position tables are allocated
at startup and reused; random sampling and shuffling do not reproduce C's sequence.
The seeded, address-tagged data format differs from C and is not compatible
with the C spitchecker. Do not substitute this binary into those workflows yet.

`cargo test` on Linux runs the original 15 C safety tests and 11 deeper regression
specifications unchanged, including their libaio fault-injection shim. The native
Windows/macOS tests cover the portable core; the libaio shim is Linux-only.
Windows drive-root diagnostics are tested natively. Administrator-only raw read
tests are opt-in: in PowerShell set `$env:SPIT_TEST_RAW_VOLUME = '\\.\C:'`,
then run `cargo test --release windows_raw_volume_read_only`. This performs
16 reads inside the first MiB and never opens the volume for writing.
See the top-level README for Windows volume and filesystem examples.

Additional opt-in tests (only disposable devices):

```sh
sudo python3 ../c/tests/test_spit_block.py target/release/spit
sudo python3 tests/ram_compare.py --c ../c/build/spit/spit --rust target/release/spit
```

`tests/uring_validate.py` temporarily enables io_uring, runs applicable safety and
block tests plus the RAM comparison, and restores the original sysctl. Run it
only with administrator authorization. The C libaio fault shim cannot inject
faults into io_uring, so passing its non-fault tests does not establish equivalent
fault coverage for that backend.

## Packaging

```sh
make dist       # release build + tests, then native RPM/DEB/PKG
make rpm        # needs rpm-build
make deb        # needs dpkg-dev
make source     # source tarball with locked, vendored crates for offline builds
make install    # Linux: /usr/bin/spit and man page; macOS: /usr/local
```

Native binary packages use the locally built/tested executable and calculate
runtime shared-library dependencies. Build each package on its target distribution
or an older compatible baseline; a binary built against newer glibc may not run
on an older release. RPM builds strip the binary as part of normal packaging.
The spec also supports full offline source builds; Debian source recipes are in
`debian/`. Source builds require Rust/Cargo >=1.85, which older distro
repositories may not provide. Install an appropriate Rust toolchain separately.
For a Rust toolchain installed outside dpkg, set `STUTOOLS_EXTERNAL_RUST=1`
when running `make deb`. This enables the `pkg.stutools.external-rust` build
profile and checks the toolchain version; other Debian build dependencies are
still checked normally. DEBs use debhelper and dpkg-buildpackage, including
copyright, changelog, man page, automatic shared-library dependencies and checksums.

The package is named `stutools` and supplies `/usr/bin/spit`. The C package is
`stutools-c`, with `/usr/bin/spitc`, fsfiller and the other C utilities. Native portable archives include the executable, README and license.
Linux make/CMake defaults also use `/usr`; override PREFIX or CMAKE_INSTALL_PREFIX
only when you deliberately want another installation location. macOS installers
use `/usr/local`, since macOS reserves `/usr/bin` for the operating system.
FreeBSD needs `pkg install rust`; OpenBSD needs `pkg_add rust`. Both build with
`cargo build --release --locked` and use buffered positional I/O. CI builds and
tests Rust and the portable C file backend in FreeBSD 15.1 and OpenBSD 7.9 VMs.
