#!/bin/sh
# Reservation survival across faults on the RAM targets (SPC-3 5.6.4: registrations and reservations persist
# until released, cleared or the power is lost). Run as root, after harness/lio-ram/setup-all.sh.
# Each scenario takes a type-5 reservation through a holder (spit --hold), injects the fault, then checks the
# reservation from another session (spit --hold-probe). Exit status is 0 only when every check passes.
set -u
SPIT=${SPIT:-$(cd "$(dirname "$0")/../.." && pwd)/rust/target/debug/spit}
I=iqn.2026-11.com.stutools:spit-initiator
B=iqn.2026-11.com.stutools:spit-initiator-b
PR=iqn.2026-12.com.stutools:spit-pr-ram
ALUA=iqn.2026-12.com.stutools:spit-alua-ram
TMP=$(mktemp -d)
fails=0

check() { # name expected actual
    if [ "$2" = "$3" ]; then echo "PASS  $1"; else echo "FAIL  $1: expected [$2] got [$3]"; fails=$((fails + 1)); fi
}
probe() { # portal target
    timeout 30 "$SPIT" --iscsi "$1/$2" --iscsi-initiator "$B" --hold-probe 2>&1 | tail -1
}
hold_start() { # name portal target
    mkfifo "$TMP/$1.in"
    "$SPIT" --iscsi "$2/$3" --iscsi-initiator "$I" --hold 5 < "$TMP/$1.in" > "$TMP/$1.out" 2>&1 &
    echo $! > "$TMP/$1.pid"
    exec 9>"$TMP/$1.in"
    sleep 3
}
hold_end() { # name: quit (CLEAR) then close
    echo quit >&9
    exec 9>&-
    sleep 2
}
holding() { # name: the holder's state line
    echo status >&9 2>/dev/null
}

HELD="status: key 0x53504954000000a1, type 5; keys: 0x53504954000000a1"
CLEAR="status: none; keys: none"

# 1. Session kill: the holder dies without logging out; the reservation must survive.
hold_start k 127.0.0.2 "$PR"
check "held before the session is killed" "held: type 5, key 0x53504954000000a1" "$(head -1 "$TMP/k.out")"
kill -9 "$(cat "$TMP/k.pid")" 2>/dev/null; exec 9>&-; sleep 2
check "reservation survives a killed session" "$HELD" "$(probe 127.0.0.2 "$PR")"
hold_start k2 127.0.0.2 "$PR"; hold_end; sleep 1

# 2. Portal loss: the holder's portal is removed; the reservation must be seen through another portal.
hold_start p 127.0.0.2 "$PR"
targetcli /iscsi/$PR/tpg1/portals delete 127.0.0.2 3260 >/dev/null 2>&1; sleep 2
check "reservation seen through 127.0.0.3 after 127.0.0.2 is removed" "$HELD" "$(probe 127.0.0.3 "$PR")"
targetcli /iscsi/$PR/tpg1/portals create 127.0.0.2 3260 >/dev/null 2>&1
kill -9 "$(cat "$TMP/p.pid")" 2>/dev/null; exec 9>&-; sleep 2
hold_start p2 127.0.0.3 "$PR"; hold_end; sleep 1

# 3. ALUA state change: the default group goes active/non-optimized; the standby portal must still see it.
hold_start a 127.0.0.7 "$ALUA"
G=$(ls -d /sys/kernel/config/target/core/fileio_*/spitalua | head -1)/alua/default_tg_pt_gp
echo 1 > "$G/alua_access_state"; sleep 2
check "reservation seen through the standby portal after an ALUA change" "$HELD" "$(probe 127.0.0.8 "$ALUA")"
echo 0 > "$G/alua_access_state"
hold_end; sleep 1
# Clear the ALUA target through a fresh holder on the optimized portal, which then receives quit (CLEAR).
hold_start c 127.0.0.7 "$ALUA"
hold_end; sleep 2
check "ALUA target is clear after the scenarios" "$CLEAR" "$(probe 127.0.0.8 "$ALUA")"
check "PR target is clear after the scenarios" "$CLEAR" "$(probe 127.0.0.2 "$PR")"

for f in "$TMP"/*.out; do [ -f "$f" ] && echo "[$(basename "$f")] $(tail -1 "$f")"; done
rm -rf "$TMP"
echo "failures: $fails"
[ "$fails" -eq 0 ]
