//! The score of a `--compliance` run: per section and in total, how many cases passed, warned
//! (a "should" missed), failed (a "shall" missed) or only informed, and the share that passed of
//! those that count. Informational cases are not scored.

use super::case::{Case, Verdict};
use super::sections::{Section, SECTIONS};

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

    /// Passed cases over all cases in the section, in percent: pass / total * 100. A section with no
    /// passes scores 0, and so does an empty one.
    pub fn percent(&self) -> f64 {
        match self.total() {
            0 => 0.0,
            n => self.passed as f64 / n as f64 * 100.0,
        }
    }

    /// Every case counted: passed, warned, failed and informational.
    pub fn total(&self) -> usize {
        self.passed + self.warned + self.failed + self.info
    }
}

/// Each section that has cases, in run order, with its tally.
pub fn by_section(cases: &[Case]) -> Vec<(&'static Section, Tally)> {
    SECTIONS
        .iter()
        .filter_map(|section| {
            let t = Tally::of(cases.iter().filter(|c| c.section == section.name));
            (t.scored() + t.info > 0).then_some((section, t))
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
        assert!((t.percent() - 100.0 / 3.0).abs() < 1e-9);
        assert_eq!(by_section(&cases)[0].0.name, "pr");
    }

    #[test]
    fn nothing_passed_is_zero_percent() {
        assert_eq!(Tally::default().percent(), 0.0);
        let only_info = Tally {
            info: 6,
            ..Tally::default()
        };
        assert_eq!(only_info.percent(), 0.0);
        let half = Tally {
            passed: 1,
            failed: 1,
            ..Tally::default()
        };
        assert_eq!(half.percent(), 50.0);
    }
}
