# Test harnesses

Scripts that build the targets spit's tests run against, so a setup can be rebuilt after a reboot.
Each harness has a `setup.sh` (idempotent), a `teardown.sh` where the target is local, and a `run.sh`.
They are run as root on the test host. None of them save LIO's configuration, and none contain secrets.

## lio-pr-ram

A RAM-backed LIO target for the persistent reservation cases (SPC-3 5.6, 6.11, 6.12).

- One LUN: a sparse 2 GB file in `/dev/shm`. It is thin and holds nothing after a reboot.
- Three portals (127.0.0.2, 127.0.0.3, 127.0.0.4, port 3260) in the one target port group, which is
  active/optimized. A reservation through one portal must be enforced on all of them (LU scope).
- One ACL per initiator name (`spit-initiator`, `spit-initiator-b`, `spit-initiator-c`), no CHAP.

    harness/lio-pr-ram/setup.sh
    harness/lio-pr-ram/run.sh /tmp/spit-pr-ram.log
    harness/lio-pr-ram/teardown.sh

The lab targets (`spitlunsbc-*`, `spit-test`) are not touched by any of these.

## Destructive tests

FORMAT UNIT, REASSIGN BLOCKS, SANITIZE, WRITE LONG, the state-changing commands and the others that
destroy data run only on a LUN that is designated for them, and only with `--destructive`. The RAM LUN
here is the designated one: its contents are lost on teardown anyway. A destructive test never runs on a
disk or a lab LUN, however the command is given.
