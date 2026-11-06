//! The key negotiation cases of the `login` section. The first login offers exactly the operational
//! keys the Windows Server 2022 Microsoft initiator offered in a capture of it onlining a disk; the
//! second offers iSCSIProtocolLevel=1 (RFC 7144 7.1.1) and a private X- key on top. Each answer must
//! follow the RFC 7143 13 result function of its key (minimum, maximum, OR, AND, a list choice, or
//! declarative); RFC 7143 6.2.2 lets the target leave an OR key offered Yes, or an AND key offered No,
//! unanswered. An unknown key is answered NotUnderstood (RFC 7143 6.2.1). Both sessions log out at once
//! and run no commands.

use super::cases::record;
use super::login::{self, Negotiated};
use super::lun_access::Creds;
use super::path::Path;
use crate::compliance::Case;

const SECTION: &str = "login";

/// The Windows Server 2022 offer, in the order it was sent.
const WINDOWS: [(&str, &str); 14] = [
    ("HeaderDigest", "None,CRC32C"),
    ("DataDigest", "None,CRC32C"),
    ("ErrorRecoveryLevel", "2"),
    ("InitialR2T", "No"),
    ("ImmediateData", "Yes"),
    ("MaxRecvDataSegmentLength", "65536"),
    ("MaxBurstLength", "262144"),
    ("FirstBurstLength", "65536"),
    ("MaxConnections", "32"),
    ("DataPDUInOrder", "Yes"),
    ("DataSequenceInOrder", "Yes"),
    ("DefaultTime2Wait", "0"),
    ("DefaultTime2Retain", "60"),
    ("MaxOutstandingR2T", "16"),
];

/// The probe's additions to the Windows offer.
const PROBE: [(&str, &str); 2] = [("iSCSIProtocolLevel", "1"), ("X-com.stutools.probe", "1")];

/// How a key's result is computed (RFC 7143 13).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Rule {
    /// The result is at most the offer, and at least `floor`.
    Minimum { floor: u64 },
    /// The result is at least the offer, and at most `ceiling`.
    Maximum { ceiling: u64 },
    /// Boolean OR: an offer of Yes leaves Yes.
    Or,
    /// Boolean AND: Yes or No.
    And,
    /// Only one of the values offered.
    Choice,
    /// The responder's own value, from `floor` to `ceiling`.
    Declarative { floor: u64, ceiling: u64 },
    /// RFC 7143 6.2.1: an unknown key is answered NotUnderstood.
    NotUnderstood,
}

fn rule(key: &str) -> Rule {
    match key {
        "ErrorRecoveryLevel" => Rule::Minimum { floor: 0 },
        "MaxBurstLength" | "FirstBurstLength" => Rule::Minimum { floor: 512 },
        "MaxOutstandingR2T" | "MaxConnections" => Rule::Minimum { floor: 1 },
        "DefaultTime2Retain" => Rule::Minimum { floor: 0 },
        "iSCSIProtocolLevel" => Rule::Minimum { floor: 0 },
        "DefaultTime2Wait" => Rule::Maximum { ceiling: 3600 },
        "InitialR2T" | "DataPDUInOrder" | "DataSequenceInOrder" => Rule::Or,
        "ImmediateData" => Rule::And,
        "HeaderDigest" | "DataDigest" => Rule::Choice,
        "MaxRecvDataSegmentLength" => Rule::Declarative {
            floor: 512,
            ceiling: (1 << 24) - 1,
        },
        _ => Rule::NotUnderstood,
    }
}

/// Whether `answer` is a correct result for `offer` under `rule`. `irrelevant` allows "Irrelevant", which
/// RFC 7143 13.14 permits for FirstBurstLength when InitialR2T=Yes and ImmediateData=No.
fn obeys(rule: Rule, offer: &str, answer: &str, irrelevant: bool) -> bool {
    let num = |v: &str| v.parse::<u64>().ok();
    match rule {
        Rule::Minimum { floor } => {
            (irrelevant && answer == "Irrelevant")
                || matches!((num(offer), num(answer)), (Some(o), Some(a)) if a <= o && a >= floor)
        }
        Rule::Maximum { ceiling } => {
            matches!((num(offer), num(answer)), (Some(o), Some(a)) if a >= o && a <= ceiling)
        }
        Rule::Or => {
            if offer == "Yes" {
                answer == "Yes"
            } else {
                matches!(answer, "Yes" | "No")
            }
        }
        Rule::And => {
            if offer == "No" {
                answer == "No"
            } else {
                matches!(answer, "Yes" | "No")
            }
        }
        Rule::Choice => offer.split(',').any(|v| v == answer),
        Rule::Declarative { floor, ceiling } => {
            num(answer).is_some_and(|a| (floor..=ceiling).contains(&a))
        }
        Rule::NotUnderstood => answer == "NotUnderstood",
    }
}

/// Whether RFC 7143 6.2.2 lets the target leave the key unanswered: the offer already decides the result.
fn answer_optional(rule: Rule, offer: &str) -> bool {
    matches!((rule, offer), (Rule::Or, "Yes") | (Rule::And, "No"))
}

/// Logs in with `offer` and checks every answer. Returns the keys the target answered.
fn login(
    path: &Path,
    name: &str,
    what: &str,
    initiator: &str,
    creds: Creds,
    offer: &[(&str, &str)],
    cases: &mut Vec<Case>,
) -> Option<Vec<(String, String)>> {
    let n: Negotiated = match login::negotiate(&path.portal, &path.target, initiator, creds, offer)
    {
        Ok(n) => n,
        Err(err) => {
            record(
                cases,
                SECTION,
                format!("{name} login offering {what}"),
                "login ok",
                err,
                false,
            );
            return None;
        }
    };
    record(
        cases,
        SECTION,
        format!("{name} login offering {what}"),
        "login ok",
        "ok",
        true,
    );
    let answer = |key: &str| {
        n.keys
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    };
    let irrelevant = answer("InitialR2T") == Some("Yes") && answer("ImmediateData") == Some("No");
    for &(key, value) in offer {
        // MaxRecvDataSegmentLength is declarative: the target declares its own (checked below).
        if key == "MaxRecvDataSegmentLength" {
            continue;
        }
        let r = rule(key);
        let got = answer(key);
        let passed = match got {
            // RFC 7144 7.1.1: a target that predates RFC 7144 answers NotUnderstood, which is correct.
            Some("NotUnderstood") if key == "iSCSIProtocolLevel" => true,
            Some(a) => obeys(r, value, a, irrelevant),
            None => answer_optional(r, value),
        };
        record(
            cases,
            SECTION,
            format!("{name} {what}: {key}={value}"),
            format!("{r:?}"),
            got.unwrap_or("not answered").to_string(),
            passed,
        );
    }
    // RFC 7143 13.12: declarative, default 8192; a target need not declare it.
    let declared = answer("MaxRecvDataSegmentLength");
    record(
        cases,
        SECTION,
        format!("{name} {what}: MaxRecvDataSegmentLength"),
        "512 to 16777215, or not declared (8192)",
        declared.unwrap_or("not declared").to_string(),
        declared.is_none_or(|v| obeys(rule("MaxRecvDataSegmentLength"), "", v, false)),
    );
    Some(n.keys)
}

/// Runs both logins and returns the iSCSIProtocolLevel the target answered, if it did.
pub(super) fn run(
    path: &Path,
    name: &str,
    initiator: &str,
    creds: Creds,
    cases: &mut Vec<Case>,
) -> Option<u32> {
    login(
        path,
        name,
        "the Windows Server 2022 keys",
        initiator,
        creds,
        &WINDOWS,
        cases,
    );
    let mut probe = WINDOWS.to_vec();
    probe.extend(PROBE);
    let keys = login(
        path,
        name,
        "iSCSIProtocolLevel and an X- key",
        initiator,
        creds,
        &probe,
        cases,
    )?;
    keys.iter()
        .find(|(k, _)| k == "iSCSIProtocolLevel")
        .and_then(|(_, v)| v.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn results_follow_each_key_function() {
        assert!(obeys(rule("ErrorRecoveryLevel"), "2", "0", false));
        assert!(!obeys(rule("ErrorRecoveryLevel"), "2", "3", false));
        assert!(obeys(rule("MaxBurstLength"), "262144", "65536", false));
        assert!(!obeys(rule("MaxBurstLength"), "262144", "524288", false));
        assert!(obeys(rule("FirstBurstLength"), "65536", "Irrelevant", true));
        assert!(!obeys(
            rule("FirstBurstLength"),
            "65536",
            "Irrelevant",
            false
        ));
        assert!(obeys(rule("DefaultTime2Wait"), "0", "2", false));
        assert!(obeys(rule("InitialR2T"), "No", "Yes", false));
        assert!(!obeys(rule("DataPDUInOrder"), "Yes", "No", false));
        assert!(obeys(rule("ImmediateData"), "Yes", "No", false));
        assert!(!obeys(rule("HeaderDigest"), "None", "CRC32C", false));
        assert!(obeys(
            rule("X-com.stutools.probe"),
            "1",
            "NotUnderstood",
            false
        ));
        assert!(obeys(rule("MaxRecvDataSegmentLength"), "", "262144", false));
        assert!(obeys(rule("HeaderDigest"), "None,CRC32C", "CRC32C", false));
        assert!(answer_optional(rule("DataPDUInOrder"), "Yes"));
        assert!(answer_optional(rule("ImmediateData"), "No"));
        assert!(!answer_optional(rule("ImmediateData"), "Yes"));
    }
}
