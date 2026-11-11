# Compliance testing plan: SCSI standards on any transport

Status: plan. The iSCSI suite exists (`src/iscsi/`); the NVMe-oF and SATA layers below do not.

## Goal

For each standard the suite claims, every requirement that applies to a transport is either
tested, or recorded as not applicable with the reason. The standards are the SCSI command set
for block devices (SBC-3, SBC-4, SBC-5) and the SCSI primary commands they sit on (SPC-3 to
SPC-6). Each case names the document and clause it tests, in the document's own words.

## Layers

1. **Core (`compliance/core`)**: the case record (section, label, expected, actual, verdict), the
   expectation types (`Want`: GOOD, CHECK CONDITION with sense key, ASC or ASCQ), and the report.
   No transport code. This is shared by every transport.
2. **Commands (`compliance/scsi`)**: CDB builders and parsers for each command the suite sends
   (READ and WRITE in each length, VERIFY, UNMAP, mode pages, VPD pages, REPORT SUPPORTED
   OPERATION CODES). Pure functions, unit-tested without a device.
3. **Transport (`compliance/transport`)**: one trait with the operations every transport can do:
   send a CDB with optional data in or out, and return status, sense and data. Implementations:
   - iSCSI: the existing `Port` and `PduConnection` (`src/iscsi/`).
   - NVMe-oF: NVMe/TCP, with the SCSI-to-NVMe translation (NVMe SCSI Translation Reference)
     for the commands that have a translation, and NVMe commands for the rest.
   - SATA: SAT (SCSI-ATA Command Translation) through SG_IO on Linux, and the platform
     equivalent elsewhere.
4. **Cases (`compliance/cases/<area>`)**: one module per area, written against the trait only.
   Areas follow the standards: `login`, `session`, `spc`, `disk`, `io`, `unmap`, `mpio`, `pr`, `tmf`.
   A case that needs a transport feature the transport lacks is recorded as not applicable, with
   the reason.
5. **Runner**: runs the cases for each target the user names, in a fixed order. A variable-length
   or otherwise fragile command runs last on its own session, so one failure does not hide the
   rest. A watchdog stops a run that makes no progress.

## Requirement map

For each standard, a generated map: clause, number of "shall" statements, the cases that cite it,
and the result. It is generated from the compliance log, so it stays true as cases change. The
map holds clause numbers and our own labels, never the standard's text (the licence allows clause
references only).

Current coverage (iSCSI, level 3, from the map): about 16% of SBC-3 clauses and 10% of SPC-3
clauses are cited by a case. The gaps are listed largest first in the map.

## Milestones

1. Finish the iSCSI audit of SPC-3 and SBC-3: the failing expectations checked against the
   clause, the standby command list complete (MODE SELECT, LOG SELECT, RECEIVE and SEND DIAGNOSTIC,
   READ and WRITE BUFFER echo), and the login statuses checked against RFC 7143.
2. Run the iSCSI suite on every lab target, and record each target's standby, CHAP and length
   limits as the reason a case did not run.
3. Split `src/iscsi/` into the layers above without changing results (the score must match
   before and after).
4. NVMe-oF transport: connect, identify, the translation table, and the same case modules
   run unchanged where the translation exists.
5. SATA transport: SAT through SG_IO, the same case modules.

## Rules

- A case cites the document and clause it tests, by the edition the suite runs at.
- A failure is first checked against the clause. Only a confirmed target fault is a finding;
  a wrong expectation in the suite is fixed, and the commit says so.
- A case that cannot run is recorded as "not run" with the reason, never as a pass.
