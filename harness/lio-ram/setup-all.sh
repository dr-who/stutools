#!/bin/sh
# Builds the set of RAM-backed LIO targets spit's tests run against. Every LUN is a sparse file on tmpfs:
# thin, holds nothing after a reboot, and is never saved to /etc/target (no saveconfig). Run as root.
# Idempotent: a target that already exists is left as it is.
#
#   pr-ram     one LUN, three portals (127.0.0.2-4) in the active/optimized group, three initiator ACLs:
#              the persistent reservation cases, with the initiators spread over the portals.
#   thin-ram   a 4 TB sparse LUN with UNMAP enabled, on 127.0.0.5: LBAs past 32 bits.
#   chap-ram   one LUN on 127.0.0.6 with one-way CHAP (secret in /var/tmp/spit-chap/secret) and mutual CHAP
#              (secret in /var/tmp/spit-chap/mutual). Both files are created 0600 if missing, and never printed.
#   exa-ram    a 1 EiB sparse LUN on 127.0.0.9: LBAs past 2^51, READ CAPACITY (16) and the 64-bit paths.
#   alua-ram   one LUN in two TPGs: 127.0.0.7 in the default group (active/optimized) and 127.0.0.8 in a
#              second group set to standby. A path to the standby portal must answer as standby (SPC-3 5.8.2).
set -eu

INITIATORS="iqn.2026-11.com.stutools:spit-initiator iqn.2026-11.com.stutools:spit-initiator-b iqn.2026-11.com.stutools:spit-initiator-c"
CHAP_USER=spit-chap-user
CHAP_TARGET=spit-chap-target
SECRETS=/var/tmp/spit-chap
ALUA_BACKSTORE=spitalua

mkdir -p -m 700 "$SECRETS"
[ -f "$SECRETS/secret" ] || (umask 077; openssl rand -hex 16 > "$SECRETS/secret")
[ -f "$SECRETS/mutual" ] || (umask 077; openssl rand -hex 16 > "$SECRETS/mutual")

fileio() { # name size
    targetcli /backstores/fileio ls "$1" >/dev/null 2>&1 || \
        targetcli /backstores/fileio create name="$1" file_or_dev=/dev/shm/"$1".img size="$2" >/dev/null
}

acls() { # target
    for iqn in $INITIATORS; do
        targetcli "/iscsi/$1/tpg1/acls" create "$iqn" >/dev/null 2>&1 || true
    done
}

target() { # iqn backstore portal
    if targetcli "/iscsi/$1" ls >/dev/null 2>&1; then
        echo "exists: $1"
        return 1
    fi
    targetcli /iscsi create "$1" >/dev/null
    targetcli "/iscsi/$1/tpg1/luns" create "/backstores/fileio/$2" >/dev/null
    targetcli "/iscsi/$1/tpg1" set attribute authentication=0 generate_node_acls=0 demo_mode_write_protect=0 >/dev/null
    targetcli "/iscsi/$1/tpg1/portals" create "$3" 3260 >/dev/null
    acls "$1"
    echo "created: $1 on $3"
}

# pr-ram
fileio spitram 2G
if target iqn.2026-12.com.stutools:spit-pr-ram spitram 127.0.0.2; then
    for ip in 127.0.0.3 127.0.0.4; do
        targetcli /iscsi/iqn.2026-12.com.stutools:spit-pr-ram/tpg1/portals create "$ip" 3260 >/dev/null
    done
fi

# thin-ram: UNMAP and WRITE SAME UNMAP on, so the provisioning cases apply
fileio spitthin 4T
targetcli /backstores/fileio/spitthin set attribute emulate_tpu=1 emulate_tpws=1 >/dev/null
target iqn.2026-12.com.stutools:spit-thin-ram spitthin 127.0.0.5 || true

# chap-ram: one-way and mutual CHAP, the same credentials on each ACL
fileio spitchap 1G
T=iqn.2026-12.com.stutools:spit-chap-ram
if ! targetcli "/iscsi/$T" ls >/dev/null 2>&1; then
    target "$T" spitchap 127.0.0.6 || true
    P=$(cat "$SECRETS/secret"); M=$(cat "$SECRETS/mutual")
    targetcli "/iscsi/$T/tpg1" set attribute authentication=1 >/dev/null
    targetcli "/iscsi/$T/tpg1" set auth userid="$CHAP_USER" password="$P" mutual_userid="$CHAP_TARGET" mutual_password="$M" >/dev/null
    for iqn in $INITIATORS; do
        targetcli "/iscsi/$T/tpg1/acls/$iqn" set auth userid="$CHAP_USER" password="$P" mutual_userid="$CHAP_TARGET" mutual_password="$M" >/dev/null
    done
    unset P M
fi

# exa-ram: a 1 EiB sparse LUN (2^51 blocks of 512 B) on tmpfs, thin with UNMAP on, on 127.0.0.9. LIO takes the
# size in bytes only. It holds no data beyond what is written, so it is safe to create and destroy at will.
fileio spitexa 1152921504606846976
targetcli /backstores/fileio/spitexa set attribute emulate_tpu=1 emulate_tpws=1 >/dev/null
target iqn.2026-12.com.stutools:spit-exa-ram spitexa 127.0.0.9 || true

# Block sizes and protection information. LIO takes block_size 512 or 4096 only: a 64 KiB logical block cannot be
# presented by LIO (the Linux block layer caps logical blocks at the page size), so there is no 64 KiB LUN here.
# LIO implements PI types 1 and 3; type 2 is refused ("Function not implemented"), so it is not configured.
fileio spitblk4k 1G
targetcli /backstores/fileio/spitblk4k set attribute block_size=4096 >/dev/null
target iqn.2026-12.com.stutools:spit-blk4k-ram spitblk4k 127.0.0.10 || true
fileio spitpi1 1G
targetcli /backstores/fileio/spitpi1 set attribute pi_prot_type=1 >/dev/null
target iqn.2026-12.com.stutools:spit-pi1-ram spitpi1 127.0.0.11 || true
fileio spitpi3 1G
targetcli /backstores/fileio/spitpi3 set attribute pi_prot_type=3 >/dev/null
target iqn.2026-12.com.stutools:spit-pi3-ram spitpi3 127.0.0.12 || true

# alua-ram: group 1 is the standby group. It needs a nonzero ID and standby support before its state can be
# set (LIO refuses the state otherwise), and the second TPG's LUN is attached to it through configfs.
fileio "$ALUA_BACKSTORE" 1G
T=iqn.2026-12.com.stutools:spit-alua-ram
if ! targetcli "/iscsi/$T" ls >/dev/null 2>&1; then
    target "$T" "$ALUA_BACKSTORE" 127.0.0.7 || true
    G=$(ls -d /sys/kernel/config/target/core/fileio_*/"$ALUA_BACKSTORE" | head -1)
    mkdir -p "$G/alua/standby"
    echo 1 > "$G/alua/standby/tg_pt_gp_id"
    echo 1 > "$G/alua/standby/alua_support_standby"
    echo 2 > "$G/alua/standby/alua_access_state"
    C=/sys/kernel/config/target/iscsi/$T
    mkdir "$C/tpgt_2" "$C/tpgt_2/np/127.0.0.8:3260"
    echo 0 > "$C/tpgt_2/attrib/generate_node_acls"
    mkdir -p "$C/tpgt_2/lun/lun_0"
    ln -s "$G" "$C/tpgt_2/lun/lun_0/$ALUA_BACKSTORE"
    echo standby > "$C/tpgt_2/lun/lun_0/alua_tg_pt_gp"
    for iqn in $INITIATORS; do
        mkdir -p "$C/tpgt_2/acls/$iqn"
        ln -sf "$G" "$C/tpgt_2/acls/$iqn/" 2>/dev/null || true
    done
    echo 1 > "$C/tpgt_2/enable"
    echo "created: $T with a standby TPG on 127.0.0.8"
fi
