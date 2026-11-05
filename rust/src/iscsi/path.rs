//! One iSCSI path: the portal to connect to and the target name, from `HOST[:PORT]/IQN`. An IQN never
//! contains a slash, so the first slash separates the two; the port defaults to 3260 at login.

use crate::Result;

/// One target path: the portal to connect to and the target name.
#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    pub portal: String,
    pub target: String,
}

/// Parses `HOST[:PORT]/IQN`. The IQN never contains a slash, so the first slash separates the two.
pub fn parse_path(text: &str) -> Result<Path> {
    let (portal, target) = text
        .split_once('/')
        .ok_or_else(|| format!("--iscsi takes HOST[:PORT]/IQN, not {text}"))?;
    if portal.is_empty() || !target.starts_with("iqn.") {
        return Err(format!("--iscsi takes HOST[:PORT]/IQN, not {text}"));
    }
    Ok(Path {
        portal: portal.to_string(),
        target: target.to_string(),
    })
}
