#!/bin/sh
set -eu
# Run in a disposable Debian/Rocky/Fedora container, with the C source archive
# mounted at /input and an empty artifact directory mounted at /artifacts.
if command -v apt-get >/dev/null; then
    export DEBIAN_FRONTEND=noninteractive
    apt-get update
    apt-get install -y build-essential cmake git libaio-dev libpci-dev \
        libnuma-dev libreadline-dev uuid-dev python3 debhelper dpkg-dev fakeroot lintian
else
    dnf install -y gcc gcc-c++ make cmake git libaio-devel pciutils-devel \
        numactl-devel readline-devel libuuid-devel python3 rpm-build
fi
archive=${STUTOOLS_SOURCE_ARCHIVE:?Set STUTOOLS_SOURCE_ARCHIVE}
case "$archive" in stutools-*.tar.gz) ;; *) exit 2 ;; esac
mkdir -p /work
tar -xzf "/input/$archive" -C /work
cd "/work/${archive%.tar.gz}"
JOBS=4 ./packaging/build.sh auto
if command -v dpkg >/dev/null; then
    lintian --fail-on error dist/*.deb
    dpkg -i dist/*.deb
    verification=$(dpkg -V stutools-c)
    test -z "$verification"
else
    rpm -Uvh dist/*.$(uname -m).rpm
    rpm -V stutools-c
fi
test -x /usr/bin/fsfiller
test -x /usr/bin/spitchecker
/usr/bin/spitc -h
/usr/bin/spitc -f /work/scratch.img -G 1MiB -c ws1k4q8j4x1 -v -K 0,256
if command -v dpkg >/dev/null; then dpkg -r stutools-c; else rpm -e stutools-c; fi
test ! -e /usr/bin/spitc
cp dist/* /artifacts/
