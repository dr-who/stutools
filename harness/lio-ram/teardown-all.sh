#!/bin/sh
# Removes the RAM targets setup-all.sh makes, their LUNs and their RAM files. Only these names are touched.
# Run as root.
set -eu
for T in iqn.2026-12.com.stutools:spit-pr-ram iqn.2026-12.com.stutools:spit-thin-ram \
         iqn.2026-12.com.stutools:spit-chap-ram iqn.2026-12.com.stutools:spit-alua-ram; do
    targetcli "/iscsi" delete "$T" >/dev/null 2>&1 || true
done
for B in spitram spitthin spitchap spitalua; do
    targetcli "/backstores/fileio" delete "$B" >/dev/null 2>&1 || true
    rm -f "/dev/shm/$B.img"
done
echo "removed the RAM targets"
