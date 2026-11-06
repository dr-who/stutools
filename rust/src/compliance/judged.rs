//! The cases as judged at each SCSI level: those that hold at every level and those recorded for that
//! one, with every citation moved to the level's editions (`citation`). A case that holds at every
//! level, is "shall" or "should", and whose name and expectation cite clauses none of which the
//! level's editions have, asks for something those editions do not define, so at that level it only
//! informs, and says why. A case recorded for one level was already judged for it by its section
//! (a command a host still sends stays strict there) and keeps its weight. A case for what a later
//! edition introduced (`Case::since`) only informs at a level before that edition. `rows` lines the
//! levels up: each case once, with its verdict at every level.

use super::case::{Case, Level};
use super::citation::cite;
use super::scsi_level::ScsiLevel;

/// One case across the levels judged: the case as judged at each, in the order of `levels`.
pub struct Row {
    pub at: Vec<Case>,
}

impl Row {
    /// The case as the report shows it: at the lowest level.
    pub fn first(&self) -> &Case {
        &self.at[0]
    }
}

/// Every case once, in run order, judged at each of `levels` (lowest first, never empty). A case
/// recorded per level is one row with each level's record: its section records one per level, one
/// after the other.
pub fn rows(cases: &[Case], levels: &[ScsiLevel]) -> Vec<Row> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < cases.len() {
        let c = &cases[i];
        if c.scsi_level.is_none() {
            out.push(Row {
                at: levels.iter().map(|&l| judge(c, l)).collect(),
            });
            i += 1;
            continue;
        }
        let run = cases[i..]
            .iter()
            .zip(levels)
            .take_while(|(c, &l)| c.scsi_level == Some(l))
            .count();
        if run == levels.len() {
            out.push(Row {
                at: cases[i..i + run]
                    .iter()
                    .zip(levels)
                    .map(|(c, &l)| judge(c, l))
                    .collect(),
            });
        }
        // A record for a level not judged is no row of its own.
        i += run.max(1);
    }
    out
}

fn judge(c: &Case, level: ScsiLevel) -> Case {
    let name = cite(&c.name, level);
    let expected = cite(&c.expected, level);
    let mut actual = cite(&c.actual, level).text;
    let cited = name.cited + expected.cited;
    let undefined = c.scsi_level.is_none() && cited > 0 && name.absent + expected.absent == cited;
    let mut weight = c.level;
    let newer = c.since.filter(|&e| !level.holds(e));
    if let Some(edition) = newer.filter(|_| weight != Level::Info) {
        weight = Level::Info;
        actual = format!("{actual} (no requirement at {level}: introduced in {edition})");
    } else if undefined && weight != Level::Info {
        weight = Level::Info;
        actual =
            format!("{actual} (no requirement at {level}: its clause is not in those editions)");
    }
    Case {
        name: name.text,
        expected: expected.text,
        actual,
        level: weight,
        scsi_level: Some(level),
        ..c.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::super::case::{case, Verdict};
    use super::*;

    /// The cases as judged at one level.
    fn at(cases: &[Case], level: ScsiLevel) -> Vec<Case> {
        rows(cases, &[level])
            .into_iter()
            .map(|r| r.at[0].clone())
            .collect()
    }

    fn level(n: u8) -> ScsiLevel {
        ScsiLevel::new(n).unwrap()
    }

    #[test]
    fn a_level_sees_its_own_cases_and_the_shared_ones() {
        let cases = vec![
            case("pr", "shared".into(), "e".into(), "a".into(), true),
            case("sbc", "three".into(), "e".into(), "a".into(), true).at(level(3)),
            case("sbc", "five".into(), "e".into(), "a".into(), true).at(level(5)),
        ];
        let names = |n| {
            at(&cases, level(n))
                .into_iter()
                .map(|c| c.name)
                .collect::<Vec<_>>()
        };
        assert_eq!(names(3), vec!["shared", "three"]);
        assert_eq!(names(5), vec!["shared", "five"]);
        assert_eq!(names(4), vec!["shared"]);
    }

    #[test]
    fn rows_line_up_a_case_recorded_per_level() {
        let levels = [level(3), level(6)];
        let cases = vec![
            case("pr", "shared".into(), "e".into(), "a".into(), true),
            case("sbc", "x".into(), "e".into(), "a".into(), true).at(level(3)),
            case("sbc", "x".into(), "e".into(), "a".into(), false).at(level(6)),
            case("sbc", "y".into(), "e".into(), "a".into(), true).at(level(3)),
            case("sbc", "y".into(), "e".into(), "a".into(), true).at(level(6)),
        ];
        let r = rows(&cases, &levels);
        assert_eq!(r.len(), 3);
        assert_eq!(r[0].at.len(), 2);
        assert_eq!(
            r[1].at.iter().map(|c| c.verdict()).collect::<Vec<_>>(),
            vec![Verdict::Pass, Verdict::Fail]
        );
        assert_eq!(r[2].first().name, "y");
    }

    #[test]
    fn a_failure_against_a_clause_the_level_lacks_only_informs() {
        let missed = || {
            vec![case(
                "features",
                "SPC-6 A.2 Discovery 2016".into(),
                "met".into(),
                "misses VPD 92h".into(),
                false,
            )]
        };
        let three = at(&missed(), level(3));
        assert_eq!(three[0].verdict(), Verdict::Info);
        assert!(three[0].actual.contains("no requirement at SCSI level 3"));
        assert_eq!(at(&missed(), level(5))[0].verdict(), Verdict::Fail);
    }

    #[test]
    fn what_a_later_edition_introduced_only_informs_before_it() {
        use super::super::scsi_level::Edition;
        let cases = vec![case(
            "mpio",
            "REPORT TARGET PORT GROUPS, extended header".into(),
            "GOOD".into(),
            "CHECK CONDITION 5/24/00".into(),
            false,
        )
        .since(Edition::Spc(4))];
        let three = at(&cases, level(3));
        assert_eq!(three[0].verdict(), Verdict::Info);
        assert!(three[0].actual.contains("introduced in SPC-4"));
        assert_eq!(at(&cases, level(4))[0].verdict(), Verdict::Fail);
    }

    #[test]
    fn a_case_judged_for_its_level_keeps_its_weight() {
        let cases = vec![case(
            "sbc",
            "SPC-6 6.37 SECURITY PROTOCOL IN [Windows, Linux]".into(),
            "works".into(),
            "refused".into(),
            false,
        )
        .at(level(3))];
        assert_eq!(at(&cases, level(3))[0].verdict(), Verdict::Fail);
    }

    #[test]
    fn a_failure_citing_a_clause_the_level_has_still_fails() {
        let cases = vec![case(
            "pr",
            "SPC-6 5.14.11.2.2 RELEASE of the wrong type".into(),
            "5/26/04".into(),
            "RESERVATION CONFLICT".into(),
            false,
        )];
        let three = at(&cases, level(3));
        assert_eq!(three[0].verdict(), Verdict::Fail);
        assert_eq!(three[0].name, "SPC-3 5.6.10.2 RELEASE of the wrong type");
    }
}
