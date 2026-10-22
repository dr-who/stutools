#!/bin/sh
set -eu
# Invoked inside a disposable distribution container. Mount the source archive
# at /input, an official Rust toolchain at /opt/rust and artifacts at /artifacts.
export PATH=/opt/rust/bin:$PATH
if command -v apt-get >/dev/null; then
    export DEBIAN_FRONTEND=noninteractive
    apt-get update
    apt-get install -y build-essential libaio-dev python3 dpkg-dev debhelper lintian
    # The mounted Rust toolchain is checked by the package builder. Other
    # build dependencies are still checked by dpkg-buildpackage.
    export STUTOOLS_EXTERNAL_RUST=1
    . /etc/os-release
    export STUTOOLS_DEB_SUFFIX="${ID}${VERSION_ID}"
else
    dnf install -y gcc libaio python3 rpm-build diffutils findutils
    if dnf list --available libaio-devel >/dev/null 2>&1; then
        dnf install -y libaio-devel
    else
        # UBI provides the runtime but omits the -devel package. Rust declares
        # this ABI itself; the test shim has an ABI header fallback. Supply only
        # a build-local linker name, without modifying system library files.
        mkdir -p /tmp/stutools-link
        ln -s /usr/lib64/libaio.so.1 /tmp/stutools-link/libaio.so
        export RUSTFLAGS="-L native=/tmp/stutools-link"
    fi
fi
mkdir -p /work
cd /work
# The caller selects one archive, so an old artifact cannot be packaged by accident.
archive=${STUTOOLS_SOURCE_ARCHIVE:?Set STUTOOLS_SOURCE_ARCHIVE to the source archive basename}
case "$archive" in stutools-*.tar.gz) ;; *) echo 'Invalid source archive name' >&2; exit 2 ;; esac
tar -xzf "/input/$archive"
cd "${archive%.tar.gz}"
cargo build --release --locked --offline
cargo test --locked --offline
python3 packaging/build.py auto
if command -v dpkg >/dev/null; then
    lintian --fail-on error dist/*.deb
    # Minimal container images may exclude documentation globally. Exercise
    # the package's man page and licence files as well as its executable.
    dpkg --path-include='/usr/share/doc/stutools/*' --path-include='/usr/share/man/man1/spit.1*' -i dist/*.deb
else
    rpm -Uvh dist/*.rpm
fi
/usr/bin/spit --help
/usr/bin/spit -f /work/scratch.img -G 1MiB -c ws1k4q8j4x1 -v -K 0,256
if command -v dpkg >/dev/null; then
    test -f /usr/share/doc/stutools/copyright
    test -f /usr/share/man/man1/spit.1.gz
    verification=$(dpkg -V stutools)
    test -z "$verification"
    dpkg -r stutools
else
    rpm -V stutools
    rpm -e stutools
fi
test ! -e /usr/bin/spit
cp dist/* /artifacts/
