//! The iSCSI cases of `--compliance`. Every `--iscsi` HOST is discovered (SendTargets) and each target
//! it reports is tested at every address it gives; HOST/IQN tests that target. The expectations come
//! from RFC 7143, SPC-4 and SBC-3 and from what the target itself reports, never from a profile:
//! - login: each digest offer is answered with a value that was offered, and "CRC32C,None" gets
//!   CRC32C when the target takes CRC32C alone; a CHAP target refuses no credentials and a wrong
//!   secret;
//! - identity: paths whose LUN designators (VPD 0x83) agree report one LUN, and the verdict does not
//!   depend on the order the paths are compared in;
//! - scsi: the identity commands answer, the capacity is sane, and the answering port is in a group
//!   REPORT TARGET PORT GROUPS lists;
//! - chap: CHAP_A 5 to 8 are accepted as offered or refused, any other value is refused, and a wrong
//!   secret is refused for every accepted value;
//! - session, and the SCSI operations in `conformance`.
//!
//! A target that refuses this initiator (its ACL) or needs CHAP without credentials is tested no further.

use super::conformance;
use super::credentials;
use super::identity;
use super::identity::Collected;
use super::login_cases::{self, Reachable};
use super::path;
use super::path::Path;
use super::send_targets;
use super::{chap_cases, chap_mutual_cases, identity_cases, scsi_cases, session_cases};
use crate::compliance::{Case, Device, ScsiLevel};
use crate::{cli::Config, Result};

/// Adds the iSCSI cases for the `--iscsi` targets to `cases`, the targets tested no further to
/// `untested`, and each logical unit reached to `devices`: the login, identity, scsi, chap and session
/// sections, then the SCSI sections, whose level-dependent cases are judged at each of `levels`.
pub fn cases(
    config: &Config,
    levels: &[ScsiLevel],
    cases: &mut Vec<Case>,
    untested: &mut Vec<(String, String)>,
    devices: &mut Vec<Device>,
) -> Result<()> {
    let initiator = config.iscsi_initiator.as_deref().unwrap_or_default();
    let paths = targets(&config.iscsi, initiator)?;
    let secret = credentials::chap_from_config(config)?;
    let chap_good = secret.as_ref().map(|(u, s)| (u.as_str(), s.as_slice()));
    let wrong = b"not-the-secret".to_vec();
    let chap_bad = config.chap_user.as_deref().map(|u| (u, wrong.as_slice()));
    let reachable = login_cases::run(&paths, initiator, chap_good, chap_bad, cases, untested);
    super::login_phase_cases::run(&reachable, initiator, chap_good, cases);
    super::data_transfer_cases::run(&reachable, initiator, chap_good, cases);
    let creds_of = |r: &Reachable| if r.chap { chap_good } else { None };
    let collected: Vec<Result<Collected>> = reachable
        .iter()
        .map(|r| identity::collect(r.path, initiator, creds_of(r)))
        .collect();
    for (r, c) in reachable.iter().zip(&collected) {
        let Ok(c) = c else { continue };
        let portal = r.path.portal.to_string();
        match devices.iter_mut().find(|d| d.target == r.path.target) {
            Some(d) => d.paths.push(portal),
            None => devices.push(Device {
                name: super::lun_access::short_name(r.path),
                transport: "iSCSI",
                target: r.path.target.clone(),
                paths: vec![portal],
                vendor: c.inquiry.vendor.clone(),
                product: c.inquiry.product.clone(),
                revision: c.inquiry.revision.clone(),
                serial: c.serial.clone(),
                blocks: c.last_lba.saturating_add(1),
                block_len: c.block_len,
            }),
        }
    }
    identity_cases::run(&reachable, &collected, cases);
    let standby = scsi_cases::run(&reachable, &collected, cases);
    if chap_good.is_some() {
        chap_cases::run(&reachable, initiator, chap_good, chap_bad, cases);
        let t_secret = match &config.chap_target_secret_file {
            Some(f) => credentials::read_secret(f)?,
            None => Vec::new(),
        };
        let target_creds = config
            .chap_target_user
            .as_deref()
            .filter(|_| !t_secret.is_empty())
            .map(|u| (u, t_secret.as_slice()));
        let secret_b = match &config.chap_secret_b_file {
            Some(f) => credentials::read_secret(f)?,
            None => Vec::new(),
        };
        let second = match (&config.chap_initiator_b, chap_good) {
            (Some(name_b), Some((user, secret))) if !secret_b.is_empty() => {
                Some((name_b.as_str(), secret_b.as_slice(), user, secret))
            }
            _ => None,
        };
        chap_mutual_cases::run(
            &reachable,
            initiator,
            chap_good,
            chap_bad,
            target_creds,
            second,
            cases,
        );
    }
    for r in &reachable {
        session_cases::run(r.path, initiator, creds_of(r), cases);
    }
    // Section 6: the SCSI sections, judged at each SCSI level. A LUN in the standby
    // state is checked for the commands a standby port must and must not take instead.
    let targets: Vec<conformance::Target> = reachable
        .iter()
        .zip(&standby)
        .map(|(r, &standby)| conformance::Target {
            path: r.path,
            creds: creds_of(r),
            standby,
            protocol_level: r.level,
        })
        .collect();
    conformance::run(
        &targets,
        initiator,
        config.iscsi_second_initiator.as_deref(),
        config.iscsi_third_initiator.as_deref(),
        levels,
        cases,
    );
    Ok(())
}

/// The targets to test: each HOST/IQN as given, and every target a HOST alone reports, at each of its
/// addresses.
fn targets(entries: &[String], initiator: &str) -> Result<Vec<Path>> {
    let mut out = Vec::new();
    for entry in entries {
        if entry.contains('/') {
            out.push(path::parse_path(entry)?);
            continue;
        }
        for t in send_targets::discover_targets(entry, initiator)? {
            for address in t.addresses {
                out.push(Path {
                    portal: address,
                    target: t.target.clone(),
                    lun: 0,
                });
            }
        }
    }
    // Several hosts often report the same targets; each path is tested once.
    let mut unique: Vec<Path> = Vec::new();
    for p in out {
        if !unique
            .iter()
            .any(|u| u.target == p.target && super::portal::same(&u.portal, &p.portal))
        {
            unique.push(p);
        }
    }
    Ok(unique)
}
