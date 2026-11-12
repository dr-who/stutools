# Fault scenarios

`pr-faults.sh` runs the reservation-survival scenarios on the RAM targets (`harness/lio-ram`) with spit's
`--hold` and `--hold-probe` modes. Each scenario takes a type-5 reservation through a holder, injects a fault,
and checks the reservation from another session:

- **Session kill:** the holder is killed without logout. The reservation and registration survive (SPC-3 5.6.4).
- **Portal loss:** the holder's portal is removed from the target. The reservation is seen through another portal.
- **ALUA change:** the default port group goes to active/non-optimized. The standby portal still sees the
  reservation. The holder's first CLEAR after the change returns CHECK CONDITION 6/2A/06 (ALUA state changed):
  the unit attention is reported before the command is served, as SPC-3 requires.

Run it as root after `harness/lio-ram/setup-all.sh`. It exits non-zero on any failed check, and it leaves the
targets clear. It does not touch the lab targets.
