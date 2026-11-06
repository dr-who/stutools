//! What a `--compliance` run prints: every case with its verdict at the lowest SCSI level asked, then
//! for each level the score per section and in total and its failures and warnings, and the targets
//! tested no further. The document form is `asciidoc`.

use super::case::{Case, Verdict};
use super::score::{by_section, Tally};
use super::scsi_level::ScsiLevel;

/// Prints the run; `views` holds the cases as judged at each level, lowest first.
pub fn print(views: &[(ScsiLevel, Vec<Case>)], untested: &[(String, String)]) {
    let Some((_, lowest)) = views.first() else {
        return;
    };
    for c in lowest {
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
                    println!(
                        "  [{}] {}: expected {}, got {}",
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
