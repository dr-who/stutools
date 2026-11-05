//! The score of a `--compliance` run: per section and in total, how many cases passed, warned
//! (a "should" missed), failed (a "shall" missed) or only informed, and the share that passed of
//! those that count. Informational cases are not scored.

use super::case::{Case, Verdict};
use super::sections::SECTIONS;

/// The counts for one section or for the whole run.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Tally {
    pub passed: usize,
    pub warned: usize,
    pub failed: usize,
    pub info: usize,
}

impl Tally {
    pub fn of<'a>(cases: impl Iterator<Item = &'a Case>) -> Tally {
        let mut t = Tally::default();
        for c in cases {
            match c.verdict() {
                Verdict::Pass => t.passed += 1,
                Verdict::Warn => t.warned += 1,
                Verdict::Fail => t.failed += 1,
                Verdict::Info => t.info += 1,
            }
        }
        t
    }

    /// The cases that count toward the score: all but the informational ones.
    pub fn scored(&self) -> usize {
        self.passed + self.warned + self.failed
    }

    /// The share of scored cases that passed, in percent; 100 when nothing was scored.
    pub fn percent(&self) -> f64 {
        match self.scored() {
            0 => 100.0,
            n => 100.0 * self.passed as f64 / n as f64,
        }
    }

    /// "pass", "warn" or "FAIL" for a section or the run.
    pub fn result(&self) -> &'static str {
        if self.failed > 0 {
            "FAIL"
        } else if self.warned > 0 {
            "warn"
        } else {
            "pass"
        }
    }
}

/// Each section that has cases, in run order, with its standard and tally.
pub fn by_section(cases: &[Case]) -> Vec<(&'static str, &'static str, Tally)> {
    SECTIONS
        .iter()
        .filter_map(|&(section, spec)| {
            let t = Tally::of(cases.iter().filter(|c| c.section == section));
            (t.scored() + t.info > 0).then_some((section, spec, t))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::case::{case, info, should};
    use super::*;

    #[test]
    fn warnings_and_information_do_not_fail_a_section() {
        let cases = vec![
            case("pr", "a".into(), "e".into(), "a".into(), true),
            should("pr", "b".into(), "e".into(), "a".into(), false),
            info("pr", "c".into(), "a".into()),
        ];
        let t = Tally::of(cases.iter());
        assert_eq!((t.passed, t.warned, t.failed, t.info), (1, 1, 0, 1));
        assert_eq!(t.scored(), 2);
        assert_eq!(t.percent(), 50.0);
        assert_eq!(t.result(), "warn");
        assert_eq!(by_section(&cases)[0].0, "pr");
    }

    #[test]
    fn nothing_scored_is_full_marks() {
        assert_eq!(Tally::default().percent(), 100.0);
    }
}
