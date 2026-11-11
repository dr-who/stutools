//! What the SCSI sections of `--compliance` share: logging in as a given initiator port, describing a
//! SCSI answer in one line, judging it against what the specification allows (`Want`), the block
//! reads and writes the data cases use, and draining unit attentions.

use super::ops::{CHECK_CONDITION, RESERVATION_CONFLICT};
use super::session::Outcome;
use crate::compliance::{case, Case};
use crate::Result;

/// The sense key, additional sense code and qualifier, from fixed (0x70, 0x71) or descriptor (0x72,
/// 0x73) sense data.
pub(super) fn sense(data: &[u8]) -> (u8, u8, u8) {
    let at = |i: usize| data.get(i).copied().unwrap_or(0);
    match at(0) & 0x7f {
        0x72 | 0x73 => (at(1) & 0x0f, at(2), at(3)),
        _ => (at(2) & 0x0f, at(12), at(13)),
    }
}

/// The INFORMATION field of fixed or descriptor sense data (SPC-6 4.4.2.2, 4.4.3), when it is valid.
pub(super) fn information(data: &[u8]) -> Option<u64> {
    match data.first().map(|b| b & 0x7f) {
        Some(0x72 | 0x73) => {
            let mut i = 8;
            while i + 1 < data.len() {
                let len = data[i + 1] as usize;
                if data[i] == 0x00 && len >= 0x0a {
                    return data
                        .get(i + 4..i + 12)
                        .map(|b| u64::from_be_bytes(b.try_into().unwrap()));
                }
                i += 2 + len;
            }
            None
        }
        Some(0x70 | 0x71) if data[0] & 0x80 != 0 => data
            .get(3..7)
            .map(|b| u64::from(u32::from_be_bytes(b.try_into().unwrap()))),
        _ => None,
    }
}

/// One line that says what a SCSI answer was, for the expected and actual columns.
pub(super) fn describe(outcome: &Outcome) -> String {
    match outcome.status {
        0 => "GOOD".into(),
        RESERVATION_CONFLICT => "RESERVATION CONFLICT".into(),
        CHECK_CONDITION => {
            let (key, asc, ascq) = sense(&outcome.sense);
            format!("CHECK CONDITION {key:x}/{asc:02x}/{ascq:02x}")
        }
        other => format!("status {other:#04x}"),
    }
}

/// An answer the specification allows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Want {
    Good,
    Conflict,
    /// CHECK CONDITION with this sense key.
    Key(u8),
    /// CHECK CONDITION with this sense key and additional sense code.
    Asc(u8, u8),
    /// CHECK CONDITION with this key, code and qualifier.
    Ascq(u8, u8, u8),
}

impl Want {
    pub(super) fn matches(self, o: &Outcome) -> bool {
        let (key, asc, ascq) = sense(&o.sense);
        let check = o.status == CHECK_CONDITION;
        match self {
            Want::Good => o.status == 0,
            Want::Conflict => o.status == RESERVATION_CONFLICT,
            Want::Key(k) => check && key == k,
            Want::Asc(k, a) => check && (key, asc) == (k, a),
            Want::Ascq(k, a, q) => check && (key, asc, ascq) == (k, a, q),
        }
    }

    fn text(self) -> String {
        match self {
            Want::Good => "GOOD".into(),
            Want::Conflict => "RESERVATION CONFLICT".into(),
            Want::Key(k) => format!("CHECK CONDITION {k:x}"),
            Want::Asc(k, a) => format!("CHECK CONDITION {k:x}/{a:02x}"),
            Want::Ascq(k, a, q) => format!("CHECK CONDITION {k:x}/{a:02x}/{q:02x}"),
        }
    }
}

/// The answer to expect when a nexus is allowed (GOOD) or not (RESERVATION CONFLICT).
/// Whether the transport reports the whole allocation as received, though the device's own length field says it
/// returned less. Such a length cannot be checked on that transport (a local HBA that reports no residual), so
/// the case is not run rather than failed.
pub(super) fn residual_not_reported(received: usize, device_length: usize) -> bool {
    NO_RESIDUAL.load(std::sync::atomic::Ordering::Relaxed) && received != device_length
}

/// Set while a local disk is tested: its transport (SG_IO on a local HBA) may report no residual. iSCSI
/// reports the bytes each Data-In carried, so its counts are checked as they are.
pub(super) static NO_RESIDUAL: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub(super) fn allowed(yes: bool) -> Want {
    if yes {
        Want::Good
    } else {
        Want::Conflict
    }
}

/// Records one case that passes when the answer is one of `wants`. Returns whether it passed and the
/// answer, when there was one.
pub(super) fn expect(
    cases: &mut Vec<Case>,
    section: &'static str,
    name: String,
    wants: &[Want],
    result: Result<Outcome>,
) -> (bool, Option<Outcome>) {
    let expected: Vec<String> = wants.iter().map(|w| w.text()).collect();
    let expected = expected.join(" or ");
    match result {
        Ok(o) => {
            let passed = wants.iter().any(|w| w.matches(&o));
            cases.push(case(section, name, expected, describe(&o), passed));
            (passed, Some(o))
        }
        Err(err) => {
            cases.push(case(section, name, expected, err, false));
            (false, None)
        }
    }
}

/// Records one case whose verdict the caller has already decided. Returns the verdict.
pub(super) fn record(
    cases: &mut Vec<Case>,
    section: &'static str,
    name: String,
    expected: impl Into<String>,
    actual: impl Into<String>,
    passed: bool,
) -> bool {
    cases.push(case(section, name, expected.into(), actual.into(), passed));
    passed
}

/// Records a case for a clause that says "should": a miss is a warning, not a failure.
pub(super) fn record_should(
    cases: &mut Vec<Case>,
    section: &'static str,
    name: String,
    expected: impl Into<String>,
    actual: impl Into<String>,
    passed: bool,
) -> bool {
    cases.push(crate::compliance::should(
        section,
        name,
        expected.into(),
        actual.into(),
        passed,
    ));
    passed
}

/// Records what the device offers; it never fails.
pub(super) fn record_info(
    cases: &mut Vec<Case>,
    section: &'static str,
    name: String,
    actual: impl Into<String>,
) {
    cases.push(crate::compliance::info(section, name, actual.into()));
}

/// Marks the case just recorded as a test of what `edition` introduced: before it, the case only
/// informs.
pub(super) fn introduced_in(cases: &mut Vec<Case>, edition: crate::compliance::Edition) {
    if let Some(c) = cases.pop() {
        cases.push(c.since(edition));
    }
}

/// Whether none of `cases` failed: passes, warnings (a "should" missed) and information all count as
/// clean.
pub(super) fn no_failures(cases: &[Case]) -> bool {
    cases
        .iter()
        .all(|c| c.verdict() != crate::compliance::Verdict::Fail)
}

/// Like `expect`, for a clause that says "should".
pub(super) fn expect_should(
    cases: &mut Vec<Case>,
    section: &'static str,
    name: String,
    wants: &[Want],
    result: Result<Outcome>,
) -> (bool, Option<Outcome>) {
    let expected: Vec<String> = wants.iter().map(|w| w.text()).collect();
    let (actual, passed, outcome) = match result {
        Ok(o) => (describe(&o), wants.iter().any(|w| w.matches(&o)), Some(o)),
        Err(e) => (e, false, None),
    };
    record_should(cases, section, name, expected.join(" or "), actual, passed);
    (passed, outcome)
}

/// Whether a command was refused as not implemented: ILLEGAL REQUEST with INVALID COMMAND OPERATION
/// CODE (20h/00h, SPC-6 4.2.5.1), or, for a command with a service action, INVALID FIELD IN CDB
/// (24h/00h), which a device server returns for a service action it does not implement (SPC-6 4.2.5.2).
pub(super) fn refused(o: &Outcome, service_action: bool) -> bool {
    o.status == CHECK_CONDITION
        && match sense(&o.sense) {
            (0x5, 0x20, 0x00) => true,
            (0x5, 0x24, 0x00) => service_action,
            _ => false,
        }
}

/// The clean refusal of an unimplemented command without a service action (see `refused`).
pub(super) const REFUSED: [Want; 1] = [Want::Ascq(0x5, 0x20, 0x00)];

/// The clean refusals of an unimplemented command with a service action (see `refused`).
pub(super) const REFUSED_SA: [Want; 2] = [Want::Ascq(0x5, 0x20, 0x00), Want::Ascq(0x5, 0x24, 0x00)];

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(status: u8, sense: &[u8]) -> Outcome {
        Outcome {
            status,
            sense: sense.to_vec(),
            data: Vec::new(),
            received: 0,
        }
    }

    #[test]
    fn sense_reads_fixed_and_descriptor_formats() {
        let mut fixed = [0u8; 18];
        fixed[0] = 0x70;
        fixed[2] = 0x05;
        fixed[12] = 0x24;
        fixed[13] = 0x01;
        assert_eq!(sense(&fixed), (5, 0x24, 1));
        assert_eq!(sense(&[0x72, 0x06, 0x2a, 0x05]), (6, 0x2a, 5));
        assert_eq!(sense(&[]), (0, 0, 0));
    }

    #[test]
    fn wants_match_status_key_code_and_qualifier() {
        let check = outcome(CHECK_CONDITION, &[0x72, 0x05, 0x26, 0x04]);
        assert!(Want::Key(5).matches(&check));
        assert!(Want::Asc(5, 0x26).matches(&check));
        assert!(Want::Ascq(5, 0x26, 0x04).matches(&check));
        assert!(!Want::Ascq(5, 0x26, 0x00).matches(&check));
        assert!(!Want::Good.matches(&check));
        assert!(Want::Conflict.matches(&outcome(RESERVATION_CONFLICT, &[])));
        let mut fixed = [0u8; 18];
        fixed[0] = 0xf0;
        fixed[3..7].copy_from_slice(&100u32.to_be_bytes());
        assert_eq!(information(&fixed), Some(100));
        fixed[0] = 0x70;
        assert_eq!(information(&fixed), None);
        let mut desc = vec![0x72, 0x0e, 0x1d, 0x00, 0, 0, 0, 12, 0x00, 0x0a, 0x80, 0];
        desc.extend_from_slice(&4196u64.to_be_bytes());
        assert_eq!(information(&desc), Some(4196));
        assert_eq!(describe(&check), "CHECK CONDITION 5/26/04");
    }

    #[test]
    fn expect_records_every_allowed_answer() {
        let mut cases = Vec::new();
        let (passed, _) = expect(
            &mut cases,
            "spc",
            "x".into(),
            &[Want::Good, Want::Asc(5, 0x20)],
            Ok(outcome(0, &[])),
        );
        assert!(passed);
        assert_eq!(cases[0].expected, "GOOD or CHECK CONDITION 5/20");
    }

    #[test]
    fn refusals_are_invalid_opcode_or_service_action() {
        assert!(refused(
            &outcome(
                CHECK_CONDITION,
                &[0x70, 0, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x20, 0]
            ),
            false
        ));
        assert!(refused(
            &outcome(CHECK_CONDITION, &[0x72, 5, 0x24, 0]),
            true
        ));
        assert!(
            !refused(&outcome(CHECK_CONDITION, &[0x72, 5, 0x24, 0]), false),
            "a field, not the command"
        );
        assert!(!refused(
            &outcome(CHECK_CONDITION, &[0x72, 5, 0x24, 1]),
            true
        ));
        assert!(!refused(&outcome(0, &[]), false));
    }
}
