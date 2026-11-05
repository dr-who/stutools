//! The nvme-disc cases of `--compliance` (Base 2.1 Discovery log page, 5.1.12.3.1): a host's discovery
//! controller connects, enables and returns a log page, and every record in it names the TCP
//! transport, an address, a port, a subsystem NQN and a defined SUBTYPE. Returns the records so the
//! driver can test each NVM subsystem they name.

use super::{discover_subsystems, Discovered};
use crate::compliance::{case, Case};

/// SUBTYPE in a discovery log record: an NVM subsystem (2), not a discovery controller (1 or 3).
pub const SUBTYPE_NVM: u8 = 2;

/// The discovery cases for one host, and the records its log page holds.
pub fn run(host: &str, cases: &mut Vec<Case>) -> Vec<Discovered> {
    match discover_subsystems(host) {
        Err(e) => {
            cases.push(case(
                "nvme-disc",
                format!("{host} log page"),
                "read".into(),
                e,
                false,
            ));
            Vec::new()
        }
        Ok(list) => {
            cases.push(case(
                "nvme-disc",
                format!("{host} log page"),
                "read".into(),
                format!("{} records", list.len()),
                true,
            ));
            for d in &list {
                let well_formed =
                    d.transport == "tcp" && !d.traddr.is_empty() && !d.trsvcid.is_empty();
                cases.push(case(
                    "nvme-disc",
                    format!("{host} record {}", d.subnqn),
                    "tcp, an address, a port and a subtype".into(),
                    format!(
                        "{} {}:{} subtype {}",
                        d.transport, d.traddr, d.trsvcid, d.subtype
                    ),
                    well_formed && !d.subnqn.is_empty() && (1..=3).contains(&d.subtype),
                ));
            }
            list
        }
    }
}
