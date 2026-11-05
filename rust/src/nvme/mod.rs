//! A user-space NVMe-over-Fabrics initiator for spit, over TCP (NVMe/TCP). It speaks the protocol
//! directly, so spit can connect to a subsystem without the kernel's nvme driver.

pub mod controller;
pub mod link;
pub mod pdu;
pub mod session;
pub mod sqe;

use std::net::{SocketAddr, ToSocketAddrs};

use controller::{Association, Controller};

pub type Result<T> = std::result::Result<T, String>;

/// The NVMe/TCP port when a target names none.
pub const DEFAULT_PORT: u16 = 4420;

/// The host NQN spit presents when none is given.
pub const DEFAULT_HOSTNQN: &str = "nqn.2026-11.com.stutools:spit-initiator";

/// Splits `HOST[:PORT]/NQN[/NSID]` into the socket address, the subsystem NQN and the namespace.
pub fn parse_path(text: &str) -> Result<session::NvmePath> {
    let mut parts = text.splitn(3, '/');
    let host = parts.next().unwrap_or_default();
    let nqn = parts
        .next()
        .filter(|n| !n.is_empty())
        .ok_or_else(|| format!("--nvmeof takes HOST[:PORT]/NQN[/NSID], not {text}"))?;
    let nsid = match parts.next() {
        Some(n) => Some(
            n.parse::<u32>()
                .map_err(|_| format!("--nvmeof {text}: the namespace ID is not a number"))?,
        ),
        None => None,
    };
    let addr = host_address(host)?;
    Ok(session::NvmePath {
        addr,
        subnqn: nqn.to_string(),
        nsid,
    })
}

/// The socket address of `host`, with the NVMe/TCP port unless the host names one.
fn host_address(host: &str) -> Result<SocketAddr> {
    let with_port = if host.matches(':').count() > 1 && !host.starts_with('[') {
        format!("[{host}]:{DEFAULT_PORT}")
    } else if host.contains(':') {
        host.to_string()
    } else {
        format!("{host}:{DEFAULT_PORT}")
    };
    with_port
        .to_socket_addrs()
        .map_err(|e| format!("--nvmeof {host}: {e}"))?
        .next()
        .ok_or_else(|| format!("--nvmeof {host}: no address"))
}

/// One record of the discovery log page: a subsystem and the address it is reachable on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discovered {
    pub subnqn: String,
    pub transport: String,
    pub traddr: String,
    pub trsvcid: String,
}

/// Connects to a discovery controller and reads its log page: one record per subsystem it reports.
pub fn discover_subsystems(host: &str) -> Result<Vec<Discovered>> {
    let addr = host_address(host)?;
    let hostid = controller::random_hostid()?;
    let assoc = Association {
        addr,
        subnqn: controller::DISCOVERY_NQN,
        hostnqn: DEFAULT_HOSTNQN,
        hostid,
    };
    let mut c = Controller::connect(&assoc)?;
    c.enable()?;
    let header = c.get_log_page(controller::LOG_DISCOVERY, 0, 1024)?;
    let records = u64::from_le_bytes(header[8..16].try_into().unwrap_or([0; 8])) as usize;
    let log = c.get_log_page(controller::LOG_DISCOVERY, 0, 1024 + records * 1024)?;
    Ok(log[1024..]
        .chunks_exact(1024)
        .map(|r| Discovered {
            transport: match r[0] {
                3 => "tcp".into(),
                other => format!("transport {other}"),
            },
            traddr: ascii(&r[512..768]),
            trsvcid: ascii(&r[32..64]),
            subnqn: ascii(&r[256..512]),
        })
        .collect())
}

/// `--nvmeof HOST` with no NQN: lists the subsystems the discovery controller reports.
pub fn discover(hosts: &[String]) -> Result<()> {
    for host in hosts {
        println!("{host}");
        for d in discover_subsystems(host)? {
            println!(
                "  {}\n    {} {}:{}",
                d.subnqn, d.transport, d.traddr, d.trsvcid
            );
        }
    }
    Ok(())
}

fn ascii(b: &[u8]) -> String {
    String::from_utf8_lossy(b)
        .trim_end_matches(['\0', ' '])
        .to_string()
}

/// Connects to a subsystem, enables the controller and identifies it and its namespaces.
pub fn identify(text: &str) -> Result<String> {
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
    let info = controller::parse_controller(&c.identify(controller::CNS_CONTROLLER, 0)?);
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
    for nsid in
        controller::parse_active_namespaces(&c.identify(controller::CNS_ACTIVE_NAMESPACES, 0)?)
    {
        let ns = controller::parse_namespace(&c.identify(controller::CNS_NAMESPACE, nsid)?);
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
        print!("{}", identify(target)?);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_defaults_the_port() {
        let p = parse_path("127.0.0.1/nqn.2026-11.com.stutools:spit-nvme").unwrap();
        assert_eq!(p.addr.port(), DEFAULT_PORT);
        assert_eq!(p.subnqn, "nqn.2026-11.com.stutools:spit-nvme");
        assert_eq!(p.nsid, None);
    }

    #[test]
    fn target_takes_a_namespace() {
        let p = parse_path("127.0.0.1/nqn.example:x/3").unwrap();
        assert_eq!(p.nsid, Some(3));
        assert!(parse_path("127.0.0.1/nqn.example:x/three").is_err());
    }

    #[test]
    fn target_takes_an_explicit_port() {
        let p = parse_path("127.0.0.1:4421/nqn.example:x").unwrap();
        assert_eq!(p.addr.port(), 4421);
    }

    #[test]
    fn target_needs_a_subsystem() {
        assert!(parse_path("127.0.0.1").is_err());
        assert!(parse_path("127.0.0.1/").is_err());
    }
}
