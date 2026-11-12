//! What a `--compliance` run prints: every case with its verdict at the lowest SCSI level asked, then
//! for each level the score per section and in total and its failures and warnings, and the targets
//! tested no further. The document form is `asciidoc`.

use super::case::{Case, Verdict};
use super::score::{by_section, Tally};
use super::scsi_level::ScsiLevel;

/// Prints the run; `views` holds the cases as judged at each level, lowest first.
pub fn print(views: &[(ScsiLevel, Vec<Case>)], untested: &[(String, String)], verbose: bool) {
    let Some((_, lowest)) = views.first() else {
        return;
    };
    // Without --verbose only the warnings and failures are listed case by case; the summary below has all.
    for c in lowest
        .iter()
        .filter(|c| verbose || matches!(c.verdict(), Verdict::Warn | Verdict::Fail))
    {
        println!(
            "{}  {:<10} {:<58} expect {:<22} got {}",
            c.verdict().word(),
            c.section,
            c.name,
            c.expected,
            c.actual
        );
    }
    for (level, cases) in views {
        println!();
        println!("{level}");
        println!(
            "{:<12}{:>7}{:>7}{:>7}{:>7}{:>7}{:>9}",
            "section", "cases", "pass", "warn", "fail", "info", "score"
        );
        let row = |name: &str, t: &Tally| {
            println!(
                "{name:<12}{:>7}{:>7}{:>7}{:>7}{:>7}{:>8.1}%",
                t.scored(),
                t.passed,
                t.warned,
                t.failed,
                t.info,
                t.percent()
            );
        };
        for (section, t) in by_section(cases) {
            row(section.name, &t);
        }
        row("total", &Tally::of(cases.iter()));
        for (verdict, title) in [(Verdict::Fail, "failed cases"), (Verdict::Warn, "warnings")] {
            let of: Vec<&Case> = cases.iter().filter(|c| c.verdict() == verdict).collect();
            if !of.is_empty() {
                println!();
                println!("{title} at {level}:");
                for c in of {
                    // The spec clause the case tests goes at the end, so a failure says where to look.
                    let clause = spec_clause(&c.name)
                        .unwrap_or_else(|| "(no spec clause in its name)".into());
                    println!(
                        "  [{}] {}: expected {}, got {} ({clause})",
                        c.section, c.name, c.expected, c.actual
                    );
                }
            }
        }
    }
    if !untested.is_empty() {
        println!();
        println!("not tested past login:");
        for (name, why) in untested {
            println!("  {name}: {why}");
        }
    }
}

/// The first spec citation in a case name: an edition, then a clause ("SPC-3 6.4.1", "SBC-5 5.21.2").
fn spec_clause(name: &str) -> Option<String> {
    let words: Vec<&str> = name.split_whitespace().collect();
    for (i, w) in words.iter().enumerate() {
        let edition = w.trim_end_matches([',', ':', ';']);
        let known = ["SPC-", "SBC-", "SAM-", "ZBC-", "SCC-"]
            .iter()
            .any(|p| edition.starts_with(p));
        let Some(clause) = words.get(i + 1) else {
            continue;
        };
        let clause = clause.trim_end_matches([',', ':', ';']);
        if known && clause.starts_with(|c: char| c.is_ascii_digit()) {
            return Some(format!("{edition} {clause}"));
        }
    }
    None
}

#[cfg(test)]
mod clause_tests {
    use super::spec_clause;

    #[test]
    fn spec_clause_is_the_first_citation() {
        assert_eq!(
            spec_clause("SPC-3 6.4.1 INQUIRY with EVPD 0").as_deref(),
            Some("SPC-3 6.4.1")
        );
        assert_eq!(
            spec_clause("SBC-3 5.13, 5.42 1000 random").as_deref(),
            Some("SBC-3 5.13")
        );
        assert_eq!(spec_clause("disk.sys enumeration"), None);
    }
}
