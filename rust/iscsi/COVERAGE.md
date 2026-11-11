# SPC-3 and SBC-3 coverage: spitlunsbc-1

Scope: SPC-3 and SBC-3 at SCSI level 3, against the optimized iSCSI LUN `spitlunsbc-1` (LIO, block
backstore, 20 GiB). Nothing here covers the standby port, the CHAP port, or the NVMe-oF and SATA
transports.

## How the numbers are made

- `clause_map.py` reads the licensed standards (read from a local folder, cached as text outside the
  repository) and the compliance log of one run. Only clause numbers, counts and results are written.
- A clause is *cited* when a judged case (PASS or FAIL) label names it, as `SPC-3 6.8` or `SBC-3 5.33`.
- A clause's *shall* count is the number of "shall" statements under its heading in the standard.
- *Score* is passed / (passed + failed) over the judged cases whose labels cite the standard. INFO and
  not-run cases are not judged.

Regenerate:

    spit -f ... (no: iSCSI) spit --iscsi 103.187.7.170/iqn.2026-10.com.stutools:spitlunsbc-1 \
        --iscsi-initiator iqn.2026-11.com.stutools:spit-initiator \
        --chap-user spit-initiator --chap-secret-file /var/tmp/spit-chap/secret --compliance > run.log
    python3 iscsi/clause_map.py --log run.log --standards /root \
        --edition "SPC-3=INCITS 408" --edition SBC-3=INCITS+514 > map.csv

## Result (run of this commit's binary)

| Standard | Clauses with shall | Cited | Clause coverage | shall in cited clauses | Judged cases | Score |
|---|---|---|---|---|---|---|
| SPC-3 | 301 | 17 | 5.6% | 174 of 1715 (10.1%) | 96 | 91.7% |
| SBC-3 | 229 | 32 | 14.0% | 450 of 1229 (36.6%) | 89 | 87.6% |

The earlier figures in this project (about 16% and 10%) used a looser match and are superseded.

## Largest uncovered clauses, and what each needs

Each clause is classified: testable on this LUN, not applicable to a block LUN, or destructive (not run
on a device with data).

SPC-3:

| Clause | shall | Title | Class |
|---|---|---|---|
| 6.3.3 | 29 | Errors detected during processing of segment descriptors | not applicable (copy/segment operations) |
| 6.34 | 28 | WRITE ATTRIBUTE | not applicable (medium auxiliary memory) |
| 7.5.2.4.1 | 28 | iSCSI-specific alias entry designations | testable via VPD 8Ah/87h alias data |
| 8.3.2.3.2 | 28 | REPORT LU DESCRIPTORS parameter data | testable via REPORT LUNS |
| 6.28 | 27 | SEND DIAGNOSTIC | testable (refusal or self test; active state) |
| 6.3.7.17 | 25 | Tape device image copy | not applicable |
| 6.27 | 21 | REQUEST SENSE | testable (more forms) |
| 6.3.7.5 | 21 | Block device to block device operations | not applicable unless EXTENDED COPY is offered |
| 6.3.7.6 | 21 | (copy operations) | not applicable unless EXTENDED COPY is offered |
| 6.12.3 | 19 | Basic PERSISTENT RESERVE OUT parameter list | testable (PR OUT forms) |
| 6.30 | 19 | SET PRIORITY | testable (refusal or accept) |
| 7.2.1 | 19 | Log page structure and page codes | testable via LOG SENSE (LIO refuses LOG SENSE) |

SBC-3:

| Clause | shall | Title | Class |
|---|---|---|---|
| 5.3.2.2 | 34 | FORMAT UNIT parameter list header | destructive (FORMAT UNIT) |
| 5.46.2 | 20 | WRITE USING TOKEN parameter list | testable only if ODX is offered |
| 6.5.4 | 19 | Background Control mode page | testable (mode page forms, as for the other pages) |
| 5.19 | 17 | READ LONG (10) | testable: refusal when not implemented |
| 5.3.1 | 17 | FORMAT UNIT overview | destructive |
| 5.7.2 | 17 | POPULATE TOKEN parameter list | testable only if ODX is offered |
| 5.43 | 16 | WRITE SAME (10) | testable (data forms) |
| 5.41 | 15 | WRITE LONG (10) | testable: refusal when not implemented |
| 4.13.2 | 12 | Generation of defect lists | not applicable to LIO (no defect list) |
| 5.3.2.3 | 12 | Initialization pattern descriptor | destructive (FORMAT UNIT) |
| 5.17.1 | 11 | (READ CAPACITY) | testable |
| 5.28.2 | 11 | (SYNCHRONIZE CACHE) | testable |

## Next batch (testable on this LUN)

1. SBC-3 6.5.4 Background Control mode page: the page's fields and changeable bits, as the other pages.
2. SBC-3 5.43 WRITE SAME (10) forms, and SBC-3 5.19 and 5.41: refusal of READ LONG and WRITE LONG
   when the LUN does not implement them (confirmed by REPORT SUPPORTED OPERATION CODES).
3. SPC-3 6.28 SEND DIAGNOSTIC and 6.30 SET PRIORITY: refusal when not implemented, agreeing with the
   supported-operation answer.
4. SPC-3 6.27 REQUEST SENSE and 7.2.1 log structure, through the forms the LUN answers.

FORMAT UNIT (SBC-3 5.3) and its parameter list stay not run on a device with data.
