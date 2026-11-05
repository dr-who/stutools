//! The `chap` cases of `--compliance` (RFC 7143 12.1.3, RFC 7144): on every target that needs CHAP,
//! each CHAP_A value from 1 to 10 is offered alone. The values defined for iSCSI (5 MD5, 6 SHA-1, 7
//! SHA-256, 8 SHA3-256) must be accepted as offered or refused, any other value refused, and at least
//! one defined value accepted; a wrong secret is refused under every accepted value.

use super::login::{self, reason};
use super::login_cases::Reachable;
use super::lun_access::{short_name, Creds};
use crate::compliance::{case, Case};

/// The CHAP_A values defined for iSCSI (MD5, SHA-1, SHA-256, SHA3-256), all of which spit implements.
const DEFINED_CHAP: [u32; 4] = [5, 6, 7, 8];

/// Runs the chap cases with the right credentials `chap_good` and a wrong secret `chap_bad`.
pub(super) fn run(
    reachable: &[Reachable],
    initiator: &str,
    chap_good: Creds,
    chap_bad: Creds,
    cases: &mut Vec<Case>,
) {
    if let Some(good) = chap_good {
        for r in reachable.iter().filter(|r| r.chap) {
            let p = r.path;
            let mut accepted = Vec::new();
            for a in 1..=10u32 {
                let defined = DEFINED_CHAP.contains(&a);
                let (actual, passed) = match login::login_with_offer(
                    &p.portal,
                    &p.target,
                    initiator,
                    Some(good),
                    "CRC32C,None",
                    &a.to_string(),
                ) {
                    Ok(s) => {
                        let got = s.negotiated.chap_algorithm;
                        let _ = s.logout();
                        if got == Some(a) {
                            accepted.push(a);
                        }
                        (
                            format!("ok CHAP_A={}", got.unwrap_or(0)),
                            defined && got == Some(a),
                        )
                    }
                    Err(err) => (format!("refused ({})", reason(&err)), true),
                };
                cases.push(case(
                    "chap",
                    format!("{} offer CHAP_A={a}", short_name(p)),
                    if defined {
                        format!("ok CHAP_A={a}, or refused")
                    } else {
                        "refused".into()
                    },
                    actual,
                    passed,
                ));
            }
            cases.push(case(
                "chap",
                format!("{} accepts a defined algorithm", short_name(p)),
                "one of CHAP_A 5, 6, 7, 8".into(),
                format!("{accepted:?}"),
                !accepted.is_empty(),
            ));
            for a in accepted {
                let (actual, passed) = match login::login_with_offer(
                    &p.portal,
                    &p.target,
                    initiator,
                    chap_bad,
                    "CRC32C,None",
                    &a.to_string(),
                ) {
                    Ok(s) => {
                        let _ = s.logout();
                        ("ok (wrong secret accepted)".to_string(), false)
                    }
                    Err(err) => (format!("refused ({})", reason(&err)), true),
                };
                cases.push(case(
                    "chap",
                    format!("{} wrong secret CHAP_A={a}", short_name(p)),
                    "refused".into(),
                    actual,
                    passed,
                ));
            }
        }
    }
}
