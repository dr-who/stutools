//! The multi-path machinery shared by every block transport. A workload over N paths picks a path per
//! request with a policy (`rr`, `rrr`, `random`, `fastest`), keeps per-path counts, and fails over when a
//! path drops. iSCSI portals and NVMe-oF controllers are both paths: each transport implements `Fabric`,
//! which opens a path, and `Transport`, which runs one session.

pub mod any;
pub mod lanes;
pub mod pool;
pub mod stats;

pub use any::{open_paths, MultiPool, PathSpec};
pub use lanes::Policy;
pub use pool::{Done, Fabric, Geometry, Io, Pool, Transport, SENSE_CAPACITY};
pub use stats::{print_totals, set_dump};

/// Expands `TARGET,N` into N copies of the target, one per connection. A target without a count is one
/// connection. The count is written after the last comma, which no IQN or NQN contains.
pub fn expand_connections(text: &str) -> Result<Vec<String>, String> {
    let (target, count) = match text.rsplit_once(',') {
        Some((target, count)) => {
            let n: usize = count.parse().map_err(|_| {
                format!("{text}: the connection count after the comma is not a number")
            })?;
            (target, n)
        }
        None => (text, 1),
    };
    if !(1..=64).contains(&count) {
        return Err(format!(
            "{text}: a target takes 1 to 64 connections, not {count}"
        ));
    }
    Ok(vec![target.to_string(); count])
}
