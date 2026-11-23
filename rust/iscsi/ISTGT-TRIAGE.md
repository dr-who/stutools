# istgt (openebs-archive/istgt, built from source, user space) on a 1 GB LUN: triage

Run: `spit --iscsi 10.77.0.2/iqn.2026-12.com.stutools.istgt:istgt-disk1 --compliance` (istgt in the `spit-tgt`
network namespace, LUN file `/dev/shm/istgt-lun.img`, recreated before the run). Built with `-fcommon` and the
host's `sys/queue.h` in the scratch clone (old code: the link needs common symbols). Two runs gave identical
results, and a restart read a fresh reservation area, so the PR results are istgt's behaviour, not leftover state.

## Confirmed deviations (spec text)

- **RESERVE with another TYPE while a reservation is held returns GOOD** (SPC-3 5.6.8: RESERVATION CONFLICT).
  Consequence: the reservation changes type silently, and later cases see a reservation they did not expect.
- **READ RESERVATION of an all-registrants reservation reports the holder's key** (SPC-3 5.6.9: the key is zero).
- **PREEMPT with a service action key that matches no registration returns GOOD** (SPC-3 5.6.10.4: RESERVATION
  CONFLICT).
- **Unregistering the holder does not release the reservation** (SPC-3 5.6.10.1.2, 5.6.10.1.3, 5.6.10.1.4).
- **READ and WRITE past the last LBA return CHECK CONDITION 0/00/00, not 5/21/00** (SBC-3 5.11, 5.13: LBA OUT OF
  RANGE). The sense key is 0.
- **REPORT SUPPORTED OPERATION CODES says READ (10/12/16) and WRITE (10/12/16) are not supported** while they
  work (SPC-3 6.23.1 must agree with the command set).
- **VERIFY (10/16) past the last LBA and BYTCHK miscompare return GOOD** (SBC-3 5.29, 5.31). The same as LIO and tgt.
- **MODE SELECT (10) that clears WCE is not applied:** WCE reads back 1 (SPC-3 6.8, 6.10).
- **STOP UNIT (IMMED 0 and 1) is not enforced:** TEST UNIT READY and READ return GOOD (SBC-3 4.21.4).
- **REGISTER with a 16-byte list returns 5/24/00, not 5/1A/00** (SPC-3 6.12). The same as tgt.
- **ABORT TASK of a task that does not exist returns function complete** (SAM: task does not exist).

## istgt behaviour that stalls the run

- **Zero-length READ (10), WRITE (10), WRITE (16) and COMPARE AND WRITE of 0 blocks get no answer:** the
  initiator waits until its timeout ("Resource temporarily unavailable"). A command that transfers nothing must
  complete (SBC-3 5.11, 5.13: a transfer length of zero is not an error). This is the most serious finding.

## Spit issues found here

- **COMPARE AND WRITE expectation:** the case expects CHECK CONDITION 5/20/00 when istgt answers GOOD. The
  expectation must follow the support REPORT SUPPORTED OPERATION CODES gives, not a fixed value.
- **PREEMPT and PREEMPT AND ABORT, READ FULL STATUS** expectations are fixed: an unimplemented optional command
  is judged as a failure. Follow the support answer, as for the tgt triage.

## Not yet checked against the clause

- Unlisted LUN REQUEST SENSE (5/25/00 expected, CHECK CONDITION returned: consistent with the spec).
- INQUIRY EVPD=0 with a page code (GOOD returned; SPC-3 6.4.1 requires CHECK CONDITION): the same as LIO and SAT.
- VPD 83h designator type 8h framing (SPC-3 7.6.3).

## Zero-length commands (checked 2026-11-13 against the kernel initiator)

- **Zero-length WRITE(10), WRITE(16) and COMPARE AND WRITE: no answer.** The Linux initiator (sg_raw) sends them
  and gets no response within 15 s, so this is istgt's behaviour, not spit's. spit reports it as a stall.
- **Zero-length READ(10) and READ CAPACITY(16) with allocation length 0: GOOD** from the kernel initiator. spit
  used to clear the R bit for a zero-length READ, and istgt then did not answer; spit now sets R by direction
  (rust/src/iscsi/session.rs), and these cases pass on istgt.
