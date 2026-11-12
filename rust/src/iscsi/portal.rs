//! A portal: the address a target listens on, as `HOST`, `HOST:PORT`, `IPv4:PORT`, `IPv6` or `[IPv6]:PORT`.
//! Two spellings of the same portal (`10.0.0.1` and `10.0.0.1:3260`) are one portal, so every comparison
//! goes through this type, and the default iSCSI port (3260) is applied here and nowhere else.

use std::fmt;

/// The iSCSI port used when a portal names none (RFC 7143 3.1).
pub const DEFAULT_PORT: u16 = 3260;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Portal {
    host: String,
    port: u16,
}

impl Portal {
    /// Parses a portal, adding the default port when none is given. An unbracketed IPv6 address has
    /// several colons and no port; a bracketed one takes `:PORT` after `]`.
    pub fn parse(text: &str) -> Result<Portal, String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("empty portal address".into());
        }
        if let Some(rest) = text.strip_prefix('[') {
            let (host, tail) = rest
                .split_once(']')
                .ok_or_else(|| format!("{text}: unclosed bracket"))?;
            let port = match tail {
                "" => DEFAULT_PORT,
                t => parse_port(
                    t.strip_prefix(':')
                        .ok_or_else(|| format!("{text}: junk after ]"))?,
                    text,
                )?,
            };
            return Ok(Portal {
                host: host.to_string(),
                port,
            });
        }
        match text.matches(':').count() {
            0 => Ok(Portal {
                host: text.to_string(),
                port: DEFAULT_PORT,
            }),
            1 => {
                let (host, port) = text.split_once(':').unwrap_or((text, ""));
                if host.is_empty() {
                    return Err(format!("{text}: no host"));
                }
                Ok(Portal {
                    host: host.to_string(),
                    port: parse_port(port, text)?,
                })
            }
            _ => Ok(Portal {
                host: text.to_string(),
                port: DEFAULT_PORT,
            }),
        }
    }
}

fn parse_port(text: &str, whole: &str) -> Result<u16, String> {
    text.parse::<u16>()
        .ok()
        .filter(|p| *p != 0)
        .ok_or_else(|| format!("{whole}: not a port"))
}

impl fmt::Display for Portal {
    /// Always `HOST:PORT`, with IPv6 hosts bracketed, so the text is a valid target address.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.host.contains(':') {
            write!(f, "[{}]:{}", self.host, self.port)
        } else {
            write!(f, "{}:{}", self.host, self.port)
        }
    }
}

/// Whether two addresses name the same portal.
pub fn same(a: &str, b: &str) -> bool {
    match (Portal::parse(a), Portal::parse(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_port_is_applied_once_here() {
        assert_eq!(
            Portal::parse("10.0.0.1").unwrap().to_string(),
            "10.0.0.1:3260"
        );
        assert_eq!(
            Portal::parse("10.0.0.1:3260").unwrap().to_string(),
            "10.0.0.1:3260"
        );
        assert_eq!(
            Portal::parse("host.example").unwrap().to_string(),
            "host.example:3260"
        );
        assert_eq!(Portal::parse("::1").unwrap().to_string(), "[::1]:3260");
        assert_eq!(
            Portal::parse("fe80::1:2").unwrap().to_string(),
            "[fe80::1:2]:3260"
        );
        assert_eq!(Portal::parse("[::1]").unwrap().to_string(), "[::1]:3260");
        assert_eq!(
            Portal::parse("[::1]:3261").unwrap().to_string(),
            "[::1]:3261"
        );
        assert_eq!(
            Portal::parse("10.0.0.1:3261").unwrap().to_string(),
            "10.0.0.1:3261"
        );
    }

    #[test]
    fn two_spellings_of_one_portal_are_the_same_portal() {
        assert!(same("127.0.0.2", "127.0.0.2:3260"));
        assert!(same("[::1]", "[::1]:3260"));
        assert!(!same("127.0.0.2", "127.0.0.3"));
        assert!(!same("127.0.0.2:3260", "127.0.0.2:3261"));
    }

    #[test]
    fn bad_portals_are_refused_not_guessed() {
        assert!(Portal::parse("").is_err());
        assert!(Portal::parse(":3260").is_err());
        assert!(Portal::parse("10.0.0.1:0").is_err());
        assert!(Portal::parse("10.0.0.1:70000").is_err());
        assert!(Portal::parse("10.0.0.1:port").is_err());
        assert!(Portal::parse("[::1").is_err());
        assert!(Portal::parse("[::1]x").is_err());
    }
}
