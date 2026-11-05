//! The multipath identity cases of the nvme-ident section: a second association to the first address,
//! and every other address of the subsystem, must report the identity the first controller does:
//! the serial number, model and subsystem NQN, which name the NVM subsystem (Base 2.1 Figure 312),
//! and each namespace's unique IDs, which are fixed for its life (Figure 315). Windows MPIO groups
//! paths by these, so a path that differs is a separate disk to it.

use std::net::SocketAddr;

use super::compliance::Target;
use super::controller::{self, Association, Controller};
use super::identify::{self, parse_descriptors, unique_ids, Descriptor};
use super::DEFAULT_HOSTNQN;
use crate::compliance::{case, Case};
use crate::Result;

/// The identity one controller reports: serial, model, subsystem NQN, and each namespace's IDs.
#[derive(Debug, PartialEq)]
struct PathIdentity {
    serial: String,
    model: String,
    nqn: String,
    namespaces: Vec<(u32, Vec<Descriptor>)>,
}

fn path_identity(c: &mut Controller) -> Result<PathIdentity> {
    let info = identify::parse_controller(&c.identify(identify::CNS_CONTROLLER, 0)?);
    let active =
        identify::parse_active_namespaces(&c.identify(identify::CNS_ACTIVE_NAMESPACES, 0)?);
    let mut namespaces = Vec::new();
    for nsid in active {
        let list = parse_descriptors(&c.identify(identify::CNS_NS_DESCRIPTORS, nsid)?)?;
        namespaces.push((nsid, unique_ids(&list)));
    }
    Ok(PathIdentity {
        serial: info.serial,
        model: info.model,
        nqn: info.nqn,
        namespaces,
    })
}

fn connect_path(addr: SocketAddr, subnqn: &str) -> Result<Controller> {
    let assoc = Association {
        addr,
        subnqn,
        hostnqn: DEFAULT_HOSTNQN,
        hostid: controller::random_hostid()?,
    };
    let mut c = Controller::connect(&assoc)?;
    c.enable()?;
    Ok(c)
}

/// Compares every other path with the enabled controller `c`. Returns whether all agree.
pub fn run(c: &mut Controller, name: &str, t: &Target, cases: &mut Vec<Case>) -> bool {
    let mut same = true;
    // Every other path: a second association to this address, and every other address of the
    // subsystem. Each must report the identity this controller does (Base 2.1 Figure 312: SN, MN and
    // SUBNQN name the NVM subsystem; Figure 315: a NID is fixed for the namespace's life).
    let here = path_identity(c);
    let mut others: Vec<(String, SocketAddr)> =
        vec![(format!("second association at {}", t.addrs[0]), t.addrs[0])];
    others.extend(t.addrs[1..].iter().map(|&a| (format!("path {a}"), a)));
    for (label, addr) in others {
        let there = connect_path(addr, &t.subnqn).and_then(|mut o| path_identity(&mut o));
        let (actual, passed) = match (&here, &there) {
            (Ok(h), Ok(o)) if h == o => ("the same".to_string(), true),
            (Ok(h), Ok(o)) => {
                let mut diff = Vec::new();
                if h.serial != o.serial {
                    diff.push(format!("serial {:?} vs {:?}", h.serial, o.serial));
                }
                if h.model != o.model {
                    diff.push(format!("model {:?} vs {:?}", h.model, o.model));
                }
                if h.nqn != o.nqn {
                    diff.push(format!("NQN {} vs {}", h.nqn, o.nqn));
                }
                if h.namespaces != o.namespaces {
                    diff.push("namespace IDs differ".into());
                }
                (diff.join(", "), false)
            }
            (Err(e), _) | (_, Err(e)) => (e.clone(), false),
        };
        same &= passed;
        cases.push(case(
            "nvme-ident",
            format!("{name} {label}"),
            "the same serial, model, NQN and namespace IDs".into(),
            actual,
            passed,
        ));
    }
    same
}
