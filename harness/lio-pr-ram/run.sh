#!/bin/sh
# Runs spit's compliance suite against the setup.sh target through the first portal, with the first two
# initiator names, so the three-port persistent reservation cases run. Writes the log to LOG.
#   harness/lio-pr-ram/run.sh [LOG]
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
TARGET_IQN=${TARGET_IQN:-iqn.2026-12.com.stutools:spit-pr-ram}
PORTAL=${PORTAL:-127.0.0.2}
INITIATORS=${INITIATORS:-"iqn.2026-11.com.stutools:spit-initiator iqn.2026-11.com.stutools:spit-initiator-b"}
LOG=${1:-/tmp/spit-pr-ram.log}
SPIT=${SPIT:-$ROOT/rust/target/debug/spit}
set -- $INITIATORS
"$SPIT" --iscsi "$PORTAL/$TARGET_IQN" --iscsi-initiator "$1,$2" --compliance >"$LOG" 2>&1 || true
echo "log: $LOG"
sed -n '/^SCSI level 3/,/^total/p' "$LOG"
