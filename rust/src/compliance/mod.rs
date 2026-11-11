//! `--compliance`: tests every target behind the `--iscsi` and `--nvme` hosts against the standards and
//! scores the result. A HOST alone is discovered first, so every target it reports is tested; HOST/IQN
//! or HOST/NQN tests that one. Each case states what the specification requires and what came back;
//! nothing is read from a profile, so it needs no other options. CHAP targets are tested further only
//! when `--chap-user` and `--chap-secret-file` are given. The device is probed once; the SCSI cases are
//! then judged at each `--scsilevel` (level 3 by default, `scsi_level`), and the report shows each
//! level. Ends with an error when any "shall" case fails at any level; "should" cases only warn
//! (`case`).
//!
//! The parts: `case` (one case and its weight), `scsi_level`, `clause_map` and `citation` (a case's
//! clauses at a level), `judged` (the cases at one level), `sections` and `parts` (what each section
//! tests and the chapter it belongs to), `score`, `device` (what was tested), `report` (the printout)
//! and `asciidoc` (the document).

use crate::{cli::Config, Result};

mod asciidoc;
mod case;
mod citation;
mod clause_map;
mod compatibility;
mod device;
mod judged;
mod parts;
mod report;
mod score;
mod scsi_level;
mod sections;
mod suite;
mod suite_report;
mod suites;

pub use case::{case, info, should, Case, Level, Verdict};
#[cfg(test)]
pub use citation::cite;
pub use device::Device;
pub use scsi_level::{parse as parse_scsi_levels, Edition, ScsiLevel};

/// Runs the iSCSI and NVMe/TCP cases, prints each case and the score at every level, writes the
/// AsciiDoc report when asked, and fails when any "shall" case fails.
pub fn run(config: &Config) -> Result<()> {
    crate::watchdog::start();
    let levels: &[ScsiLevel] = if config.scsi_levels.is_empty() {
        &[ScsiLevel::DEFAULT]
    } else {
        &config.scsi_levels
    };
    let mut cases = Vec::new();
    let mut untested = Vec::new();
    let mut devices = Vec::new();
    if !config.iscsi.is_empty() {
        crate::iscsi::compliance::cases(config, levels, &mut cases, &mut untested, &mut devices)?;
    }
    if !config.nvmeof.is_empty() {
        crate::nvme::compliance::cases(&config.nvmeof, &mut cases, &mut devices)?;
    }
    let local: Vec<String> = config
        .files
        .iter()
        .map(|f| f.display().to_string())
        .filter(|f| f.starts_with("/dev/"))
        .collect();
    #[cfg(target_os = "linux")]
    crate::iscsi::local_disk::cases(&local, levels, &mut cases, &mut untested, &mut devices);
    let rows = judged::rows(&cases, levels);
    let views: Vec<(ScsiLevel, Vec<Case>)> = levels
        .iter()
        .enumerate()
        .map(|(i, &l)| (l, rows.iter().map(|r| r.at[i].clone()).collect()))
        .collect();
    report::print(&views, &untested);
    if let Some(path) = &config.compliance_report {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let heading = asciidoc::Heading {
            version: env!("CARGO_PKG_VERSION"),
            utc: crate::cctv::files::utc(now, true),
            machine: std::fs::read_to_string("/proc/sys/kernel/hostname")
                .map(|h| h.trim().to_string())
                .unwrap_or_else(|_| "this machine".into()),
            command: command_line(),
            iscsi: &config.iscsi,
            nvme: &config.nvmeof,
            local: &local,
            initiators: config
                .iscsi_initiator
                .iter()
                .chain(&config.iscsi_second_initiator)
                .cloned()
                .collect(),
            devices: &devices,
            untested: &untested,
        };
        std::fs::write(path, asciidoc::document(&heading, levels, &rows))
            .map_err(|e| format!("writing {}: {e}", path.display()))?;
        println!("compliance report written to {}", path.display());
        let suites = suites_path(path);
        std::fs::write(&suites, asciidoc::suites_document(&heading, levels, &rows))
            .map_err(|e| format!("writing {}: {e}", suites.display()))?;
        println!("conformance suites written to {}", suites.display());
    }
    let failing: Vec<String> = views
        .iter()
        .filter_map(|(level, cases)| {
            let t = score::Tally::of(cases.iter());
            (t.failed > 0)
                .then(|| format!("{} of {} cases failed at {level}", t.failed, t.scored()))
        })
        .collect();
    if !failing.is_empty() {
        return Err(failing.join("; "));
    }
    if views
        .iter()
        .all(|(_, c)| score::Tally::of(c.iter()).scored() == 0)
    {
        return Err("no target could be tested".into());
    }
    Ok(())
}

/// The command as typed, each argument quoted when it holds a space or a quote. Secrets never reach
/// argv (they come from `--chap-secret-file`), so it is safe to print.
fn command_line() -> String {
    std::env::args()
        .map(|a| {
            if a.is_empty() || a.contains([' ', '\'', '"']) {
                format!("'{}'", a.replace('\'', "'\\''"))
            } else {
                a
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The conformance suites document beside the report: REPORT.adoc gives REPORT-suites.adoc.
fn suites_path(report: &std::path::Path) -> std::path::PathBuf {
    let stem = report
        .file_stem()
        .map_or_else(|| "report".into(), |s| s.to_string_lossy().into_owned());
    let ext = report
        .extension()
        .map_or_else(|| "adoc".into(), |e| e.to_string_lossy().into_owned());
    report.with_file_name(format!("{stem}-suites.{ext}"))
}
