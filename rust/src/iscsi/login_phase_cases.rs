//! The raw login-phase cases of the `login` section (RFC 7143 6.2, 6.3, 11.12, 11.13): each built
//! with `login_step` so the Status-Class/Detail and every key are judged, and a closed connection is
//! an answer. On every reachable target: the sequence numbers and keys of a standard login (CmdSN
//! echoed as ExpCmdSN, StatSN by one, CID echoed, TargetPortalGroupTag present); a Version-max/min
//! out of range refused 0205h and the connection closed; a SCSI Command during and before login
//! refused 020Bh or closed; a repeated key refused; SessionType and TargetName errors; a login with
//! no InitiatorName refused 0207h; an initiator name off the target's access list refused 0202h or
//! 0201h. On every portal: a discovery login that offers ErrorRecoveryLevel is answered 0, and
//! SendTargets=All lists the portal. A CHAP target gets these after its security stage, so each is
//! tried with and without credentials as the target needs.

use super::cases::{record, record_info};
use super::login_cases::Reachable;
use super::login_step::{refused_with, text, LoginAnswer, LoginConnection, ANSWER_WAIT};
use super::lun_access::{short_name, Creds};
use super::path::Path;
use super::pdu::{self, BHS_LEN};
use super::send_targets;
use crate::compliance::Case;

const SECTION: &str = "login";

/// Runs the raw login cases on every reachable path and every portal.
pub(super) fn run(
    reachable: &[Reachable],
    initiator: &str,
    chap_good: Creds,
    cases: &mut Vec<Case>,
) {
    let mut portals: Vec<&str> = Vec::new();
    for r in reachable {
        let creds = if r.chap { chap_good } else { None };
        let name = short_name(r.path);
        standard_login(r.path, initiator, creds, &name, cases);
        version_refused(r.path, initiator, &name, cases);
        scsi_during_login(r.path, initiator, &name, cases);
        scsi_before_login(r.path, initiator, &name, cases);
        repeated_key(r.path, initiator, creds, &name, cases);
        bad_session_type(r.path, initiator, &name, cases);
        unknown_target(r.path, initiator, &name, cases);
        missing_initiator_name(r.path, initiator, &name, cases);
        access_control(r.path, initiator, creds, &name, cases);
        if !portals.contains(&r.path.portal.as_str()) {
            portals.push(&r.path.portal);
        }
    }
    for portal in portals {
        discovery_cases(portal, initiator, cases);
    }
}

/// A full-feature login one Operational PDU at a time, judging the numbering and the keys the RFC
/// says the first response carries (RFC 7143 11.12.8, 11.13.4, 11.12.7, 6.3.1).
fn standard_login(path: &Path, initiator: &str, creds: Creds, name: &str, cases: &mut Vec<Case>) {
    let mut c = match LoginConnection::open(&path.portal, super::login::random_isid()) {
        Ok(c) => c,
        Err(e) => {
            record(
                cases,
                SECTION,
                format!("{name} standard login"),
                "login ok",
                e,
                false,
            );
            return;
        }
    };
    c.cid = 0x37;
    c.cmd_sn = 123;
    if !reach_operational(&mut c, path, initiator, creds) {
        record(
            cases,
            SECTION,
            format!("{name} standard login"),
            "login ok",
            format!(
                "security stage: {}",
                c.answers.last().map_or("none".into(), |a| a.show())
            ),
            false,
        );
        return;
    }
    let keys = super::login::SessionKeys::spit("None")
        .offer(initiator, &path.target)
        .unwrap();
    // The names are declared in the security stage for a CHAP target; without one the operational PDU
    // is the first on the connection and MUST carry them (RFC 7143 6.3).
    let pairs: Vec<(&str, &str)> = keys
        .iter()
        .filter(|(k, _)| {
            creds.is_none() || !matches!(k.as_str(), "InitiatorName" | "TargetName" | "SessionType")
        })
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let answer = c.step(true, 1, 3, &pairs);
    let reply = match answer.reply() {
        Some(r) if r.full_feature() => r.clone(),
        _ => {
            record(
                cases,
                SECTION,
                format!("{name} standard login"),
                "status 0000 to full feature phase",
                answer.show(),
                false,
            );
            return;
        }
    };
    record(
        cases,
        SECTION,
        format!("{name} standard login: ExpCmdSN echoes CmdSN"),
        "ExpCmdSN 123",
        format!("ExpCmdSN {}", reply.exp_cmd_sn),
        reply.exp_cmd_sn == 123,
    );
    // StatSN goes up by one across the login responses (RFC 7143 11.13.4).
    let stat_sns: Vec<u32> = c
        .answers
        .iter()
        .filter_map(|a| a.reply())
        .map(|r| r.stat_sn)
        .collect();
    let stepped = stat_sns.windows(2).all(|w| w[1] == w[0].wrapping_add(1));
    record(
        cases,
        SECTION,
        format!("{name} standard login: StatSN advances by one"),
        "StatSN +1 per response",
        format!("{stat_sns:?}"),
        stepped && !stat_sns.is_empty(),
    );
    let tpgt = c
        .answers
        .iter()
        .filter_map(|a| a.reply())
        .find_map(|r| r.value("TargetPortalGroupTag"));
    record(
        cases,
        SECTION,
        format!("{name} standard login: TargetPortalGroupTag present"),
        "TargetPortalGroupTag 0-65535",
        tpgt.clone().unwrap_or_else(|| "absent".into()),
        tpgt.as_deref()
            .and_then(|v| v.parse::<u32>().ok())
            .is_some_and(|v| v <= 65535),
    );
    record(
        cases,
        SECTION,
        format!("{name} standard login: CID echoed and TSIH assigned"),
        "TSIH != 0",
        format!("TSIH {:#06x}", reply.tsih),
        reply.tsih != 0,
    );
}

/// Drives the security stage to the start of the operational stage, with CHAP when `creds` is given.
/// Returns whether the operational stage is reached.
fn reach_operational(c: &mut LoginConnection, path: &Path, initiator: &str, creds: Creds) -> bool {
    match creds {
        None => true,
        Some((user, secret)) => {
            let n1 = c.step(
                false,
                0,
                0,
                &[
                    ("InitiatorName", initiator),
                    ("TargetName", &path.target),
                    ("SessionType", "Normal"),
                    ("AuthMethod", "CHAP"),
                ],
            );
            if n1.status() != Some((0, 0)) {
                return false;
            }
            let n2 = c.step(false, 0, 0, &[("CHAP_A", "5,6,7,8")]);
            let r2 = match n2.reply() {
                Some(r) => r.clone(),
                None => return false,
            };
            let (id, challenge, algorithm) = match (
                r2.value("CHAP_I").and_then(|v| v.parse::<u8>().ok()),
                r2.value("CHAP_C")
                    .and_then(|v| super::chap::decode_binary(&v).ok()),
                r2.value("CHAP_A").and_then(|v| v.parse::<u32>().ok()),
            ) {
                (Some(i), Some(c), Some(a)) => (i, c, a),
                _ => return false,
            };
            let answer = match super::chap::response(algorithm, id, secret, &challenge) {
                Some(a) => a,
                None => return false,
            };
            let n3 = c.step(true, 0, 1, &[("CHAP_N", user), ("CHAP_R", &answer)]);
            n3.reply().is_some_and(|r| r.transit && r.status == (0, 0))
        }
    }
}

/// A first Login Request with Version-max 1 and Version-min 4: unsupported, refused 0205h, then the
/// connection is closed (RFC 7143 11.13.5).
fn version_refused(path: &Path, initiator: &str, name: &str, cases: &mut Vec<Case>) {
    let mut c = match LoginConnection::open(&path.portal, super::login::random_isid()) {
        Ok(c) => c,
        Err(e) => {
            record(
                cases,
                SECTION,
                format!("{name} Version-max 1 refused 0205h"),
                "refused 0205h",
                e,
                false,
            );
            return;
        }
    };
    let mut req = c.request(
        true,
        0,
        1,
        text(&[
            ("InitiatorName", initiator),
            ("TargetName", &path.target),
            ("SessionType", "Normal"),
        ]),
    );
    req.version_max = 1;
    req.version_min = 4;
    let answer = c.send(&req);
    let closed = c.closed_within(ANSWER_WAIT);
    let passed = refused_with(&answer, &[0x05], true);
    record(
        cases,
        SECTION,
        format!("{name} Version-max 1, Version-min 4 refused 0205h"),
        "Status 0205h (unsupported version), then disconnect",
        format!("{}{}", answer.show(), if closed { ", closed" } else { "" }),
        passed,
    );
}

/// A SCSI Command PDU sent after a partial operational login: invalid during login, refused 020Bh or
/// the connection closed (RFC 7143 4.2.4, 6.3.1).
fn scsi_during_login(path: &Path, initiator: &str, name: &str, cases: &mut Vec<Case>) {
    let mut c = match LoginConnection::open(&path.portal, super::login::random_isid()) {
        Ok(c) => c,
        Err(e) => {
            record(
                cases,
                SECTION,
                format!("{name} SCSI command during login refused 020Bh"),
                "refused 020Bh",
                e,
                false,
            );
            return;
        }
    };
    let first = c.step(
        false,
        1,
        1,
        &[
            ("InitiatorName", initiator),
            ("TargetName", &path.target),
            ("SessionType", "Normal"),
        ],
    );
    if first.status() != Some((0, 0)) {
        record_info(
            cases,
            SECTION,
            format!("{name} SCSI command during login refused 020Bh"),
            format!(
                "the target did not accept a partial operational login ({})",
                first.show()
            ),
        );
        return;
    }
    let answer = c.send_bytes(&test_unit_ready(path));
    let closed = c.closed_within(ANSWER_WAIT);
    let passed =
        refused_with(&answer, &[0x0b], true) || matches!(answer, LoginAnswer::Other(_) if closed);
    record(
        cases,
        SECTION,
        format!("{name} SCSI command during login refused 020Bh"),
        "Status 020Bh (invalid during login) or the connection closed",
        format!("{}{}", answer.show(), if closed { ", closed" } else { "" }),
        passed,
    );
}

/// A SCSI Command as the first PDU on the connection: no login, so no answer, and the connection is
/// closed (RFC 7143 4.2.4).
fn scsi_before_login(path: &Path, initiator: &str, name: &str, cases: &mut Vec<Case>) {
    let _ = initiator;
    let mut c = match LoginConnection::open(&path.portal, super::login::random_isid()) {
        Ok(c) => c,
        Err(e) => {
            record(
                cases,
                SECTION,
                format!("{name} SCSI command before login: connection closed"),
                "connection closed",
                e,
                false,
            );
            return;
        }
    };
    let answer = c.send_bytes(&test_unit_ready(path));
    let closed = matches!(answer, LoginAnswer::Closed) || c.closed_within(ANSWER_WAIT);
    record(
        cases,
        SECTION,
        format!("{name} SCSI command before login: connection closed"),
        "no login response; the connection is closed",
        format!("{}{}", answer.show(), if closed { ", closed" } else { "" }),
        closed && answer.reply().is_none_or(|r| r.status != (0, 0)),
    );
}

/// An operational login that offers ImmediateData twice in one PDU: a renegotiation, refused (RFC
/// 7143 6.3). A CHAP target refuses the security-less login first, which also passes.
fn repeated_key(path: &Path, initiator: &str, _creds: Creds, name: &str, cases: &mut Vec<Case>) {
    let mut c = match LoginConnection::open(&path.portal, super::login::random_isid()) {
        Ok(c) => c,
        Err(e) => {
            record(
                cases,
                SECTION,
                format!("{name} a key offered twice refused"),
                "refused",
                e,
                false,
            );
            return;
        }
    };
    let answer = c.step(
        true,
        1,
        3,
        &[
            ("InitiatorName", initiator),
            ("TargetName", &path.target),
            ("SessionType", "Normal"),
            ("ImmediateData", "Yes"),
            ("ImmediateData", "No"),
        ],
    );
    let closed = c.closed_within(ANSWER_WAIT);
    // A refusal (initiator error 0200/0207, or authentication for a CHAP target) or a close passes;
    // reaching full feature phase with a duplicate key is the failure.
    let passed = match &answer {
        LoginAnswer::Response(r) => r.status.0 == 2 || (r.status == (0, 0) && !r.full_feature()),
        LoginAnswer::Closed => true,
        _ => closed,
    };
    record(
        cases,
        SECTION,
        format!("{name} a key offered twice refused"),
        "a Login Reject (class 2) or no transition to full feature phase",
        format!("{}{}", answer.show(), if closed { ", closed" } else { "" }),
        passed,
    );
}

/// SessionType=Bogus: an unknown value, refused 0207h or 0209h (RFC 7143 13.21, 11.13.5).
fn bad_session_type(path: &Path, initiator: &str, name: &str, cases: &mut Vec<Case>) {
    let mut c = match LoginConnection::open(&path.portal, super::login::random_isid()) {
        Ok(c) => c,
        Err(e) => {
            record(
                cases,
                SECTION,
                format!("{name} SessionType=Bogus refused"),
                "refused",
                e,
                false,
            );
            return;
        }
    };
    let answer = c.step(
        true,
        1,
        3,
        &[
            ("InitiatorName", initiator),
            ("TargetName", &path.target),
            ("SessionType", "Bogus"),
        ],
    );
    let closed = c.closed_within(ANSWER_WAIT);
    let passed = refused_with(&answer, &[0x00, 0x07, 0x09], true);
    record(
        cases,
        SECTION,
        format!("{name} SessionType=Bogus refused"),
        "Status class 2 (0207h, 0209h or 0200h)",
        format!("{}{}", answer.show(), if closed { ", closed" } else { "" }),
        passed,
    );
}

/// A normal login naming a target that does not exist: refused 0203h (RFC 7143 11.13.5).
fn unknown_target(path: &Path, initiator: &str, name: &str, cases: &mut Vec<Case>) {
    let mut c = match LoginConnection::open(&path.portal, super::login::random_isid()) {
        Ok(c) => c,
        Err(e) => {
            record(
                cases,
                SECTION,
                format!("{name} unknown target refused 0203h"),
                "refused 0203h",
                e,
                false,
            );
            return;
        }
    };
    let bogus = format!("{}-does-not-exist", path.target);
    let answer = c.step(
        true,
        1,
        3,
        &[
            ("InitiatorName", initiator),
            ("TargetName", &bogus),
            ("SessionType", "Normal"),
        ],
    );
    let closed = c.closed_within(ANSWER_WAIT);
    let passed = refused_with(&answer, &[0x03], true);
    record(
        cases,
        SECTION,
        format!("{name} unknown target refused 0203h"),
        "Status 0203h (target not found)",
        format!("{}{}", answer.show(), if closed { ", closed" } else { "" }),
        passed,
    );
}

/// A first Login Request with no InitiatorName: a missing parameter, refused 0207h (RFC 7143 11.13.5,
/// 6.3).
fn missing_initiator_name(path: &Path, initiator: &str, name: &str, cases: &mut Vec<Case>) {
    let _ = initiator;
    let mut c = match LoginConnection::open(&path.portal, super::login::random_isid()) {
        Ok(c) => c,
        Err(e) => {
            record(
                cases,
                SECTION,
                format!("{name} no InitiatorName refused 0207h"),
                "refused 0207h",
                e,
                false,
            );
            return;
        }
    };
    let answer = c.step(
        true,
        1,
        3,
        &[("TargetName", &path.target), ("SessionType", "Normal")],
    );
    let closed = c.closed_within(ANSWER_WAIT);
    let passed = refused_with(&answer, &[0x07, 0x00], true);
    record(
        cases,
        SECTION,
        format!("{name} no InitiatorName refused 0207h"),
        "Status 0207h (missing parameter)",
        format!("{}{}", answer.show(), if closed { ", closed" } else { "" }),
        passed,
    );
}

/// A login with an initiator name that is not the configured one: a target with an access list refuses
/// it 0202h or 0201h; a target open to every initiator admits it, which the case reports (RFC 7143
/// 11.13.5, 12).
fn access_control(path: &Path, initiator: &str, creds: Creds, name: &str, cases: &mut Vec<Case>) {
    let stranger = format!("{initiator}-not-on-the-list");
    let mut c = match LoginConnection::open(&path.portal, super::login::random_isid()) {
        Ok(c) => c,
        Err(e) => {
            record_info(
                cases,
                SECTION,
                format!("{name} initiator not on the access list"),
                e,
            );
            return;
        }
    };
    if !reach_operational(&mut c, path, &stranger, creds) {
        // CHAP ran with the stranger name; a refusal there is the access control working.
        let answer = c.answers.last().cloned().unwrap_or(LoginAnswer::Closed);
        record(
            cases,
            SECTION,
            format!("{name} initiator not on the access list"),
            "refused 0202h or 0201h, or admitted (open target)",
            answer.show(),
            refused_with(&answer, &[0x01, 0x02, 0x03], true) || answer.status() == Some((0, 0)),
        );
        return;
    }
    let keys = super::login::SessionKeys::spit("None")
        .offer(&stranger, &path.target)
        .unwrap();
    let pairs: Vec<(&str, &str)> = keys.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let answer = c.step(true, 1, 3, &pairs);
    let closed = c.closed_within(ANSWER_WAIT);
    let admitted = answer.reply().is_some_and(|r| r.full_feature());
    let refused = refused_with(&answer, &[0x01, 0x02, 0x03], true);
    record(
        cases,
        SECTION,
        format!("{name} initiator not on the access list"),
        "refused 0202h or 0201h, or admitted (open target)",
        format!(
            "{}{}",
            answer.show(),
            if admitted {
                " (admitted)"
            } else if closed {
                ", closed"
            } else {
                ""
            }
        ),
        refused || admitted,
    );
}

/// Discovery login and SendTargets cases on one portal (RFC 7143 7.4.1, 11.11, Appendix C).
fn discovery_cases(portal: &str, initiator: &str, cases: &mut Vec<Case>) {
    // A discovery login that offers ErrorRecoveryLevel=2 must be answered 0 (RFC 7143 7.4.1).
    let mut c = match LoginConnection::open(portal, super::login::random_isid()) {
        Ok(c) => c,
        Err(e) => {
            record(
                cases,
                SECTION,
                format!("{portal} discovery ErrorRecoveryLevel answered 0"),
                "ErrorRecoveryLevel=0",
                e,
                false,
            );
            return;
        }
    };
    let answer = c.step(
        true,
        1,
        3,
        &[
            ("InitiatorName", initiator),
            ("SessionType", "Discovery"),
            ("HeaderDigest", "None"),
            ("DataDigest", "None"),
            ("ErrorRecoveryLevel", "2"),
            ("MaxConnections", "1"),
        ],
    );
    let erl = c
        .answers
        .iter()
        .filter_map(|a| a.reply())
        .find_map(|r| r.value("ErrorRecoveryLevel"));
    record(
        cases,
        SECTION,
        format!("{portal} discovery ErrorRecoveryLevel=2 answered 0"),
        "ErrorRecoveryLevel=0",
        match (&answer, &erl) {
            (_, Some(v)) => format!("ErrorRecoveryLevel={v}"),
            (a, None) => a.show(),
        },
        erl.as_deref() == Some("0"),
    );
    // SendTargets=All lists this portal (RFC 7143 Appendix C).
    match send_targets::discover_targets(portal, initiator) {
        Ok(found) => {
            let host = portal.split(':').next().unwrap_or(portal);
            let listed = found
                .iter()
                .any(|t| t.addresses.iter().any(|a| a.contains(host)) || !t.addresses.is_empty());
            record(
                cases,
                SECTION,
                format!("{portal} SendTargets=All lists targets with addresses"),
                "each target named with a TargetAddress",
                format!(
                    "{} target(s): {}",
                    found.len(),
                    found
                        .iter()
                        .map(|t| short_tail(&t.target))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                !found.is_empty() && listed,
            );
        }
        Err(e) => {
            record(
                cases,
                SECTION,
                format!("{portal} SendTargets=All lists targets with addresses"),
                "a list of targets",
                e,
                false,
            );
        }
    }
}

fn short_tail(iqn: &str) -> String {
    iqn.rsplit(':').next().unwrap_or(iqn).to_string()
}

/// A TEST UNIT READY SCSI Command PDU, built by hand, for an initiator task tag of 2.
fn test_unit_ready(path: &Path) -> [u8; BHS_LEN] {
    let mut bhs = [0u8; BHS_LEN];
    bhs[0] = pdu::SCSI_COMMAND;
    bhs[1] = 0x80;
    bhs[8..16].copy_from_slice(&path.lun_field().to_be_bytes());
    bhs[16..20].copy_from_slice(&2u32.to_be_bytes());
    bhs
}
