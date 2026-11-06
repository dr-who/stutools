//! One iSCSI path: the portal to connect to, the target name and the logical unit, from
//! `HOST[:PORT]/IQN[/LUN]`. An IQN never contains a slash, so slashes separate the three; the port
//! defaults to 3260 at login and the LUN to 0. A LUN other than 0 is for targets that keep LUN 0 for
//! themselves (tgt's controller LUN).

use crate::Result;

/// One target path: the portal to connect to, the target name and the logical unit.
#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    pub portal: String,
    pub target: String,
    pub lun: u16,
}

impl Path {
    /// The 8-byte LUN field of a PDU (SAM-6 4.7.7): peripheral device addressing below 256, flat space
    /// addressing above.
    pub fn lun_field(&self) -> u64 {
        let first_level = if self.lun < 256 {
            self.lun
        } else {
            0x4000 | (self.lun & 0x3fff)
        };
        u64::from(first_level) << 48
    }
}

/// Parses `HOST[:PORT]/IQN[/LUN]`. The IQN never contains a slash, so slashes separate the parts.
pub fn parse_path(text: &str) -> Result<Path> {
    let refused = || format!("--iscsi takes HOST[:PORT]/IQN[/LUN], not {text}");
    let (portal, rest) = text.split_once('/').ok_or_else(refused)?;
    let (target, lun) = match rest.split_once('/') {
        Some((target, lun)) => (target, lun.parse::<u16>().map_err(|_| refused())?),
        None => (rest, 0),
    };
    if portal.is_empty() || !target.starts_with("iqn.") {
        return Err(refused());
    }
    Ok(Path {
        portal: portal.to_string(),
        target: target.to_string(),
        lun,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_may_name_a_logical_unit() {
        let p = parse_path("10.0.0.5:3261/iqn.2026-11.x:disk/1").unwrap();
        assert_eq!(
            (p.portal.as_str(), p.target.as_str(), p.lun),
            ("10.0.0.5:3261", "iqn.2026-11.x:disk", 1)
        );
        assert_eq!(p.lun_field(), 0x0001_0000_0000_0000);
        assert_eq!(parse_path("h/iqn.a").unwrap().lun, 0);
        let big = Path {
            lun: 300,
            ..p.clone()
        };
        assert_eq!(big.lun_field(), 0x412c_0000_0000_0000);
        assert!(parse_path("h/iqn.a/x").is_err());
        assert!(parse_path("h/iqn.a/70000").is_err());
    }
}
