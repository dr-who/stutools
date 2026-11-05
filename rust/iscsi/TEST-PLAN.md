# iSCSI initiator in spit: test plan

Status: the plan this work started from, kept for its lab notes. What spit tests now, and the
standards each case cites, is in `STANDARDS.md`; the cases are in `src/iscsi/`. The SBC command list
below predates the standards being on hand and is superseded by the tables in `STANDARDS.md`.

## Sources used

- **RFC 7143** (iSCSI), read from the RFC Editor text. Sections checked so far: 11.2 BHS
  layout, 11.12 Login Request (T/C bits, CSG/NSG, TSIH, CID, ISID), 11.12.5 ISID formats,
  6.3 login phase, 9.2.1 CHAP considerations, 12.1.3 CHAP keys.
- **SBC (SCSI Block Commands)**: the T10 drafts (sbc2r16, sbc3r36, sbc4r22, sbc5r08, sbc6r02)
  could not be retrieved from this host. The T10 PDF links return an error page.
  The SBC command list below is from memory and must be checked against those PDFs
  before the tests are written. Ask for the PDFs or a download route.

## Requirement: same LUN across all paths (gate)

Spit must refuse to run a workload unless every configured path is the same LUN.
Before any I/O, on each path, spit reads:

1. INQUIRY, standard data: peripheral type must be direct-access block (0x00).
2. VPD page 0x80 (unit serial number).
3. VPD page 0x83 (device identification): the NAA or T10 designator. This is the primary identity.
4. READ CAPACITY(16): last LBA and logical block length.
5. Reported LUN number.

All paths must match on 2 to 4. If any differs, spit exits before the workload with a
table of each path's values. A path that cannot be reached is also a failure.

Gate tests (each must be a refusal unless noted):

| Case | Setup | Expect |
|---|---|---|
| G1 same LUN, three IQNs | spitlunsbc-1/2/3 on one backstore | pass |
| G2 different size | second backstore, different size, on target 3 | refuse, size shown |
| G3 different identity, same size | second backstore, same size, different WWN | refuse, 0x83 shown |
| G4 different block length | 512 vs 4096 backstore | refuse |
| G5 one path unreachable | stop one portal | refuse |
| G6 one path wrong LUN number | map the same backstore at LUN 1 on one target | refuse |
| G7 reordered paths | same paths in a different order | pass |

G2 to G6 need a second backstore on the lab target. That is a change to the target
and needs your go-ahead.

## Login phase (PDU level)

- Request/response header fields: opcode, immediate bit, T, C, CSG, NSG, version,
  ISID, TSIH, ITT, CID, CmdSN, ExpStatSN; response StatSN, status class/detail.
- Security stage: AuthMethod negotiation (None, CHAP), CHAP_A, CHAP_I, CHAP_C,
  CHAP_N, CHAP_R, CHAP_R reject on wrong secret, challenge reuse.
- Operational stage: HeaderDigest, DataDigest, MaxRecvDataSegmentLength,
  MaxBurstLength, FirstBurstLength, ImmediateData, InitialR2T, ErrorRecoveryLevel.
- Full feature: T=1, NSG=3, TSIH assigned, MaxCmdSN window.
- Logout: reason codes, TSIH release.
- Negative: wrong IQN (target refuses), ACL missing (refuses), wrong secret, truncated
  response, bad opcode, oversized text.

Known item: the current test ISID uses the EN format (T=01). RFC 11.12.5 recommends a
random ISID (T=10b) for new sessions. Change the default before the live login.

## SBC command matrix (to verify against SBC PDFs)

For each command: CDB encoding, valid transfer, zero length, last-LBA boundary,
one-past-end (ILLEGAL REQUEST, LBA OUT OF RANGE), invalid CDB field, data round-trip
with the seeded pattern, and multi-PDU transfers (larger than MaxRecvDataSegmentLength).

- READ(6, 10, 12, 16, 32); WRITE(6, 10, 12, 16, 32): flags DPO, FUA, FUA_NV.
- WRITE AND VERIFY(10, 12, 16, 32); VERIFY(10, 12, 16, 32): BYTCHK, data mismatch sense.
- READ CAPACITY(10); SERVICE ACTION IN(16) READ CAPACITY(16): capacity above 2 TiB.
- SYNCHRONIZE CACHE(10, 16): range, immediate bit.
- WRITE SAME(10, 16, 32): UNMAP bit, ANCHOR bit, zero-fill data.
- UNMAP: descriptor counts, block-descriptor limits, read-back zeros.
- COMPARE AND WRITE: miscompare sense and no write.
- ORWRITE, XDWRITEREAD(10), XPWRITE(10): optional, check support via INQUIRY/VPD.
- PREFETCH(10, 16): accepted, no data.
- START STOP UNIT, FORMAT UNIT, REASSIGN BLOCKS: support and refusal.
- GET LBA STATUS (service action 0x12): descriptor format.
- Support discovery: REPORT SUPPORTED OPERATION CODES, VPD 0xB0/0xB1/0xB2 (block limits).

## Sequence

1. No-auth login to one target (ACL present, no-auth on the lab target).
2. Logout; repeat on all three targets.
3. Same-LUN gate G1 with the three-target setup.
4. INQUIRY and READ CAPACITY through the codec.
5. CHAP login (lab targets need CHAP enabled; a separate target is safer).
6. Gate G2 to G7, once the second backstore is approved.
7. SBC matrix, one command at a time, starting with READ/WRITE 10 and READ CAPACITY.
8. Multi-path consistency tests (write on one path, read on another) once the gate passes.
