//! What a `--compliance` run prints and writes: each case with its verdict, the score per section
//! and in total, the targets tested no further, and the failures and warnings with what was expected
//! and what came back. `markdown` is the same report as a document (`--compliance-report OUT`),
//! headed by what was tested, when, by which spit, and against which editions of the standards.

use super::case::{Case, Verdict};
use super::score::{by_section, Tally};

/// The editions of the standards the iSCSI cases are judged against.
pub const SCSI_STANDARDS: [&str; 4] = [
    "SAM-6 (INCITS 546-2021)",
    "SPC-6 (INCITS 566-2025)",
    "SBC-3 (INCITS 514-2014), SBC-4 (INCITS 506-2021), SBC-5 (INCITS 571-2025)",
    "RFC 7143 and RFC 7144 (iSCSI)",
];

/// The editions of the standards the NVMe/TCP cases are judged against.
pub const NVME_STANDARDS: [&str; 4] = [
    "NVM Express Base 2.1",
    "NVM Express NVM Command Set 1.1",
    "NVM Express TCP Transport 1.1",
    "NVM Express SCSI Translation Reference 1.5",
];

/// What a run tested, for the report's heading.
pub struct Heading<'a> {
    pub version: &'a str,
    pub utc: String,
    pub iscsi: &'a [String],
    pub nvme: &'a [String],
}

/// Prints every case, the score table, the targets tested no further, and the failures and
/// warnings.
pub fn print(cases: &[Case], untested: &[(String, String)]) {
    for c in cases {
        println!(
            "{}  {:<10} {:<58} expect {:<22} got {}",
            c.verdict().word(),
            c.section,
            c.name,
            c.expected,
            c.actual
        );
    }
    println!();
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
    for (section, _, t) in by_section(cases) {
        row(section, &t);
    }
    row("total", &Tally::of(cases.iter()));
    if !untested.is_empty() {
        println!();
        println!("not tested past login:");
        for (name, why) in untested {
            println!("  {name}: {why}");
        }
    }
    for (verdict, title) in [
        (Verdict::Fail, "failed cases:"),
        (Verdict::Warn, "warnings:"),
    ] {
        let of: Vec<&Case> = cases.iter().filter(|c| c.verdict() == verdict).collect();
        if !of.is_empty() {
            println!();
            println!("{title}");
            for c in of {
                println!(
                    "  [{}] {}: expected {}, got {}",
                    c.section, c.name, c.expected, c.actual
                );
            }
        }
    }
}

/// The report as Markdown.
pub fn markdown(h: &Heading, cases: &[Case], untested: &[(String, String)]) -> String {
    let mut out = String::from("# Compliance report\n\n");
    out.push_str(&format!("- spit {}, run {}\n", h.version, h.utc));
    for (kind, hosts, standards) in [
        ("iSCSI", h.iscsi, &SCSI_STANDARDS),
        ("NVMe/TCP", h.nvme, &NVME_STANDARDS),
    ] {
        if !hosts.is_empty() {
            out.push_str(&format!(
                "- {kind}: {}, judged against {}\n",
                hosts.join(", "),
                standards.join("; ")
            ));
        }
    }
    out.push_str("\nA *shall* that is not met is a failure, a *should* a warning; informational cases record what the device offers.\n\n");
    out.push_str("| Section | Standard | Cases | Pass | Warn | Fail | Info | Score | Result |\n");
    out.push_str("|---|---|---:|---:|---:|---:|---:|---:|---|\n");
    for (section, spec, t) in by_section(cases) {
        out.push_str(&format!(
            "| {section} | {spec} | {} | {} | {} | {} | {} | {:.1}% | {} |\n",
            t.scored(),
            t.passed,
            t.warned,
            t.failed,
            t.info,
            t.percent(),
            t.result()
        ));
    }
    let t = Tally::of(cases.iter());
    out.push_str(&format!(
        "| **total** | | {} | {} | {} | {} | {} | {:.1}% | {} |\n",
        t.scored(),
        t.passed,
        t.warned,
        t.failed,
        t.info,
        t.percent(),
        t.result()
    ));
    let list = |out: &mut String, title: &str, verdict: Verdict| {
        let of: Vec<&Case> = cases.iter().filter(|c| c.verdict() == verdict).collect();
        if of.is_empty() {
            return;
        }
        out.push_str(&format!("\n## {title}\n\n"));
        for c in of {
            if verdict == Verdict::Info {
                out.push_str(&format!("- **{}**, {}: {}\n", c.section, c.name, c.actual));
            } else {
                out.push_str(&format!(
                    "- **{}**, {}: expected `{}`, got `{}`\n",
                    c.section, c.name, c.expected, c.actual
                ));
            }
        }
    };
    list(&mut out, "Failures", Verdict::Fail);
    list(&mut out, "Warnings", Verdict::Warn);
    list(&mut out, "What the devices offer", Verdict::Info);
    if !untested.is_empty() {
        out.push_str("\n## Not tested past login\n\n");
        for (name, why) in untested {
            out.push_str(&format!("- {name}: {why}\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::case::{case, should};
    use super::*;

    #[test]
    fn the_report_names_the_editions_and_lists_failures_and_warnings() {
        let cases = vec![
            case(
                "pr",
                "B WRITE".into(),
                "RESERVATION CONFLICT".into(),
                "GOOD".into(),
                false,
            ),
            should(
                "nvme-resv",
                "release".into(),
                "Invalid Field".into(),
                "conflict".into(),
                false,
            ),
        ];
        let h = Heading {
            version: "2.2.0",
            utc: "2026-11-06 10:00:00".into(),
            iscsi: &["10.0.0.5".into()],
            nvme: &[],
        };
        let md = markdown(&h, &cases, &[]);
        assert!(md.contains("SBC-5 (INCITS 571-2025)"));
        assert!(md.contains("## Failures\n\n- **pr**, B WRITE"));
        assert!(md.contains("## Warnings\n\n- **nvme-resv**, release"));
        assert!(md.contains("- iSCSI: 10.0.0.5, judged against SAM-6"));
    }
}
