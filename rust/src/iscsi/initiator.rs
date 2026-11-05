//! The initiator name spit logs in with when --iscsi-initiator is not given: the host's own
//! (`InitiatorName=` in /etc/iscsi/initiatorname.iscsi, the name open-iscsi uses, so a target that
//! admits this host admits spit), else one made from the host name.

use std::path::Path;

const OPEN_ISCSI: &str = "/etc/iscsi/initiatorname.iscsi";

/// The name to log in with and where it came from: the host's open-iscsi name, else
/// `iqn.2026-11.spit:HOST`.
pub fn default_name() -> (String, &'static str) {
    match from_file(Path::new(OPEN_ISCSI)) {
        Some(name) => (name, OPEN_ISCSI),
        None => (format!("iqn.2026-11.spit:{}", host_name()), "the host name"),
    }
}

/// The `InitiatorName=` value in an open-iscsi name file, if it has one.
fn from_file(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .find_map(|l| l.trim().strip_prefix("InitiatorName="))
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// This machine's name, lower case and limited to the characters an IQN allows, or "host".
fn host_name() -> String {
    let raw = ["/proc/sys/kernel/hostname", "/etc/hostname"]
        .iter()
        .find_map(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default();
    let name: String = raw
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.'))
        .collect();
    if name.is_empty() {
        "host".into()
    } else {
        name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(text: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "spit-initiator-{}-{}",
            std::process::id(),
            text.len()
        ));
        std::fs::write(&p, text).unwrap();
        p
    }

    #[test]
    fn reads_the_open_iscsi_name() {
        let p = file("## generated\n#InitiatorName=iqn.old:x\nInitiatorName=iqn.1994-05.com.redhat:4fdc297d15fd\n");
        assert_eq!(
            from_file(&p).as_deref(),
            Some("iqn.1994-05.com.redhat:4fdc297d15fd")
        );
        std::fs::remove_file(p).unwrap();
    }

    #[test]
    fn a_file_without_a_name_gives_none() {
        let p = file("# nothing here\n");
        assert_eq!(from_file(&p), None);
        std::fs::remove_file(p).unwrap();
        assert_eq!(
            from_file(Path::new("/nonexistent/initiatorname.iscsi")),
            None
        );
    }

    #[test]
    fn the_default_is_an_iqn() {
        let (name, from) = default_name();
        assert!(name.starts_with("iqn."), "{name}");
        assert!(!from.is_empty());
    }
}
