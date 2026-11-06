//! What `spit --iscsi HOST` prints: the targets SendTargets reports, then for each target and address
//! spit logs in and prints a row of the LUN's details (size, block length, vendor, product, revision,
//! serial, ALUA state, authentication and digest), or why the login was refused and what to do about
//! it, and the command to run a workload on the first target that answered.

use super::credentials::chap_from_config;
use super::identity::{collect, Collected};
use super::path::Path;
use super::scsi::{self};
use crate::columns::{human_bytes, print_columns};
use crate::{cli::Config, Result};

/// Prints the table and the verdict. Returns an error when the paths are not one LUN.
/// `--iscsi HOST` with no IQN: lists the targets the portal reports with SendTargets, then logs in to each
/// at every address it gives and prints a row of what its LUN reports: size, block size, vendor,
/// product, revision, serial, ALUA state, authentication and digest. A target that refuses the login
/// says why. CHAP targets are logged in to with `--chap-user` and `--chap-secret-file` when given.
pub fn discover(config: &Config) -> Result<()> {
    let initiator = config.iscsi_initiator.as_deref().unwrap_or_default();
    let secret = chap_from_config(config)?;
    let chap = secret.as_ref().map(|(u, s)| (u.as_str(), s.as_slice()));
    let mut rows = vec![DISCOVERY_COLUMNS.map(String::from).to_vec()];
    let mut first = None;
    for host in &config.iscsi {
        for target in super::send_targets::discover_targets(host, initiator)? {
            for address in &target.addresses {
                let path = Path {
                    portal: address.clone(),
                    target: target.target.clone(),
                    lun: 0,
                };
                let row = match collect(&path, initiator, chap) {
                    Ok(c) => {
                        first.get_or_insert_with(|| format!("{address}/{}", target.target));
                        discovery_row(&path, &c)
                    }
                    Err(e) => {
                        let mut row = vec![target.target.clone(), address.clone()];
                        row.extend(
                            std::iter::repeat("-".to_string()).take(DISCOVERY_COLUMNS.len() - 3),
                        );
                        row.push(refusal(&e));
                        row
                    }
                };
                rows.push(row);
            }
        }
    }
    print_columns(&rows);
    if let Some(path) = first {
        println!("# run a workload: spit --iscsi {path} -c rs0k4q32j4 -t 5");
    }
    Ok(())
}

pub(super) const DISCOVERY_COLUMNS: [&str; 12] = [
    "TARGET", "ADDRESS", "SIZE", "BLOCK", "VENDOR", "PRODUCT", "REV", "SERIAL", "ALUA", "AUTH",
    "DIGEST", "NOTE",
];

/// One row of the discovery table for a target spit logged in to.
pub(super) fn discovery_row(path: &Path, c: &Collected) -> Vec<String> {
    let alua = c
        .own_port
        .and_then(|port| c.groups.iter().find(|g| g.ports.contains(&port)))
        .map(|g| format!("{} (group {})", scsi::state_name(g.state), g.id))
        .unwrap_or_else(|| "-".into());
    vec![
        path.target.clone(),
        path.portal.clone(),
        human_bytes((c.last_lba + 1) * c.block_len as u64),
        c.block_len.to_string(),
        c.inquiry.vendor.clone(),
        c.inquiry.product.clone(),
        c.inquiry.revision.clone(),
        c.serial.clone(),
        alua,
        if c.login.chap_algorithm.is_some() {
            "CHAP"
        } else {
            "none"
        }
        .into(),
        if c.login.header_digest {
            "CRC32C"
        } else {
            "none"
        }
        .into(),
        String::new(),
    ]
}

/// Why a login was refused, in a few words, with what to do about it.
pub(super) fn refusal(error: &str) -> String {
    if error.contains("authentication failure") {
        "needs CHAP: --chap-user, --chap-secret-file".into()
    } else if error.contains("authorization failure") {
        "refused: initiator not in the target's ACL".into()
    } else {
        error
            .trim_start_matches("*error* ")
            .chars()
            .take(60)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refusals_say_what_to_do() {
        assert!(
            refusal("login refused: status class 2 detail 1 (authentication failure)")
                .contains("--chap-user")
        );
        assert!(
            refusal("login refused: status class 2 detail 2 (authorization failure)")
                .contains("ACL")
        );
    }
}
