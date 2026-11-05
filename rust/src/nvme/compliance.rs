//! The NVMe/TCP cases of `--compliance`. Every `--nvme` HOST is discovered (the discovery log page) and
//! each NVM subsystem it reports is tested, through the first address the log gives for it and, for
//! identity, ANA and multipath reservations, through every other one; HOST/NQN tests that one. Each
//! case names the NVMe revision that defines what it tests (`revisions`) and is judged against the
//! revision the controller's VS property reports. This file runs the sections in order; each lives in
//! its own file:
//! - nvme-disc (`discovery_cases`): the discovery log page and its records;
//! - nvme-conn, nvme-prop, nvme-sess (`fabric_cases`): Connect, CAP, VS and enabling, and repeated
//!   associations;
//! - nvme-ident (`identify_cases`): Identify Controller and Namespace, identification descriptors,
//!   and the same identity on every path;
//! - nvme-admin (`log_page_cases`, `feature_cases`, `admin_cases`): log pages, features, Abort, Keep
//!   Alive, Asynchronous Event Request;
//! - nvme-io (`io_cases`): reads and writes, FUA, errors, MDTS, Flush, Compare, Write Zeroes and
//!   Dataset Management, changing only blocks it writes back;
//! - nvme-ana (`ana_cases`): the ANA log page through every address;
//! - nvme-resv (`reservation_cases`): every reservation command and type with four hosts;
//! - windows (`windows_cases`): what Windows needs of the subsystem, from the results above.

use std::net::SocketAddr;

use super::controller::{self, Association};
use super::discovery_cases::SUBTYPE_NVM;
use super::{
    admin_cases, ana_cases, discovery_cases, fabric_cases, feature_cases, identify_cases, io_cases,
    log_page_cases, reservation_cases, windows_cases, DEFAULT_HOSTNQN,
};
use crate::compliance::{case, Case};
use crate::Result;

/// One NVM subsystem under test: its NQN and every address the discovery log gives for it, the
/// first of which runs every case.
pub struct Target {
    pub subnqn: String,
    pub addrs: Vec<SocketAddr>,
}

/// Adds the NVMe/TCP cases for the `--nvme` targets to `cases`.
pub fn cases(entries: &[String], cases: &mut Vec<Case>) -> Result<()> {
    let mut targets: Vec<Target> = Vec::new();
    let mut add =
        |subnqn: String, addr: SocketAddr| match targets.iter_mut().find(|t| t.subnqn == subnqn) {
            Some(t) if !t.addrs.contains(&addr) => t.addrs.push(addr),
            Some(_) => {}
            None => targets.push(Target {
                subnqn,
                addrs: vec![addr],
            }),
        };
    for entry in entries {
        if entry.contains('/') {
            let path = super::parse_path(entry)?;
            add(path.subnqn, path.addr);
            continue;
        }
        let records = discovery_cases::run(entry, cases);
        for r in records.into_iter().filter(|r| r.subtype == SUBTYPE_NVM) {
            let host = if r.traddr.contains(':') {
                format!("[{}]:{}", r.traddr, r.trsvcid)
            } else {
                format!("{}:{}", r.traddr, r.trsvcid)
            };
            add(r.subnqn, super::host_address(&host)?);
        }
    }
    for t in &targets {
        subsystem(t, cases);
    }
    Ok(())
}

/// Every section for one subsystem, in order. Each later section needs what Identify learned, so a
/// failed Connect, enable or Identify ends the subsystem after the association cases.
fn subsystem(t: &Target, cases: &mut Vec<Case>) {
    let addr = t.addrs[0];
    let subnqn = t.subnqn.as_str();
    let name = format!("{} at {addr}", subnqn.rsplit(':').next().unwrap_or(subnqn));
    let hostid = match controller::random_hostid() {
        Ok(id) => id,
        Err(e) => {
            cases.push(case(
                "nvme-conn",
                format!("{name} host ID"),
                "made".into(),
                e,
                false,
            ));
            return;
        }
    };
    let assoc = Association {
        addr,
        subnqn,
        hostnqn: DEFAULT_HOSTNQN,
        hostid,
    };
    let Some(mut c) = fabric_cases::connect(&assoc, &name, cases) else {
        return;
    };
    let Some(props) = fabric_cases::enable(&mut c, &name, cases) else {
        return;
    };
    let Some(facts) = identify_cases::run(&mut c, &name, t, props.cap, props.vs, cases) else {
        fabric_cases::sessions(&assoc, &name, cases);
        return;
    };
    log_page_cases::run(&mut c, &name, &facts, cases);
    feature_cases::run(&mut c, &name, hostid, &facts, cases);
    admin_cases::run(&mut c, &name, &facts, cases);
    drop(c);
    let io = io_cases::run(t, &name, &facts, cases);
    let ana = ana_cases::run(t, &name, &facts, cases);
    let resv = reservation_cases::run(t, &name, &facts, cases);
    fabric_cases::sessions(&assoc, &name, cases);
    admin_cases::async_event(addr, subnqn, &name, cases);
    windows_cases::run(&name, t, &facts, &io, ana.as_deref(), &resv, cases);
}
