//! The `login` cases of `--compliance` (RFC 7143 6.3, 12.1, 13.1): each target path is first logged
//! in to without credentials, so a CHAP target shows itself by refusing (authentication failure); a
//! target that refuses this initiator (its ACL) or needs CHAP without credentials is tested no further.
//! Each reachable path then gets the three digest offers, the key negotiation cases (`negotiation`),
//! and, with CHAP, a wrong secret that must be refused. Returns the paths the later sections test.

use super::login::{self, reason, Negotiated};
use super::lun_access::{short_name, Creds};
use super::path::Path;
use crate::compliance::{case, Case};
use crate::Result;

/// A target path that can be logged in to, and whether it needs CHAP.
pub(super) struct Reachable<'a> {
    pub(super) path: &'a Path,
    pub(super) chap: bool,
    /// The iSCSIProtocolLevel the target answered (RFC 7144 7.1.1), when it did.
    pub(super) level: Option<u32>,
}

/// Runs the login cases on every path; `chap_bad` is the CHAP user with a wrong secret.
pub(super) fn run<'a>(
    paths: &'a [Path],
    initiator: &str,
    chap_good: Creds,
    chap_bad: Creds,
    cases: &mut Vec<Case>,
    untested: &mut Vec<(String, String)>,
) -> Vec<Reachable<'a>> {
    let mut reachable = Vec::new();
    for p in paths {
        let name = format!("{} at {}", short_name(p), p.portal);
        let probe = login::login(&p.portal, &p.target, initiator, None, "CRC32C,None");
        let chap = match probe {
            Ok(s) => {
                let _ = s.logout();
                false
            }
            Err(e) if e.contains("authentication failure") => {
                cases.push(case(
                    "login",
                    format!("{name} no credentials"),
                    "refused (authentication failure)".into(),
                    "refused (authentication failure)".into(),
                    true,
                ));
                if chap_good.is_none() {
                    untested.push((
                        name,
                        "needs CHAP: give --chap-user and --chap-secret-file".into(),
                    ));
                    continue;
                }
                true
            }
            Err(e) => {
                untested.push((name, format!("login refused: {}", reason(&e))));
                continue;
            }
        };
        let creds = if chap { chap_good } else { None };
        login_digests(p, &name, initiator, creds, cases);
        let level = super::negotiation::run(p, &name, initiator, creds, cases);
        if chap {
            let (actual, passed) = match report_login(p, initiator, chap_bad, "CRC32C,None") {
                Ok(_) => ("ok (wrong secret accepted)".to_string(), false),
                Err(e) => (
                    format!("refused ({})", reason(&e)),
                    e.contains("authentication failure"),
                ),
            };
            cases.push(case(
                "login",
                format!("{name} wrong secret"),
                "refused (authentication failure)".into(),
                actual,
                passed,
            ));
        }
        reachable.push(Reachable {
            path: p,
            chap,
            level,
        });
    }
    reachable
}

/// The three digest offers. Each must be answered with a value it offered; "CRC32C,None" must log in
/// when either alone does, with CRC32C when the target takes it (RFC 7143: the first value it supports).
fn login_digests(
    p: &Path,
    name: &str,
    initiator: &str,
    creds: Option<(&str, &[u8])>,
    cases: &mut Vec<Case>,
) {
    let none = report_login(p, initiator, creds, "None");
    let crc = report_login(p, initiator, creds, "CRC32C");
    let both = report_login(p, initiator, creds, "CRC32C,None");
    let shown = |r: &Result<Negotiated>| match r {
        Ok(n) => format!("ok digest={}", digest_name(n.header_digest)),
        Err(e) => format!("refused ({})", reason(e)),
    };
    cases.push(case(
        "login",
        format!("{name} digest=None"),
        "ok digest=none, or refused".into(),
        shown(&none),
        none.as_ref().map_or(true, |n| !n.header_digest),
    ));
    cases.push(case(
        "login",
        format!("{name} digest=CRC32C"),
        "ok digest=CRC32C, or refused".into(),
        match &crc {
            Ok(n) if !n.header_digest => {
                "ok digest=none: a value not offered (RFC 7143 6.2.1)".into()
            }
            _ => shown(&crc),
        },
        crc.as_ref().map_or(true, |n| n.header_digest),
    ));
    // RFC 7143 6.2.1: the answer is the first offered value the target supports. CRC32C is supported
    // only when it was negotiated: an answer of None to an offer of CRC32C alone is a value not
    // offered (a protocol error, failed above), not support.
    let crc_supported = crc.as_ref().is_ok_and(|n| n.header_digest);
    let expected = if crc_supported {
        "ok digest=CRC32C"
    } else if none.is_ok() {
        "ok digest=none"
    } else {
        "ok"
    };
    let passed = match &both {
        Ok(n) => crc_supported == n.header_digest || (crc.is_err() && none.is_err()),
        Err(_) => false,
    };
    cases.push(case(
        "login",
        format!("{name} digest=CRC32C,None"),
        expected.into(),
        shown(&both),
        passed,
    ));
}

fn digest_name(on: bool) -> &'static str {
    if on {
        "CRC32C"
    } else {
        "none"
    }
}

/// One login attempt, closed again at once. Returns what the target negotiated.
fn report_login(
    p: &Path,
    initiator: &str,
    chap: Option<(&str, &[u8])>,
    digest: &str,
) -> Result<Negotiated> {
    let session = login::login(&p.portal, &p.target, initiator, chap, digest)?;
    let negotiated = session.negotiated.clone();
    let _ = session.logout();
    Ok(negotiated)
}
