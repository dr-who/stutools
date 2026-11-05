//! The nvme-ana cases of `--compliance` (NVMe base specification, Asymmetric Namespace Access): the
//! multipath states a host uses to pick paths, as the ALUA states are for SCSI. When Identify
//! Controller CMIC bit 3 says the controller reports ANA, the ANA log page (0Ch) must parse through
//! every address of the subsystem, list every active namespace under the ANAGRPID its Identify
//! Namespace gives, in a valid state that ANACAP allows, with no group above ANAGRPMAX. Without
//! CMIC bit 3 the log page must be refused. The page itself is parsed in `ana_log`; Figure 312 gives
//! CMIC bit 3 (ANARS), ANACAP, ANAGRPMAX, NANAGRPID.

use super::ana_log::{self, anacap_bit, state_name, LOG_ANA, LOG_LEN};
use super::compliance::Target;
use super::controller::{self, Association, Controller};
use super::identify_cases::Facts;
use super::revisions as rev;
use super::verdict::{rev_case, status_case, INVALID_FIELD, INVALID_LOG_PAGE, SUCCESS};
use super::DEFAULT_HOSTNQN;
use crate::compliance::{case, Case};
use crate::Result;

/// CMIC bit 3: the controller reports Asymmetric Namespace Access.
const CMIC_ANA: u8 = 0x08;

fn read_log(addr: std::net::SocketAddr, subnqn: &str) -> Result<(Vec<u8>, super::sqe::Completion)> {
    let assoc = Association {
        addr,
        subnqn,
        hostnqn: DEFAULT_HOSTNQN,
        hostid: controller::random_hostid()?,
    };
    let mut c = Controller::connect(&assoc)?;
    c.enable()?;
    let mut buf = vec![0u8; LOG_LEN];
    let done = c.log_page(LOG_ANA, 0, 0, &mut buf)?;
    Ok((buf, done))
}

/// Runs the nvme-ana cases through every address. Returns each address's states for the windows
/// section, or None when the controller does not report ANA.
pub fn run(t: &Target, name: &str, facts: &Facts, cases: &mut Vec<Case>) -> Option<Vec<String>> {
    let section = "nvme-ana";
    let ctrl = &facts.ctrl;
    if ctrl.cmic & CMIC_ANA == 0 {
        let done = read_log(t.addrs[0], &t.subnqn).map(|(_, d)| d);
        status_case(
            cases,
            section,
            format!("{name} {} (CMIC bit 3 clear)", rev::LOG_ANA.name()),
            &[INVALID_LOG_PAGE, INVALID_FIELD],
            "Invalid Log Page or Invalid Field",
            &done,
        );
        return None;
    }
    let mut summary = Vec::new();
    for &addr in &t.addrs {
        let label = format!("{name} via {addr}");
        let (buf, done) = match read_log(addr, &t.subnqn) {
            Ok(v) => v,
            Err(e) => {
                cases.push(case(
                    section,
                    format!("{label} {}", rev::LOG_ANA.name()),
                    "read".into(),
                    e,
                    false,
                ));
                continue;
            }
        };
        let done = Ok(done);
        if !rev_case(
            cases,
            section,
            &label,
            &rev::LOG_ANA,
            facts.vs,
            &[SUCCESS],
            "success",
            &done,
        ) || !done.as_ref().is_ok_and(|d| d.ok())
        {
            continue;
        }
        let groups = match ana_log::parse(&buf) {
            Ok(g) => g,
            Err(e) => {
                cases.push(case(
                    section,
                    format!("{label} ANA log page parses"),
                    "well formed".into(),
                    e,
                    false,
                ));
                continue;
            }
        };
        cases.push(case(
            section,
            format!("{label} ANA groups"),
            format!(
                "no more than {} groups, IDs 1 to ANAGRPMAX {}",
                ctrl.nanagrpid.max(1),
                ctrl.anagrpmax
            ),
            groups
                .iter()
                .map(|g| format!("group {} {}", g.id, state_name(g.state)))
                .collect::<Vec<_>>()
                .join(", "),
            groups.len() as u32 <= ctrl.nanagrpid.max(1)
                && groups.iter().all(|g| g.id >= 1 && g.id <= ctrl.anagrpmax),
        ));
        for g in &groups {
            cases.push(case(
                section,
                format!("{label} group {} NSIDs", g.id),
                "ascending".into(),
                format!("{:?}", g.nsids),
                g.nsids.windows(2).all(|w| w[0] < w[1]),
            ));
            let allowed = anacap_bit(g.state).is_some_and(|bit| ctrl.anacap & bit != 0);
            cases.push(case(
                section,
                format!("{label} group {} state", g.id),
                format!("a valid state ANACAP {:#04x} allows", ctrl.anacap),
                state_name(g.state).into(),
                allowed,
            ));
        }
        for (nsid, ns) in &facts.namespaces {
            let listed = groups.iter().find(|g| g.nsids.contains(nsid));
            cases.push(case(
                section,
                format!("{label} ns {nsid} in its ANA group"),
                format!("listed in group {} (its ANAGRPID)", ns.anagrpid),
                listed.map_or("not listed".into(), |g| {
                    format!("group {} {}", g.id, state_name(g.state))
                }),
                listed.is_some_and(|g| g.id == ns.anagrpid)
                    && ns.anagrpid >= 1
                    && ns.anagrpid <= ctrl.anagrpmax,
            ));
            if let Some(g) = listed {
                summary.push(format!("{addr} {}", state_name(g.state)));
            }
        }
    }
    Some(summary)
}
