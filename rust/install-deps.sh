#!/usr/bin/env sh
set -eu
if [ "$(id -u)" = 0 ]; then sudo_cmd=''; else sudo_cmd=sudo; fi
if [ "$(uname -s)" = Darwin ]; then
    xcode-select -p >/dev/null || { echo 'Install Xcode Command Line Tools: xcode-select --install' >&2; exit 1; }
elif [ -f /etc/os-release ]; then
    . /etc/os-release
    case " $ID ${ID_LIKE:-} " in
      *debian*|*ubuntu*|*linuxmint*)
        $sudo_cmd apt-get update
        $sudo_cmd apt-get install -y build-essential libaio-dev python3 curl ca-certificates dpkg-dev debhelper lintian
        ;;
      *rhel*|*fedora*|*centos*|*rocky*|*almalinux*)
        $sudo_cmd dnf install -y gcc libaio-devel python3 curl ca-certificates rpm-build
        ;;
      *) echo 'Install a C linker, libaio development headers (Linux), Python 3 and package tools.' >&2; exit 1;;
    esac
fi
if ! command -v cargo >/dev/null 2>&1; then
    echo 'Install Rust 1.85 or newer from https://rustup.rs, then run cargo build.' >&2
    exit 1
fi
cargo --version
rustc --version
