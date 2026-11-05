//! The iSCSI cases of `--compliance`. Every `--iscsi` HOST is discovered (SendTargets) and each target
//! it reports is tested at every address it gives; HOST/IQN tests that target. The expectations come
//! from RFC 7143, SPC-4 and SBC-3 and from what the target itself reports, never from a profile:
//! - login: each digest offer is answered with a value that was offered, and "CRC32C,None" gets
//!   CRC32C when the target takes CRC32C alone; a CHAP target refuses no credentials and a wrong
//!   secret;
//! - identity: paths whose LUN designators (VPD 0x83) agree report one LUN, and the verdict does not
//!   depend on the order the paths are compared in;
//! - scsi: the identity commands answer, the capacity is sane, and the answering port is in a group
//!   REPORT TARGET PORT GROUPS lists;
//! - chap: CHAP_A 5 to 8 are accepted as offered or refused, any other value is refused, and a wrong
//!   secret is refused for every accepted value;
//! - session, and the SCSI operations in `conformance`.
//!
//! A target that refuses this initiator (its ACL) or needs CHAP without credentials is tested no further.

use super::report::{self, identity_differences, Collected, Path};
use super::session::{self, Negotiated};
use crate::compliance::{case, Case};
use crate::{cli::Config, Result};

/// The CHAP_A values defined for iSCSI (MD5, SHA-1, SHA-256, SHA3-256), all of which spit implements.
const DEFINED_CHAP: [u32; 4] = [5, 6, 7, 8];

/// Orderings are tried over every subset of up to this many paths, and over pairs beyond it.
const ORDERING_LIMIT: usize = 5;

/// The short name of a target: the part after the last colon of its IQN.
pub(super) fn short(path: &Path) -> &str {
    path.target.rsplit(':').next().unwrap_or(&path.target)
}

/// A target that can be logged in to, and whether it needs CHAP.
struct Reachable<'a> {
    path: &'a Path,
    chap: bool,
}

/// Adds the iSCSI cases for the `--iscsi` targets to `cases`, and the targets tested no further to
/// `untested`.
pub fn cases(
    config: &Config,
    cases: &mut Vec<Case>,
    untested: &mut Vec<(String, String)>,
) -> Result<()> {
    let initiator = config.iscsi_initiator.as_deref().unwrap_or_default();
    let paths = targets(&config.iscsi, initiator)?;
    let secret = report::chap_from_config(config)?;
    let chap_good = secret.as_ref().map(|(u, s)| (u.as_str(), s.as_slice()));
    let wrong = b"not-the-secret".to_vec();
    let chap_bad = config.chap_user.as_deref().map(|u| (u, wrong.as_slice()));

    // Section 1: login. Without credentials first, so a CHAP target shows itself by refusing.
    let mut reachable = Vec::new();
    for p in &paths {
        let name = format!("{} at {}", short(p), p.portal);
        let probe = session::login(&p.portal, &p.target, initiator, None, "CRC32C,None");
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
                untested.push((name, format!("login refused: {}", short_reason(&e))));
                continue;
            }
        };
        let creds = if chap { chap_good } else { None };
        login_digests(p, &name, initiator, creds, cases);
        if chap {
            let (actual, passed) = match report_login(p, initiator, chap_bad, "CRC32C,None") {
                Ok(_) => ("ok (wrong secret accepted)".to_string(), false),
                Err(e) => (
                    format!("refused ({})", short_reason(&e)),
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
        reachable.push(Reachable { path: p, chap });
    }
    let creds_of = |r: &Reachable| if r.chap { chap_good } else { None };

    // Section 2: identity, over every ordering of every subset of the paths (pairs past the limit).
    let collected: Vec<Result<Collected>> = reachable
        .iter()
        .map(|r| report::collect(r.path, initiator, creds_of(r)))
        .collect();
    let groups = if reachable.len() <= ORDERING_LIMIT {
        orderings(reachable.len())
    } else {
        pairs(reachable.len())
    };
    for order in groups.into_iter().filter(|o| o.len() > 1) {
        let names: Vec<&str> = order.iter().map(|&i| short(reachable[i].path)).collect();
        let results: Vec<Result<Collected>> = order.iter().map(|&i| collected[i].clone()).collect();
        let keys: Vec<Option<String>> = results
            .iter()
            .map(|r| r.as_ref().ok().map(lun_key))
            .collect();
        if keys.iter().any(|k| k.is_none()) {
            cases.push(case(
                "identity",
                names.join(" + "),
                "every path answers".into(),
                "a path failed".into(),
                false,
            ));
            continue;
        }
        let expect_same = keys.windows(2).all(|w| w[0] == w[1]);
        let differences = identity_differences(&results);
        let actual_same = differences.is_empty();
        cases.push(case(
            "identity",
            names.join(" + "),
            if expect_same {
                "same LUN"
            } else {
                "different LUN"
            }
            .into(),
            if actual_same {
                "same LUN".to_string()
            } else {
                format!("different: {}", differences.join(", "))
            },
            actual_same == expect_same,
        ));
    }

    // Section 3: each target's SCSI answers, its capacity, and its port in REPORT TARGET PORT GROUPS.
    let mut standby = vec![false; reachable.len()];
    for (i, (r, result)) in reachable.iter().zip(&collected).enumerate() {
        let name = short(r.path);
        let c = match result {
            Err(err) => {
                cases.push(case(
                    "scsi",
                    format!("{name} commands"),
                    "all answer".into(),
                    format!("failed: {}", short_reason(err)),
                    false,
                ));
                continue;
            }
            Ok(c) => c,
        };
        cases.push(case(
            "scsi",
            format!("{name} INQUIRY, VPD 0x80/0x83, READ CAPACITY(16), REPORT TPG"),
            "all answer".into(),
            "all answer".into(),
            true,
        ));
        let size_ok = c.last_lba > 0 && c.block_len >= 512 && c.block_len.is_power_of_two();
        cases.push(case(
            "scsi",
            format!("{name} capacity"),
            "blocks > 0 of 512 B or more, a power of two".into(),
            format!("{} blocks of {} B", c.last_lba + 1, c.block_len),
            size_ok,
        ));
        let own = c
            .own_port
            .and_then(|port| c.groups.iter().find(|g| g.ports.contains(&port)))
            .map(|g| super::scsi::state_name(g.state).to_string());
        standby[i] = own.as_deref() == Some("standby");
        cases.push(case(
            "scsi",
            format!("{name} ALUA state"),
            "its port in a listed group".into(),
            own.clone().unwrap_or_else(|| "port not listed".into()),
            own.is_some(),
        ));
    }

    // Section 4: CHAP algorithms, on every CHAP target, each CHAP_A value offered alone.
    if let Some(good) = chap_good {
        for r in reachable.iter().filter(|r| r.chap) {
            let p = r.path;
            let mut accepted = Vec::new();
            for a in 1..=10u32 {
                let defined = DEFINED_CHAP.contains(&a);
                let (actual, passed) = match session::login_with_offer(
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
                    Err(err) => (format!("refused ({})", short_reason(&err)), true),
                };
                cases.push(case(
                    "chap",
                    format!("{} offer CHAP_A={a}", short(p)),
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
                format!("{} accepts a defined algorithm", short(p)),
                "one of CHAP_A 5, 6, 7, 8".into(),
                format!("{accepted:?}"),
                !accepted.is_empty(),
            ));
            for a in accepted {
                let (actual, passed) = match session::login_with_offer(
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
                    Err(err) => (format!("refused ({})", short_reason(&err)), true),
                };
                cases.push(case(
                    "chap",
                    format!("{} wrong secret CHAP_A={a}", short(p)),
                    "refused".into(),
                    actual,
                    passed,
                ));
            }
        }
    }

    // Section 5: sessions. Repeated login and logout on one target, several sessions open at once, and a
    // login after everything has logged out. Each login must get its own session handle (TSIH).
    for r in &reachable {
        sessions(r.path, initiator, creds_of(r), cases);
    }

    // Section 6: SBC and SPC operations (UNMAP, COMPARE AND WRITE, PERSISTENT RESERVE). A LUN in the
    // standby state is checked for refusing data commands instead.
    let ops: Vec<(&Path, super::conformance::Creds, bool)> = reachable
        .iter()
        .zip(&standby)
        .map(|(r, &sb)| (r.path, creds_of(r), sb))
        .collect();
    super::conformance::run(&ops, initiator, cases);
    Ok(())
}

/// The targets to test: each HOST/IQN as given, and every target a HOST alone reports, at each of its
/// addresses.
fn targets(entries: &[String], initiator: &str) -> Result<Vec<Path>> {
    let mut out = Vec::new();
    for entry in entries {
        if entry.contains('/') {
            out.push(report::parse_path(entry)?);
            continue;
        }
        for t in session::discover_targets(entry, initiator)? {
            for address in t.addresses {
                out.push(Path {
                    portal: address,
                    target: t.target.clone(),
                });
            }
        }
    }
    Ok(out)
}

/// The LUN's own designators (association 0), which name it the same way on every path.
fn lun_key(c: &Collected) -> String {
    let mut ids: Vec<String> = c
        .designators
        .iter()
        .filter(|d| d.association == 0)
        .map(|d| format!("{}:{}", d.kind, d.value))
        .collect();
    ids.sort();
    ids.join(",")
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
        Err(e) => format!("refused ({})", short_reason(e)),
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
        shown(&crc),
        crc.as_ref().map_or(true, |n| n.header_digest),
    ));
    let expected = if crc.is_ok() {
        "ok digest=CRC32C"
    } else if none.is_ok() {
        "ok digest=none"
    } else {
        "ok"
    };
    let passed = match &both {
        Ok(n) => crc.is_ok() == n.header_digest || (crc.is_err() && none.is_err()),
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

/// Five login and logout cycles, three sessions at once with distinct handles, and a login after.
fn sessions(p: &Path, initiator: &str, creds: Option<(&str, &[u8])>, cases: &mut Vec<Case>) {
    let name = short(p).to_string();
    let login_one = || session::login(&p.portal, &p.target, initiator, creds, "CRC32C,None");
    let cycle = |label: String, cases: &mut Vec<Case>| {
        let (actual, passed) = match login_one() {
            Ok(s) => {
                let tsih = s.negotiated.tsih;
                let out = s.logout();
                (
                    format!(
                        "tsih {tsih}, logout {}",
                        if out.is_ok() { "ok" } else { "failed" }
                    ),
                    out.is_ok(),
                )
            }
            Err(err) => (format!("refused ({})", short_reason(&err)), false),
        };
        cases.push(case(
            "session",
            label,
            "login ok, logout ok".into(),
            actual,
            passed,
        ));
    };
    for k in 1..=5 {
        cycle(format!("{name} login/logout cycle {k}"), cases);
    }
    let mut open = Vec::new();
    let mut failure = None;
    for _ in 0..3 {
        match login_one() {
            Ok(s) => open.push(s),
            Err(err) => {
                failure = Some(short_reason(&err));
                break;
            }
        }
    }
    let mut handles: Vec<u16> = open.iter().map(|s| s.negotiated.tsih).collect();
    handles.sort_unstable();
    handles.dedup();
    let distinct = handles.len() == 3;
    let mut logouts_ok = true;
    for s in open {
        logouts_ok &= s.logout().is_ok();
    }
    let actual = match &failure {
        Some(reason) => format!("refused ({reason})"),
        None => format!(
            "3 sessions, {} distinct handles, logouts {}",
            handles.len(),
            if logouts_ok { "ok" } else { "failed" }
        ),
    };
    cases.push(case(
        "session",
        format!("{name} 3 sessions at once"),
        "3 distinct handles, logouts ok".into(),
        actual,
        distinct && logouts_ok && failure.is_none(),
    ));
    cycle(format!("{name} login after all logouts"), cases);
}

/// All orderings of every non-empty subset of `0..n`. Their verdicts must not depend on the order.
fn orderings(n: usize) -> Vec<Vec<usize>> {
    fn extend(prefix: &mut Vec<usize>, n: usize, out: &mut Vec<Vec<usize>>) {
        if !prefix.is_empty() {
            out.push(prefix.clone());
        }
        for i in 0..n {
            if !prefix.contains(&i) {
                prefix.push(i);
                extend(prefix, n, out);
                prefix.pop();
            }
        }
    }
    let mut out = Vec::new();
    extend(&mut Vec::new(), n, &mut out);
    out
}

/// Every ordered pair of distinct indices below `n`.
fn pairs(n: usize) -> Vec<Vec<usize>> {
    (0..n)
        .flat_map(|a| (0..n).filter(move |&b| b != a).map(move |b| vec![a, b]))
        .collect()
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
    let session = session::login(&p.portal, &p.target, initiator, chap, digest)?;
    let negotiated = session.negotiated.clone();
    let _ = session.logout();
    Ok(negotiated)
}

fn short_reason(error: &str) -> String {
    match error.find('(') {
        Some(i) => error[i + 1..].trim_end_matches(')').to_string(),
        None => error.chars().take(40).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orderings_cover_every_subset_in_every_order() {
        // 3 + 6 + 6 orderings of the subsets of three paths.
        assert_eq!(orderings(3).len(), 15);
        assert!(orderings(3).contains(&vec![2, 0, 1]));
    }

    #[test]
    fn pairs_are_ordered_and_distinct() {
        assert_eq!(pairs(3).len(), 6);
        assert!(pairs(3).iter().all(|p| p[0] != p[1]));
    }

    #[test]
    fn reasons_are_taken_from_the_brackets() {
        assert_eq!(
            short_reason("login refused (authentication failure)"),
            "authentication failure"
        );
    }
}
