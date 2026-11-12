#!/bin/sh
# Builds the LIO test target for spit's persistent reservation cases: one RAM-backed LUN (a sparse file on
# tmpfs, so it is thin and holds no data after a reboot), exposed through several portals in the one
# default target port group (active/optimized), with one ACL per initiator name. Nothing is saved to
# /etc/target: the target lasts until teardown.sh or a reboot. Run as root. Idempotent.
#
#   harness/lio-pr-ram/setup.sh
#
# Override with environment variables:
#   TARGET_IQN   the target name             (default iqn.2026-12.com.stutools:spit-pr-ram)
#   PORTALS      space-separated IP addresses (default 127.0.0.2 127.0.0.3 127.0.0.4), port 3260
#   INITIATORS   space-separated initiator IQNs (default the three spit initiator names below)
#   LUN_SIZE     size of the sparse file     (default 2G)
set -eu

TARGET_IQN=${TARGET_IQN:-iqn.2026-12.com.stutools:spit-pr-ram}
PORTALS=${PORTALS:-"127.0.0.2 127.0.0.3 127.0.0.4"}
INITIATORS=${INITIATORS:-"iqn.2026-11.com.stutools:spit-initiator iqn.2026-11.com.stutools:spit-initiator-b iqn.2026-11.com.stutools:spit-initiator-c"}
LUN_SIZE=${LUN_SIZE:-2G}
BACKSTORE=spitram
IMAGE=/dev/shm/$BACKSTORE.img

if targetcli ls "/iscsi/$TARGET_IQN" >/dev/null 2>&1; then
    echo "target $TARGET_IQN already exists; nothing to do"
    exit 0
fi

# Refuse to start if a portal address is already listening on 3260: LIO would not bind it.
for ip in $PORTALS; do
    if ss -ltn | grep -q "$ip:3260 "; then
        echo "error: $ip:3260 is already in use" >&2
        exit 1
    fi
done

targetcli /backstores/fileio create name=$BACKSTORE file_or_dev=$IMAGE size=$LUN_SIZE
targetcli /iscsi create "$TARGET_IQN"
targetcli "/iscsi/$TARGET_IQN/tpg1/luns" create "/backstores/fileio/$BACKSTORE"
# No CHAP, no generated ACLs: each initiator is listed, and only those log in.
targetcli "/iscsi/$TARGET_IQN/tpg1" set attribute authentication=0 generate_node_acls=0 demo_mode_write_protect=0
for ip in $PORTALS; do
    targetcli "/iscsi/$TARGET_IQN/tpg1/portals" create "$ip" 3260
done
for iqn in $INITIATORS; do
    targetcli "/iscsi/$TARGET_IQN/tpg1/acls" create "$iqn"
done
echo "created $TARGET_IQN with portals: $PORTALS"
