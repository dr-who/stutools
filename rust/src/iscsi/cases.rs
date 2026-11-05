//! What the SCSI sections of `--compliance` share: logging in as a given initiator port, describing a
//! SCSI answer in one line, judging it against what the specification allows (`Want`), the block
//! reads and writes the data cases use, and draining unit attentions.

use super::ops::{CHECK_CONDITION, RESERVATION_CONFLICT};
use super::report::Path;
use super::scsi;
use super::session::{self, Outcome, Session};
use crate::compliance::{case, Case};
use crate::Result;

/// Credentials for one target: the CHAP user and secret, when the target requires CHAP.
pub(super) type Creds<'a> = Option<(&'a str, &'a [u8])>;

const DIGEST: &str = "CRC32C,None";
const CHAP_OFFER: &str = "5,6,7,8";

/// Logs in to the target as the initiator port `isid` (one I_T nexus per ISID), offering CRC32C digests.
pub(super) fn open(path: &Path, initiator: &str, creds: Creds, isid: [u8; 6]) -> Result<Session> {
    open_with(path, initiator, creds, isid, DIGEST)
}

/// `open`, offering `digest` for HeaderDigest and DataDigest.
pub(super) fn open_with(
    path: &Path,
    initiator: &str,
    creds: Creds,
    isid: [u8; 6],
    digest: &str,
) -> Result<Session> {
    session::login_as(
        &path.portal,
        &path.target,
        initiator,
        isid,
        creds,
        digest,
        CHAP_OFFER,
    )
}

/// The short name of a target: the part after the last colon of its IQN.
pub(super) fn short_name(path: &Path) -> String {
    path.target
        .rsplit(':')
        .next()
        .unwrap_or(&path.target)
        .to_string()
}

/// The sense key, additional sense code and qualifier, from fixed (0x70, 0x71) or descriptor (0x72,
/// 0x73) sense data.
pub(super) fn sense(data: &[u8]) -> (u8, u8, u8) {
    let at = |i: usize| data.get(i).copied().unwrap_or(0);
    match at(0) & 0x7f {
        0x72 | 0x73 => (at(1) & 0x0f, at(2), at(3)),
        _ => (at(2) & 0x0f, at(12), at(13)),
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

/// Text safe to print: non-printable bytes as \xNN (a NUL in a serial number would otherwise end up in
/// the report).
pub(super) fn printable(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| {
            if (0x20..0x7f).contains(&b) {
                (b as char).to_string()
            } else {
                format!("\\x{b:02x}")
            }
        })
        .collect()
}

/// INQUIRY of one VPD page with `len` bytes of allocation.
pub(super) fn vpd(s: &mut Session, page: u8, len: u16) -> Result<Outcome> {
    let l = len.to_be_bytes();
    s.command(&[0x12, 0x01, page, l[0], l[1], 0], len as u32, None)
}

/// The last LBA of the LUN and its block length. The data cases work on the last two blocks.
pub(super) fn last_block(s: &mut Session) -> Result<(u32, usize)> {
    let (last, block_len) = scsi::capacity(s)?;
    let lba = u32::try_from(last).map_err(|_| "LUN is larger than 32-bit LBAs")?;
    Ok((lba, block_len as usize))
}

/// READ(10) of one block, as an outcome.
pub(super) fn read_cmd(s: &mut Session, lba: u32, len: usize) -> Result<Outcome> {
    s.command(&scsi::block_cdb(0x28, lba, 1), len as u32, None)
}

/// WRITE(10) of one block, as an outcome.
pub(super) fn write_cmd(s: &mut Session, lba: u32, data: &[u8]) -> Result<Outcome> {
    s.command(&scsi::block_cdb(0x2a, lba, 1), 0, Some(data))
}

/// READ(10) of one block, which must succeed.
pub(super) fn read_block(s: &mut Session, lba: u32, len: usize) -> Result<Vec<u8>> {
    let o = read_cmd(s, lba, len)?;
    scsi::check_status("READ(10)", o.status, &o.sense)?;
    Ok(o.data)
}

/// WRITE(10) of one block, which must succeed.
pub(super) fn write_block(s: &mut Session, lba: u32, data: &[u8]) -> Result<()> {
    let o = write_cmd(s, lba, data)?;
    scsi::check_status("WRITE(10)", o.status, &o.sense)
}

/// TEST UNIT READY, as an outcome.
pub(super) fn test_unit_ready(s: &mut Session) -> Result<Outcome> {
    s.command(&[0x00, 0, 0, 0, 0, 0], 0, None)
}

/// Drains the unit attentions pending for this nexus with TEST UNIT READY (SPC-4 allows it under every
/// reservation type) and returns each one's additional sense code and qualifier, in order.
pub(super) fn unit_attentions(s: &mut Session) -> Vec<(u8, u8)> {
    let mut seen = Vec::new();
    for _ in 0..8 {
        match test_unit_ready(s) {
            Ok(o) if o.status == CHECK_CONDITION && sense(&o.sense).0 == 0x6 => {
                let (_, asc, ascq) = sense(&o.sense);
                seen.push((asc, ascq));
            }
            _ => break,
        }
    }
    seen
}

/// The unit attentions as text for the actual column.
pub(super) fn show_attentions(seen: &[(u8, u8)]) -> String {
    if seen.is_empty() {
        return "none".into();
    }
    let all: Vec<String> = seen
        .iter()
        .map(|(a, q)| format!("{a:02x}/{q:02x}"))
        .collect();
    all.join(", ")
}

/// What the checks of one command found, so the `sbc` and `features` sections can use it without
/// sending the command again.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Probe {
    /// The section whose cases sent it.
    pub section: &'static str,
    /// The device server took it (it was not refused as an unsupported operation code or service
    /// action).
    pub supported: bool,
    /// Every case that sent it passed.
    pub passed: bool,
    pub actual: String,
}

/// The commands sent so far, by their SBC-5 table 34 or SPC-5 name, and named facts (a bit or field
/// a feature set requires) that the cases established.
#[derive(Debug, Default)]
pub(super) struct Seen {
    probes: Vec<(&'static str, Probe)>,
    facts: Vec<(&'static str, bool)>,
}

impl Seen {
    /// Records one case's verdict for `command`. A later case for the same command can only make the
    /// verdict worse; `supported` keeps the first answer that was not a refusal.
    pub(super) fn note(
        &mut self,
        command: &'static str,
        section: &'static str,
        supported: bool,
        passed: bool,
        actual: impl Into<String>,
    ) {
        match self.probes.iter_mut().find(|(c, _)| *c == command) {
            Some((_, p)) => {
                p.supported |= supported;
                p.passed &= passed;
            }
            None => self.probes.push((
                command,
                Probe {
                    section,
                    supported,
                    passed,
                    actual: actual.into(),
                },
            )),
        }
    }

    /// Records the answer to `command` as `expect` judged it.
    pub(super) fn answer(
        &mut self,
        command: &'static str,
        section: &'static str,
        (passed, outcome): &(bool, Option<Outcome>),
    ) {
        let (supported, actual) = match outcome {
            Some(o) => {
                let sa =
                    super::command_set::row(command).is_some_and(|r| r.service_action.is_some());
                (!refused(o, sa), describe(o))
            }
            None => (false, "no answer".to_string()),
        };
        self.note(command, section, supported, *passed, actual);
    }

    pub(super) fn probe(&self, command: &str) -> Option<&Probe> {
        self.probes
            .iter()
            .find(|(c, _)| *c == command)
            .map(|(_, p)| p)
    }

    /// Records a named fact. A fact recorded twice keeps the worse value.
    pub(super) fn fact(&mut self, name: &'static str, value: bool) {
        match self.facts.iter_mut().find(|(n, _)| *n == name) {
            Some((_, v)) => *v &= value,
            None => self.facts.push((name, value)),
        }
    }

    pub(super) fn has(&self, name: &str) -> Option<bool> {
        self.facts.iter().find(|(n, _)| *n == name).map(|(_, v)| *v)
    }
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

pub(super) fn be16(d: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([
        d.get(at).copied().unwrap_or(0),
        d.get(at + 1).copied().unwrap_or(0),
    ])
}

pub(super) fn be32(d: &[u8], at: usize) -> u32 {
    scsi::be32(d, at).unwrap_or(0)
}

pub(super) fn be64(d: &[u8], at: usize) -> u64 {
    d.get(at..at + 8)
        .and_then(|b| b.try_into().ok())
        .map(u64::from_be_bytes)
        .unwrap_or(0)
}

/// A block of `len` bytes filled with `seed` mixed with each byte's offset, so two patterns never
/// match by accident.
pub(super) fn pattern(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|i| seed ^ (i as u8).wrapping_mul(31))
        .collect()
}

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
    fn seen_keeps_the_worst_verdict_and_the_first_support() {
        let mut seen = Seen::default();
        seen.note("READ (16)", "io", true, true, "GOOD");
        seen.note("READ (16)", "io", false, false, "CHECK CONDITION");
        let p = seen.probe("READ (16)").unwrap();
        assert!(p.supported && !p.passed);
        assert_eq!(p.actual, "GOOD");
        seen.fact("FUA", true);
        seen.fact("FUA", false);
        assert_eq!(seen.has("FUA"), Some(false));
        assert_eq!(seen.has("other"), None);
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

    #[test]
    fn patterns_differ_by_seed() {
        assert_ne!(pattern(512, 1), pattern(512, 2));
        assert_eq!(pattern(512, 7).len(), 512);
    }
}
