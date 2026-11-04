//! `--scsitest`: runs a matrix of cases over the `--iscsi` targets and scores them. Each case states
//! what the target should do; the case passes when the login or SCSI result matches that.
//!
//! The expectations come from a file (`--scsitest-expect`), one line per target:
//!     # target  lun-group  auth  digest-required  alua-state  size-gib
//!     spitlunsbc-1  A  none  no  active/optimized  20

use super::report::{self, identity_differences, Collected, Path};
use super::session::Negotiated;
use crate::{cli::Config, Result};
use std::path::Path as FsPath;

/// What one target is expected to do.
#[derive(Debug, Clone)]
struct Expect {
    name: String,
    group: String,
    chap: bool,
    digest_required: bool,
    alua: String,
    size_gib: f64,
    /// CHAP algorithms (CHAP_A values) the target accepts. Empty for targets without CHAP.
    chap_algs: Vec<u32>,
}

fn parse_expectations(path: &FsPath) -> Result<Vec<Expect>> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() != 6 && f.len() != 7 {
            return Err(format!(
                "{}:{}: expected 6 or 7 fields, found {}",
                path.display(),
                n + 1,
                f.len()
            ));
        }
        let chap_algs = match f.get(6) {
            Some(list) if *list != "-" => list
                .split(',')
                .map(|a| {
                    a.parse::<u32>().map_err(|_| {
                        format!("{}:{}: bad CHAP algorithm {a}", path.display(), n + 1)
                    })
                })
                .collect::<Result<Vec<u32>>>()?,
            _ => Vec::new(),
        };
        out.push(Expect {
            name: f[0].to_string(),
            group: f[1].to_string(),
            chap: match f[2] {
                "chap" => true,
                "none" => false,
                other => {
                    return Err(format!(
                        "{}:{}: auth must be none or chap, not {other}",
                        path.display(),
                        n + 1
                    ))
                }
            },
            digest_required: match f[3] {
                "yes" => true,
                "no" => false,
                other => {
                    return Err(format!(
                        "{}:{}: digest-required must be yes or no, not {other}",
                        path.display(),
                        n + 1
                    ))
                }
            },
            alua: f[4].to_string(),
            chap_algs,
            size_gib: f[5]
                .parse()
                .map_err(|_| format!("{}:{}: size-gib must be a number", path.display(), n + 1))?,
        });
    }
    Ok(out)
}

/// The short name of a target: the part after the last colon of its IQN.
pub(super) fn short(path: &Path) -> &str {
    path.target.rsplit(':').next().unwrap_or(&path.target)
}

/// The CHAP_A values this initiator implements: MD5, SHA-1, SHA-256 and SHA3-256.
const IMPLEMENTED_CHAP: [u32; 4] = [5, 6, 7, 8];

/// One scored case.
pub(super) struct Case {
    pub(super) section: &'static str,
    pub(super) name: String,
    pub(super) expected: String,
    pub(super) actual: String,
    pub(super) passed: bool,
}

pub(super) fn case(
    section: &'static str,
    name: String,
    expected: String,
    actual: String,
    passed: bool,
) -> Case {
    Case {
        section,
        name,
        expected,
        actual,
        passed,
    }
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

fn digest_name(on: bool) -> &'static str {
    if on {
        "CRC32C"
    } else {
        "none"
    }
}

/// Runs the matrix and prints each case and the score.
pub fn run(config: &Config) -> Result<()> {
    let initiator = config
        .iscsi_initiator
        .as_deref()
        .ok_or("--scsitest needs --iscsi-initiator IQN")?;
    let expect_file = config
        .scsitest_expect
        .as_ref()
        .ok_or("--scsitest needs --scsitest-expect FILE")?;
    let expectations = parse_expectations(expect_file)?;
    let paths: Vec<Path> = config
        .iscsi
        .iter()
        .map(|p| report::parse_path(p))
        .collect::<Result<_>>()?;
    let secret = match (&config.chap_user, &config.chap_secret_file) {
        (Some(_), Some(file)) => Some(report::read_secret(file)?),
        (None, None) => None,
        _ => return Err("CHAP needs both --chap-user and --chap-secret-file".into()),
    };
    let user = config.chap_user.clone().unwrap_or_default();
    let chap_good = secret.as_deref().map(|s| (user.as_str(), s));
    let wrong = b"not-the-secret".to_vec();
    let chap_bad = config.chap_user.as_deref().map(|u| (u, wrong.as_slice()));

    let mut cases = Vec::new();
    let expect_of = |p: &Path| expectations.iter().find(|e| e.name == short(p)).cloned();

    // Section 1: login, for each digest offer and each credential mode.
    for p in &paths {
        let Some(e) = expect_of(p) else {
            cases.push(case(
                "login",
                short(p).into(),
                "profile line".into(),
                "missing from --scsitest-expect".into(),
                false,
            ));
            continue;
        };
        for digest in ["None", "CRC32C", "CRC32C,None"] {
            for creds in ["none", "correct", "wrong"] {
                let (expect_ok, expect_digest) = (
                    !((e.chap && creds != "correct") || (digest == "None" && e.digest_required)),
                    if digest == "None" { "none" } else { "CRC32C" },
                );
                let result: Result<Negotiated> = match creds {
                    "none" => report_login(p, initiator, None, digest),
                    "correct" => report_login(p, initiator, chap_good, digest),
                    _ => report_login(p, initiator, chap_bad, digest),
                };
                let (actual, passed) = match &result {
                    Ok(n) => {
                        let got = digest_name(n.header_digest);
                        (
                            format!("ok digest={got}"),
                            expect_ok && got == expect_digest,
                        )
                    }
                    Err(err) => (format!("refused ({})", short_reason(err)), !expect_ok),
                };
                let expected = if expect_ok {
                    format!("ok digest={expect_digest}")
                } else {
                    "refused".to_string()
                };
                cases.push(case(
                    "login",
                    format!("{} digest={digest} creds={creds}", short(p)),
                    expected,
                    actual,
                    passed,
                ));
            }
        }
    }

    // Section 2: identity over every ordering of every subset of the targets.
    let collected: Vec<Result<Collected>> = paths
        .iter()
        .map(|p| report::collect(p, initiator, chap_good))
        .collect();
    for order in orderings(paths.len()) {
        let names: Vec<&str> = order.iter().map(|&i| short(&paths[i])).collect();
        let groups: Vec<String> = order
            .iter()
            .map(|&i| {
                expect_of(&paths[i])
                    .map(|e| e.group)
                    .unwrap_or_else(|| "?".into())
            })
            .collect();
        let expect_same = groups.windows(2).all(|w| w[0] == w[1]);
        let results: Vec<Result<Collected>> = order
            .iter()
            .map(|&i| match &collected[i] {
                Ok(c) => Ok(c.clone()),
                Err(e) => Err(e.clone()),
            })
            .collect();
        let differences = identity_differences(&results);
        let all_ok = results.iter().all(|r| r.is_ok());
        let actual_same = all_ok && differences.is_empty();
        let expected = if expect_same {
            "same LUN"
        } else {
            "different LUN"
        };
        let actual = if !all_ok {
            "a path failed".to_string()
        } else if actual_same {
            "same LUN".to_string()
        } else {
            format!("different: {}", differences.join(", "))
        };
        cases.push(case(
            "identity",
            names.join(" + "),
            expected.into(),
            actual,
            actual_same == expect_same,
        ));
    }

    // Section 3: each target's SCSI answers and its ALUA state and size.
    for (p, result) in paths.iter().zip(&collected) {
        let Some(e) = expect_of(p) else { continue };
        match result {
            Err(err) => cases.push(case(
                "scsi",
                format!("{} commands", short(p)),
                "all answer".into(),
                format!("failed: {}", short_reason(err)),
                false,
            )),
            Ok(c) => {
                cases.push(case(
                    "scsi",
                    format!(
                        "{} INQUIRY, VPD 0x80/0x83, READ CAPACITY(16), REPORT TPG",
                        short(p)
                    ),
                    "all answer".into(),
                    "all answer".into(),
                    true,
                ));
                let own = c
                    .own_port
                    .and_then(|port| c.groups.iter().find(|g| g.ports.contains(&port)))
                    .map(|g| super::scsi::state_name(g.state).to_string())
                    .unwrap_or_else(|| "unknown".into());
                cases.push(case(
                    "scsi",
                    format!("{} ALUA state", short(p)),
                    e.alua.clone(),
                    own.clone(),
                    own == e.alua,
                ));
                let size = (c.last_lba + 1) as f64 * c.block_len as f64 / 1_073_741_824.0;
                let size_ok = (size - e.size_gib).abs() < 0.005;
                cases.push(case(
                    "scsi",
                    format!("{} size", short(p)),
                    format!("{} GiB", e.size_gib),
                    format!("{size:.2} GiB"),
                    size_ok,
                ));
                let chap_seen = c.login.chap_algorithm.is_some();
                cases.push(case(
                    "scsi",
                    format!("{} auth", short(p)),
                    if e.chap { "CHAP" } else { "none" }.into(),
                    if chap_seen { "CHAP" } else { "none" }.into(),
                    chap_seen == e.chap,
                ));
            }
        }
    }

    // Section 4: CHAP algorithms. Each CHAP_A value is offered alone with the right secret. A value passes
    // when the target accepts it and spit implements it. The wrong secret is tried for every value the
    // target accepts.
    if let Some(good) = chap_good {
        for p in &paths {
            let Some(e) = expect_of(p) else { continue };
            if !e.chap {
                continue;
            }
            for a in 1..=10u32 {
                let expect_ok = e.chap_algs.contains(&a) && IMPLEMENTED_CHAP.contains(&a);
                let offer = a.to_string();
                let (actual, passed) = match super::session::login_with_offer(
                    &p.portal,
                    &p.target,
                    initiator,
                    Some(good),
                    "CRC32C,None",
                    &offer,
                ) {
                    Ok(session) => {
                        let got = session.negotiated.chap_algorithm;
                        let _ = session.logout();
                        (
                            format!("ok CHAP_A={}", got.unwrap_or(0)),
                            expect_ok && got == Some(a),
                        )
                    }
                    Err(err) => (format!("refused ({})", short_reason(&err)), !expect_ok),
                };
                let expected = if expect_ok {
                    format!("ok CHAP_A={a}")
                } else {
                    "refused".into()
                };
                cases.push(case(
                    "chap",
                    format!("{} offer CHAP_A={a}", short(p)),
                    expected,
                    actual,
                    passed,
                ));
            }
            for &a in &e.chap_algs {
                let (actual, passed) = match super::session::login_with_offer(
                    &p.portal,
                    &p.target,
                    initiator,
                    chap_bad,
                    "CRC32C,None",
                    &a.to_string(),
                ) {
                    Ok(session) => {
                        let _ = session.logout();
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
    for p in &paths {
        let Some(e) = expect_of(p) else { continue };
        let creds = if e.chap { chap_good } else { None };
        if e.chap && creds.is_none() {
            continue;
        }
        let name = short(p).to_string();
        let login_one =
            || super::session::login(&p.portal, &p.target, initiator, creds, "CRC32C,None");
        for k in 1..=5 {
            let (actual, passed) = match login_one() {
                Ok(session) => {
                    let tsih = session.negotiated.tsih;
                    let out = session.logout();
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
                format!("{name} login/logout cycle {k}"),
                "login ok, logout ok".into(),
                actual,
                passed,
            ));
        }
        let mut open = Vec::new();
        let mut failure = None;
        for _ in 0..3 {
            match login_one() {
                Ok(session) => open.push(session),
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
        for session in open {
            logouts_ok &= session.logout().is_ok();
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
        let (actual, passed) = match login_one() {
            Ok(session) => {
                let tsih = session.negotiated.tsih;
                let out = session.logout();
                (
                    format!(
                        "login ok, tsih {tsih}, logout {}",
                        if out.is_ok() { "ok" } else { "failed" }
                    ),
                    out.is_ok(),
                )
            }
            Err(err) => (format!("refused ({})", short_reason(&err)), false),
        };
        cases.push(case(
            "session",
            format!("{name} login after all logouts"),
            "login ok, logout ok".into(),
            actual,
            passed,
        ));
    }

    // Section 6: SBC and SPC operations (UNMAP, COMPARE AND WRITE, PERSISTENT RESERVE) on every target
    // whose credentials are known.
    let mut ops_targets: Vec<(&Path, super::conformance::Creds, bool)> = Vec::new();
    for p in &paths {
        let Some(e) = expect_of(p) else { continue };
        if e.chap && chap_good.is_none() {
            continue;
        }
        ops_targets.push((
            p,
            if e.chap { chap_good } else { None },
            e.alua == "standby",
        ));
    }
    super::conformance::run(&ops_targets, initiator, &mut cases);

    print_cases(&cases);
    let total = cases.len();
    let passed = cases.iter().filter(|c| c.passed).count();
    let failed = total - passed;
    println!();
    println!(
        "{:<12}{:>8}{:>8}{:>8}{:>9}",
        "section", "cases", "passed", "failed", "score"
    );
    for section in [
        "login", "identity", "scsi", "chap", "session", "spc", "unmap", "ats", "pr", "mpio",
    ] {
        let of: Vec<&Case> = cases.iter().filter(|c| c.section == section).collect();
        let ok = of.iter().filter(|c| c.passed).count();
        println!(
            "{section:<12}{:>8}{ok:>8}{:>8}{:>8.1}%",
            of.len(),
            of.len() - ok,
            percent(ok, of.len())
        );
    }
    println!(
        "{:<12}{total:>8}{passed:>8}{failed:>8}{:>8.1}%",
        "total",
        percent(passed, total)
    );
    if failed > 0 {
        println!();
        println!("failed cases:");
        for c in cases.iter().filter(|c| !c.passed) {
            println!(
                "  [{}] {}: expected {}, got {}",
                c.section, c.name, c.expected, c.actual
            );
        }
        return Err(format!("{failed} of {total} cases failed"));
    }
    Ok(())
}

fn percent(ok: usize, total: usize) -> f64 {
    if total == 0 {
        100.0
    } else {
        100.0 * ok as f64 / total as f64
    }
}

/// One login attempt, closed again at once. Returns what the target negotiated.
fn report_login(
    p: &Path,
    initiator: &str,
    chap: Option<(&str, &[u8])>,
    digest: &str,
) -> Result<Negotiated> {
    let session = if chap.is_some() {
        report::login_with_fallback(p, initiator, chap, digest)?
    } else {
        super::session::login(&p.portal, &p.target, initiator, None, digest)?
    };
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

fn print_cases(cases: &[Case]) {
    for c in cases {
        println!(
            "{}  {:<9} {:<58} expect {:<22} got {}",
            if c.passed { "PASS" } else { "FAIL" },
            c.section,
            c.name,
            c.expected,
            c.actual
        );
    }
}
