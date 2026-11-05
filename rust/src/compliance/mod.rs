//! `--compliance`: tests every target behind the `--iscsi` and `--nvme` hosts against the standards and
//! scores the result. A HOST alone is discovered first, so every target it reports is tested; HOST/IQN
//! or HOST/NQN tests that one. Each case states what the specification requires and what came back;
//! nothing is read from a profile, so it needs no other options. CHAP targets are tested further only
//! when `--chap-user` and `--chap-secret-file` are given. Ends with an error when any "shall" case fails;
//! "should" cases only warn (`case`).

use crate::{cli::Config, Result};

mod case;
mod report;
mod score;
mod sections;

pub use case::{case, info, should, Case};

/// Runs the iSCSI and NVMe/TCP cases, prints each case and the score, writes the Markdown report
/// when asked, and fails when any "shall" case fails.
pub fn run(config: &Config) -> Result<()> {
    let mut cases = Vec::new();
    let mut untested = Vec::new();
    if !config.iscsi.is_empty() {
        crate::iscsi::compliance::cases(config, &mut cases, &mut untested)?;
    }
    if !config.nvmeof.is_empty() {
        crate::nvme::compliance::cases(&config.nvmeof, &mut cases)?;
    }
    report::print(&cases, &untested);
    if let Some(path) = &config.compliance_report {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let heading = report::Heading {
            version: env!("CARGO_PKG_VERSION"),
            utc: crate::cctv::files::utc(now, true),
            iscsi: &config.iscsi,
            nvme: &config.nvmeof,
        };
        std::fs::write(path, report::markdown(&heading, &cases, &untested))
            .map_err(|e| format!("writing {}: {e}", path.display()))?;
        println!("compliance report written to {}", path.display());
    }
    let total = score::Tally::of(cases.iter());
    if total.failed > 0 {
        return Err(format!(
            "{} of {} cases failed",
            total.failed,
            total.scored()
        ));
    }
    if total.scored() == 0 {
        return Err("no target could be tested".into());
    }
    Ok(())
}
