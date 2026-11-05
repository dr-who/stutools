//! `--compliance`: tests every target behind the `--iscsi` and `--nvme` hosts against the standards and
//! scores the result. A HOST alone is discovered first, so every target it reports is tested; HOST/IQN
//! or HOST/NQN tests that one. Each case states what the specification requires and what came back;
//! nothing is read from a profile, so it needs no other options. CHAP targets are tested further only
//! when `--chap-user` and `--chap-secret-file` are given. Ends with an error when any case fails.

use crate::{cli::Config, Result};
use std::path::Path;

/// One scored case.
pub struct Case {
    pub section: &'static str,
    pub name: String,
    pub expected: String,
    pub actual: String,
    pub passed: bool,
}

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
    }
}

/// The sections in the order they run, with the specification each one tests against and, where
/// Windows depends on it, the Windows component that sends those commands.
const FEATURES: [(&str, &str); 24] = [
    ("login", "iSCSI, RFC 7143: login phase and key negotiation"),
    (
        "identity",
        "iSCSI, SPC-4: INQUIRY standard data and VPD 0x80, 0x83",
    ),
    (
        "scsi",
        "iSCSI, SPC-4: identity, capacity and target port groups",
    ),
    ("chap", "iSCSI, RFC 7143: CHAP authentication"),
    (
        "session",
        "iSCSI, RFC 7143: sessions, NOP-Out and reinstatement",
    ),
    (
        "spc",
        "iSCSI, SPC-4: mandatory commands and invalid opcodes",
    ),
    (
        "disk",
        "iSCSI, SPC-4/SBC-3: what Windows disk.sys asks at discovery",
    ),
    (
        "io",
        "iSCSI, SBC-3: reads, writes, FUA, VERIFY, cache flush (NTFS)",
    ),
    (
        "unmap",
        "iSCSI, SBC-3: thin provisioning, UNMAP (Windows TRIM)",
    ),
    ("ats", "iSCSI, SBC-3: COMPARE AND WRITE"),
    ("odx", "iSCSI, SBC-3: ROD token copy (Windows ODX)"),
    (
        "pr",
        "iSCSI, SPC-4: PERSISTENT RESERVE (Failover Clustering)",
    ),
    (
        "tmf",
        "iSCSI, RFC 7143/SAM-5: task management (MPIO recovery)",
    ),
    (
        "mpio",
        "iSCSI, SPC-4: ALUA target port groups (Windows MSDSM)",
    ),
    ("nvme-disc", "NVMe/TCP, NVMe-oF: discovery log page"),
    (
        "nvme-conn",
        "NVMe/TCP, NVMe-oF: Fabrics Connect and controller IDs",
    ),
    ("nvme-prop", "NVMe-oF: CAP, VS, CC and CSTS properties"),
    (
        "nvme-ident",
        "NVMe base: Identify and namespace IDs on every path",
    ),
    (
        "nvme-admin",
        "NVMe base: log pages, features, Abort, Keep Alive",
    ),
    (
        "nvme-io",
        "NVMe NVM command set: Read, Write, FUA, Flush, DSM",
    ),
    (
        "nvme-ana",
        "NVMe base: asymmetric namespace access (multipath)",
    ),
    ("nvme-resv", "NVMe base: reservations (Failover Clustering)"),
    ("nvme-sess", "NVMe-oF: repeated associations"),
    (
        "windows",
        "What Windows MPIO, NTFS and clustering need of a disk",
    ),
];

/// Runs the iSCSI and NVMe/TCP cases, prints each case and the score, and fails when any case fails.
pub fn run(config: &Config) -> Result<()> {
    let mut cases = Vec::new();
    let mut untested = Vec::new();
    if !config.iscsi.is_empty() {
        crate::iscsi::compliance::cases(config, &mut cases, &mut untested)?;
    }
    if !config.nvmeof.is_empty() {
        crate::nvme::compliance::cases(&config.nvmeof, &mut cases)?;
    }
    print_cases(&cases);
    let total = cases.len();
    let passed = cases.iter().filter(|c| c.passed).count();
    let failed = total - passed;
    println!();
    println!(
        "{:<12}{:>8}{:>8}{:>8}{:>9}",
        "section", "cases", "passed", "failed", "score"
    );
    for (section, _) in FEATURES {
        let of: Vec<&Case> = cases.iter().filter(|c| c.section == section).collect();
        if of.is_empty() {
            continue;
        }
        let ok = of.iter().filter(|c| c.passed).count();
        println!(
            "{section:<12}{:>8}{ok:>8}{:>8}{:>8.1}%",
            of.len(),
            of.len() - ok,
            percent(ok, of.len())
        );
    }
    println!(
        "{:<12}{total:>8}{passed:>8}{failed:>8}{:>8.1}%",
        "total",
        percent(passed, total)
    );
    if !untested.is_empty() {
        println!();
        println!("not tested past login:");
        for (name, why) in &untested {
            println!("  {name}: {why}");
        }
    }
    if let Some(path) = &config.compliance_report {
        write_report(path, &cases)?;
        println!("compliance report written to {}", path.display());
    }
    if failed > 0 {
        println!();
        println!("failed cases:");
        for c in cases.iter().filter(|c| !c.passed) {
            println!(
                "  [{}] {}: expected {}, got {}",
                c.section, c.name, c.expected, c.actual
            );
        }
        return Err(format!("{failed} of {total} cases failed"));
    }
    if total == 0 {
        return Err("no target could be tested".into());
    }
    Ok(())
}

/// Writes the compliance report: one row per section with its specification and result, then every
/// failed case with what was expected and what came back.
fn write_report(path: &Path, cases: &[Case]) -> Result<()> {
    let mut out = String::from("# Compliance report\n\n");
    out.push_str("| Feature | Specification | Cases | Passed | Failed | Score | Result |\n");
    out.push_str("|---|---|---:|---:|---:|---:|---|\n");
    for (section, reference) in FEATURES {
        let of: Vec<&Case> = cases.iter().filter(|c| c.section == section).collect();
        if of.is_empty() {
            continue;
        }
        let ok = of.iter().filter(|c| c.passed).count();
        let result = if ok == of.len() { "pass" } else { "FAIL" };
        out.push_str(&format!(
            "| {section} | {reference} | {} | {ok} | {} | {:.1}% | {result} |\n",
            of.len(),
            of.len() - ok,
            percent(ok, of.len())
        ));
    }
    let total = cases.len();
    let passed = cases.iter().filter(|c| c.passed).count();
    out.push_str(&format!(
        "| **total** | | {total} | {passed} | {} | {:.1}% | |\n",
        total - passed,
        percent(passed, total)
    ));
    let failed: Vec<&Case> = cases.iter().filter(|c| !c.passed).collect();
    if !failed.is_empty() {
        out.push_str("\n## Failed cases\n\n");
        for c in failed {
            out.push_str(&format!(
                "- **{}**, {}: expected `{}`, got `{}`\n",
                c.section, c.name, c.expected, c.actual
            ));
        }
    }
    std::fs::write(path, out).map_err(|e| format!("writing {}: {e}", path.display()))
}

fn percent(ok: usize, total: usize) -> f64 {
    if total == 0 {
        100.0
    } else {
        100.0 * ok as f64 / total as f64
    }
}

fn print_cases(cases: &[Case]) {
    for c in cases {
        println!(
            "{}  {:<10} {:<58} expect {:<22} got {}",
            if c.passed { "PASS" } else { "FAIL" },
            c.section,
            c.name,
            c.expected,
            c.actual
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_section_has_a_specification() {
        let mut names: Vec<&str> = FEATURES.iter().map(|(s, _)| *s).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), FEATURES.len());
        assert!(FEATURES.iter().all(|(_, spec)| !spec.is_empty()));
    }

    #[test]
    fn percent_of_nothing_is_full() {
        assert_eq!(percent(0, 0), 100.0);
        assert_eq!(percent(1, 4), 25.0);
    }
}
