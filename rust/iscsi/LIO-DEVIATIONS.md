# LIO deviations found by spit

Found by `spit --iscsi 127.0.0.2/iqn.2026-12.com.stutools:spit-pr-ram --compliance` (harness/lio-ram),
which runs the persistent reservation cases over three portals. Each entry names the clause, what the
standard requires, what LIO returned, and the control that separates a LIO fault from a spit fault.
These are findings to confirm with LIO's maintainers, not yet filed.

## 1. RELEASE of a reservation with a different TYPE or SCOPE, by the holder

- Clause: SPC-3 5.6.10.2. The holder's RELEASE whose TYPE or SCOPE does not match the reservation
  must complete with CHECK CONDITION, ILLEGAL REQUEST, INVALID RELEASE OF PERSISTENT RESERVATION
  (ASC 26h, ASCQ 04h, the code in the SPC-4 table INCITS 513), and the reservation is not altered.
- LIO: RESERVATION CONFLICT (status 18h).
- Seen in every reservation type (6 cases per run). The control is the holder's correct RELEASE, which
  gets GOOD in the same run, so the holder and the reservation are right.

## 2. Non-holder RELEASE by a registered sibling port (same initiator name, different ISID)

- Clause: SPC-3 5.6.10.2 (a registered nexus that is not the holder: GOOD, nothing released).
- LIO: RESERVATION CONFLICT.
- Control: a non-holder from a different initiator name (`spit-initiator-b`) gets GOOD for RELEASE in
  every type. So LIO handles the release when the initiator name differs.

## 3. Unregistered I_T nexus sharing a name with a registered port (different ISID)

- Clause: SPC-3 6.11.1 and table 13 (SBC-3 5.13 reservation rules): reads and writes from an
  unregistered nexus must conflict under registrants-only and all-registrants types.
- LIO, with the port named by B's initiator name and a different ISID: GOOD for READ and WRITE, and for
  SYNCHRONIZE CACHE, WRITE SAME, WRITE AND VERIFY and COMPARE AND WRITE.
- Control: the same port under its own initiator name (`spit-initiator-c`, passed as the third name of
  `--iscsi-initiator`): every case gets RESERVATION CONFLICT as the standard requires. So the conflict
  is applied when the initiator name differs, and LIO does not apply it when only the ISID differs.

## 4. A second path of the holder (same name, ISID differs in its qualifier) writes while the holder holds

- Clause: SPC-3 5.6.1 and table 13: a write from a nexus other than the holder must conflict.
- LIO: GOOD. Same cause as 3: the name matches, so LIO treats the path as the holder's nexus.

## Not yet explained

- READ FULL STATUS lists three distinct iSCSI TransportIDs with their ISIDs, so LIO keeps the
  registrations apart. What is not yet known is why the conflict check does not use the ISID the same
  way. A wire trace of the same run with the registrations is the next check.
- The wire trace shows the sessions carry distinct ISIDs (80a1, 80b2, 80c3, 80d4), and that the
  unregistered port's key is never registered, so spit sends what the test describes.
