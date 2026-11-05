//! The NVMe/TCP discovery log page (Base 2.1 5.1.12.3.1): connect to a host's discovery controller,
//! read the page, and return one record per entry: its SUBTYPE (referral, NVM subsystem or the
//! discovery controller itself), subsystem NQN, transport, address and port.

use super::controller::{self, Association, Controller};
use super::{host_address, Result, DEFAULT_HOSTNQN};

/// One record of the discovery log page: a subsystem and the address it is reachable on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discovered {
    /// SUBTYPE: 1 a referral to another discovery controller, 2 an NVM subsystem, 3 this discovery
    /// controller itself.
    pub subtype: u8,
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
            subtype: r[2],
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

fn ascii(b: &[u8]) -> String {
    String::from_utf8_lossy(b)
        .trim_end_matches(['\0', ' '])
        .to_string()
}
