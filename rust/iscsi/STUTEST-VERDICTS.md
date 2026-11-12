# stutest (EXABA appliance, 20 PiB, 6 portals): verdict on each failure

Run: `spit --iscsi 10.100.7.202/iqn.2026-10.com.exaba:179391858000:stutest --compliance` with three initiator
names; 685 cases, 671 pass, 13 fail, 1 warning. Each failure is judged against the spec text, and the verdict
says whether the device fails the standard or spit's expectation is wrong.

| Failure | Verdict | Basis |
|---|---|---|
| Login: a key offered twice is accepted (status 0000) | **Device fails** | RFC 7143 4.3 (repeated declarations): the target MUST answer with a Login reject |
| REQUEST SENSE, DESC 1: CHECK CONDITION 5/24/00 | **Device fails (probable)** | SPC-3 6.27: DESC 1 returns descriptor format when supported; the device refuses it instead of returning fixed format |
| MODE SENSE (10): no short block descriptor | **Spit bug** | SBC-3 6.5.1: a direct-access device supports zero or one block descriptor; 6.5.2.1 requires the short form only when one is returned |
| MODE SENSE (10) LLBAA 1: no long block descriptor | **Spit bug** | SBC-3 6.5.2.1: a long descriptor is optional ("may return") |
| REPORT SUPPORTED OPERATION CODES: WRITE (16) not supported | **Device fails** | SPC-3 6.23.1 must agree with the command; WRITE (16) answers GOOD and reads back in the same run |
| QUERY TASK, QUERY TASK SET, QUERY ASYNC EVENT, I_T NEXUS RESET: function not supported | **Spit bug** | The device's REPORT SUPPORTED TASK MANAGEMENT FUNCTIONS is itself refused (5/24/00), so it gives no support statement; spit must not expect these to succeed without one |
| START STOP UNIT STOP accepted; TUR and READ after it GOOD | **Inconclusive, leaning device** | SBC-3 4.21.4 requires NOT READY, INITIALIZING COMMAND REQUIRED once stopped. The device accepts STOP and stays ready. Needs the device's power-condition support to say whether it has a stopped condition at all |
| Microsoft: Caching page not saveable (PS 0) | **Device conforms; warning only** | The device refuses saved values (5/39/00), so it has no saved page; the Windows requirement is a preference, not a SCSI shall |

## What settles the open verdicts

- **STOP:** read the device's power-condition support (its Power Condition mode page, 1Ah, and the START STOP
  UNIT power-condition bits in the INQUIRY/RSOC answers). If it has a stopped condition, the device fails
  SBC-3 4.21.4; if not, spit must not expect NOT READY.
- **REQUEST SENSE DESC 1:** a trace of the device's answer to DESC 1 with the descriptor support bit, to
  confirm whether it has descriptor format at all.
