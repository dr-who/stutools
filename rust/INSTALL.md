# Install stutools 2.0.0

Downloads are on the [GitHub releases page](https://github.com/dr-who/stutools/releases).
The 2.0.0 release is not published there yet; until it is, build from source (see
[Building instead](#building-instead)). The tables below list the files the release
is built to provide. No Rust or C compiler is required. Choose your OS and architecture, then compare
the file's SHA-256 with the release's `SHA256SUMS` before installing.

## Linux: DEB or RPM

| System | Package | Install |
|---|---|---|
| Debian 12 / compatible newer Debian | `stutools_2.0.0-1+debian12_amd64.deb` | `sudo apt install ./stutools_2.0.0-1+debian12_amd64.deb` |
| Ubuntu 24.04 / Mint 22 | `stutools_2.0.0-1+ubuntu24.04_amd64.deb` | `sudo apt install ./stutools_2.0.0-1+ubuntu24.04_amd64.deb` |
| RHEL / Rocky / Alma 9 | `stutools-2.0.0-1.el9.x86_64.rpm` | `sudo dnf install ./stutools-2.0.0-1.el9.x86_64.rpm` |
| Fedora 44 | `stutools-2.0.0-1.fc44.x86_64.rpm` | `sudo dnf install ./stutools-2.0.0-1.fc44.x86_64.rpm` |

Linux packages install **`/usr/bin/spit`** and `man spit`, with runtime dependencies
resolved by the package manager. They do not install into `/usr/local/bin`.
The optional `stutools-c` DEB/RPM installs **`/usr/bin/spitc`**, fsfiller and the
remaining C utilities; it can coexist with Rust stutools 2.0.

To remove: `sudo apt remove stutools` or `sudo dnf remove stutools`.
These are upstream GitHub downloads, not official distro archive packages.
The RHEL-family RPM is built on Rocky 9 and checked on Rocky/RHEL UBI; Alma uses
the compatible EL9 baseline. Fedora packages must match the Fedora release.

## macOS: PKG installer or portable archive

The universal `.pkg` includes Apple silicon and Intel code. On macOS 11 or newer:

```sh
sudo installer -pkg spit-2.0.0-macos-universal.pkg -target /
spit --help
man spit
```

It installs `/usr/local/bin/spit`, a man page and docs under `/usr/local/share`.
macOS reserves `/usr/bin` for the OS; this differs deliberately from Linux.
The package is unsigned/not notarized; it is not an Apple-verified installer.
If macOS blocks opening the downloaded installer, review it and use the explicit
Open Anyway action in System Settings, or the Terminal installer command above.

Alternatively extract `spit-2.0.0-macos-arm64.tar.gz` (Apple silicon) or
`spit-2.0.0-macos-x86_64.tar.gz` (Intel), then run `./spit --help` there.
To install an extracted binary manually:

```sh
sudo install -m755 spit /usr/local/bin/spit
```

The installer contains no background service. To remove its installed files,
remove `/usr/local/bin/spit`, `/usr/local/share/man/man1/spit.1` and
`/usr/local/share/doc/stutools`; then forget the receipt with
`sudo pkgutil --forget org.stutools.spit`.

## Windows x64: EXE or ZIP

Download `spit-2.0.0-windows-x86_64.exe`, rename it to `spit.exe` if convenient,
and run it in PowerShell: `.\spit.exe --help`. Or extract the portable
`spit-2.0.0-windows-x86_64.zip` and run the included `spit.exe`.
Keep that directory and optionally add it to your user PATH. No administrator
rights are required to test a normal file. This is a command-line application;
launch it from a terminal so you can read its output.

The executable statically links the Microsoft C runtime. Visual Studio and the
Visual C++ redistributable are not required. Delete the executable/directory to
uninstall. Regular files use direct (unbuffered) I/O by default; `--backend pagecache` selects
buffered I/O. Raw volumes can be read (reads only). Raw-volume writes and IOCP are
not implemented.

## Check the download

`SHA256SUMS` covers every asset except the checksum list itself:

```sh
sha256sum FILE                  # Linux
shasum -a 256 FILE               # macOS
```

PowerShell: `Get-FileHash FILE -Algorithm SHA256`. Compare the complete hash with
the matching filename in `SHA256SUMS`.

## First run

Write workloads overwrite their selected range. Use a disposable file:

```sh
spit -f scratch.img -G 16MiB -c ws1k4q32j4x1 -v
spit -f scratch.img -c rs1k4q32j4 -t 5
```

Rust 2.0 supports the common C workload syntax, not every C workflow. Its data
format differs from C spitchecker; use `spitc` for C-format verification, NUMA,
copy/trim and other advanced C modes. Read README.md and `spit --help` before
replacing scripts. On Linux the default direct backend uses io_uring when available,
otherwise libaio; Windows and macOS use direct I/O. `--help` shows the detected default.

## Building instead

The source archive vendors locked dependencies for offline Cargo builds.
From the repository, `cd rust; cargo build --release --locked` builds spit;
`make dist` builds/tests and creates a native package. Linux `make install`
defaults to `/usr/bin`, macOS to `/usr/local/bin`; PREFIX can be overridden.
See README.md for build prerequisites and compatibility details.
