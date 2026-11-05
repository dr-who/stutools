//! What `spit --nvme HOST` and `spit --nvme HOST/NQN` print without a workload: the subsystems a
//! host's discovery controller reports, with a row per namespace (size, block size, model, serial,
//! firmware), or the identity of one named subsystem and its namespaces.

use super::controller::{self, Association, Controller};
use super::discovery::discover_subsystems;
use super::{host_address, identify, parse_path, Result, DEFAULT_HOSTNQN};

/// `--nvmeof HOST` with no NQN: lists the subsystems the discovery controller reports, then connects to
/// each NVM subsystem and prints a row per namespace: size, block size, model, serial and firmware. A
/// discovery controller in the log page is listed as one; a subsystem that refuses says why.
pub fn discover(hosts: &[String]) -> Result<()> {
    let mut rows = vec![[
        "SUBSYSTEM",
        "ADDRESS",
        "NSID",
        "SIZE",
        "BLOCK",
        "MODEL",
        "SERIAL",
        "FIRMWARE",
        "NOTE",
    ]
    .map(String::from)
    .to_vec()];
    let mut first = None;
    for host in hosts {
        for d in discover_subsystems(host)? {
            let address = if d.traddr.contains(':') {
                format!("[{}]:{}", d.traddr, d.trsvcid)
            } else {
                format!("{}:{}", d.traddr, d.trsvcid)
            };
            let row = |nsid: &str,
                       size: String,
                       block: String,
                       c: Option<&identify::ControllerInfo>,
                       note: &str| {
                vec![
                    d.subnqn.clone(),
                    address.clone(),
                    nsid.to_string(),
                    size,
                    block,
                    c.map_or("-".into(), |c| c.model.clone()),
                    c.map_or("-".into(), |c| c.serial.clone()),
                    c.map_or("-".into(), |c| c.firmware.clone()),
                    note.to_string(),
                ]
            };
            if d.subtype != 2 {
                rows.push(row(
                    "-",
                    "-".into(),
                    "-".into(),
                    None,
                    "discovery controller",
                ));
                continue;
            }
            match namespaces(&address, &d.subnqn) {
                Ok((info, list)) if list.is_empty() => rows.push(row(
                    "-",
                    "-".into(),
                    "-".into(),
                    Some(&info),
                    "no active namespaces",
                )),
                Ok((info, list)) => {
                    for (nsid, ns) in list {
                        first.get_or_insert_with(|| format!("{address}/{}/{nsid}", d.subnqn));
                        rows.push(row(
                            &nsid.to_string(),
                            crate::iscsi::report::human_bytes(ns.blocks * ns.block_len as u64),
                            ns.block_len.to_string(),
                            Some(&info),
                            "",
                        ));
                    }
                }
                Err(e) => rows.push(row("-", "-".into(), "-".into(), None, &e)),
            }
        }
    }
    crate::iscsi::report::print_columns(&rows);
    if let Some(path) = first {
        println!("# run a workload: spit --nvme {path} -c rs0k4q32j4 -t 5");
    }
    Ok(())
}

/// Connects to a subsystem and identifies its controller and every active namespace.
fn namespaces(
    address: &str,
    subnqn: &str,
) -> Result<(
    identify::ControllerInfo,
    Vec<(u32, identify::NamespaceInfo)>,
)> {
    let assoc = Association {
        addr: host_address(address)?,
        subnqn,
        hostnqn: DEFAULT_HOSTNQN,
        hostid: controller::random_hostid()?,
    };
    let mut c = Controller::connect(&assoc)?;
    c.enable()?;
    let info = identify::parse_controller(&c.identify(identify::CNS_CONTROLLER, 0)?);
    let mut out = Vec::new();
    for nsid in identify::parse_active_namespaces(&c.identify(identify::CNS_ACTIVE_NAMESPACES, 0)?)
    {
        out.push((
            nsid,
            identify::parse_namespace(&c.identify(identify::CNS_NAMESPACE, nsid)?),
        ));
    }
    Ok((info, out))
}

/// Connects to a subsystem, enables the controller and identifies it and its namespaces.
fn describe(text: &str) -> Result<String> {
    let path = parse_path(text)?;
    let hostid = controller::random_hostid()?;
    let assoc = Association {
        addr: path.addr,
        subnqn: &path.subnqn,
        hostnqn: DEFAULT_HOSTNQN,
        hostid,
    };
    let mut c = Controller::connect(&assoc)?;
    c.enable()?;
    let info = identify::parse_controller(&c.identify(identify::CNS_CONTROLLER, 0)?);
    let mut out = format!(
        "{} {}\n  controller {:#06x}, model {}, serial {}, firmware {}\n  identified subsystem NQN {}\n  namespaces {}, MDTS {}\n",
        path.addr,
        path.subnqn,
        c.cntlid(),
        info.model,
        info.serial,
        info.firmware,
        info.nqn,
        info.namespaces,
        info.mdts
    );
    for nsid in identify::parse_active_namespaces(&c.identify(identify::CNS_ACTIVE_NAMESPACES, 0)?)
    {
        let ns = identify::parse_namespace(&c.identify(identify::CNS_NAMESPACE, nsid)?);
        out.push_str(&format!(
            "  namespace {nsid}: {} blocks of {} bytes ({} bytes)\n",
            ns.blocks,
            ns.block_len,
            ns.blocks * ns.block_len as u64
        ));
    }
    Ok(out)
}

/// Prints the identity of each subsystem named by `--nvmeof`.
pub fn report(targets: &[String]) -> Result<()> {
    for target in targets {
        print!("{}", describe(target)?);
    }
    Ok(())
}
