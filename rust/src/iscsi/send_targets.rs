//! iSCSI discovery (RFC 7143 4.3, 13.4 and 13.21): a discovery session to a portal, the text request
//! `SendTargets=All`, and the targets its answer names with every address each is reachable on.

use super::login::{connect_stream, exchange, random_isid, LOGIN_CMD_SN, LOGIN_ITT};
use super::pdu::{self, LoginRequest, Stage};
use crate::Result;
use std::io::{Read, Write};

/// A target a discovery session reports: its name and the portal addresses it is reachable on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discovered {
    pub target: String,
    pub addresses: Vec<String>,
}

/// Logs in to `portal` as a discovery session and asks it for its targets with `SendTargets=All`. A
/// discovery session names no target and offers no digests. Targets that need CHAP to see are not listed.
pub fn discover_targets(portal: &str, initiator: &str) -> Result<Vec<Discovered>> {
    let mut stream = connect_stream(portal)?;
    let (response, raw) = exchange(
        &mut stream,
        &LoginRequest {
            isid: random_isid(),
            tsih: 0,
            cid: 0,
            itt: LOGIN_ITT,
            cmd_sn: LOGIN_CMD_SN,
            exp_stat_sn: 0,
            current: Stage::Operational,
            next: Stage::FullFeature,
            transit: true,
            text: discovery_keys(initiator),
        },
    )?;
    if !response.transit || response.next != Stage::FullFeature as u8 {
        return Err("discovery did not reach full feature phase".into());
    }
    let seq = pdu::sequence(&raw);
    let mut cmd_sn = seq.exp_cmd_sn;
    let mut exp_stat_sn = seq.stat_sn.wrapping_add(1);
    let mut ttt = pdu::NO_TTT;
    let mut text = Vec::new();
    loop {
        let body: &[u8] = if ttt == pdu::NO_TTT {
            b"SendTargets=All\0"
        } else {
            &[]
        };
        let header = pdu::text_request(LOGIN_ITT, ttt, cmd_sn, exp_stat_sn, body.len());
        stream
            .write_all(&header)
            .and_then(|_| stream.write_all(body))
            .and_then(|_| stream.write_all(&[0u8; 3][..(4 - body.len() % 4) % 4]))
            .map_err(|e| format!("sending SendTargets: {e}"))?;
        cmd_sn = cmd_sn.wrapping_add(1);
        let mut head = [0u8; pdu::BHS_LEN];
        stream
            .read_exact(&mut head)
            .map_err(|e| format!("reading SendTargets response: {e}"))?;
        if head[0] & 0x3f != pdu::TEXT_RESPONSE {
            return Err(format!(
                "expected a text response to SendTargets, got opcode {:#x}",
                head[0] & 0x3f
            ));
        }
        let len = u32::from_be_bytes([0, head[5], head[6], head[7]]) as usize;
        let mut data = vec![0u8; (len + 3) & !3];
        stream
            .read_exact(&mut data)
            .map_err(|e| format!("reading SendTargets data: {e}"))?;
        text.extend_from_slice(&data[..len]);
        exp_stat_sn = pdu::sequence(&head).stat_sn.wrapping_add(1);
        ttt = pdu::be32(&head, 20);
        if head[1] & 0x80 != 0 {
            break;
        }
        if ttt == pdu::NO_TTT {
            return Err("SendTargets response is not final and names no continuation".into());
        }
    }
    Ok(parse_targets(&text))
}

/// The discovery session's login keys: no target, and no digests.
pub(super) fn discovery_keys(initiator: &str) -> Vec<(String, String)> {
    [
        ("InitiatorName", initiator),
        ("SessionType", "Discovery"),
        ("HeaderDigest", "None"),
        ("DataDigest", "None"),
        ("MaxRecvDataSegmentLength", "8192"),
        ("MaxConnections", "1"),
        ("ErrorRecoveryLevel", "0"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

/// Reads the `TargetName` and `TargetAddress` pairs out of a SendTargets answer. An address is
/// `host:port,tpgt`; the portal group tag is dropped.
pub(super) fn parse_targets(text: &[u8]) -> Vec<Discovered> {
    let mut found: Vec<Discovered> = Vec::new();
    for item in text.split(|&b| b == 0).filter(|s| !s.is_empty()) {
        let item = String::from_utf8_lossy(item);
        if let Some(name) = item.strip_prefix("TargetName=") {
            found.push(Discovered {
                target: name.to_string(),
                addresses: Vec::new(),
            });
        } else if let (Some(address), Some(last)) =
            (item.strip_prefix("TargetAddress="), found.last_mut())
        {
            last.addresses
                .push(address.split(',').next().unwrap_or(address).to_string());
        }
    }
    found
}

#[cfg(test)]
mod discovery_tests {
    use super::*;

    /// A SendTargets answer: NUL-separated key=value pairs, one target with two addresses.
    #[test]
    fn send_targets_answer_lists_each_target_with_its_portals() {
        let text = b"TargetName=iqn.a:one\0TargetAddress=10.0.0.1:3260,1\0TargetAddress=10.0.0.2:3260,2\0TargetName=iqn.a:two\0TargetAddress=10.0.0.3:3260,1\0";
        let found = parse_targets(text);
        assert_eq!(
            found,
            vec![
                Discovered {
                    target: "iqn.a:one".into(),
                    addresses: vec!["10.0.0.1:3260".into(), "10.0.0.2:3260".into()],
                },
                Discovered {
                    target: "iqn.a:two".into(),
                    addresses: vec!["10.0.0.3:3260".into()],
                },
            ]
        );
    }

    #[test]
    fn empty_answer_lists_nothing() {
        assert!(parse_targets(b"").is_empty());
    }

    #[test]
    fn text_request_is_one_final_pdu_carrying_its_length() {
        let bhs = pdu::text_request(7, pdu::NO_TTT, 3, 4, 16);
        assert_eq!(bhs[0], pdu::TEXT_REQUEST);
        assert_eq!(bhs[1], 0x80);
        assert_eq!(&bhs[5..8], &[0, 0, 16]);
        assert_eq!(pdu::be32(&bhs, 16), 7);
        assert_eq!(pdu::be32(&bhs, 20), pdu::NO_TTT);
        assert_eq!(pdu::be32(&bhs, 24), 3);
        assert_eq!(pdu::be32(&bhs, 28), 4);
    }
}
