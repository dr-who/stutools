# Clause audit, first set: the mandatory LUN commands (SPC-3 and SBC-3)

Status key: **covered** = every shall in the clause has a case that checks it; **partial** = some shalls have
a case; **gap** = no case cites the clause. Case references are the labels in the source that cite the clause.
Commands are the ones every direct-access LUN must answer (SPC-3 table 44 type M; SBC-3 5.x for READ/WRITE).

| Clause | Command | Status | Cases citing it (source) | Gaps |
|---|---|---|---|---|
| SPC-3 6.4 | INQUIRY | partial | standard INQUIRY, EVPD framing, VPD 00h/80h/83h (identity_cases, inquiry_framing, vpd_framing) | EVPD=0 with a nonzero page code (6.4.1) fails on SAT and LIO; the case is in the failure list. Unlisted LUN answer (6.4.2) not checked against the text |
| SPC-3 6.21 | REPORT LUNS | partial | REPORT LUNS SELECT 00h/01h/02h, allocation framing (report_luns_cases) | SELECT 01h must list only well-known LUNs (LIO and tgt both list LUN 0: deviation) |
| SPC-3 6.27 | REQUEST SENSE | partial | fixed and descriptor format, response code (unit_state_cases) | DESC 1 refusal allowed by no text (LIO and SAT refuse: deviation); sense after a command not checked |
| SPC-3 6.33 | TEST UNIT READY | gap-ish | one label, after STOP (disk section) | a TUR GOOD and CHECK CONDITION sequence (unit attention) not checked against 6.33 |
| SBC-3 5.15 | READ CAPACITY (10) | gap | agreement with READ CAPACITY (16) only (capacity_cases) | no case cites 5.15 directly; the returned length and FFFF_FFFFh rule not checked for a LUN past 2^32 |
| SBC-3 5.16 | READ CAPACITY (16) | covered for block length and alignment | capacity_cases, block-size rule (SBC-3 4.5) | the LBPME/LBPRZ agreement is cited to SBC-5 5.21.2; the LIO/SAT disagreement is recorded |
| SBC-3 5.11 | READ (10) | partial | READ (10) returns what WRITE (10) wrote; past last LBA | 5.11 READ (10) with transfer length 0 covered |
| SBC-3 5.13 | READ (16) | partial | READ (16) after WRITE (10) and WRITE (16); past last LBA | the 10-byte rounds past 2^32 are now not run (truncation) |
| SBC-3 5.33 | WRITE (10) | partial | answered correctly; read back | write cache and FUA cases (io section) |
| SBC-3 5.35 | WRITE (16) | partial | answered correctly; read back | write cache and FUA cases (io section) |

## Next in this set

1. Cite SBC-3 5.15 from a READ CAPACITY (10) case, and check the FFFF_FFFFh rule on a LUN past 2^32 blocks
   (the 1 EiB and 4 TB LUNs of harness/lio-ram).
2. Write a TEST UNIT READY case that checks the unit attention sequence after a state change (6.33).
3. Check REQUEST SENSE after a command, the sense-clearing rule (SAM-3), with a test on the RAM LUN.
