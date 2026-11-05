//! A user-space NVMe-over-Fabrics initiator for spit, over TCP (NVMe/TCP). It speaks the protocol
//! directly, so spit can connect to a subsystem without the kernel's nvme driver.

mod admin_cases;
mod ana_cases;
mod ana_log;
pub mod compliance;
pub mod controller;
mod discovery;
mod discovery_cases;
mod fabric_cases;
mod feature_cases;
mod host;
pub mod identify;
mod identify_cases;
mod io_cases;
mod io_limit_cases;
pub mod link;
mod listing;
mod log_page_cases;
mod nvm;
mod oncs_cases;
mod path_identity_cases;
pub mod pdu;
mod reservation;
mod reservation_cases;
mod reservation_hosts;
mod revisions;
pub mod session;
pub mod sqe;
mod verdict;
mod windows_cases;

use std::net::{SocketAddr, ToSocketAddrs};

pub use discovery::{discover_subsystems, Discovered};
pub use listing::{discover, report};

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
