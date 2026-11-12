# Clause audit: COMPARE AND WRITE (SBC-3 5.2), the ats section

Source: SBC-3 (INCITS 514) 5.2, the twelve shall and must statements of the clause. Cases are in
`rust/src/iscsi/compare_and_write.rs` (section `ats`) and `rust/src/iscsi/ats_race.rs`.

| Shall (SBC-3 5.2) | Covered by | Status |
|---|---|---|
| Miscompare: the offset of the first unequal byte goes in INFORMATION | compare fails case with INFORMATION check | covered, where the target answers a miscompare |
| OPERATION CODE 89h and CDB layout (table 33) | the command is sent in the 16-byte form and judged | covered |
| LBA beyond the capacity: CHECK CONDITION, LBA OUT OF RANGE | LBA range case | covered |
| NUMBER OF LOGICAL BLOCKS of 0: no reads, transfers or writes, not an error | not run | gap |
| Read, compare and write operate on the same LBA range (the Data-Out buffer holds compare and write data) | compare matches case, block holds the written data | covered |
| Compare instance: user data only, protection information not used | not testable without protection information on the LUN | gap (PI) |
| Write instance: user data and protection information as set by the command | not testable without PI | gap (PI) |
| Miscompare: no write takes place | compare fails case: block unchanged | covered |
| More blocks than MAXIMUM COMPARE AND WRITE LENGTH: CHECK CONDITION, INVALID FIELD IN CDB | length limit case | covered |
| MAXIMUM COMPARE AND WRITE LENGTH 0: the command is not supported (SBC-5 6.6.4) | advertised-or-not logic | covered |
| Not advertised: a refusal is the answer; refused as invalid opcode | refusal accepted when not advertised | covered |
| Two hosts at once: atomicity (ats_race) | ats_race | covered (lab target) |

## Gaps to implement

1. NUMBER OF LOGICAL BLOCKS 0 returns GOOD and does nothing (zero-length, not an error): add the case. A
   zero-length COMPARE AND WRITE is the same stall istgt shows for zero-length READ and WRITE, so it is also a
   target test.
2. Protection information in the compare and write instances, on the PI RAM targets (spit-pi1-ram, spit-pi3-ram).

Clause numbers: the ats module's comments cite SBC-5 5.3; the SBC-3 clause is 5.2. The label citations should
name the edition the case tests (SBC-3 5.2 at level 3).
