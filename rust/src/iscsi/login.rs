//! Getting an iSCSI connection to full feature phase (RFC 7143 6.3): the TCP connection to a portal
//! (port 3260 when none is named), the login request and response exchange, the security stage with
//! CHAP when credentials are given, and the operational stage offering spit's keys (or exactly the keys
//! a caller gives, for the negotiation cases). What the target chose is kept in `Negotiated`. An ISID
//! names the initiator port; `random_isid` makes one in the random format. A refused login is an
//! error naming its status class and detail.

use super::chap;
use super::pdu::{self, LoginRequest, LoginResponse, Stage, BHS_LEN};
use super::session::Session;
use crate::Result;
use std::{
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub(super) const TIMEOUT: Duration = Duration::from_secs(10);

pub(super) const LOGIN_ITT: u32 = 1;

/// The reference login uses CmdSN 0; the first command after login carries the same value (RFC 11.12.8).
pub(super) const LOGIN_CMD_SN: u32 = 0;

/// Values the target chose during login. Digests apply to every PDU after login.
#[derive(Debug, Clone, PartialEq)]
pub struct Negotiated {
    pub header_digest: bool,
    pub data_digest: bool,
    /// The most data the target accepts in one PDU (its MaxRecvDataSegmentLength).
    pub max_recv: usize,
    pub initial_r2t: bool,
    pub tsih: u16,
    /// The CHAP algorithm the target chose (CHAP_A), when the login used CHAP.
    pub chap_algorithm: Option<u32>,
    /// Every key the target answered in the operational stage, as it sent them.
    pub keys: Vec<(String, String)>,
}

/// Adds the default iSCSI port when the portal has none: `HOST`, `IPv4` and `[IPv6]` get `:3260`, and
/// a bare IPv6 address is bracketed first.
pub(super) fn with_port(portal: &str) -> String {
    if portal.starts_with('[') {
        return if portal.contains("]:") {
            portal.to_string()
        } else {
            format!("{portal}:3260")
        };
    }
    match portal.matches(':').count() {
        0 => format!("{portal}:3260"),
        1 => portal.to_string(),
        _ => format!("[{portal}]:3260"),
    }
}

/// A random ISID in the RFC 7143 "random" format (T = 10b).
pub fn random_isid() -> [u8; 6] {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let mut x = nanos ^ ((std::process::id() as u64) << 32) ^ 0x9E37_79B9_7F4A_7C15;
    let mut out = [0u8; 6];
    for byte in &mut out {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *byte = x as u8;
    }
    out[0] = 0x80 | (out[0] & 0x3f);
    out
}

/// Turns a non-zero status class and detail into a readable login error.
pub(super) fn refused(status: (u8, u8)) -> String {
    let meaning = match status {
        (2, 1) => "authentication failure",
        (2, 2) => "authorization failure",
        (2, 3) => "target not found",
        (2, 7) => "missing parameter",
        (2, _) => "initiator error",
        (1, _) => "redirect",
        (3, _) => "target error",
        _ => "unknown",
    };
    format!(
        "login refused: status class {} detail {} ({meaning})",
        status.0, status.1
    )
}

pub(super) fn value<'a>(text: &'a [(String, String)], key: &str) -> Option<&'a str> {
    text.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

/// The reason in brackets of a login error (`refused`), for a case's actual column; the first 40
/// characters of any other error.
pub(super) fn reason(error: &str) -> String {
    match error.find('(') {
        Some(i) => error[i + 1..].trim_end_matches(')').to_string(),
        None => error.chars().take(40).collect(),
    }
}

/// Reads one login-phase response: the header and its data segment. Digests are not in use yet.
pub(super) fn read_login_pdu(stream: &mut TcpStream) -> Result<Vec<u8>> {
    let mut bhs = [0u8; BHS_LEN];
    stream
        .read_exact(&mut bhs)
        .map_err(|e| format!("reading login response header: {e}"))?;
    let mut pdu = bhs.to_vec();
    pdu.resize(BHS_LEN + pdu::padded(pdu::data_len(&bhs)), 0);
    stream
        .read_exact(&mut pdu[BHS_LEN..])
        .map_err(|e| format!("reading login response data: {e}"))?;
    Ok(pdu)
}

/// Sends a login request and returns the response with its raw bytes.
pub(super) fn exchange(
    stream: &mut TcpStream,
    request: &LoginRequest,
) -> Result<(LoginResponse, Vec<u8>)> {
    stream
        .write_all(&request.encode()?)
        .map_err(|e| format!("sending login request: {e}"))?;
    let raw = read_login_pdu(stream)?;
    let response = LoginResponse::decode(&raw)?;
    if response.itt != request.itt {
        return Err(format!(
            "login response has task tag {} for request {}",
            response.itt, request.itt
        ));
    }
    if response.status != (0, 0) {
        return Err(refused(response.status));
    }
    Ok((response, raw))
}

/// The initiator's login keys, offered in the operational stage.
pub(super) fn operational_keys(
    initiator: &str,
    target: &str,
    digest: &str,
) -> Vec<(String, String)> {
    [
        ("InitiatorName", initiator),
        ("TargetName", target),
        ("SessionType", "Normal"),
        ("HeaderDigest", digest),
        ("DataDigest", digest),
        ("DefaultTime2Wait", "2"),
        ("DefaultTime2Retain", "0"),
        ("IFMarker", "No"),
        ("OFMarker", "No"),
        ("ErrorRecoveryLevel", "0"),
        ("InitialR2T", "Yes"),
        ("ImmediateData", "No"),
        ("MaxBurstLength", "16776192"),
        ("FirstBurstLength", "262144"),
        ("MaxOutstandingR2T", "1"),
        ("MaxConnections", "1"),
        ("DataPDUInOrder", "Yes"),
        ("DataSequenceInOrder", "Yes"),
        ("MaxRecvDataSegmentLength", "262144"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

/// Logs in to `target` through `portal`. Without `chap`, the login skips security and starts at
/// operational negotiation (RFC 7143 6.3.1). With `chap = Some((user, secret))`, it runs the CHAP
/// exchange in the security stage first, with MD5 (CHAP_A=5).
/// `digest` is what the initiator offers for HeaderDigest and DataDigest, such as "CRC32C,None".
pub fn login(
    portal: &str,
    target: &str,
    initiator: &str,
    chap: Option<(&str, &[u8])>,
    digest: &str,
) -> Result<Session> {
    login_with_offer(portal, target, initiator, chap, digest, "5,6")
}

/// `login`, with the CHAP algorithms offered given explicitly, in order of preference (CHAP_A).
pub fn login_with_offer(
    portal: &str,
    target: &str,
    initiator: &str,
    chap: Option<(&str, &[u8])>,
    digest: &str,
    chap_offer: &str,
) -> Result<Session> {
    login_as(
        portal,
        target,
        initiator,
        random_isid(),
        chap,
        digest,
        chap_offer,
    )
}

/// `login_with_offer` with an explicit ISID. Each ISID is a separate initiator port to the target,
/// which persistent reservations need in order to tell two sessions apart.
pub fn login_as(
    portal: &str,
    target: &str,
    initiator: &str,
    isid: [u8; 6],
    chap: Option<(&str, &[u8])>,
    digest: &str,
    chap_offer: &str,
) -> Result<Session> {
    let keys = operational_keys(initiator, target, digest);
    login_with_keys(portal, target, initiator, isid, chap, chap_offer, keys)
}

/// Logs in offering exactly `offer` in the operational stage (after the initiator and target names and
/// the session type), and logs out at once. Returns what the target answered. The data path assumes
/// spit's own keys, so a session negotiated this way runs no commands.
pub fn negotiate(
    portal: &str,
    target: &str,
    initiator: &str,
    chap: Option<(&str, &[u8])>,
    offer: &[(&str, &str)],
) -> Result<Negotiated> {
    let mut keys: Vec<(String, String)> = [
        ("InitiatorName", initiator),
        ("TargetName", target),
        ("SessionType", "Normal"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    keys.extend(offer.iter().map(|(k, v)| (k.to_string(), v.to_string())));
    let session = login_with_keys(portal, target, initiator, random_isid(), chap, "5,6", keys)?;
    let negotiated = session.negotiated.clone();
    session.logout()?;
    Ok(negotiated)
}

pub(super) fn login_with_keys(
    portal: &str,
    target: &str,
    initiator: &str,
    isid: [u8; 6],
    chap: Option<(&str, &[u8])>,
    chap_offer: &str,
    keys: Vec<(String, String)>,
) -> Result<Session> {
    let mut stream = connect_stream(portal)?;
    let pairs = |list: &[(&str, &str)]| -> Vec<(String, String)> {
        list.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    };
    let mut exp_stat_sn = 0u32;
    let mut chap_algorithm = None;
    if let Some((user, secret)) = chap {
        let (r1, _) = exchange(
            &mut stream,
            &LoginRequest {
                isid,
                tsih: 0,
                cid: 0,
                itt: LOGIN_ITT,
                cmd_sn: LOGIN_CMD_SN,
                exp_stat_sn,
                current: Stage::Security,
                next: Stage::Security,
                transit: false,
                text: pairs(&[
                    ("InitiatorName", initiator),
                    ("TargetName", target),
                    ("SessionType", "Normal"),
                    ("AuthMethod", "CHAP"),
                ]),
            },
        )?;
        exp_stat_sn = r1.stat_sn.wrapping_add(1);
        let (r2, _) = exchange(
            &mut stream,
            &LoginRequest {
                isid,
                tsih: 0,
                cid: 0,
                itt: LOGIN_ITT,
                cmd_sn: LOGIN_CMD_SN,
                exp_stat_sn,
                current: Stage::Security,
                next: Stage::Security,
                transit: false,
                text: pairs(&[("CHAP_A", chap_offer)]),
            },
        )?;
        exp_stat_sn = r2.stat_sn.wrapping_add(1);
        let identifier = value(&r2.text, "CHAP_I")
            .and_then(|v| v.parse::<u8>().ok())
            .ok_or("target did not send CHAP_I")?;
        let challenge =
            chap::decode_hex(value(&r2.text, "CHAP_C").ok_or("target did not send CHAP_C")?)?;
        let algorithm = value(&r2.text, "CHAP_A")
            .and_then(|v| v.parse::<u32>().ok())
            .ok_or("target did not choose a CHAP algorithm")?;
        chap_algorithm = Some(algorithm);
        let answer =
            chap::response(algorithm, identifier, secret, &challenge).ok_or_else(|| {
                format!("target chose CHAP_A {algorithm}, which this initiator does not implement")
            })?;
        let (r3, _) = exchange(
            &mut stream,
            &LoginRequest {
                isid,
                tsih: 0,
                cid: 0,
                itt: LOGIN_ITT,
                cmd_sn: LOGIN_CMD_SN,
                exp_stat_sn,
                current: Stage::Security,
                next: Stage::Operational,
                transit: true,
                text: pairs(&[("CHAP_N", user), ("CHAP_R", &answer)]),
            },
        )?;
        exp_stat_sn = r3.stat_sn.wrapping_add(1);
        if !r3.transit {
            return Err("target did not accept CHAP and stay in the security stage".into());
        }
    }
    let (response, raw) = exchange(
        &mut stream,
        &LoginRequest {
            isid,
            tsih: 0,
            cid: 0,
            itt: LOGIN_ITT,
            cmd_sn: LOGIN_CMD_SN,
            exp_stat_sn,
            current: Stage::Operational,
            next: Stage::FullFeature,
            transit: true,
            text: keys,
        },
    )?;
    if !response.transit || response.next != Stage::FullFeature as u8 {
        return Err("target did not reach full feature phase".into());
    }
    if response.tsih == 0 {
        return Err("target did not assign a session handle".into());
    }
    let digest = |key: &str| value(&response.text, key) == Some("CRC32C");
    let max_recv = value(&response.text, "MaxRecvDataSegmentLength")
        .and_then(|v| v.parse::<usize>().ok())
        .ok_or("target did not report MaxRecvDataSegmentLength")?;
    let exp_cmd_sn = u32::from_be_bytes([raw[28], raw[29], raw[30], raw[31]]);
    let max_cmd_sn = u32::from_be_bytes([raw[32], raw[33], raw[34], raw[35]]);
    let negotiated = Negotiated {
        header_digest: digest("HeaderDigest"),
        data_digest: digest("DataDigest"),
        max_recv,
        initial_r2t: value(&response.text, "InitialR2T") != Some("No"),
        tsih: response.tsih,
        chap_algorithm,
        keys: response.text.clone(),
    };
    Ok(Session::established(
        stream,
        negotiated,
        exp_cmd_sn,
        response.stat_sn.wrapping_add(1),
        max_cmd_sn,
    ))
}

/// Opens the TCP connection to a portal, with the session timeouts set.
pub(super) fn connect_stream(portal: &str) -> Result<TcpStream> {
    let address = with_port(portal);
    let addr = address
        .to_socket_addrs()
        .map_err(|e| format!("resolving {address}: {e}"))?
        .next()
        .ok_or_else(|| format!("no address for {address}"))?;
    let stream = TcpStream::connect_timeout(&addr, TIMEOUT)
        .map_err(|e| format!("connecting to {address}: {e}"))?;
    stream
        .set_read_timeout(Some(TIMEOUT))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(TIMEOUT))
        .map_err(|e| e.to_string())?;
    Ok(stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reasons_are_taken_from_the_brackets() {
        assert_eq!(
            reason("login refused (authentication failure)"),
            "authentication failure"
        );
    }

    #[test]
    fn default_port_is_added_only_when_missing() {
        assert_eq!(with_port("10.0.0.1"), "10.0.0.1:3260");
        assert_eq!(with_port("10.0.0.1:3261"), "10.0.0.1:3261");
        assert_eq!(with_port("host.example"), "host.example:3260");
        assert_eq!(with_port("::1"), "[::1]:3260");
        assert_eq!(with_port("fe80::1:2"), "[fe80::1:2]:3260");
        assert_eq!(with_port("[::1]"), "[::1]:3260");
        assert_eq!(with_port("[::1]:3261"), "[::1]:3261");
    }

    #[test]
    fn random_isid_uses_the_random_format() {
        assert_eq!(random_isid()[0] >> 6, 0b10);
    }

    #[test]
    #[ignore]
    fn live_no_auth_login_and_logout() {
        let portal = std::env::var("SPIT_ISCSI_PORTAL").expect("set SPIT_ISCSI_PORTAL");
        let target = std::env::var("SPIT_ISCSI_TARGET").expect("set SPIT_ISCSI_TARGET");
        let initiator = std::env::var("SPIT_ISCSI_INITIATOR").expect("set SPIT_ISCSI_INITIATOR");
        let session =
            login(&portal, &target, &initiator, None, "CRC32C,None").expect("no-auth login");
        println!("logged in: {:?}", session.negotiated);
        session.logout().expect("logout");
    }
}
