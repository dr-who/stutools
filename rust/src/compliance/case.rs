//! One scored case of `--compliance` and how much it matters. The standards say "shall" for what a
//! device must do and "should" for what it is recommended to do, and some cases only report what a
//! device offers. So a case has a level: a `Shall` case that does not pass is a failure, a `Should`
//! case a warning, and an `Info` case never fails. A case whose verdict depends on the SCSI level is
//! recorded once per level, tagged with it (`at`); every other case holds at every level. A case for
//! a field or command a later edition introduced names that edition (`since`): at a level before it,
//! the case only informs.

use super::scsi_level::{Edition, ScsiLevel};

/// How much a case matters, from the wording of the clause it tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// The clause says "shall", or a host the case names depends on it.
    Shall,
    /// The clause says "should".
    Should,
    /// What the device offers; reported, never a failure.
    Info,
}

/// The verdict a case gets once its result is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Warn,
    Fail,
    Info,
}

impl Verdict {
    /// The word printed in front of the case.
    pub fn word(self) -> &'static str {
        match self {
            Verdict::Pass => "PASS",
            Verdict::Warn => "WARN",
            Verdict::Fail => "FAIL",
            Verdict::Info => "INFO",
        }
    }
}

/// One scored case.
#[derive(Debug, Clone)]
pub struct Case {
    pub section: &'static str,
    pub name: String,
    pub expected: String,
    pub actual: String,
    pub passed: bool,
    pub level: Level,
    /// The one SCSI level this verdict is for; None when it holds at every level.
    pub scsi_level: Option<ScsiLevel>,
    /// The edition that introduced what the case tests; None when every level has it.
    pub since: Option<Edition>,
}

impl Case {
    /// This case as the verdict for one SCSI level only.
    pub fn at(self, level: ScsiLevel) -> Case {
        Case {
            scsi_level: Some(level),
            ..self
        }
    }

    /// This case as a test of what `edition` introduced.
    pub fn since(self, edition: Edition) -> Case {
        Case {
            since: Some(edition),
            ..self
        }
    }

    pub fn verdict(&self) -> Verdict {
        match (self.level, self.passed) {
            (Level::Info, _) => Verdict::Info,
            (_, true) => Verdict::Pass,
            (Level::Should, false) => Verdict::Warn,
            (Level::Shall, false) => Verdict::Fail,
        }
    }
}

/// A "shall" case: one that does not pass fails the run.
pub fn case(
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
        level: Level::Shall,
        scsi_level: None,
        since: None,
    }
}

/// A "should" case: one that does not pass is a warning.
pub fn should(
    section: &'static str,
    name: String,
    expected: String,
    actual: String,
    passed: bool,
) -> Case {
    Case {
        level: Level::Should,
        ..case(section, name, expected, actual, passed)
    }
}

/// An informational case: what the device offers.
pub fn info(section: &'static str, name: String, actual: String) -> Case {
    Case {
        level: Level::Info,
        ..case(section, name, String::new(), actual, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_level_decides_what_a_miss_is() {
        let s = |passed| case("x", "n".into(), "e".into(), "a".into(), passed);
        assert_eq!(s(true).verdict(), Verdict::Pass);
        assert_eq!(s(false).verdict(), Verdict::Fail);
        assert_eq!(
            should("x", "n".into(), "e".into(), "a".into(), false).verdict(),
            Verdict::Warn
        );
        assert_eq!(info("x", "n".into(), "a".into()).verdict(), Verdict::Info);
    }
}
