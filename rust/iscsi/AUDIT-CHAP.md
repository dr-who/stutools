# Clause audit: CHAP (RFC 7143 12.1.3, with RFC 1994)

Source: RFC 7143 12.1.3 (the text in the scratchpad), eight MUST statements. CHAP_A values: RFC 7143 requires
only **5 (CHAP with MD5)**. The other values spit implements (6 SHA-1, 7 SHA-256, 8 SHA3-256) are extensions the
RFC does not require, so they are not shalls, and the audit does not count them against any target.

| MUST (RFC 7143 12.1.3) | Covered by | Status |
|---|---|---|
| The initiator MUST use CHAP (5) as the algorithm when it offers CHAP | one-way CHAP cases on spitchap-ram | covered (LIO) |
| The target MUST answer a failed initiator authentication with a Login reject (authentication failure) | wrong-secret cases | covered (LIO): refused as authentication failure |
| The initiator MUST continue with CHAP_N, CHAP_R (the response) | one-way login cases | covered (LIO) |
| If the initiator authentication fails the target MUST answer with a Login reject | wrong-secret, wrong-name cases | covered (LIO) |
| Target-required mutual CHAP: the target MUST either answer or refuse the login | mutual CHAP cases on spitchap-ram | covered (LIO, 26 of 26 pass) |
| If target authentication fails the initiator MUST close the connection | mutual-CHAP wrong target secret case | covered where the target offers mutual CHAP (LIO) |
| CHAP_N, CHAP_C and CHAP_R together MUST NOT exceed 1024 bytes | challenge length cases (0, 1, 2, 3, 16, 1024) | covered (LIO) |
| Initiators MUST always offer CHAP (5) as one of the proposed algorithms | the offer is part of the login cases | covered (LIO) |

## Findings

- **FIPS:** the required algorithm is MD5, and LIO on this FIPS-enabled host passes the one-way and mutual CHAP
  cases with it. So MD5 CHAP works on the FIPS host; the earlier open question is closed.
- **Algorithms 7 and 8:** spit implements them, but no target tested so far offers them, so they have no coverage
  yet. Needs a target that offers SHA-256 or SHA3-256 (none in the current lab).
- **tgt:** run with CHAP configured (a tgtadm account bound to the LUN, the secret from /var/tmp/spit-chap/secret):
  19 of 19 CHAP cases pass, including MD5 accepted, the wrong secret refused, and the algorithm offers.
- **stutest:** every portal reports AUTH none, so CHAP has no coverage there.

## Gaps

1. A CHAP run on a target that offers 7 or 8 (the SHA-256 and SHA3 cases), when one exists.
2. The mutual wrong-target-secret case on tgt (tgt's mutual CHAP account is not configured yet).
