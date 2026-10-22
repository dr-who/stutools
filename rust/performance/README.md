# RAM-device measurements, 22 October 2026

Rust is faster than C in these sequential RAM tests. Random mixed I/O is reliable,
but io_uring CPU efficiency remains an open issue. These results do not establish
parity for every workload or device.

The host has 128 logical CPUs and runs Linux 6.12.0-124.45.1.el10_1.x86_64.
Each campaign uses a disposable 256 MiB loop device backed by `/dev/shm`, 4 KiB
I/O, `q32`, seed 1, 1/8/64 workers and three two-second repetitions. C is the
release build of f61a02b. Both programs receive the same workload arguments.
Ordering reverses on alternate repetitions. The host is shared, not isolated.

## Sequential I/O

Libaio results, medians of three repetitions:

| Workers | Operation | C IOPS | Rust IOPS | Rust / C |
|---:|---|---:|---:|---:|
| 1 | Read | 296,555 | 543,254 | 1.83x |
| 1 | Write | 291,341 | 511,259 | 1.75x |
| 8 | Read | 480,241 | 930,263 | 1.94x |
| 8 | Write | 424,157 | 930,409 | 2.19x |
| 64 | Read | 415,554 | 922,886 | 2.22x |
| 64 | Write | 385,767 | 808,954 | 2.10x |

The final io_uring campaign has its own interleaved C baseline:

| Workers | Operation | C libaio IOPS | Rust io_uring IOPS | Rust / C |
|---:|---|---:|---:|---:|
| 1 | Read | 298,489 | 357,591 | 1.20x |
| 1 | Write | 287,182 | 343,618 | 1.20x |
| 8 | Read | 453,427 | 1,028,776 | 2.27x |
| 8 | Write | 438,905 | 916,127 | 2.09x |
| 64 | Read | 403,143 | 968,358 | 2.40x |
| 64 | Write | 395,121 | 869,889 | 2.20x |

At 64 workers, median process CPU use was 237%/234% for Rust libaio reads/writes
versus 579%/491% for C in that campaign. Rust io_uring used 227%/227% versus C's
567%/506% in its campaign. CPU includes startup/shutdown; IOPS uses each
program's timed interval. A startup barrier prepares all Rust workers before
starting a shared measurement clock.

The initial 90-run campaign exercised C plus Rust libaio, io_uring, synchronous
direct and buffered I/O. It exposed an io_uring refill regression at 64 workers:
433,146/385,490 read/write IOPS and 1,457%/1,167% CPU. Pure-direction io_uring
workloads now use bounded batches, restoring the final results above. Libaio
continues to refill completed slots promptly. A 36-run campaign verified the fix.
Both campaigns passed without errors or Rust hot-path allocations.

Buffered Rust I/O reached median 4.58 million read / 5.41 million write IOPS at
64 workers, using about 6,148%/5,801% CPU. Page-cache results are separate from
direct I/O and cannot demonstrate physical-device throughput.

## Random mixed I/O

For `rws0k4q32`, medians of three two-second repetitions:

| Workers | C libaio IOPS | Rust libaio IOPS | Rust io_uring IOPS |
|---:|---:|---:|---:|
| 1 | 294,578 | 316,203 | 295,537 |
| 8 | 462,265 | 415,393 | 389,584 |
| 64 | 361,751 | 401,218 | 365,398 |

All 27 runs passed; all 18 Rust runs reported `hot_path_allocations=0`.
At 64 workers C used median 461% CPU, Rust libaio 515%, and Rust io_uring 1,142%.
At eight workers Rust also trailed C's throughput. Do not claim universal
performance parity from the sequential results. On this host, libaio currently
uses less CPU than io_uring for mixed workloads.

Profiling located contention in Linux block-queue dispatch, especially
`native_queued_spin_lock_slowpath` beneath `blk_mq_dispatch_plug_list` and
its delayed-work scheduling. Direction-aware submission groups help libaio.
Explicit one-request io_uring submissions and deferred task-run/single-issuer
experiments did not remove the mixed-I/O CPU cost and were not made defaults.
Automatic selection remains io_uring when queue creation succeeds, else libaio.

## Evidence and reproduction

[results-2026-10-22.json](results-2026-10-22.json) retains individual measurements,
campaign labels, final source/binary hashes and zero-allocation results. The
initial refill campaign is labelled as a regression, not the final io_uring
implementation. Full subprocess logs remain in `/tmp/spit-rust-*.json` on the
development host. These short RAM tests need sustained and physical-storage
measurements before broader performance claims.

Verification IOPS is not directly comparable: C performs final verification
outside its timed workload; Rust verifies ordinary writes immediately inside
the loop. Metadata modes schedule read phases and finish pending checks after
workload limits. Assess correctness and total elapsed time as well.

From the repository root, with administrator authorization:

```sh
python3 rust/tests/ram_compare.py --c c/build/spit/spit --rust rust/target/release/spit --backends libaio --seconds 2 --repeats 3
python3 rust/tests/uring_validate.py --backends io_uring --seconds 2 --repeats 3
python3 rust/tests/uring_validate.py --backends libaio,io_uring --operations rw --sequential 0 --seconds 2 --repeats 3
```

The helpers create/detach their own loop devices and never accept an existing
device. `uring_validate.py` enables io_uring, runs the full release suite,
applicable safety/block checks and benchmarks, then restores the original
sysctl. Libaio fault injection does not cover io_uring.
