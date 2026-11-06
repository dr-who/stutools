//! Which path device of a local disk is which I_T nexus. A dm-multipath device's paths (its sysfs
//! slaves) differ by initiator port (the host adapter, `hostN`) and target port (the disk port's SAS
//! address); each pair is one I_T nexus. The `--compliance` cases name nexuses as iSCSI names them,
//! by initiator name and ISID, so the first initiator name takes the first host adapter and the
//! second the next, and A's second path, C and the sibling port take the disk's second target port
//! where it has one. A disk that is not multipath, or has one path, is one nexus for every name.
//! Fails only when sysfs cannot be read for a device that has slaves.

use crate::Result;
use std::fs;

/// The initiator names a local run uses for its two initiator ports.
pub const FIRST: &str = "local initiator port 1";
pub const SECOND: &str = "local initiator port 2";

/// One path device and the nexus it is.
#[derive(Debug, Clone, PartialEq)]
struct PathDevice {
    device: String,
    host: String,
    target_port: String,
}

fn kernel_name(device: &str) -> Result<String> {
    let real = fs::canonicalize(device).map_err(|e| format!("{device}: {e}"))?;
    Ok(real
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default())
}

fn paths(device: &str) -> Result<Vec<PathDevice>> {
    let name = kernel_name(device)?;
    let Ok(slaves) = fs::read_dir(format!("/sys/block/{name}/slaves")) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for s in slaves.flatten() {
        let sd = s.file_name().to_string_lossy().into_owned();
        let link = fs::canonicalize(format!("/sys/block/{sd}/device"))
            .map_err(|e| format!("/sys/block/{sd}/device: {e}"))?;
        let host = link
            .iter()
            .map(|c| c.to_string_lossy().into_owned())
            .find(|c| c.starts_with("host"))
            .unwrap_or_default();
        let target_port = fs::read_to_string(format!("/sys/block/{sd}/device/sas_address"))
            .map(|a| a.trim().to_string())
            .unwrap_or_default();
        out.push(PathDevice {
            device: format!("/dev/{sd}"),
            host,
            target_port,
        });
    }
    out.sort_by(|a, b| a.device.cmp(&b.device));
    Ok(out)
}

/// The path of `paths` for initiator port `initiator` (0 or 1) and target port `port` (0 or 1): the
/// adapters and the disk's ports in sorted order, the nearest there is when the disk has fewer.
fn choose(paths: &[PathDevice], initiator: usize, port: usize) -> Option<&PathDevice> {
    let mut hosts: Vec<&str> = paths.iter().map(|p| p.host.as_str()).collect();
    hosts.sort_unstable();
    hosts.dedup();
    let mut ports: Vec<&str> = paths.iter().map(|p| p.target_port.as_str()).collect();
    ports.sort_unstable();
    ports.dedup();
    let host = hosts.get(initiator.min(hosts.len().saturating_sub(1)))?;
    let target = ports.get(port.min(ports.len().saturating_sub(1)))?;
    paths
        .iter()
        .find(|p| p.host == *host && p.target_port == *target)
        .or_else(|| paths.iter().find(|p| p.host == *host))
}

/// The device to open for the nexus `initiator` (`FIRST` or `SECOND`) names on its second target
/// port when `second_port`: a path of the multipath device, or the device itself.
pub fn device_for(device: &str, initiator: &str, second_port: bool) -> Result<String> {
    let all = paths(device)?;
    let initiator = usize::from(initiator == SECOND);
    Ok(choose(&all, initiator, usize::from(second_port))
        .map_or_else(|| device.to_string(), |p| p.device.clone()))
}

/// Every path of the device, as "/dev/sdX (hostN to port ADDRESS)", or the device itself.
pub fn all_paths(device: &str) -> Result<Vec<String>> {
    let all = paths(device)?;
    if all.is_empty() {
        return Ok(vec![device.to_string()]);
    }
    Ok(all
        .iter()
        .map(|p| format!("{} ({} to port {})", p.device, p.host, p.target_port))
        .collect())
}

/// Every path device of the device, or none when it is not multipath.
pub fn path_devices(device: &str) -> Result<Vec<String>> {
    Ok(paths(device)?.into_iter().map(|p| p.device).collect())
}

/// How many initiator ports reach the device: its host adapters, or one.
pub fn initiator_ports(device: &str) -> Result<usize> {
    let all = paths(device)?;
    let mut hosts: Vec<&str> = all.iter().map(|p| p.host.as_str()).collect();
    hosts.sort_unstable();
    hosts.dedup();
    Ok(hosts.len().max(1))
}

/// How many target ports of the disk its paths reach: the distinct SAS addresses, or one.
pub fn target_ports(device: &str) -> Result<usize> {
    let all = paths(device)?;
    let mut ports: Vec<&str> = all.iter().map(|p| p.target_port.as_str()).collect();
    ports.sort_unstable();
    ports.dedup();
    Ok(ports.len().max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(device: &str, host: &str, port: &str) -> PathDevice {
        PathDevice {
            device: device.into(),
            host: host.into(),
            target_port: port.into(),
        }
    }

    #[test]
    fn each_initiator_and_target_port_is_one_path() {
        let all = [
            p("/dev/sde", "host0", "0x75"),
            p("/dev/sdx", "host15", "0x75"),
            p("/dev/sdiw", "host0", "0x76"),
            p("/dev/sdik", "host15", "0x76"),
        ];
        assert_eq!(choose(&all, 0, 0).unwrap().device, "/dev/sde");
        assert_eq!(choose(&all, 0, 1).unwrap().device, "/dev/sdiw");
        assert_eq!(choose(&all, 1, 0).unwrap().device, "/dev/sdx");
        assert_eq!(choose(&all, 1, 1).unwrap().device, "/dev/sdik");
        let one_adapter = [p("/dev/sda", "host0", "0x75")];
        assert_eq!(choose(&one_adapter, 1, 1).unwrap().device, "/dev/sda");
        assert!(choose(&[], 0, 0).is_none());
    }
}
