//! What the SCSI sections have found so far, so a later section uses it without sending a command
//! again: for each command (by its SBC-5 table 34 or SPC-6 name) whether the device server took it and
//! whether every case that sent it passed (`Probe`), and named facts a feature set requires. A later
//! finding can only make a verdict worse.

use super::cases::{describe, refused};
use super::session::Outcome;

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
    pub(super) probes: Vec<(&'static str, Probe)>,
    pub(super) facts: Vec<(&'static str, bool)>,
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
