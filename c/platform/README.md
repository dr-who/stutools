# Native macOS, Windows and FreeBSD builds

CMake 3.20+ and a C11 compiler are required. Apple Clang, MSVC and LLVM-MinGW
are supported by the build configuration. Use an out-of-source build:

```
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build --config Release --parallel 4
ctest --test-dir build -C Release --output-on-failure -L smoke
cmake --build build --config Release --target package
```

The executable is `build/platform/spit` on macOS and FreeBSD. Windows uses
`build/platform/spit.exe` (Ninja/Makefiles) or
`build/platform/Release/spit.exe` (Visual Studio).

FreeBSD prerequisites: `pkg install rust cmake ninja`. CMake selects this portable C
backend automatically on FreeBSD; the complete libaio C suite remains Linux-only.
GitHub Actions builds and tests Rust spit and this C backend in a real FreeBSD VM.

This is an initial native file backend, not the Linux libaio engine. It uses
buffered synchronous I/O, one job, one worker, and queue depth one. It supports
`-f`, `-G` (including ranges), `-c`, `-t`, `-R`, `-v`, and `--recreate`;
workload commands include r/w/rw, p, s0/s1, k/M, q1, j1, x, X, P, and z.
`-v` reads each written block back and compares every byte. Read verification
can be served from the filesystem cache; it is not a power-loss durability test.
Existing files are never resized unless a writing job uses `--recreate`.

Raw block devices, Linux direct/AIO behavior, higher queue depths, multiple
workers, preconditioning, metadata jobs, and saved-position replay are not yet
ported. Unsupported options are rejected. Do not compare native buffered
throughput directly with Linux O_DIRECT/AIO results. The Linux engine and its
existing options are unchanged by the native build selection.

Example:

```
spit -f test.dat -G 64MiB -c ws1k4q1x1 -v
spit -f test.dat -c rs1k4q1x1
```
