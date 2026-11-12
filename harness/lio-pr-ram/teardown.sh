#!/bin/sh
# Removes what setup.sh made: the target, the LUN's backstore and its RAM file. Does not touch any other
# target. Run as root.
set -eu
TARGET_IQN=${TARGET_IQN:-iqn.2026-12.com.stutools:spit-pr-ram}
BACKSTORE=spitram
if targetcli ls "/iscsi/$TARGET_IQN" >/dev/null 2>&1; then
    targetcli "/iscsi" delete "$TARGET_IQN"
fi
if targetcli ls "/backstores/fileio/$BACKSTORE" >/dev/null 2>&1; then
    targetcli "/backstores/fileio" delete "$BACKSTORE"
fi
rm -f "/dev/shm/$BACKSTORE.img"
echo "removed $TARGET_IQN"
