//! The `scsi` cases of `--compliance`: on each reachable path the identity commands answer (INQUIRY,
//! VPD 80h and 83h, READ CAPACITY (16), REPORT TARGET PORT GROUPS; SPC-6 6.7, 6.34; SBC-5 5.21), the
//! capacity is sane (blocks of 512 bytes or more, a power of two), and the answering port is in a group
//! REPORT TARGET PORT GROUPS lists. Returns, per path, whether that port is in the standby state, for
//! the SCSI sections to test it as such.

use super::identity::Collected;
use super::login::reason;
use super::login_cases::Reachable;
use super::lun_access::short_name;
use crate::compliance::{case, Case};
use crate::Result;

/// Runs the scsi cases over what `collected` read from each reachable path.
pub(super) fn run(
    reachable: &[Reachable],
    collected: &[Result<Collected>],
    cases: &mut Vec<Case>,
) -> Vec<bool> {
    let mut standby = vec![false; reachable.len()];
    for (i, (r, result)) in reachable.iter().zip(collected).enumerate() {
        let name = short_name(r.path);
        let c = match result {
            Err(err) => {
                cases.push(case(
                    "scsi",
                    format!("{name} commands"),
                    "all answer".into(),
                    format!("failed: {}", reason(err)),
                    false,
                ));
                continue;
            }
            Ok(c) => c,
        };
        cases.push(case(
            "scsi",
            format!("{name} INQUIRY, VPD 0x80/0x83, READ CAPACITY(16), REPORT TPG"),
            "all answer".into(),
            "all answer".into(),
            true,
        ));
        let size_ok = c.last_lba > 0 && c.block_len >= 512 && c.block_len.is_power_of_two();
        cases.push(case(
            "scsi",
            format!("{name} capacity"),
            "blocks > 0 of 512 B or more, a power of two".into(),
            format!("{} blocks of {} B", c.last_lba + 1, c.block_len),
            size_ok,
        ));
        let own = c
            .own_port
            .and_then(|port| c.groups.iter().find(|g| g.ports.contains(&port)))
            .map(|g| super::scsi::state_name(g.state).to_string());
        standby[i] = own.as_deref() == Some("standby");
        // SPC-6 6.7.2: TPGS 0 means no target port groups, so there is no state to find.
        if c.inquiry.tpgs == 0 {
            cases.push(crate::compliance::info(
                "scsi",
                format!("{name} ALUA state"),
                "no target port groups (TPGS 0)".into(),
            ));
        } else {
            cases.push(case(
                "scsi",
                format!("{name} ALUA state"),
                "its port in a listed group".into(),
                own.clone().unwrap_or_else(|| "port not listed".into()),
                own.is_some(),
            ));
        }
    }

    standby
}
