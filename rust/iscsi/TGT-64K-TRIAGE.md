# tgt (scsi-target-utils 1.0.97) on a 64 KiB-block LUN: triage

Run: `spit --iscsi 10.77.0.2/iqn.2026-12.com.stutools:spit-tgt-64k/1 --compliance` (64 MB LUN, 65536-byte
blocks, in the `spit-tgt` network namespace). Each failure is one of: a spit bug (fixed or to fix), a tgt
deviation confirmed against the spec text, or a candidate still to check.

## Spit bugs (fixed in this round)

- **Sequential and random bulk runs** wrote 1000 commands into a 511-block window on this LUN, past the end
  (WRITE at LBA 1024, CHECK CONDITION 5/21/00). Fixed: a run issues min(1000, window blocks) commands.
- **Block length** was judged a power of two. SBC-3 requires an even length (64 KiB is legal). Fixed earlier.

## tgt deviations, confirmed against the spec text

- VERIFY (10) and (16) and SYNCHRONIZE CACHE (10) one or two blocks past the last LBA return GOOD (SBC-3 5.26,
  5.29, 5.31: LBA OUT OF RANGE).
- MODE SENSE (10) and (6) list page 0Ah twice (SPC-3 6.9, 6.10: each page once, 00h last).
- SELECT REPORT 01h lists LUN 0 (SPC-3 6.21: well-known LUNs only).
- RESERVE with a RESERVATION KEY not registered for the nexus returns GOOD (SPC-3 6.12.3: RESERVATION CONFLICT).
- REGISTER with a 16-byte parameter list returns 5/24/00 (SPC-3 6.12: PARAMETER LIST LENGTH ERROR, 5/1A/00).
- PREEMPT that removes a registration gives unit attention 2Ah/03h (RESERVATIONS PREEMPTED) to the removed
  nexus, where SPC-3 5.6.10.1.2 and 5.6.10.1.3 require 2Ah/05h (REGISTRATIONS PREEMPTED); the code is in the
  SPC-3 ASC table (2Ah 05h, 2Ah 03h).
- A registrant is not told of a release (SPC-3 5.6.10.2 c: a unit attention RESERVATIONS RELEASED to every
  other registered nexus when the reservation is released).
- STOP UNIT with IMMED 0 and 1: READ and TEST UNIT READY then return GOOD (SBC-3 4.21.4: NOT READY,
  INITIALIZING COMMAND REQUIRED).
- A login that offers CRC32C only is answered with digest None (RFC 7143 6.2.1: a value not offered).
- Logout of an unknown CID answers 0 where RFC 7143 11.14 requires 1 (CID not found).
- The old session still answers after a reinstatement with the same ISID (RFC 7143 4.2.7.1).
- A Data-In sequence runs past MaxBurstLength 4096 (RFC 7143 11.4).

## Candidates, not yet checked against the clause

- A RESERVE from a non-holder after a reinstatement conflicts (follows from the registration gap above).
- Unlisted LUN 2 and 255: REQUEST SENSE returns GOOD, sense 1/00/00 where the case expects 5/25/00.
- REPORT LUNS overflow residual and INQUIRY overflow residual: the O bit where the case expects U.
- Two keys offered in one login (a key offered twice) are accepted where the case expects a login reject.
- Microsoft rows (VPD B1h rotation rate, Caching PS, Power Condition page, iSCSI version descriptor) are
  Windows requirements, reported as warnings.

## Spit expectations to check (the optional commands)

- PREEMPT AND ABORT, READ FULL STATUS and COMPARE AND WRITE are expected to work, but tgt does not implement
  them. The expectation should follow REPORT CAPABILITIES and REPORT SUPPORTED OPERATION CODES, so an
  unimplemented optional command is not judged as a failure.
