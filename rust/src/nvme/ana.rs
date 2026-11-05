//! The nvme-ana cases of `--compliance` (NVMe base specification, Asymmetric Namespace Access): the
//! multipath states a host uses to pick paths, as the ALUA states are for SCSI. When Identify
//! Controller CMIC bit 3 says the controller reports ANA, the ANA log page (0Ch) must parse through
//! every address of the subsystem, list every active namespace under the ANAGRPID its Identify
//! Namespace gives, in a valid state that ANACAP allows, with no group above ANAGRPMAX. Without
//! CMIC bit 3 the log page must be refused. Base 2.1 5.1.12.1.13 (Figures 223 and 224): one descriptor
//! per ANA group holding namespaces attached to this controller, NSIDs in ascending order; Figure 312
//! CMIC bit 3 (ANARS), ANACAP, ANAGRPMAX, NANAGRPID.

use super::compliance::{rev_case, status_case, Target, INVALID_FIELD, INVALID_LOG_PAGE, SUCCESS};
use super::controller::{self, Association, Controller};
use super::identify_cases::Facts;
use super::revisions as rev;
use super::DEFAULT_HOSTNQN;
use crate::compliance::{case, Case};
use crate::Result;

const LOG_ANA: u8 = 0x0c;
/// CMIC bit 3: the controller reports Asymmetric Namespace Access.
const CMIC_ANA: u8 = 0x08;
/// The log page buffer: a header, and per group a descriptor and its namespace IDs.
const LOG_LEN: usize = 4096;

/// One ANA group descriptor: its ID, state and namespaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub id: u32,
    pub state: u8,
    pub nsids: Vec<u32>,
}

/// Parses the ANA log page: a 16-byte header with the group count at bytes 8-9, then per group a
/// 32-byte descriptor (ANAGRPID, NNSIDS, change count, ANA state in byte 16 bits 3:0) and NNSIDS
/// namespace IDs of 4 bytes.
pub fn parse(d: &[u8]) -> std::result::Result<Vec<Group>, String> {
    let groups = u16::from_le_bytes([d[8], d[9]]) as usize;
    let mut out = Vec::new();
    let mut i = 16;
    for g in 0..groups {
        let desc = d
            .get(i..i + 32)
            .ok_or_else(|| format!("descriptor {g} runs past the page"))?;
        let id = u32::from_le_bytes(desc[0..4].try_into().unwrap());
        let n = u32::from_le_bytes(desc[4..8].try_into().unwrap()) as usize;
        let list = d
            .get(i + 32..i + 32 + 4 * n)
            .ok_or_else(|| format!("group {id} lists {n} namespaces past the page"))?;
        out.push(Group {
            id,
            state: desc[16] & 0x0f,
            nsids: list
                .chunks_exact(4)
                .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
                .collect(),
        });
        i += 32 + 4 * n;
    }
    Ok(out)
}

/// The ANA state names, and the ANACAP bit that says the controller may report each.
pub fn state_name(state: u8) -> &'static str {
    match state {
        0x1 => "optimized",
        0x2 => "non-optimized",
        0x3 => "inaccessible",
        0x4 => "persistent loss",
        0xf => "change",
        _ => "reserved",
    }
}

fn anacap_bit(state: u8) -> Option<u8> {
    match state {
        0x1 => Some(0x01),
        0x2 => Some(0x02),
        0x3 => Some(0x04),
        0x4 => Some(0x08),
        0xf => Some(0x10),
        _ => None,
    }
}

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
        let groups = match parse(&buf) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_log_page_lists_groups_and_their_namespaces() {
        let mut d = vec![0u8; 256];
        d[8] = 2;
        // group 1: optimized, namespaces 1 and 2
        d[16..20].copy_from_slice(&1u32.to_le_bytes());
        d[20..24].copy_from_slice(&2u32.to_le_bytes());
        d[32] = 0x1;
        d[48..52].copy_from_slice(&1u32.to_le_bytes());
        d[52..56].copy_from_slice(&2u32.to_le_bytes());
        // group 2: inaccessible, no namespaces
        d[56..60].copy_from_slice(&2u32.to_le_bytes());
        d[72] = 0x3;
        let g = parse(&d).unwrap();
        assert_eq!(g.len(), 2);
        assert_eq!(
            g[0],
            Group {
                id: 1,
                state: 1,
                nsids: vec![1, 2]
            }
        );
        assert_eq!(g[1].state, 3);
        assert_eq!(anacap_bit(3), Some(0x04));
    }

    #[test]
    fn a_group_past_the_page_is_an_error() {
        let mut d = vec![0u8; 40];
        d[8] = 1;
        d[20..24].copy_from_slice(&100u32.to_le_bytes());
        assert!(parse(&d).is_err());
    }
}
