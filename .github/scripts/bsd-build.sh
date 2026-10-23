#!/bin/sh
# Run inside a real BSD VM. Build outside the synced tree; copy back only assets.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
case "$(uname -s)" in
    FreeBSD) system=freebsd ;;
    OpenBSD) system=openbsd; ulimit -d unlimited ;;
    *) echo 'Run this script inside FreeBSD or OpenBSD.' >&2; exit 2 ;;
esac
version=$(awk -F '"' '/^version =/ {print $2; exit}' "$root/rust/Cargo.toml")
work=$(mktemp -d /tmp/stutools-bsd.XXXXXXXX)
trap 'rm -rf "$work"' EXIT HUP INT TERM
uname -a
rustc --version
cargo --version
cmake --version
tar -xzf "$root/rust/dist/stutools-$version.tar.gz" -C "$work"
source_dir="$work/stutools-$version"
(
    cd "$source_dir"
    cargo build --release --locked --offline
    cargo test --release --locked --offline
)
cmake -S "$root/c" -B "$work/c" -G Ninja -DCMAKE_BUILD_TYPE=Release
cmake --build "$work/c" --parallel 2
ctest --test-dir "$work/c" --output-on-failure -L smoke
DESTDIR="$work/install" cmake --install "$work/c" --prefix /usr/local
"$work/install/usr/local/bin/spitc" --version
name="spit-$version-$system-$(uname -m)"
mkdir -p "$work/$name" "$root/rust/dist"
cp "$source_dir/target/release/spit" "$work/$name/spit"
cp "$work/install/usr/local/bin/spitc" "$work/$name/spitc"
cp "$root/rust/packaging/spit.1" "$root/rust/README.md" "$root/rust/THIRD_PARTY_LICENSES.txt" "$root/LICENSE" "$work/$name/"
cp "$root/c/platform/README.md" "$work/$name/README-spitc.md"
cp "$root/rust/CCTV.md" "$work/$name/"
tar -czf "$root/rust/dist/$name.tar.gz" -C "$work" "$name"
