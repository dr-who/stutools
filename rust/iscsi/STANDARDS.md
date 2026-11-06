# The standards spit's --compliance is built from

This is the analysis behind `spit --compliance` and `--scsilevel N`: which documents spit
tests against, what each one requires of a disk, how the editions differ, and which
commands Windows, macOS and Linux actually send. Clause and table numbers are given
so each case in the code can be traced; the standards themselves are licensed
documents and none of their text is copied here.

## Documents

| Document | Edition | Held | Used for |
|---|---|---|---|
| SAM-6, SCSI Architecture Model | INCITS 546-2021 | yes | status codes, unit attentions, task management, what a reset clears |
| SPC-3, SCSI Primary Commands | INCITS 408-2005 [R2010] | yes | the edition SBC-3-era targets claim |
| SPC-4, SCSI Primary Commands | INCITS 513-2015 (R2020) | yes | commands for every device type, earlier edition |
| SPC-5, SCSI Primary Commands | INCITS 502-2019 | yes | commands for every device type; Discovery 2016 feature set |
| SPC-6, SCSI Primary Commands | INCITS 566-2025 | yes | the current edition, which SBC-4 and SBC-5 cite |
| SBC-3, SCSI Block Commands | INCITS 514-2014 | yes | the floor hosts are written to; the SBC edition of SCSI level 3 |
| SBC-4, SCSI Block Commands | INCITS 506-2021 | yes | block commands; SBC feature sets (Annex H) |
| SBC-5, SCSI Block Commands | INCITS 571-2025 | yes | block commands; SBC feature sets (Annex A) |
| RFC 7143, iSCSI | 2014 | yes (public) | login, negotiation, sessions, NOP, task management PDUs |
| RFC 7144, iSCSI SCSI Features Update | 2014 | yes (public) | iSCSIProtocolLevel and the newer task management functions |
| SNIA Multipath Management API 1.0.1 | INCITS 412-2006 (S2026) | yes | host-side vocabulary only: path and access states, load-balance policies; nothing to test on a target |
| NVM Express SCSI Translation Reference | 1.5 (2015) | yes (public) | how a SCSI command a host sends maps to NVMe |
| NVM Express Base | 2.1 (2024) | yes (public) | admin commands, Fabrics, reservations, ANA, log pages, features |
| NVM Express NVM Command Set | 1.1 (2024) | yes (public) | Read, Write, Compare, Write Zeroes, Dataset Management, Identify Namespace |
| NVM Express TCP Transport | 1.1 (2024) | yes (public) | ICReq/ICResp, PDUs, digests |

## A disk is four layers

A block device on iSCSI is judged against all four; "primary" (SPC) alone is not enough.

1. **SAM-6**: how a command ends (GOOD, CHECK CONDITION, RESERVATION CONFLICT), unit
   attentions, and the task management functions. SAM-6 makes TARGET RESET obsolete,
   so spit never sends it.
2. **SPC-6** (and SPC-5 before it): commands every device type has. Mandatory
   (Table 88 in both): INQUIRY,
   REPORT LUNS, TEST UNIT READY. Everything else (MODE SENSE/SELECT, LOG SENSE,
   PERSISTENT RESERVE, REPORT SUPPORTED OPERATION CODES, REPORT TARGET PORT GROUPS, ...)
   is optional there and made required by a feature set or by a host that uses it.
   RESERVE and RELEASE (6) and (10), opcodes 16h, 17h, 56h, 57h, are obsolete in SPC-4,
   SPC-5 and SPC-6. SPC-6's one change to the command table is that MODE SENSE (6) and
   MODE SELECT (6) are obsolete, matching SBC-5; Discovery 2016 is unchanged apart from
   clause numbers.
3. **SBC-3/SBC-4/SBC-5**: the block commands (SBC-3 Table 32, Table 34 in the others). Mandatory in SBC-3, SBC-4 and
   SBC-5 alike: FORMAT UNIT, INQUIRY, READ (10), READ (16), READ CAPACITY (10),
   READ CAPACITY (16), REPORT LUNS, REQUEST SENSE, TEST UNIT READY. WRITE is optional in
   the table: a read-only device conforms. FORMAT UNIT is mandatory and destroys data,
   so spit only asks REPORT SUPPORTED OPERATION CODES whether it is there.
4. **RFC 7143**: the iSCSI transport.

## How the SBC editions differ

Each block command in spit carries `min` and `max`: the first and last of SBC-3, SBC-4
and SBC-5 whose command table (SBC-3 Table 32, SBC-4 and SBC-5 Table 34) lists it.
SCSI level N judges against SPC-N with SBC-N (level 6: SPC-6 with SBC-5) and keeps the
rows with min <= N <= max of each. Cases cite SPC-6 and SBC-5 clauses; at an older level
each citation moves to that edition's clause (`compliance/clause_map.rs`, matched by
clause title and checked by hand), and a requirement whose clause the edition lacks
only informs there. The device is probed once and judged at every level asked; the
report has a column per level. The full table is at the end of this document; the
changes between editions are:

| Change | Rows | Source |
|---|---|---|
| Dropped by SBC-4 | READ LONG (10)/(16), XDWRITEREAD (10)/(32), XPWRITE (10)/(32), ACCESS CONTROL IN/OUT; also the EER and DCR bits | SBC-4 Table 34, clause 1 |
| Added by SBC-4 | BACKGROUND CONTROL, FORMAT WITH PRESET, GET LBA STATUS (32), GET PHYSICAL ELEMENT STATUS, GET STREAM STATUS, REMOVE ELEMENT AND TRUNCATE, REPORT PROVISIONING INITIALIZATION PATTERN, RESTORE ELEMENTS AND REBUILD, STREAM CONTROL, WRITE ATOMIC (16)/(32), WRITE SCATTERED (16)/(32), WRITE STREAM (16)/(32), READ BUFFER (16), the zoned commands (ZBC); the SBC feature sets (Annex H) | SBC-4 Table 34, Annex H |
| Listed in SBC-4's table, defined in SPC | MAINTENANCE IN/OUT, READ MEDIA SERIAL NUMBER, REPORT/SET TIMESTAMP, REPORT ALL ROD TOKENS, and the copy commands under their SPC-4 names | SBC-4 Table 34 |
| Dropped by SBC-5 | MODE SELECT (6), MODE SENSE (6), SEQUENTIALIZE ZONE; also the ZONED field and the host aware zoned model | SBC-5 Table 34, clause 1 |
| Added by SBC-5 | REMOVE ELEMENT AND MODIFY ZONES | SBC-5 Table 34 |

The mandatory set is the same in all three editions, which is why hosts treat "SBC-3
or later" as one target: every command Windows, macOS and Linux send is in SBC-3's
table, with the same fields in SBC-4 and SBC-5. SBC-3 defines no feature sets (its
Annex A is numeric codes); they start in SBC-4.

## Feature sets: the standard's own profiles

A target lists the sets it claims in the SCSI Feature Sets VPD page (92h, SPC-5
7.7.14). spit evaluates every set whether claimed or not, and fails a claimed set that
is not met.

| Code | Set | Defined in | What it adds |
|---|---|---|---|
| 0001h | Discovery 2016 | SPC-5 and SPC-6 Annex A.2 | INQUIRY, MODE SENSE (10), REPORT LUNS, REPORT SUPPORTED OPERATION CODES, TEST UNIT READY; Control and Control Extension mode pages; Device Identification, Extended INQUIRY Data (SIMPSUP, LUICLR), Mode Page Policy, SCSI Feature Sets and Supported VPD Pages; ABORT TASK, ABORT TASK SET, CLEAR TASK SET, LOGICAL UNIT RESET |
| 0102h | SBC Base 2010 | SBC-4 H.2, SBC-5 A.2 | the 10-byte command set: READ/WRITE (10), READ CAPACITY (10), SYNCHRONIZE CACHE (10), WRITE SAME (10), MODE SELECT/SENSE (10), START STOP UNIT, FORMAT UNIT; Caching, Control, Control Extension and Read-Write Error Recovery mode pages; Block Device Characteristics and the discovery VPD pages |
| 0101h | SBC Base 2016 | SBC-4 H.3, SBC-5 A.3 | the 16-byte set, plus REPORT SUPPORTED OPERATION CODES (with timeouts and all three reporting options) and REPORT SUPPORTED TASK MANAGEMENT FUNCTIONS; Block Limits with at least 1 MiB transfers and write-same; a non-zero rotation rate; LBA 0 aligned; FUA when the cache is volatile; Informational Exceptions Control page |
| 0103h | Basic Provisioning 2016 | SBC-4 H.4, SBC-5 A.4 | thin provisioning: GET LBA STATUS (16), UNMAP, WRITE SAME (16) with UNMAP and NDOB, LBPME, LBPU, LBPWS, at least 64 unmap descriptors, a defined LBPRZ |
| 0104h | Drive Maintenance 2016 | SBC-4 H.5, SBC-5 A.5 | SANITIZE, FORMAT UNIT, WRITE LONG, REASSIGN BLOCKS, buffers, diagnostics, defect data and maintenance log pages; judged from REPORT SUPPORTED OPERATION CODES and log pages only, since those commands destroy data |

SBC-4 Annex H and SBC-5 Annex A define the same requirements. The differences are
references (SAT-4 to SAT-6, clause numbers) and one footnote: SBC-4 also says VPD
pages that are readable before the medium is ready must not hold placeholder spaces
or zeros. At SCSI level 3 no feature set applies (they start in SBC-4 and SPC-5); at
level 4 only the SBC sets do; Discovery 2016 applies from level 5. One rule differs
between SPC editions beyond numbering: SPC-3 lets the Unit Serial Number end in NUL
bytes (SPC-3 4.4.1, 7.6.10), while SPC-4 on make it right-aligned ASCII.

## What hosts send

Two packet captures are the evidence: a Windows Server 2022 Microsoft initiator
bringing a LUN online, and Linux open-iscsi scanning one. "trace" below means the
command is in that capture; anything else is known host behaviour that the captures
do not show (they have no MPIO, cluster, TRIM or ODX activity). A command a host sends
is held to its rule at every SCSI level, and spit replays each trace in order.

Windows Server 2022 offered at login: HeaderDigest and DataDigest None,CRC32C,
ErrorRecoveryLevel 2, InitialR2T No, ImmediateData Yes, MaxRecvDataSegmentLength
65536, MaxBurstLength 262144, FirstBurstLength 65536, MaxConnections 32,
DefaultTime2Wait 0, DefaultTime2Retain 60, MaxOutstandingR2T 16.

| Command | Windows | macOS | Linux |
|---|---|---|---|
| TEST UNIT READY | trace | yes | yes |
| REPORT LUNS | trace (allocation 16) | yes | yes |
| INQUIRY, standard | trace (allocation 36 and 74) | yes | trace (allocation 254) |
| INQUIRY VPD 00h, 80h, 83h | trace | yes | trace |
| INQUIRY VPD B0h, B2h, B1h | trace (B1h with allocation 64) | | yes |
| READ CAPACITY (10) | trace | yes | trace |
| READ CAPACITY (16) | trace (allocation 12 and 32) | above 2 TiB | trace (allocation 12) |
| MODE SENSE (6) | trace: pages 1Ch, 0Ah, 08h; never the (10) form | yes | |
| MODE SENSE (10) | | | yes (caching page) |
| SYNCHRONIZE CACHE (10) | trace | yes | yes |
| READ/WRITE (10), (16) | yes; FUA writes when DPOFUA is set | (10); (16) above 2 TiB | trace: READ (16) partition scans |
| VERIFY (10), (16) | chkdsk /r | | |
| UNMAP | TRIM, Optimize-Volume | | discard |
| WRITE SAME (16) | | | discard, zeroing |
| POPULATE TOKEN, WRITE USING TOKEN | ODX, when VPD 8Fh advertises ROD tokens | | |
| PERSISTENT RESERVE IN/OUT | Failover Clustering: REGISTER AND IGNORE EXISTING KEY, RESERVE and PREEMPT with Write Exclusive Registrants Only, READ KEYS, READ RESERVATION, RELEASE, CLEAR | | Pacemaker fencing |
| REPORT/SET TARGET PORT GROUPS | MSDSM (ALUA) | | dm-multipath |
| REPORT SUPPORTED OPERATION CODES | yes | | yes |
| START STOP UNIT, PREVENT ALLOW MEDIUM REMOVAL | | yes | |
| WRITE ATOMIC (16) | | | 6.11 and later |
| COMPARE AND WRITE | | | (VMware's ATS; tested anyway) |
| RESERVE/RELEASE (6) | Windows Server 2003 clusters (obsolete; informational) | | |

macOS ships no iSCSI initiator; a Mac reaches an iSCSI disk through a third-party
initiator, and its disk driver sends the small set in the macOS column.

## NVMe

The NVMe cases follow the same rule with no option: each command, log page and
feature carries the NVM Express revision that introduced it, and is judged against the
VS property the controller reports. A capability newer than the controller's version
passes when it is refused. Windows reaches NVMe through stornvme, which translates the
SCSI commands above: READ CAPACITY to Identify Namespace, SYNCHRONIZE CACHE to Flush,
UNMAP to Dataset Management (deallocate), WRITE SAME with zeros to Write Zeroes,
PERSISTENT RESERVE to the NVMe reservation commands, and ALUA to ANA.

## Appendix: block commands by SBC edition

M mandatory, O optional, X requirements given in the referenced clause, - not listed.

| Command | Opcode | SBC-3 | SBC-4 | SBC-5 |
|---|---|---|---|---|
| ACCESS CONTROL IN | 86h | O | - | - |
| ACCESS CONTROL OUT | 87h | O | - | - |
| ATA PASS-THROUGH (12) | A1h | O | O | O |
| ATA PASS-THROUGH (16) | 85h | O | O | O |
| BACKGROUND CONTROL | 9Eh/15h | - | O | O |
| CHANGE ALIASES | A4h/0Bh | O | O | O |
| CLOSE ZONE | 94h/01h | - | X | X |
| COMPARE AND WRITE | 89h | O | O | O |
| COPY OPERATION ABORT | 83h/1Ch | - | O | O |
| EXTENDED COPY | 83h | O | O | O |
| FINISH ZONE | 94h/02h | - | X | X |
| FORMAT UNIT | 04h | M | M | M |
| FORMAT WITH PRESET | 38h | - | O | O |
| GET LBA STATUS (16) | 9Eh/12h | O | O | O |
| GET LBA STATUS (32) | 7Fh/0012h | - | O | O |
| GET PHYSICAL ELEMENT STATUS | 9Eh/17h | - | O | O |
| GET STREAM STATUS | 9Eh/16h | - | O | O |
| INQUIRY | 12h | M | M | M |
| LOG SELECT | 4Ch | O | O | O |
| LOG SENSE | 4Dh | O | O | O |
| MAINTENANCE IN | A3h/00h | - | X | X |
| MAINTENANCE OUT | A4h/00h | - | X | X |
| MODE SELECT (10) | 55h | O | O | O |
| MODE SELECT (6) | 15h | O | O | - |
| MODE SENSE (10) | 5Ah | O | O | O |
| MODE SENSE (6) | 1Ah | O | O | - |
| OPEN ZONE | 94h/03h | - | X | X |
| ORWRITE (16) | 8Bh | O | O | O |
| ORWRITE (32) | 7Fh/000Eh | O | O | O |
| PERSISTENT RESERVE IN | 5Eh | O | O | O |
| PERSISTENT RESERVE OUT | 5Fh | O | O | O |
| POPULATE TOKEN | 83h/10h | O | O | O |
| PRE-FETCH (10) | 34h | O | O | O |
| PRE-FETCH (16) | 90h | O | O | O |
| PREVENT ALLOW MEDIUM REMOVAL | 1Eh | O | O | O |
| READ (10) | 28h | M | M | M |
| READ (12) | A8h | O | O | O |
| READ (16) | 88h | M | M | M |
| READ (32) | 7Fh/0009h | O | O | O |
| READ ATTRIBUTE | 8Ch | O | O | O |
| READ BUFFER (10) | 3Ch | O | O | O |
| READ BUFFER (16) | 9Bh | - | O | O |
| READ CAPACITY (10) | 25h | M | M | M |
| READ CAPACITY (16) | 9Eh/10h | M | M | M |
| READ DEFECT DATA (10) | 37h | O | O | O |
| READ DEFECT DATA (12) | B7h | O | O | O |
| READ LONG (10) | 3Eh | O | - | - |
| READ LONG (16) | 9Eh/11h | O | - | - |
| READ MEDIA SERIAL NUMBER | ABh/01h | - | O | O |
| REASSIGN BLOCKS | 07h | O | O | O |
| RECEIVE COPY DATA | 84h/06h | - | O | O |
| RECEIVE COPY RESULTS | 84h | O | - | - |
| RECEIVE COPY STATUS | 84h/05h | - | O | O |
| RECEIVE DIAGNOSTIC RESULTS | 1Ch | X | X | X |
| RECEIVE ROD TOKEN INFORMATION | 84h/07h | X | X | X |
| REDUNDANCY GROUP IN | BAh | X | X | X |
| REDUNDANCY GROUP OUT | BBh | X | X | X |
| REMOVE ELEMENT AND MODIFY ZONES | 9Eh/1Ah | - | - | O |
| REMOVE ELEMENT AND TRUNCATE | 9Eh/18h | - | O | O |
| REMOVE I_T NEXUS | A4h/0Ch | O | O | O |
| REPORT ALIASES | A3h/0Bh | O | O | O |
| REPORT ALL ROD TOKENS | 84h/08h | - | O | O |
| REPORT IDENTIFYING INFORMATION | A3h/05h | O | O | O |
| REPORT LUNS | A0h | M | M | M |
| REPORT PRIORITY | A3h/0Eh | O | O | O |
| REPORT PROVISIONING INITIALIZATION PATTERN | A3h/1Dh | - | O | O |
| REPORT REALMS | 95h/06h | - | X | X |
| REPORT REFERRALS | 9Eh/13h | O | O | O |
| REPORT SUPPORTED OPERATION CODES | A3h/0Ch | O | O | O |
| REPORT SUPPORTED TASK MANAGEMENT FUNCTIONS | A3h/0Dh | O | O | O |
| REPORT TARGET PORT GROUPS | A3h/0Ah | O | O | O |
| REPORT TIMESTAMP | A3h/0Fh | - | O | O |
| REPORT ZONE DOMAINS | 95h/07h | - | X | X |
| REPORT ZONES | 95h/00h | - | X | X |
| REQUEST SENSE | 03h | M | M | M |
| RESET WRITE POINTER | 94h/04h | - | X | X |
| RESTORE ELEMENTS AND REBUILD | 9Eh/19h | - | O | O |
| SANITIZE | 48h | O | O | O |
| SECURITY PROTOCOL IN | A2h | O | O | O |
| SECURITY PROTOCOL OUT | B5h | O | O | O |
| SEND DIAGNOSTIC | 1Dh | O | O | O |
| SEQUENTIALIZE ZONE | 94h/10h | - | X | - |
| SET IDENTIFYING INFORMATION | A4h/06h | O | O | O |
| SET PRIORITY | A4h/0Eh | O | O | O |
| SET TARGET PORT GROUPS | A4h/0Ah | O | O | O |
| SET TIMESTAMP | A4h/0Fh | - | O | O |
| SPARE IN | BCh | X | X | X |
| SPARE OUT | BDh | X | X | X |
| START STOP UNIT | 1Bh | O | O | O |
| STREAM CONTROL | 9Eh/14h | - | O | O |
| SYNCHRONIZE CACHE (10) | 35h | O | O | O |
| SYNCHRONIZE CACHE (16) | 91h | O | O | O |
| TEST UNIT READY | 00h | M | M | M |
| UNMAP | 42h | X | X | X |
| VERIFY (10) | 2Fh | O | O | O |
| VERIFY (12) | AFh | O | O | O |
| VERIFY (16) | 8Fh | O | O | O |
| VERIFY (32) | 7Fh/000Ah | O | O | O |
| VOLUME SET IN | BEh | X | X | X |
| VOLUME SET OUT | BFh | X | X | X |
| WRITE (10) | 2Ah | O | O | O |
| WRITE (12) | AAh | O | O | O |
| WRITE (16) | 8Ah | O | O | O |
| WRITE (32) | 7Fh/000Bh | O | O | O |
| WRITE AND VERIFY (10) | 2Eh | O | O | O |
| WRITE AND VERIFY (12) | AEh | O | O | O |
| WRITE AND VERIFY (16) | 8Eh | O | O | O |
| WRITE AND VERIFY (32) | 7Fh/000Ch | O | O | O |
| WRITE ATOMIC (16) | 9Ch | - | O | O |
| WRITE ATOMIC (32) | 7Fh/000Fh | - | O | O |
| WRITE ATTRIBUTE | 8Dh | O | O | O |
| WRITE BUFFER | 3Bh | O | O | O |
| WRITE LONG (10) | 3Fh | O | O | O |
| WRITE LONG (16) | 9Fh/11h | O | O | O |
| WRITE SAME (10) | 41h | X | X | X |
| WRITE SAME (16) | 93h | X | X | X |
| WRITE SAME (32) | 7Fh/000Dh | X | X | X |
| WRITE SCATTERED (16) | 9Fh/12h | - | O | O |
| WRITE SCATTERED (32) | 7Fh/0011h | - | O | O |
| WRITE STREAM (16) | 9Ah | - | O | O |
| WRITE STREAM (32) | 7Fh/0010h | - | O | O |
| WRITE USING TOKEN | 83h/11h | X | X | X |
| XDWRITEREAD (10) | 53h | O | - | - |
| XDWRITEREAD (32) | 7Fh/0007h | O | - | - |
| XPWRITE (10) | 51h | O | - | - |
| XPWRITE (32) | 7Fh/0006h | O | - | - |
| ZONE ACTIVATE | 95h/08h | - | X | X |
| ZONE QUERY | 95h/09h | - | X | X |
