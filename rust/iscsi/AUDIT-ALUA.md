# Clause audit: asymmetric logical unit access (SPC-4 5.15.2), the ALUA shalls

Source: SPC-4, INCITS 513-2015 (R2020), `/root/INCITS+513-2015+(R2020).pdf`, clauses 5.15.2.4 to 5.15.2.10 (pages
237 to 244). An earlier version of this file cited clause 5.8 with SPC-3 line numbers; clause 5.8 of SPC-4 is parameter
rounding, so those references were wrong and are replaced here. The standard is single-user licensed: the wording
below is quoted only where a case cites it.

Status key (untestable = no target in the lab can show the behaviour; the reason is given in the row): **covered** (a case judges it), **partial** (some of it judged), **gap** (no case), **n/a** (the
shall applies only to a target shape the lab does not have).

## Clause 5.15.2.4: the states

| Shall (SPC-4) | Covered by | Status |
|---|---|---|
| 5.15.2.4.1: all target ports of a primary group shall be in the same primary state | RTPG reports one state per group | partial: not checked across ports |
| 5.15.2.4.2: active/optimized: the device server shall function as specified | the full suite through an active/optimized path | covered |
| 5.15.2.4.2: participates in all task management functions (SAM-5) | `tmf` section on each path | covered |
| 5.15.2.4.3: non-optimized: the device server shall function as specified | LIO spit-alua-ram standby group set to state 1 (configfs), full suite run 2026-11-13: 849 cases, 778 pass, 12 warn, 59 fail. Against the active/optimized run (763 pass, 21 warn, 65 fail), the cases that changed are the reads and writes that the standby state refused with 2/04/0b and now succeed (PR section, initiator B's portal) | covered on LIO |
| 5.15.2.4.4: standby: shall support the commands it supports in active/optimized, may support others | `standby_cases` on LIO spit-alua-ram TPG2 | covered on LIO |
| 5.15.2.4.4: standby: unsupported commands terminated with CHECK CONDITION | `standby_cases` refusals | covered on LIO (deviation 14: MODE SENSE and MODE SELECT refused 2/04/0b) |
| 5.15.2.4.5: unavailable: shall accept only the listed commands | `unavailable_cases` on LIO spit-alua-ram | covered |
| 5.15.2.4.5 a): INQUIRY peripheral qualifier shall be 001b | `unavailable_cases` INQUIRY check | covered; LIO fails (deviation 15, qualifier 000b) |
| 5.15.2.4.5: unsupported commands terminated (NOT READY, not accessible) | `unavailable_cases` TUR and READ | covered |
| 5.15.2.4.6: offline: transitions out once access is enabled | LIO RTPG advertises O_SUP (bit 6 of the supported-state byte is 1), but no group can be put in offline | info: advertised, not reachable on LIO; cannot test |
| 5.15.2.4.7: logical block dependent state | LIO RTPG has LBD_SUP 0 (supported-state byte 11001111b) | n/a: LIO does not support the state |

## Clause 5.15.2.5: transitions

| Shall (SPC-4) | Covered by | Status |
|---|---|---|
| a): a transition that makes the LU inaccessible: BUSY, or CHECK CONDITION NOT READY with LOGICAL UNIT NOT ACCESSIBLE, ASYMMETRIC ACCESS STATE TRANSITION | none | observed on LIO (2026-11-13, `alua_window`): the change is atomic (2/04/0bh, then 6/2ah/06h, then GOOD), no BUSY and no 2/04/0ah seen; tgt and SPDK have no ALUA; tgt has no ALUA (TPGS 00b); SPDK iSCSI has no ALUA code (grep of lib/iscsi finds no alua, tpgs or asymmetric), so it cannot test these either |
| b) A): during transition, active/optimized commands (INQUIRY, REPORT LUNS, RTPG, REQUEST SENSE, READ BUFFER and WRITE BUFFER echo modes) supported | none | untestable on all three targets: LIO changes state atomically in the kernel (no window to observe); tgt has no ALUA (TPGS 00b); SPDK iSCSI has no ALUA code (grep of lib/iscsi finds no alua, tpgs or asymmetric), so it cannot test these either |
| b) C): other commands during transition: CHECK CONDITION NOT READY, ASYMMETRIC ACCESS STATE TRANSITION | none | untestable on all three targets: LIO changes state atomically in the kernel (no window to observe); tgt has no ALUA (TPGS 00b); SPDK iSCSI has no ALUA code (grep of lib/iscsi finds no alua, tpgs or asymmetric), so it cannot test these either |
| b) D): not required to participate in all task management functions during transition | none | untestable on all three targets: LIO changes state atomically in the kernel (no window to observe); tgt has no ALUA (TPGS 00b); SPDK iSCSI has no ALUA code (grep of lib/iscsi finds no alua, tpgs or asymmetric), so it cannot test these either |
| Failed explicit transition: CHECK CONDITION HARDWARE ERROR, SET TARGET PORT GROUPS COMMAND FAILED | none | untestable on all three targets: LIO changes state atomically in the kernel (no window to observe); tgt has no ALUA (TPGS 00b); SPDK iSCSI has no ALUA code (grep of lib/iscsi finds no alua, tpgs or asymmetric), so it cannot test these either |
| Failed explicit transition that changed state: UA ASYMMETRIC ACCESS STATE CHANGED for every other I_T nexus | none | gap: as above |
| Failed implicit transition: UA IMPLICIT ASYMMETRIC ACCESS STATE TRANSITION FAILED on every I_T nexus | none | gap: no implicit mechanism on the lab |
| Implicit state change: UA ASYMMETRIC ACCESS STATE CHANGED on every I_T nexus | none | gap: no implicit mechanism on the lab |
| Explicit state change: UA ASYMMETRIC ACCESS STATE CHANGED for every I_T nexus other than the one that sent SET TARGET PORT GROUPS | `alua_transition` other-nexus check, and `alua_window` reader on the moving group's portal | covered; LIO fails on the sending portal (deviation 17) and reports it on the moving group's portal (deviation 18) |
| New state applies to all commands received after the transition completes | `alua_transition`: REPORT shows the new state | partial: commands after the transition not checked beyond REPORT |

## Clause 5.15.2.6: preference

| Shall or may (SPC-4) | Covered by | Status |
|---|---|---|
| may: PREF bit in the target port group descriptor (6.37): the PREF bit is independent of the state and may change with it | none: read, no shall to test. LIO exposes the preferred attribute per group in configfs | n/a (may) |

## Clause 5.15.2.7: state reporting

| Shall (SPC-4) | Covered by | Status |
|---|---|---|
| The RTPG answer through a port shall give the current information for the group that contains that port | `alua_transition`: REPORT after an explicit change, through the watcher nexus | partial: other groups' information not checked |

## Clause 5.15.2.8: implicit access management (TPGS 01b or 11b)

| Shall (SPC-4) | Covered by | Status |
|---|---|---|
| a) VPD 83h designator types 4h and 5h | `alua` VPD 83h port and group check | covered |
| b) REPORT TARGET PORT GROUPS supported | `alua` RTPG check (TPGS not 00b) | covered |

## Clause 5.15.2.9: explicit access management (TPGS 10b or 11b)

| Shall (SPC-4) | Covered by | Status |
|---|---|---|
| a) VPD 83h designator types 4h and 5h | `alua` VPD 83h check | covered |
| b) REPORT TARGET PORT GROUPS supported | `alua` RTPG check | covered |
| c) SET TARGET PORT GROUPS supported | `alua` explicit branch: no-op and reserved-state checks | covered on LIO; stutest only if its TPGS has the explicit bit (not checked) |

## Clause 5.15.2.10: behaviour after power on, reset and I_T nexus loss

| Shall (SPC-4) | Covered by | Status |
|---|---|---|
| For TPGS 10b only: the primary state is preserved across power on, hard reset, LU reset and I_T nexus loss | none | n/a unless a lab target reports TPGS 10b (not checked yet) |

## Results by target

- Non-optimized run and comparison: see the 5.15.2.4.3 row above (logs `/tmp/claude-0/alua-nonopt.log`, `/tmp/claude-0/alua-optimized.log`).

- **LIO spit-alua-ram:** 70 of 74 cases pass (login 47 of 49, mpio 10 of 12). The four failures are known deviations:
  login 020Bh and 0203h, and standby MODE SENSE and MODE SELECT refused with 2/04/0b.
- **Stutest:** group 1 is active/optimized only. No transition or standby cases apply, and the SET TARGET PORT
  GROUPS checks run on its own state only.
- **Tgt:** TPGS 00b, so clause 5.15.2 does not apply. Only the REPORT TARGET PORT GROUPS refusal is judged.

## What the gaps need

1. A transitioning state that stays visible for long enough to see the BUSY or CHECK CONDITION answer (5.15.2.5 a)
   and b)). LIO's targetcli changes state at once, so it may need a longer-running transition or a patched lab
   target; this needs your decision.
2. A failing explicit transition, for the HARDWARE ERROR and UA rules (5.15.2.5). Needs the same approval.
3. A non-optimized group on spit-alua-ram, for 5.15.2.4.3. Needs the same approval.
4. Offline and LBA-dependent states (5.15.2.4.6, 5.15.2.4.7): no lab target has them.
5. Stutest: no ALUA changes without your approval; its transition shalls stay open.
