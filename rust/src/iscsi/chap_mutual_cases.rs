//! The mutual-CHAP and CHAP-security cases of the `chap` section (RFC 7143 12.1.3, 9.2.1, RFC 1994
//! 4.1), each a raw CHAP handshake on a `login_step` connection whose third Login Request this module
//! builds key by key. On every CHAP target: a wrong-length CHAP_R (20 and 14 bytes) and a transition
//! with no CHAP_R must be refused 0201h; a CHAP_N that is not the user must be refused; a CHAP_A list
//! is answered with a value in the list; the CHAP_C the target sends is fresh on every login; a
//! second initiator name authenticates only with its own secret. On a target that authenticates
//! itself (one that answers CHAP_N and CHAP_R): spit sends its own CHAP_I and CHAP_C, as an 8-byte
//! challenge, and as 1- and 1024-byte challenges in hex and Base64, and checks the target's CHAP_R is
//! H(id || target secret || challenge); the target's own CHAP_C reflected as spit's challenge must be
//! refused. A one-way response in Base64 is accepted, a wrong one refused. Without the right target
//! secret the mutual cases print SKIP.

use super::cases::{record, record_info};
use super::chap;
use super::login::{self, check_mutual, Mutual};
use super::login_cases::Reachable;
use super::login_step::{refused_with, LoginAnswer, LoginConnection, ANSWER_WAIT};
use super::lun_access::{short_name, Creds};
use super::path::Path;
use crate::compliance::Case;

const SECTION: &str = "chap";

/// What the target sent in the second security PDU: the algorithm it chose and its challenge.
struct Challenge {
    algorithm: u32,
    identifier: u8,
    challenge: Vec<u8>,
}

/// What the initiator puts in the third Login Request.
struct ThirdRequest {
    name: String,
    response: String,
    /// CHAP_I and CHAP_C for mutual authentication, when the initiator asks for it.
    mutual: Option<(u8, String)>,
    transit: bool,
    /// Leave out CHAP_R (a premature transition with no response).
    omit_response: bool,
}

/// Runs the security stage with `offer` as CHAP_A and returns the target's challenge, or the answer
/// if the handshake did not get that far.
fn security_stage(
    c: &mut LoginConnection,
    path: &Path,
    initiator: &str,
    offer: &str,
) -> std::result::Result<Challenge, LoginAnswer> {
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
        return Err(n1);
    }
    let n2 = c.step(false, 0, 0, &[("CHAP_A", offer)]);
    let r2 = n2.reply().cloned().ok_or_else(|| n2.clone())?;
    match (
        r2.value("CHAP_A").and_then(|v| v.parse::<u32>().ok()),
        r2.value("CHAP_I").and_then(|v| v.parse::<u8>().ok()),
        r2.value("CHAP_C")
            .and_then(|v| chap::decode_binary(&v).ok()),
    ) {
        (Some(algorithm), Some(identifier), Some(challenge)) => Ok(Challenge {
            algorithm,
            identifier,
            challenge,
        }),
        _ => Err(LoginAnswer::Response(r2)),
    }
}

/// Sends the third Login Request built from `t` and returns the answer.
fn third(c: &mut LoginConnection, t: &ThirdRequest) -> LoginAnswer {
    let mut pairs: Vec<(&str, String)> = vec![("CHAP_N", t.name.clone())];
    if !t.omit_response {
        pairs.push(("CHAP_R", t.response.clone()));
    }
    let id;
    if let Some((i, chall)) = &t.mutual {
        id = i.to_string();
        pairs.push(("CHAP_I", id));
        pairs.push(("CHAP_C", chall.clone()));
    }
    let refs: Vec<(&str, &str)> = pairs.iter().map(|(k, v)| (*k, v.as_str())).collect();
    c.step(t.transit, 0, 1, &refs)
}

/// Runs the CHAP cases. `good`/`bad` are the right and a wrong secret for `initiator`; `target_creds`
/// is the name and secret the target authenticates itself with; `second` is a second initiator name
/// and its own secret.
pub(super) fn run(
    reachable: &[Reachable],
    initiator: &str,
    good: Creds,
    bad: Creds,
    target_creds: Creds,
    second: Option<(&str, &[u8], &str, &[u8])>,
    cases: &mut Vec<Case>,
) {
    let Some((user, secret)) = good else { return };
    for r in reachable.iter().filter(|r| r.chap) {
        let name = short_name(r.path);
        wrong_response_length(r.path, initiator, user, secret, &name, cases, None);
        wrong_response_length(r.path, initiator, user, secret, &name, cases, Some(14));
        no_response(r.path, initiator, user, &name, cases);
        wrong_name(r.path, initiator, secret, bad, &name, cases);
        algorithm_list(r.path, initiator, user, secret, &name, cases);
        fresh_challenge(r.path, initiator, &name, cases);
        base64_one_way(r.path, initiator, user, secret, &name, cases);
        if let Some((name_b, secret_b, _, _)) = second {
            cross_secret(r.path, name_b, secret_b, secret, &name, cases);
        }
        mutual_cases(r.path, initiator, user, secret, target_creds, &name, cases);
    }
}

/// CHAP_R of the wrong length (RFC 7143 12.1.3, RFC 1994 4.1): the correct digest padded or cut to `len`
/// bytes, or to one byte more than the digest's own length when `len` is None. The length must not be the
/// digest's own (that would be a correct answer), so the default is always wrong for the algorithm chosen.
/// The login must be refused 0201h (0200h or 0207h also pass), never reaching full feature phase.
fn wrong_response_length(
    path: &Path,
    initiator: &str,
    user: &str,
    secret: &[u8],
    name: &str,
    cases: &mut Vec<Case>,
    len: Option<usize>,
) {
    let shown = len.map_or("digest + 1".to_string(), |n| n.to_string());
    let label = format!("{name} CHAP_R of {shown} bytes refused");
    let mut c = match LoginConnection::open(&path.portal, login::random_isid()) {
        Ok(c) => c,
        Err(e) => return nope(cases, label, e),
    };
    let ch = match security_stage(&mut c, path, initiator, "5,6,7,8") {
        Ok(ch) => ch,
        Err(a) => return skip(cases, label, &a),
    };
    let mut bytes =
        chap::digest(ch.algorithm, ch.identifier, secret, &ch.challenge).unwrap_or_default();
    let len = len.unwrap_or(bytes.len() + 1);
    bytes.resize(len, 0);
    let answer = third(
        &mut c,
        &ThirdRequest {
            name: user.to_string(),
            response: chap::encode_hex(&bytes),
            mutual: None,
            transit: true,
            omit_response: false,
        },
    );
    let closed = c.closed_within(ANSWER_WAIT);
    let reached = answer.reply().is_some_and(|r| r.full_feature());
    record(
        cases,
        SECTION,
        label,
        "Login Reject 0201h (0200h/0207h), no full feature phase",
        format!("{}{}", answer.show(), if closed { ", closed" } else { "" }),
        !reached && refused_with(&answer, &[0x00, 0x01, 0x07], true),
    );
}

/// A premature transition with no CHAP_R (RFC 7143 12.1.3, UNH CHAP 1.1): an authentication bypass;
/// the login must be refused and never reach full feature phase.
fn no_response(path: &Path, initiator: &str, user: &str, name: &str, cases: &mut Vec<Case>) {
    let label = format!("{name} transition with no CHAP_R refused");
    let mut c = match LoginConnection::open(&path.portal, login::random_isid()) {
        Ok(c) => c,
        Err(e) => return nope(cases, label, e),
    };
    if let Err(a) = security_stage(&mut c, path, initiator, "5,6,7,8") {
        return skip(cases, label, &a);
    }
    let answer = third(
        &mut c,
        &ThirdRequest {
            name: user.to_string(),
            response: String::new(),
            mutual: None,
            transit: true,
            omit_response: true,
        },
    );
    let closed = c.closed_within(ANSWER_WAIT);
    let reached = answer.reply().is_some_and(|r| r.full_feature());
    record(
        cases,
        SECTION,
        label,
        "Login Reject (class 2), no full feature phase",
        format!("{}{}", answer.show(), if closed { ", closed" } else { "" }),
        !reached
            && (refused_with(&answer, &[0x00, 0x01, 0x07], true)
                || !closed && answer.status() != Some((0, 0))),
    );
}

/// CHAP_N that is not the configured user, with the right response for that challenge under the real
/// secret but the wrong name (RFC 7143 12.1.3, UNH CHAP 5.1): refused.
fn wrong_name(
    path: &Path,
    initiator: &str,
    secret: &[u8],
    _bad: Creds,
    name: &str,
    cases: &mut Vec<Case>,
) {
    let label = format!("{name} CHAP_N not the configured user refused");
    let mut c = match LoginConnection::open(&path.portal, login::random_isid()) {
        Ok(c) => c,
        Err(e) => return nope(cases, label, e),
    };
    let ch = match security_stage(&mut c, path, initiator, "5,6,7,8") {
        Ok(ch) => ch,
        Err(a) => return skip(cases, label, &a),
    };
    let response =
        chap::response(ch.algorithm, ch.identifier, secret, &ch.challenge).unwrap_or_default();
    let answer = third(
        &mut c,
        &ThirdRequest {
            name: "not-a-configured-chap-user".to_string(),
            response,
            mutual: None,
            transit: true,
            omit_response: false,
        },
    );
    let closed = c.closed_within(ANSWER_WAIT);
    let reached = answer.reply().is_some_and(|r| r.full_feature());
    record(
        cases,
        SECTION,
        label,
        "Login Reject 0201h, no full feature phase",
        format!("{}{}", answer.show(), if closed { ", closed" } else { "" }),
        !reached && refused_with(&answer, &[0x00, 0x01, 0x02, 0x07], true),
    );
}

/// A CHAP_A list "1,3,5,9": the target answers with a value in the list that it supports (MD5 at
/// least), never one not offered (RFC 7143 6.3.2, 12.1.3, UNH CHAP 2.2).
fn algorithm_list(
    path: &Path,
    initiator: &str,
    _user: &str,
    _secret: &[u8],
    name: &str,
    cases: &mut Vec<Case>,
) {
    let label = format!("{name} CHAP_A=1,3,5,9 picks a value in the list");
    let mut c = match LoginConnection::open(&path.portal, login::random_isid()) {
        Ok(c) => c,
        Err(e) => return nope(cases, label, e),
    };
    match security_stage(&mut c, path, initiator, "1,3,5,9") {
        Ok(ch) => {
            record(
                cases,
                SECTION,
                label,
                "CHAP_A in {1,3,5,9}, and 5 if only MD5",
                format!("CHAP_A={}", ch.algorithm),
                [1, 3, 5, 9].contains(&ch.algorithm),
            );
        }
        Err(a) => skip(cases, label, &a),
    }
}

/// The CHAP_C a target sends is fresh on each login (RFC 7143 9.2.1, UNH CHAP 4.1): five logins, no
/// two challenges the same.
fn fresh_challenge(path: &Path, initiator: &str, name: &str, cases: &mut Vec<Case>) {
    let label = format!("{name} a fresh CHAP_C on every login");
    let mut seen: Vec<Vec<u8>> = Vec::new();
    for _ in 0..5 {
        let mut c = match LoginConnection::open(&path.portal, login::random_isid()) {
            Ok(c) => c,
            Err(e) => return nope(cases, label, e),
        };
        match security_stage(&mut c, path, initiator, "5,6,7,8") {
            Ok(ch) => seen.push(ch.challenge),
            Err(a) => return skip(cases, label, &a),
        }
    }
    let mut unique = seen.clone();
    unique.sort();
    unique.dedup();
    record(
        cases,
        SECTION,
        label,
        "5 distinct challenges",
        format!("{} distinct of {}", unique.len(), seen.len()),
        unique.len() == seen.len(),
    );
}

/// A correct one-way CHAP response encoded in Base64 is accepted (RFC 7143 6.1, 12.1.3, HLK OCHAP.2):
/// the login reaches full feature phase.
fn base64_one_way(
    path: &Path,
    initiator: &str,
    user: &str,
    secret: &[u8],
    name: &str,
    cases: &mut Vec<Case>,
) {
    let label = format!("{name} CHAP response in Base64 accepted");
    let mut c = match LoginConnection::open(&path.portal, login::random_isid()) {
        Ok(c) => c,
        Err(e) => return nope(cases, label, e),
    };
    let ch = match security_stage(&mut c, path, initiator, "5,6,7,8") {
        Ok(ch) => ch,
        Err(a) => return skip(cases, label, &a),
    };
    let digest =
        chap::digest(ch.algorithm, ch.identifier, secret, &ch.challenge).unwrap_or_default();
    let answer = third(
        &mut c,
        &ThirdRequest {
            name: user.to_string(),
            response: chap::encode_base64(&digest),
            mutual: None,
            transit: true,
            omit_response: false,
        },
    );
    record(
        cases,
        SECTION,
        label,
        "the initiator is authenticated (transition out of security)",
        answer.show(),
        authenticated(&answer),
    );
}

/// A second initiator name authenticating with its own secret, and with the first initiator's secret
/// (WHCP.ISCSI.7, RFC 7143 12.1.3): the right secret is let in, the wrong one refused.
fn cross_secret(
    path: &Path,
    name_b: &str,
    secret_b: &[u8],
    secret_a: &[u8],
    name: &str,
    cases: &mut Vec<Case>,
) {
    let login_as = |secret: &[u8]| -> LoginAnswer {
        let mut c = match LoginConnection::open(&path.portal, login::random_isid()) {
            Ok(c) => c,
            Err(_) => return LoginAnswer::Closed,
        };
        let ch = match security_stage(&mut c, path, name_b, "5,6,7,8") {
            Ok(ch) => ch,
            Err(a) => return a,
        };
        let response =
            chap::response(ch.algorithm, ch.identifier, secret, &ch.challenge).unwrap_or_default();
        third(
            &mut c,
            &ThirdRequest {
                name: name_b.to_string(),
                response,
                mutual: None,
                transit: true,
                omit_response: false,
            },
        )
    };
    let right = login_as(secret_b);
    record(
        cases,
        SECTION,
        format!("{name} second initiator name with its own secret"),
        "the initiator is authenticated",
        right.show(),
        authenticated(&right),
    );
    let wrong = login_as(secret_a);
    record(
        cases,
        SECTION,
        format!("{name} second initiator name with the first's secret refused"),
        "Login Reject 0201h",
        wrong.show(),
        !authenticated(&wrong) && refused_with(&wrong, &[0x00, 0x01], true),
    );
}

/// The cases that need a target that authenticates itself. One correct mutual login shows whether the
/// target does; if it does not, or the target secret is wrong or missing, the mutual cases SKIP.
fn mutual_cases(
    path: &Path,
    initiator: &str,
    user: &str,
    secret: &[u8],
    target_creds: Creds,
    name: &str,
    cases: &mut Vec<Case>,
) {
    let Some((_t_user, t_secret)) = target_creds else {
        record_info(
            cases,
            SECTION,
            format!("{name} mutual CHAP"),
            "SKIP: no --chap-target-secret-file",
        );
        return;
    };
    // The correct mutual login with an 8-byte challenge.
    match mutual_login(
        path, initiator, user, secret, t_secret, &[0x11; 8], false, false,
    ) {
        MutualOutcome::Verified(tname) => {
            record(
                cases,
                SECTION,
                format!("{name} mutual CHAP_A=5: target's CHAP_R correct"),
                "a correct CHAP_R from the target, full feature phase",
                format!("target CHAP_N={tname}, CHAP_R verified"),
                true,
            );
        }
        MutualOutcome::NotMutual => {
            record_info(
                cases,
                SECTION,
                format!("{name} mutual CHAP"),
                "SKIP: the target does not authenticate itself (no CHAP_R)",
            );
            return;
        }
        MutualOutcome::Refused(why) => {
            record_info(
                cases,
                SECTION,
                format!("{name} mutual CHAP"),
                format!("SKIP: the target does not offer mutual CHAP ({why})"),
            );
            return;
        }
        MutualOutcome::Failed(why) => {
            record(
                cases,
                SECTION,
                format!("{name} mutual CHAP_A=5: target's CHAP_R correct"),
                "a correct CHAP_R from the target",
                why,
                false,
            );
            return;
        }
    }
    // Challenge lengths and encodings (MCHAP.1-4).
    for (len, base64, what) in [
        (1usize, false, "1-byte hex"),
        (1, true, "1-byte Base64"),
        (1024, false, "1024-byte hex"),
        (1024, true, "1024-byte Base64"),
    ] {
        let challenge = chap::random_bytes(len);
        let label = format!("{name} mutual CHAP, {what} challenge");
        match mutual_login(
            path, initiator, user, secret, t_secret, &challenge, base64, false,
        ) {
            MutualOutcome::Verified(_) => {
                record(
                    cases,
                    SECTION,
                    label,
                    "target's CHAP_R correct, full feature phase",
                    "verified",
                    true,
                );
            }
            MutualOutcome::NotMutual | MutualOutcome::Refused(_) => record_info(
                cases,
                SECTION,
                label,
                "SKIP: target does not authenticate itself",
            ),
            MutualOutcome::Failed(why) => {
                record(cases, SECTION, label, "target's CHAP_R correct", why, false);
            }
        }
    }
    // Reflection: spit sends the target's own CHAP_C as its challenge (RFC 7143 9.2.1, UNH CHAP 4.7).
    let label = format!("{name} mutual CHAP with the target's own challenge refused");
    match mutual_login(path, initiator, user, secret, t_secret, &[], false, true) {
        MutualOutcome::Verified(_) => {
            record(
                cases,
                SECTION,
                label,
                "Login Reject 0201h (no CHAP_R)",
                "the target answered its own challenge",
                false,
            );
        }
        MutualOutcome::NotMutual => record_info(
            cases,
            SECTION,
            label,
            "SKIP: target does not authenticate itself",
        ),
        MutualOutcome::Refused(_) | MutualOutcome::Failed(_) => {
            record(
                cases,
                SECTION,
                label,
                "Login Reject 0201h, or no CHAP_R from the target",
                "refused or not answered",
                true,
            );
        }
    }
}

enum MutualOutcome {
    Verified(String),
    NotMutual,
    /// The target refused the login (status class 2) or closed: it does not authenticate itself to
    /// this initiator, so the mutual cases do not apply.
    Refused(String),
    Failed(String),
}

/// One mutual login: spit answers the target, sends its own CHAP_I and CHAP_C (the target's own
/// CHAP_C when `reflect`), and checks the target's CHAP_N and CHAP_R against `t_secret`.
#[allow(clippy::too_many_arguments)]
fn mutual_login(
    path: &Path,
    initiator: &str,
    user: &str,
    secret: &[u8],
    t_secret: &[u8],
    challenge: &[u8],
    base64: bool,
    reflect: bool,
) -> MutualOutcome {
    let mut c = match LoginConnection::open(&path.portal, login::random_isid()) {
        Ok(c) => c,
        Err(e) => return MutualOutcome::Failed(e),
    };
    let ch = match security_stage(&mut c, path, initiator, "5,6,7,8") {
        Ok(ch) => ch,
        Err(a) => return MutualOutcome::Failed(a.show()),
    };
    let my_challenge: Vec<u8> = if reflect {
        ch.challenge.clone()
    } else {
        challenge.to_vec()
    };
    let my_id: u8 = if reflect {
        ch.identifier.wrapping_add(1)
    } else {
        0x5a
    };
    let response =
        chap::response(ch.algorithm, ch.identifier, secret, &ch.challenge).unwrap_or_default();
    let encoded = if base64 {
        chap::encode_base64(&my_challenge)
    } else {
        chap::encode_hex(&my_challenge)
    };
    let answer = third(
        &mut c,
        &ThirdRequest {
            name: user.to_string(),
            response,
            mutual: Some((my_id, encoded)),
            transit: true,
            omit_response: false,
        },
    );
    let reply = match &answer {
        LoginAnswer::Response(r) if r.status == (0, 0) => r.clone(),
        LoginAnswer::Response(r) if r.status.0 == 2 => {
            return MutualOutcome::Refused(answer.show())
        }
        LoginAnswer::Closed => return MutualOutcome::Refused("connection closed".into()),
        other => return MutualOutcome::Failed(other.show()),
    };
    if reply.value("CHAP_R").is_none() {
        return MutualOutcome::NotMutual;
    }
    let m = Mutual {
        secret: t_secret,
        identifier: my_id,
        challenge: &my_challenge,
        base64,
    };
    match check_mutual(ch.algorithm, secret, &m, &reply.pairs()) {
        Ok(tname) => MutualOutcome::Verified(tname),
        Err(why) => MutualOutcome::Failed(why),
    }
}

/// Whether a response to the third CHAP request shows the initiator authenticated: status 0000 and a
/// transition out of the security stage (to operational or full feature phase), RFC 7143 6.3.2.
fn authenticated(answer: &LoginAnswer) -> bool {
    answer
        .reply()
        .is_some_and(|r| r.status == (0, 0) && r.transit && (r.nsg == 1 || r.nsg == 3))
}

fn skip(cases: &mut Vec<Case>, label: String, answer: &LoginAnswer) {
    record_info(
        cases,
        SECTION,
        label,
        format!("SKIP: CHAP handshake did not start ({})", answer.show()),
    );
}

fn nope(cases: &mut Vec<Case>, label: String, err: impl Into<String>) {
    record(cases, SECTION, label, "a CHAP handshake", err.into(), false);
}
