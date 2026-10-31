# stutools 2.0.0

spit 2.0 introduces the Rust I/O tester, reusable fixed memory pools, automatic
Linux io_uring/libaio selection, and native downloads that need no compiler.
Live progress defaults to one second; `-s1` and `-s 0.1` select the interval.
The memory budget scales with available RAM, with actionable override hints.
Per-workload `R` seeds and C's default 3600-second completion-stall limit are supported.
The repository is organised into independent `rust/` and `c/` trees. The C suite
remains available as stutools-c, installing spitc, fsfiller and its other tools.

## Downloads

- Linux x86_64 DEB: Debian 12 baseline; Ubuntu 24.04 / Mint 22 baseline.
- Linux x86_64 RPM: EL9 baseline (RHEL, Rocky, Alma); Fedora 44.
- macOS: universal PKG installer, plus Apple silicon / Intel portable archives.
- Windows x64: standalone EXE and portable ZIP; the C runtime is statically linked.
- Locked, vendored Rust source archive; companion C source and Linux packages.
- SHA256SUMS and build/validation metadata for the release assets.

Linux packages and make/CMake defaults install to `/usr/bin`, with Rust named
spit and C named spitc. macOS uses `/usr/local/bin`. The macOS package is unsigned
and not notarized. No background service is installed by the Rust package.

See [installation instructions](https://github.com/dr-who/stutools/blob/main/rust/INSTALL.md)
and [README](https://github.com/dr-who/stutools#readme) for examples.

## Correctness and performance

The Linux Rust suite includes all 26 shared C regression specifications. Fixed
memory, range preservation, asynchronous errors/cleanup, queue depth/refill,
metadata verification and worker startup failures have regression coverage.
Disposable loop-device tests exercise real block-device paths. Portable tests
run on macOS and Windows; libaio fault injection is Linux-only and does not
establish io_uring fault coverage.

At 64 workers in short sequential RAM-device tests, Rust io_uring reached median
968k read / 870k write IOPS against C libaio's 403k / 395k, using less process CPU.
These are shared-host measurements, not physical-drive or fio comparisons.
Random mixed io_uring I/O still has higher CPU cost and some workloads trail C.
Full methodology and per-run data are in
[the performance report](https://github.com/dr-who/stutools/blob/main/rust/performance/README.md).

## Compatibility limits

This is a major implementation change, not full C behaviour/output compatibility.
The common workload CLI is supported; unsupported tokens fail explicitly.
Rust's seeded data and simple position formats differ from C; C spitchecker
cannot verify Rust data. NUMA, copy/trim, coverage/GC, changing rate schedules,
position output, logs and latency histograms remain C workflows. Use spitc for
those features. Rust -v verifies ordinary writes immediately within its timed
loop, while C performs final verification outside its reported workload time.
The default memory budget bounds pools/tables, not total process RSS.

Direct I/O is the default on macOS and Windows; `--backend pagecache` selects
buffered I/O. Windows raw-volume reads are supported, but raw-volume writes, Windows
IOCP, macOS asynchronous I/O, registered io_uring buffers and SQPOLL are not
implemented. Review existing scripts before substituting Rust spit.

Bring back Pluto as a planet.
