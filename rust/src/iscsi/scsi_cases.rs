//! The `scsi` cases of `--compliance`: on each reachable path the identity commands answer (INQUIRY,
//! VPD 80h and 83h, READ CAPACITY (16), REPORT TARGET PORT GROUPS; SPC-6 6.7, 6.34; SBC-5 5.21), the
//! capacity is sane (blocks of 512 bytes or more, a power of two), and the answering port is in a group
//! REPORT TARGET PORT GROUPS lists. Returns, per path, whether that port is in the standby state, for
//! the SCSI sections to test it as such.

use super::identity::Collected;
use super::login::reason;
use super::login_cases::Reachable;
use super::lun_access::short_name;
use crate::compliance::{case, Case};
use crate::Result;

/// Runs the scsi cases over what `collected` read from each reachable path.
pub(super) fn run(
    reachable: &[Reachable],
    collected: &[Result<Collected>],
    cases: &mut Vec<Case>,
) -> Vec<Option<u8>> {
    let mut states: Vec<Option<u8>> = vec![None; reachable.len()];
    for (i, (r, result)) in reachable.iter().zip(collected).enumerate() {
        let name = short_name(r.path);
        let c = match result {
            Err(err) => {
                // A port in a limited state refuses the identity commands with its own sense code (SPC-3 5.8.2.4):
                // that refusal is the state, and the case is about the refusal, not a failure.
                states[i] = state_from_refusal(err);
                cases.push(case(
                    "scsi",
                    format!("{name} commands"),
                    "all answer".into(),
                    format!("failed: {}", reason(err)),
                    states[i].is_some(),
                ));
                continue;
            }
            Ok(c) => c,
        };
        cases.push(case(
            "scsi",
            format!("{name} INQUIRY, VPD 0x80/0x83, READ CAPACITY(16), REPORT TPG"),
            "all answer".into(),
            "all answer".into(),
            true,
        ));
        let size_ok = c.last_lba > 0 && c.block_len >= 512 && c.block_len.is_power_of_two();
        cases.push(case(
            "scsi",
            format!("{name} capacity"),
            "blocks > 0 of 512 B or more, a power of two".into(),
            format!("{} blocks of {} B", c.last_lba + 1, c.block_len),
            size_ok,
        ));
        let own = c
            .own_port
            .and_then(|port| c.groups.iter().find(|g| g.ports.contains(&port)))
            .map(|g| super::scsi::state_name(g.state).to_string());
        states[i] = own.as_deref().map(|name| match name {
            "active/optimized" => 0,
            "active/non-optimized" => 1,
            "standby" => 2,
            "unavailable" => 3,
            _ => 0xff,
        });
        // SPC-6 6.7.2: TPGS 0 means no target port groups, so there is no state to find.
        if c.inquiry.tpgs == 0 {
            cases.push(crate::compliance::info(
                "scsi",
                format!("{name} ALUA state"),
                "no target port groups (TPGS 0)".into(),
            ));
        } else {
            cases.push(case(
                "scsi",
                format!("{name} ALUA state"),
                "its port in a listed group".into(),
                own.clone().unwrap_or_else(|| "port not listed".into()),
                own.is_some(),
            ));
        }
    }

    states
}

/// The ALUA state a refusal names: NOT READY with ASC 04h and ASCQ 0Ah (transitioning), 0Bh (standby) or
/// 0Ch (unavailable), read from the sense bytes in the error text (fixed format: ASC at byte 12, ASCQ at 13).
pub(super) fn state_from_refusal(err: &str) -> Option<u8> {
    let hex: String = err
        .split("sense ")
        .nth(1)?
        .chars()
        .take_while(|c| c.is_ascii_hexdigit())
        .collect();
    if hex.len() < 28 {
        return None;
    }
    let asc = u8::from_str_radix(&hex[24..26], 16).ok()?;
    let ascq = u8::from_str_radix(&hex[26..28], 16).ok()?;
    if asc != 0x04 {
        return None;
    }
    match ascq {
        0x0a => Some(0x0f),
        0x0b => Some(2),
        0x0c => Some(3),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::state_from_refusal;

    #[test]
    fn a_limited_state_is_read_from_its_refusal() {
        let text = "failed: SCSI status 0x02, sense 700002000000000a00000000040c00000000";
        assert_eq!(state_from_refusal(text), Some(3));
        assert_eq!(state_from_refusal(&text.replace("040c", "040b")), Some(2));
        assert_eq!(
            state_from_refusal(&text.replace("040c", "040a")),
            Some(0x0f)
        );
        assert_eq!(state_from_refusal(&text.replace("040c", "0000")), None);
        assert_eq!(state_from_refusal("no sense here"), None);
    }
}
