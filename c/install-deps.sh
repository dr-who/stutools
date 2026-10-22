#!/usr/bin/env bash
set -euo pipefail
# Install both compilation and native packaging prerequisites by default.
packaging=1
case "${1:-}" in
    --build-only) packaging=0 ;;
    '' ) ;;
    *) echo "Usage: $0 [--build-only]" >&2; exit 2 ;;
esac
if (( EUID != 0 )); then
    exec sudo -- "$0" "$@"
fi
. /etc/os-release
case " $ID ${ID_LIKE:-} " in
    *debian*|*ubuntu*|*linuxmint*)
        apt-get update
        packages=(build-essential cmake git libaio-dev libpci-dev libnuma-dev libreadline-dev uuid-dev python3)
        if ((packaging)); then packages+=(debhelper dpkg-dev fakeroot); fi
        DEBIAN_FRONTEND=noninteractive apt-get install -y "${packages[@]}"
        ;;
    *rhel*|*fedora*|*centos*|*rocky*|*almalinux*)
        manager=dnf
        command -v dnf >/dev/null || manager=yum
        packages=(gcc gcc-c++ make cmake git libaio-devel pciutils-devel numactl-devel readline-devel libuuid-devel python3)
        if ((packaging)); then packages+=(rpm-build); fi
        "$manager" install -y "${packages[@]}"
        ;;
    *) echo "Unsupported distribution: $ID. See INSTALL for dependency names." >&2; exit 1 ;;
esac
cmake --version
