#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "$0")/.." && pwd)
format=${1:-auto}
if [[ $format == auto ]]; then
    . /etc/os-release
    case " $ID ${ID_LIKE:-} " in
        *debian*|*ubuntu*|*linuxmint*) format=deb ;;
        *rhel*|*fedora*|*centos*|*rocky*|*almalinux*) format=rpm ;;
        *) echo 'Use make rpm or make deb on a matching Linux distribution.' >&2; exit 2 ;;
    esac
fi
case $format in rpm|deb|source) ;; *) echo "Unknown package format: $format" >&2; exit 2;; esac
archive=$(python3 "$root/packaging/source.py")
[[ $format == source ]] && { echo "$archive"; exit 0; }
version=$(cat "$root/VERSION")
work=$(mktemp -d "${TMPDIR:-/tmp}/stutools-package.XXXXXXXX")
trap 'rm -rf -- "$work"' EXIT
jobs=${JOBS:-4}
if [[ $format == rpm ]]; then
    command -v rpmbuild >/dev/null || { echo 'Run ./install-deps.sh to install RPM tools.' >&2; exit 1; }
    mkdir -p "$work"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}
    cp "$archive" "$work/SOURCES/"
    rpmbuild -ba --define "_topdir $work" --define "_smp_mflags -j$jobs" "$root/packaging/stutools-c.spec"
    find "$work/RPMS" "$work/SRPMS" -name '*.rpm' -exec cp -t "$root/dist/" {} +
else
    command -v dpkg-buildpackage >/dev/null || { echo 'Run ./install-deps.sh to install DEB tools.' >&2; exit 1; }
    tar -xzf "$archive" -C "$work"
    src="$work/stutools-$version"
    cp "$archive" "$work/stutools-c_${version}.orig.tar.gz"
    cat > "$src/debian/changelog" <<CHANGELOG
stutools-c ($version-1) unstable; urgency=medium

  * Build stutools from the supplied source archive.

 -- Stuart Inglis <stuart.inglis@gmail.com>  $(date -R)
CHANGELOG
    (cd "$src" && dpkg-buildpackage -us -uc -b -j"$jobs")
    find "$work" -maxdepth 1 -type f \( -name '*.deb' -o -name '*.changes' -o -name '*.buildinfo' \) -exec cp -t "$root/dist/" {} +
fi
printf 'Packages are in %s/dist/\n' "$root"
