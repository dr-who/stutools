//! Getting an iSCSI connection to full feature phase (RFC 7143 6.3): the TCP connection to a portal
//! (port 3260 when none is named), the login request and response exchange, the security stage with
//! CHAP when credentials are given (and mutual CHAP when the target's secret is given too: spit sends
//! its own CHAP_I and CHAP_C and checks the target's CHAP_N and CHAP_R, RFC 7143 12.1.3), and the
//! operational stage offering the keys of a `SessionKeys` (spit's own by default; the write path
//! hosts use with `SessionKeys::host_writes`) or exactly the keys a caller gives, for the negotiation
//! cases. What the target chose is kept in `Negotiated`; `establish` hands back the connection itself
//! for callers that drive full feature phase PDU by PDU. An ISID names the initiator port;
//! `random_isid` makes one in the random format. A refused login is an error naming its status class
//! and detail; a target that fails mutual authentication is an error too, and the connection is
//! closed.

use super::chap;
use super::pdu::{self, LoginRequest, LoginResponse, Stage, BHS_LEN};
use super::session::Session;
use crate::Result;
use std::{
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// MaxRecvDataSegmentLength when a side does not declare it (RFC 7143 13.12).
const DEFAULT_MAX_RECV: usize = 8192;

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
    /// The most data spit declared it accepts in one PDU (its own MaxRecvDataSegmentLength).
    pub own_max_recv: usize,
    pub initial_r2t: bool,
    /// ImmediateData as negotiated (RFC 7143 13.11: the AND of both sides; No when not answered).
    pub immediate_data: bool,
    /// FirstBurstLength and MaxBurstLength as negotiated (RFC 7143 13.13, 13.14), or the defaults.
    pub first_burst: usize,
    pub max_burst: usize,
    pub error_recovery: u8,
    pub tsih: u16,
    /// The CHAP algorithm the target chose (CHAP_A), when the login used CHAP.
    pub chap_algorithm: Option<u32>,
    /// The challenge the target sent (CHAP_C), when the login used CHAP.
    pub chap_challenge: Option<Vec<u8>>,
    /// The name the target answered with (CHAP_N) after a mutual CHAP exchange.
    pub target_chap_name: Option<String>,
    /// Every key the target answered in the operational stage, as it sent them.
    pub keys: Vec<(String, String)>,
}

/// The operational keys spit offers for a data session (RFC 7143 13). `spit` is what every session
/// offers unless a case asks otherwise: no immediate or unsolicited data, error recovery level 0,
/// one connection. `host_writes` offers what Linux open-iscsi and the Windows initiator offer for
/// writes: ImmediateData=Yes and InitialR2T=No, so the first burst travels unsolicited.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionKeys {
    pub header_digest: String,
    pub data_digest: String,
    pub initial_r2t: bool,
    pub immediate_data: bool,
    pub first_burst: u32,
    pub max_burst: u32,
    /// The MaxRecvDataSegmentLength spit declares: 512 to `MAX_OWN_RECV`.
    pub max_recv: u32,
    pub error_recovery: u8,
    pub max_connections: u16,
}

/// The largest MaxRecvDataSegmentLength spit declares: what its receive buffers hold.
pub(super) const MAX_OWN_RECV: u32 = 262_144;

impl SessionKeys {
    /// spit's own offer, with `digest` for both HeaderDigest and DataDigest.
    pub fn spit(digest: &str) -> SessionKeys {
        SessionKeys {
            header_digest: digest.to_string(),
            data_digest: digest.to_string(),
            initial_r2t: true,
            immediate_data: false,
            first_burst: 262_144,
            max_burst: 16_776_192,
            max_recv: MAX_OWN_RECV,
            error_recovery: 0,
            max_connections: 1,
        }
    }

    /// The write path hosts use: ImmediateData=Yes, InitialR2T=No, FirstBurstLength 65536 and
    /// MaxBurstLength 262144, as the Windows initiator offers them.
    pub fn host_writes(digest: &str) -> SessionKeys {
        SessionKeys {
            initial_r2t: false,
            immediate_data: true,
            first_burst: 65_536,
            max_burst: 262_144,
            ..SessionKeys::spit(digest)
        }
    }

    /// The Login Request keys of the operational stage, after the names and the session type.
    pub(super) fn offer(&self, initiator: &str, target: &str) -> Result<Vec<(String, String)>> {
        if !(512..=MAX_OWN_RECV).contains(&self.max_recv) {
            return Err(format!(
                "MaxRecvDataSegmentLength {} is outside 512..={MAX_OWN_RECV}",
                self.max_recv
            ));
        }
        let yes_no = |b: bool| if b { "Yes" } else { "No" }.to_string();
        Ok([
            ("InitiatorName", initiator.to_string()),
            ("TargetName", target.to_string()),
            ("SessionType", "Normal".into()),
            ("HeaderDigest", self.header_digest.clone()),
            ("DataDigest", self.data_digest.clone()),
            ("DefaultTime2Wait", "2".into()),
            ("DefaultTime2Retain", "0".into()),
            ("IFMarker", "No".into()),
            ("OFMarker", "No".into()),
            ("ErrorRecoveryLevel", self.error_recovery.to_string()),
            ("InitialR2T", yes_no(self.initial_r2t)),
            ("ImmediateData", yes_no(self.immediate_data)),
            ("MaxBurstLength", self.max_burst.to_string()),
            ("FirstBurstLength", self.first_burst.to_string()),
            ("MaxOutstandingR2T", "1".into()),
            ("MaxConnections", self.max_connections.to_string()),
            ("DataPDUInOrder", "Yes".into()),
            ("DataSequenceInOrder", "Yes".into()),
            ("MaxRecvDataSegmentLength", self.max_recv.to_string()),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect())
    }
}

/// Mutual CHAP (RFC 7143 12.1.3): the challenge spit sends with its own response, how it encodes it,
/// and the secret the target must answer it with.
#[derive(Clone, Copy)]
pub(super) struct Mutual<'a> {
    pub secret: &'a [u8],
    pub identifier: u8,
    pub challenge: &'a [u8],
    pub base64: bool,
}

/// How one login goes: the initiator port, the CHAP credentials and algorithms offered, mutual CHAP,
/// and the operational keys.
pub(super) struct Plan<'a> {
    pub isid: [u8; 6],
    pub chap: Option<(&'a str, &'a [u8])>,
    pub chap_offer: &'a str,
    pub mutual: Option<Mutual<'a>>,
    pub keys: Vec<(String, String)>,
}

/// A connection login has brought to full feature phase, with the command numbers the final login
/// response gave: the CmdSN the first command carries, the StatSN expected next, and MaxCmdSN.
pub(super) struct Established {
    pub stream: TcpStream,
    pub negotiated: Negotiated,
    pub cmd_sn: u32,
    pub exp_stat_sn: u32,
    pub max_cmd_sn: u32,
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
    let keys = SessionKeys::spit(digest).offer(initiator, target)?;
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
    let plan = Plan {
        isid,
        chap,
        chap_offer,
        mutual: None,
        keys,
    };
    establish(portal, target, initiator, plan).map(Session::from)
}

impl From<Established> for Session {
    fn from(e: Established) -> Session {
        Session::established(
            e.stream,
            e.negotiated,
            e.cmd_sn,
            e.exp_stat_sn,
            e.max_cmd_sn,
        )
    }
}

/// Logs in as `plan` says and returns the connection in full feature phase.
pub(super) fn establish(
    portal: &str,
    target: &str,
    initiator: &str,
    plan: Plan,
) -> Result<Established> {
    let Plan {
        isid,
        chap,
        chap_offer,
        mutual,
        keys,
    } = plan;
    let mut stream = connect_stream(portal)?;
    let pairs = |list: &[(&str, &str)]| -> Vec<(String, String)> {
        list.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    };
    let mut exp_stat_sn = 0u32;
    let mut chap_algorithm = None;
    let mut chap_challenge = None;
    let mut target_chap_name = None;
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
            chap::decode_binary(value(&r2.text, "CHAP_C").ok_or("target did not send CHAP_C")?)?;
        let algorithm = value(&r2.text, "CHAP_A")
            .and_then(|v| v.parse::<u32>().ok())
            .ok_or("target did not choose a CHAP algorithm")?;
        chap_algorithm = Some(algorithm);
        let answer =
            chap::response(algorithm, identifier, secret, &challenge).ok_or_else(|| {
                format!("target chose CHAP_A {algorithm}, which this initiator does not implement")
            })?;
        let mut text = pairs(&[("CHAP_N", user), ("CHAP_R", &answer)]);
        if let Some(m) = mutual {
            let encoded = if m.base64 {
                chap::encode_base64(m.challenge)
            } else {
                chap::encode_hex(m.challenge)
            };
            text.push(("CHAP_I".into(), m.identifier.to_string()));
            text.push(("CHAP_C".into(), encoded));
        }
        chap_challenge = Some(challenge);
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
                text,
            },
        )?;
        exp_stat_sn = r3.stat_sn.wrapping_add(1);
        if let Some(m) = mutual {
            target_chap_name = Some(check_mutual(algorithm, secret, &m, &r3.text)?);
        }
        if !r3.transit {
            return Err("target did not accept CHAP and stay in the security stage".into());
        }
    }
    let own_max_recv = value(&keys, "MaxRecvDataSegmentLength")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(DEFAULT_MAX_RECV);
    let offered = keys.clone();
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
    // RFC 7143 13.12: a declarative key; a target that does not declare it takes the default.
    let max_recv = match value(&response.text, "MaxRecvDataSegmentLength") {
        Some(v) => v
            .parse::<usize>()
            .map_err(|_| format!("target declared MaxRecvDataSegmentLength={v}"))?,
        None => DEFAULT_MAX_RECV,
    };
    let seq = pdu::sequence(&raw);
    let negotiated = Negotiated {
        header_digest: digest("HeaderDigest"),
        data_digest: digest("DataDigest"),
        max_recv,
        own_max_recv,
        initial_r2t: value(&response.text, "InitialR2T") != Some("No"),
        immediate_data: negotiated_immediate(&offered, &response.text),
        first_burst: negotiated_length(&offered, &response.text, "FirstBurstLength", 65_536),
        max_burst: negotiated_length(&offered, &response.text, "MaxBurstLength", 262_144),
        error_recovery: value(&response.text, "ErrorRecoveryLevel")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        tsih: response.tsih,
        chap_algorithm,
        chap_challenge,
        target_chap_name,
        keys: response.text.clone(),
    };
    Ok(Established {
        stream,
        negotiated,
        cmd_sn: seq.exp_cmd_sn,
        exp_stat_sn: response.stat_sn.wrapping_add(1),
        max_cmd_sn: seq.max_cmd_sn,
    })
}

/// ImmediateData after login (RFC 7143 13.11, 6.2.2): an offer of No is No whether or not the target
/// answers; an offer of Yes is what the target answered, and No when it did not answer.
fn negotiated_immediate(offered: &[(String, String)], answered: &[(String, String)]) -> bool {
    value(offered, "ImmediateData") == Some("Yes")
        && value(answered, "ImmediateData") == Some("Yes")
}

/// A Minimum-function length key after login (RFC 7143 13.13, 13.14): the target's answer, else the
/// offer when the target did not answer it, else the default.
fn negotiated_length(
    offered: &[(String, String)],
    answered: &[(String, String)],
    key: &str,
    default: usize,
) -> usize {
    value(answered, key)
        .or_else(|| value(offered, key))
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Checks the target's half of mutual CHAP in the response to spit's challenge (RFC 7143 12.1.3):
/// CHAP_N must be there, and CHAP_R must be H(identifier || target secret || challenge) under the
/// algorithm the target chose. A response that equals what spit's own secret gives is a failure too
/// (RFC 7143 9.2.1: one secret must not serve both directions). Returns the target's CHAP_N.
pub(super) fn check_mutual(
    algorithm: u32,
    own_secret: &[u8],
    m: &Mutual,
    text: &[(String, String)],
) -> Result<String> {
    let name = value(text, "CHAP_N").ok_or("mutual CHAP: the target sent no CHAP_N")?;
    let got = chap::decode_binary(
        value(text, "CHAP_R").ok_or("mutual CHAP: the target sent no CHAP_R")?,
    )?;
    let want = chap::digest(algorithm, m.identifier, m.secret, m.challenge)
        .ok_or("mutual CHAP: algorithm not implemented")?;
    if chap::digest(algorithm, m.identifier, own_secret, m.challenge).as_deref() == Some(&got[..]) {
        return Err("mutual CHAP: the target answered with the initiator's own secret".into());
    }
    if got != want {
        return Err(format!(
            "mutual CHAP: the target's CHAP_R is wrong ({} bytes)",
            got.len()
        ));
    }
    Ok(name.to_string())
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

    fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn mutual_chap_accepts_only_the_target_secret() {
        let challenge = [9u8; 16];
        let m = Mutual {
            secret: b"target-secret-1",
            identifier: 7,
            challenge: &challenge,
            base64: false,
        };
        let right = chap::response(chap::MD5, 7, b"target-secret-1", &challenge).unwrap();
        let text = pairs(&[("CHAP_N", "tgt"), ("CHAP_R", &right)]);
        assert_eq!(
            check_mutual(chap::MD5, b"initiator-secr", &m, &text).unwrap(),
            "tgt"
        );
        let b64 = chap::encode_base64(
            &chap::digest(chap::MD5, 7, b"target-secret-1", &challenge).unwrap(),
        );
        let text = pairs(&[("CHAP_N", "tgt"), ("CHAP_R", &b64)]);
        assert!(check_mutual(chap::MD5, b"initiator-secr", &m, &text).is_ok());
        let wrong = chap::response(chap::MD5, 7, b"another", &challenge).unwrap();
        let text = pairs(&[("CHAP_N", "tgt"), ("CHAP_R", &wrong)]);
        assert!(check_mutual(chap::MD5, b"initiator-secr", &m, &text).is_err());
        let own = chap::response(chap::MD5, 7, b"initiator-secr", &challenge).unwrap();
        let text = pairs(&[("CHAP_N", "tgt"), ("CHAP_R", &own)]);
        let same = Mutual {
            secret: b"initiator-secr",
            ..m
        };
        assert!(check_mutual(chap::MD5, b"initiator-secr", &same, &text).is_err());
        assert!(check_mutual(chap::MD5, b"x", &m, &pairs(&[("CHAP_R", &right)])).is_err());
        assert!(check_mutual(chap::MD5, b"x", &m, &pairs(&[("CHAP_N", "tgt")])).is_err());
    }

    #[test]
    fn immediate_data_and_burst_lengths_follow_their_functions() {
        let yes = pairs(&[("ImmediateData", "Yes"), ("FirstBurstLength", "65536")]);
        let no = pairs(&[("ImmediateData", "No")]);
        assert!(negotiated_immediate(&yes, &yes));
        assert!(!negotiated_immediate(&yes, &no));
        assert!(!negotiated_immediate(&no, &yes));
        assert!(!negotiated_immediate(&yes, &[]));
        let answered = pairs(&[("FirstBurstLength", "8192")]);
        assert_eq!(
            negotiated_length(&yes, &answered, "FirstBurstLength", 1),
            8192
        );
        assert_eq!(negotiated_length(&yes, &[], "FirstBurstLength", 1), 65536);
        assert_eq!(
            negotiated_length(&[], &[], "MaxBurstLength", 262144),
            262144
        );
    }

    #[test]
    fn session_keys_offer_what_they_say() {
        let spit = SessionKeys::spit("None").offer("i", "t").unwrap();
        assert_eq!(value(&spit, "ImmediateData"), Some("No"));
        assert_eq!(value(&spit, "InitialR2T"), Some("Yes"));
        assert_eq!(value(&spit, "MaxRecvDataSegmentLength"), Some("262144"));
        let host = SessionKeys::host_writes("CRC32C").offer("i", "t").unwrap();
        assert_eq!(value(&host, "ImmediateData"), Some("Yes"));
        assert_eq!(value(&host, "InitialR2T"), Some("No"));
        assert_eq!(value(&host, "FirstBurstLength"), Some("65536"));
        let mut small = SessionKeys::spit("None");
        small.max_recv = 511;
        assert!(small.offer("i", "t").is_err());
        small.max_recv = 512;
        small.header_digest = "CRC32C".into();
        let offer = small.offer("i", "t").unwrap();
        assert_eq!(value(&offer, "HeaderDigest"), Some("CRC32C"));
        assert_eq!(value(&offer, "DataDigest"), Some("None"));
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
