# stutools 2.0 - spit, now with less C

| Check / convenience | stutools / spit 2.0 | fio | dd | sysbench fileio |
|---|---|---|---|---|
| No config file required (yes is a plus!) | **Yes** | Yes | Yes | Yes |
| Mix, order, block size, queue, workers and passes in one compact argument | **Yes: `ws1k4q32j4x1`** | Separate options | Copy options; no mixed workload engine | Separate options |
| Create, write and verify a scratch file in one command | **Yes: add `-v`** | Yes, verification options | No built-in data verification | Prepare, then run with validation |
| Probe io_uring and automatically fall back to libaio on Linux | **Yes** | Select an I/O engine | No async engine selection | Select an I/O mode |
| Worker hot-path heap-allocation count in the run summary | **Yes: audited every run** | Not a standard summary field | Not a standard summary field | Not a standard summary field |
| Report throughput and IOPS together | **Yes** | Yes | Throughput; no IOPS summary | Yes |

Spit's party trick is a small command that does useful work. This is a feature
comparison, not a claim that it beats fio at everything: fio has a much broader
engine and analysis toolbox. Sources: [fio's manual](https://fio.readthedocs.io/en/latest/fio_doc.html),
[sysbench fileio](https://github.com/akopytov/sysbench/blob/master/src/tests/fileio/sb_fileio.c),
and [GNU dd](https://www.gnu.org/software/coreutils/manual/html_node/dd-invocation.html).

> When you find yourself in blocks of trouble,<br>
> many devs come to me,<br>
> seeking words of wisdom,<br>
> don't write in C...

**spit** is Stu's powerful I/O tester: describe a storage workload, run it across
workers, measure throughput, and check what was written. Version 2.0 introduces
an independent Rust implementation, with Tokio coordinating workers, reusable
memory pools and no successful worker-loop heap allocations. The original C
suite lives on as **spitc**, alongside fsfiller and the other utilities.

## Download and install

Get [2.0.0 downloads](https://github.com/dr-who/stutools/releases/tag/v2.0.0).
No compiler is required. Choose the package built for your operating system:

| System | Download |
|---|---|
| Debian 12 and compatible newer systems | `stutools_*+debian12_amd64.deb` |
| Ubuntu 24.04 / Linux Mint 22 | `stutools_*+ubuntu24.04_amd64.deb` |
| RHEL / Rocky 9 | `stutools-*.el9.x86_64.rpm` |
| Fedora 44 | `stutools-*.fc44.x86_64.rpm` |
| macOS, Apple silicon or Intel | universal `.pkg` installer, or architecture-specific `.tar.gz` |
| Windows x64 | `spit-2.0.0-windows-x86_64.exe`, or the portable `.zip` |

[Installation instructions and checksums](rust/INSTALL.md) explain each option.
Linux `stutools` installs Rust as `/usr/bin/spit`; the optional `stutools-c`
package supplies `/usr/bin/spitc`, fsfiller and the remaining C tools. These are
upstream GitHub packages, not packages in the official distro repositories.

## Take it for a spin

Start with a disposable file. **Writes overwrite data inside the selected range.**
The tool is called spit; that is not a backup strategy.

```sh
# Create a 16 MiB file, write it once with four workers, verify every write.
spit -f scratch.img -G 16MiB -c ws1k4q32j4x1 -v

# Read the same file in random order for five seconds.
spit -f scratch.img -c rs0k4q32j4 -t 5

# A random 50:50 mix, explicitly using Linux libaio.
spit -f scratch.img -c rws0k4q32j8 -t 10 --backend libaio
```

On Windows use `.\spit.exe` from the extracted directory. The first two examples
also work on macOS and Windows, using their buffered backend.

Windows drive roots such as `E:` and `E:\` name a filesystem, not a test file.
`-F` takes a **file prefix** and creates `PREFIX.0001`, `PREFIX.0002`, etc.,
one file per worker. Initialize disposable files before reading them:

```bat
spit -F E:\spit-test -G 1MiB -c ws1x1 -v
spit -F E:\spit-test -G 1MiB -c rs0 -t 10 -s 0.1
```

For a non-destructive raw **volume** read, run as Administrator:
`spit -f \\.\C: -c rs0 -G 1GiB`. Keep the two leading backslashes and omit
the trailing backslash. Spit reports the filesystem and sector size; raw Windows
writes are currently unsupported. A virtual filesystem such as ExabaFOO may
provide files without exposing a raw block device; use `-F` in that case.

`-G 1M` tests a decimal megabyte; `-G 1` tests a decimal gigabyte. Neither shrinks
an existing file. Use a new prefix when comparing small and large files.
Zero IOPS with requests in flight means the OS/driver has not completed an I/O
within the measured window. Synchronous calls can delay exit beyond `-t`;
spit explains the pending requests and excludes late completions from rates.

The compact workload `ws1k4q32j4x1` means **w**rites, **s1** sequential,
**k4** = 4 KiB, **q32** queue depth, **j4** workers splitting the range, and
**x1** = one worker-range worth of bytes per worker. Quoted workloads may contain
spaces: `-c "w s1 k4 q32 j4 x1"`. `X1` instead means one position-table pass.

`spit --help` explains the tokens and shows the detected default backend.
`man spit` is the full reference. Output reports completed operations, elapsed
seconds, MiB/s, IOPS, verification counts and the hot-path allocation count.

## Backends, memory and speed

For camera-style recording with background preallocation, retention and flushed
minute/hour logs:

```sh
spit -F recordings/camera --cctv 100,5,24 -c k64 --filesizemb 16 -t 7d
```

This simulates 100 cameras at 5 Mbit/s each, using 64 KiB sequential writes into
16 MiB files and retaining 24 hours. `-s` defaults to one second. A completed
seven-day run produces 168 hourly data rows in `cctv-test-DATETIME.hour`, with
average/max write latency in milliseconds. See [the CCTV guide](rust/CCTV.md)
for Linux/Windows examples, ownership rules and allocation behavior.

Linux automatically uses **io_uring** when queue creation succeeds, otherwise
**libaio**. You can explicitly select either, synchronous `direct`, or
`buffered` (`bufferedio`). macOS and Windows currently use buffered positional
I/O. Explicit backend choices never silently fall back. A synchronous backend
executes one I/O at a time per worker; its `q` controls reusable batch storage.

The automatic memory budget is half the available RAM, capped at 64 GiB
(half physical RAM on macOS; 512 MiB fallback if detection fails).
Linux cgroup-v2 limits are respected. `--memory SIZE` overrides this budget;
errors give a copyable value to use. This is not an RSS limit.
Stacks, Tokio, kernel queues and OS page cache are additional.
Memory is prepared before the worker clock starts and reused during I/O.

Progress uses C's `[seconds / workers / in-flight] read ... MB/s (... IOPS / bytes),
write ..., total ... GiB` layout. It appears every second; `-s 0.1` changes the
interval and `-s 0` disables it. Workload `R42` overrides the global random seed, for example
`-c wk4096s0j32G400R42` uses 4 MiB blocks, 32 workers and a 400 GiB range.
Timed progress and averages end at `-t`; pending I/O drains silently afterward.
Late completions and resource cleanup do not extend the measured window.
The asynchronous completion-stall limit is `-T 3600`, matching C. Use `-T 30`
for a shorter limit; a failed run does not establish a device's bandwidth limit.

If Linux disables io_uring, an administrator can enable it with:

```sh
sudo sh -c 'echo 0 > /proc/sys/kernel/io_uring_disabled'
spit --help
```

Value 0 enables the kernel interface; sandbox/security restrictions can still
prevent queue creation. The command changes the running host's policy. The help
reports whether spit can actually use it. Plain `sudo echo ... > ...` does not
elevate the shell redirection.

In three two-second RAM-device runs at 64 workers, Rust io_uring reached median
**968k read / 870k write IOPS**, versus C libaio's **403k / 395k**, with less
process CPU. Rust libaio also beat C in sequential tests. Random mixed I/O is
less flattering: io_uring needs more CPU, and some workloads trail C. Page-cache
numbers are not disk numbers. [Read the measurements and limitations](rust/performance/README.md).

> "Premature optimization is the root of all evil." - Donald Knuth,
> [Structured Programming with go to Statements](https://kohala.com/start/papers.others/knuth.dec74.html)

We measured anyway. The buffer is fixed; expectations may require tuning.

## C compatibility

Rust supports the common C workload syntax, multiple targets/workers, ranges,
variable block sizes, queue/submission limits, position modes, pass/time/rate
limits, preconditioning, verification and metadata/unique-seed modes. Linux runs
all 26 shared C safety/deep regressions as well as Rust tests and disposable
block-device checks.

**2.0 is not a complete C drop-in replacement.** Rust's data and simple position
file formats differ from C's; C spitchecker cannot verify Rust-written data.
NUMA policies, copy/trim/coverage/GC modes, changing rate schedules, position
output, benchmark logs and latency histograms remain C workflows. Unsupported
options fail explicitly. Use `spitc` for those features, and check
[the compatibility reference](rust/README.md) before replacing existing scripts.

## Build and contribute

```sh
cd rust
cargo build --release --locked
cargo test --locked
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
```

Rust 1.85+ is required. Linux also needs a C linker and libaio development files;
`rust/install-deps.sh` installs distro prerequisites. See [the Rust guide](rust/README.md)
for native macOS/Windows build requirements and offline sources.
FreeBSD and OpenBSD also build Rust spit with Cargo and the portable C backend
with CMake. GitHub Actions tests both implementations in real BSD VMs, alongside
native Linux/macOS/Windows and Linux distribution container builds.

The repository has two independent trees: [`rust/`](rust/) for spit 2.0, and
[`c/`](c/) for the original suite (`make -C c`). The top-level Makefile delegates
to Rust. `make -C rust dist` builds/tests and creates a native package; Debian
recipes live in `rust/debian/`, the RPM spec and platform packaging in
`rust/packaging/`. `make -C c dist` builds the companion C package.

> "There is no Rule 6." - Rob Pike,
> [Notes on Programming in C](https://www.cs.tufts.edu/~nr/cs257/archive/rob-pike/notes-on-c.pdf)

Bring back Pluto as a planet.
