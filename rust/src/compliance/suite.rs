//! An external conformance suite (Microsoft HLK, Failover Cluster validation, UNH-IOL, VMware) as a
//! projection of spit's cases: each of the suite's tests by its own ID and title, the spit cases that
//! decide it, and how far spit runs it. Nothing is sent again. A test's result is the worst of its
//! cases (any failure fails it, a warning warns, passes pass); one whose cases did not run on these
//! devices, or that spit does not run yet, is Not Run, and printed so; one no target tester can run
//! (PCIe, link-layer framing, host-side performance) is not applicable. Each suite reports in its own
//! words: HLK Passed / Failed / Not Run / Skipped, the cluster wizard Success / Warning / Failed,
//! UNH-IOL's result key, VMware Pass / Fail.

use super::case::{Case, Verdict};

/// How a suite words its results.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Style {
    Hlk,
    Cluster,
    Unh,
    Vmware,
}

/// How far spit runs a test.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Coverage {
    /// The same stimulus, judged on the same criterion.
    Covered,
    /// Sent, but judged on less, or a nearby stimulus: the note says what is left.
    Partial,
    /// Not yet sent or judged.
    Missing,
    /// No target tester can run it; the note says why.
    NotApplicable,
}

/// The spit cases of one section whose name matches `name`: words in order, `*` for any text;
/// an empty name is every case of the section.
#[derive(Debug, Clone, Copy)]
pub struct Ref {
    section: &'static str,
    name: &'static str,
}

impl Ref {
    pub const fn new(section: &'static str, name: &'static str) -> Ref {
        Ref { section, name }
    }

    fn matches(&self, c: &Case) -> bool {
        if c.section != self.section {
            return false;
        }
        let mut rest = c.name.as_str();
        for part in self
            .name
            .split('*')
            .map(str::trim)
            .filter(|p| !p.is_empty())
        {
            match rest.find(part) {
                Some(at) => rest = &rest[at + part.len()..],
                None => return false,
            }
        }
        true
    }
}

/// One test of a suite.
#[derive(Debug, Clone, Copy)]
pub struct Test {
    pub id: &'static str,
    pub title: &'static str,
    pub coverage: Coverage,
    cases: &'static [Ref],
    /// What spit leaves out of it, or why it does not apply.
    pub note: &'static str,
}

impl Test {
    pub const fn new(
        id: &'static str,
        title: &'static str,
        coverage: Coverage,
        cases: &'static [Ref],
        note: &'static str,
    ) -> Test {
        Test {
            id,
            title,
            coverage,
            cases,
            note,
        }
    }
}

/// One suite.
pub struct Suite {
    pub title: &'static str,
    pub publisher: &'static str,
    pub source: &'static str,
    pub style: Style,
    /// The device transports it is for.
    pub transports: &'static [&'static str],
    pub tests: &'static [Test],
}

/// A test's result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Outcome {
    Passed,
    /// Its cases only report what the device offers.
    Informative,
    Warned,
    Failed,
    NotRun,
    NotApplicable,
}

/// A test as judged: its result, the value behind it and how many spit cases decided it.
#[derive(Debug, Clone, PartialEq)]
pub struct Judged {
    pub outcome: Outcome,
    /// What the device answered: the first failure or warning, else the first case.
    pub got: String,
    pub cases: usize,
}

impl Style {
    /// The suite's own word for an outcome; a partly run test says so.
    pub fn word(self, outcome: Outcome, partial: bool) -> String {
        let word = match (self, outcome) {
            (Style::Hlk, Outcome::Passed | Outcome::Informative) => "Passed",
            (Style::Hlk, Outcome::Warned) => "Passed with comment",
            (Style::Hlk, Outcome::Failed) => "Failed",
            (Style::Hlk, Outcome::NotRun) => "Not Run",
            (Style::Hlk, Outcome::NotApplicable) => "Skipped",
            (Style::Cluster, Outcome::Passed | Outcome::Informative) => "Success",
            (Style::Cluster, Outcome::Warned) => "Warning",
            (Style::Cluster, Outcome::Failed) => "Failed",
            (Style::Cluster, Outcome::NotRun) => "Not Run",
            (Style::Cluster, Outcome::NotApplicable) => "Not Applicable",
            (Style::Unh, Outcome::Passed) => "PASS",
            (Style::Unh, Outcome::Informative) => "Informative",
            (Style::Unh, Outcome::Warned) => "Warning",
            (Style::Unh, Outcome::Failed) => "FAIL",
            (Style::Unh, Outcome::NotRun) => "Not Tested",
            (Style::Unh, Outcome::NotApplicable) => "Not Applicable",
            (Style::Vmware, Outcome::Passed | Outcome::Informative) => "Pass",
            (Style::Vmware, Outcome::Warned) => "Pass, with a note",
            (Style::Vmware, Outcome::Failed) => "Fail",
            (Style::Vmware, Outcome::NotRun) => "Not Run",
            (Style::Vmware, Outcome::NotApplicable) => "Not Applicable",
        };
        let ran = matches!(
            outcome,
            Outcome::Passed | Outcome::Informative | Outcome::Warned | Outcome::Failed
        );
        if partial && ran {
            format!("{word} (partly run)")
        } else {
            word.to_string()
        }
    }

    /// The column headings: test, result, value, comment.
    pub fn columns(self) -> [&'static str; 5] {
        match self {
            Style::Hlk => ["#", "Title", "Result", "Got", "Comment"],
            Style::Cluster => ["Test", "Name", "Result", "Observed", "Description"],
            Style::Unh => ["Test #", "Label", "Result", "Observed", "Comments"],
            Style::Vmware => ["Check", "Requirement", "Result", "Got", "Note"],
        }
    }
}

impl Suite {
    /// Whether the suite is for any of `transports`.
    pub fn applies(&self, transports: &[&str]) -> bool {
        self.transports.iter().any(|t| transports.contains(t))
    }
}

/// `test` judged on `cases`.
pub fn judge(test: &Test, cases: &[Case]) -> Judged {
    let not_run = |why: &str| Judged {
        outcome: Outcome::NotRun,
        got: why.to_string(),
        cases: 0,
    };
    match test.coverage {
        Coverage::Missing => return not_run(""),
        Coverage::NotApplicable => {
            return Judged {
                outcome: Outcome::NotApplicable,
                got: String::new(),
                cases: 0,
            }
        }
        Coverage::Covered | Coverage::Partial => {}
    }
    let matched: Vec<&Case> = cases
        .iter()
        .filter(|c| test.cases.iter().any(|r| r.matches(c)))
        .collect();
    let Some(first) = matched.first() else {
        return not_run(if test.cases.is_empty() {
            ""
        } else {
            "no spit case ran on these devices"
        });
    };
    let worst = |v: Verdict| matched.iter().find(|c| c.verdict() == v);
    let (outcome, shown) = if let Some(c) = worst(Verdict::Fail) {
        (Outcome::Failed, *c)
    } else if let Some(c) = worst(Verdict::Warn) {
        (Outcome::Warned, *c)
    } else if let Some(c) = worst(Verdict::Pass) {
        (Outcome::Passed, *c)
    } else {
        (Outcome::Informative, *first)
    };
    let got = if matches!(outcome, Outcome::Failed | Outcome::Warned) {
        format!("{}, expected {}", shown.actual, shown.expected)
    } else {
        shown.actual.clone()
    };
    Judged {
        outcome,
        got,
        cases: matched.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::case::{case, info};
    use super::*;

    const CASES: &[Ref] = &[
        Ref::new("disk", "standard INQUIRY VERSION"),
        Ref::new("pr", "type * : A RELEASE"),
    ];

    fn test(coverage: Coverage) -> Test {
        Test::new("SC2-20", "INQUIRY VERSION Field Test", coverage, CASES, "")
    }

    #[test]
    fn a_test_is_the_worst_of_its_cases_and_shows_the_answer() {
        let cases = vec![
            case(
                "disk",
                "lun-1 standard INQUIRY VERSION".into(),
                "05h".into(),
                "06h".into(),
                true,
            ),
            case(
                "pr",
                "lun-1 type 5: A RELEASE".into(),
                "GOOD".into(),
                "RESERVATION CONFLICT".into(),
                false,
            ),
            case(
                "pr",
                "lun-1 READ KEYS".into(),
                "x".into(),
                "y".into(),
                false,
            ),
        ];
        let j = judge(&test(Coverage::Covered), &cases);
        assert_eq!(j.outcome, Outcome::Failed);
        assert_eq!(j.cases, 2);
        assert_eq!(j.got, "RESERVATION CONFLICT, expected GOOD");
        assert_eq!(judge(&test(Coverage::Covered), &cases[..1]).got, "06h");
    }

    #[test]
    fn what_did_not_run_is_said() {
        let j = judge(&test(Coverage::Covered), &[]);
        assert_eq!(j.outcome, Outcome::NotRun);
        assert_eq!(
            judge(&test(Coverage::Missing), &[]).outcome,
            Outcome::NotRun
        );
        assert_eq!(
            judge(&test(Coverage::NotApplicable), &[]).outcome,
            Outcome::NotApplicable
        );
        let only_info = vec![info(
            "disk",
            "standard INQUIRY VERSION".into(),
            "06h".into(),
        )];
        assert_eq!(
            judge(&test(Coverage::Covered), &only_info).outcome,
            Outcome::Informative
        );
    }

    #[test]
    fn each_suite_words_results_its_own_way() {
        assert_eq!(Style::Hlk.word(Outcome::NotRun, false), "Not Run");
        assert_eq!(Style::Unh.word(Outcome::Failed, false), "FAIL");
        assert_eq!(Style::Cluster.word(Outcome::Warned, false), "Warning");
        assert_eq!(
            Style::Hlk.word(Outcome::Passed, true),
            "Passed (partly run)"
        );
        assert_eq!(Style::Unh.word(Outcome::NotRun, true), "Not Tested");
    }

    #[test]
    fn every_suite_test_names_known_sections_and_partial_tests_say_what_is_left() {
        for s in super::super::suites::SUITES {
            for t in s.tests {
                for r in t.cases {
                    assert!(
                        super::super::sections::SECTIONS
                            .iter()
                            .any(|x| x.name == r.section),
                        "{} {}: unknown section {}",
                        s.title,
                        t.id,
                        r.section
                    );
                }
                if t.coverage == Coverage::Covered {
                    assert!(!t.cases.is_empty(), "{} {} names no cases", s.title, t.id);
                }
            }
        }
    }
}
