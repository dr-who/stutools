//! A user-space iSCSI session: TCP connect, login, SCSI commands (reads with Data-In, writes with
//! R2T and Data-Out), header and data digests when negotiated, NOP replies, and logout.

use super::chap;
use super::crc32c::crc32c;
use super::pdu::{self, LoginRequest, LoginResponse, Stage, BHS_LEN, NO_TTT};
use crate::paths::{Done, SENSE_CAPACITY};
use crate::Result;
use std::{
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const TIMEOUT: Duration = Duration::from_secs(10);
const LOGIN_ITT: u32 = 1;
/// The reference login uses CmdSN 0; the first command after login carries the same value (RFC 11.12.8).
const LOGIN_CMD_SN: u32 = 0;

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
}

/// Largest data segment the session accepts: the MaxRecvDataSegmentLength it offers, plus digest room.
const RX_CAPACITY: usize = 262_144 + 64;

/// An established iSCSI session on one connection.
pub struct Session {
    stream: TcpStream,
    pub negotiated: Negotiated,
    itt: u32,
    cmd_sn: u32,
    exp_stat_sn: u32,
    max_cmd_sn: u32,
    /// The lane tier of this portal from its ALUA state, read when the session opened.
    pub access_tier: Option<u8>,
    /// Reused for every outgoing PDU, so a command allocates nothing once warm.
    tx: Vec<u8>,
    /// Reused for every incoming data segment.
    rx: Vec<u8>,
    /// Sense data of the last command that returned CHECK CONDITION.
    sense: Vec<u8>,
    /// Commands sent and not yet finished, routed by task tag.
    inflight: Vec<Inflight>,
}

/// Most commands in flight on one session.
pub const MAX_INFLIGHT: usize = 128;

/// One command in flight: its task tag, the buffer its data moves through, and its progress.
struct Inflight {
    itt: u32,
    tag: usize,
    buf: *mut u8,
    len: usize,
    received: usize,
    write: bool,
    status: Option<u8>,
    sense: [u8; SENSE_CAPACITY],
    sense_len: usize,
}

/// What a SCSI command returned: the status, sense data when there is any, and the data read.
#[derive(Debug, PartialEq)]
pub struct Outcome {
    pub status: u8,
    pub sense: Vec<u8>,
    pub data: Vec<u8>,
}

/// Adds the default iSCSI port when the portal has none: `HOST`, `IPv4` and `[IPv6]` get `:3260`, and
/// a bare IPv6 address is bracketed first.
fn with_port(portal: &str) -> String {
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
fn refused(status: (u8, u8)) -> String {
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

fn value<'a>(text: &'a [(String, String)], key: &str) -> Option<&'a str> {
    text.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

/// Reads one login-phase response: the header and its data segment. Digests are not in use yet.
fn read_login_pdu(stream: &mut TcpStream) -> Result<Vec<u8>> {
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
fn exchange(stream: &mut TcpStream, request: &LoginRequest) -> Result<(LoginResponse, Vec<u8>)> {
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
fn operational_keys(initiator: &str, target: &str, digest: &str) -> Vec<(String, String)> {
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
            text: operational_keys(initiator, target, digest),
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
    Ok(Session {
        access_tier: None,
        stream,
        tx: Vec::with_capacity(BHS_LEN + RX_CAPACITY + 16),
        rx: Vec::with_capacity(RX_CAPACITY),
        sense: Vec::with_capacity(256),
        inflight: Vec::with_capacity(MAX_INFLIGHT),
        negotiated: Negotiated {
            header_digest: digest("HeaderDigest"),
            data_digest: digest("DataDigest"),
            max_recv,
            initial_r2t: value(&response.text, "InitialR2T") != Some("No"),
            tsih: response.tsih,
            chap_algorithm,
        },
        itt: LOGIN_ITT + 1,
        cmd_sn: exp_cmd_sn,
        exp_stat_sn: response.stat_sn.wrapping_add(1),
        max_cmd_sn,
    })
}

impl Session {
    fn next_itt(&mut self) -> u32 {
        self.itt = self.itt.wrapping_add(1);
        if self.itt == NO_TTT {
            self.itt = 1;
        }
        self.itt
    }

    /// Records the sequence numbers a target PDU carries.
    fn observe(&mut self, seq: pdu::Sequence) {
        self.exp_stat_sn = seq.stat_sn.wrapping_add(1);
        self.max_cmd_sn = seq.max_cmd_sn;
    }

    /// Sends one PDU: the header, its digest, the data segment padded to 4 bytes, and the data digest
    /// when negotiated. The digests cover the header, and the data with its padding.
    fn frame(&mut self, header: &[u8; BHS_LEN], data: &[u8]) -> Result<()> {
        self.tx.clear();
        self.tx.extend_from_slice(header);
        if self.negotiated.header_digest {
            self.tx.extend_from_slice(&crc32c(header).to_le_bytes());
        }
        if !data.is_empty() {
            let start = self.tx.len();
            self.tx.extend_from_slice(data);
            self.tx.resize(start + pdu::padded(data.len()), 0);
            if self.negotiated.data_digest {
                let digest = crc32c(&self.tx[start..]);
                self.tx.extend_from_slice(&digest.to_le_bytes());
            }
        }
        self.stream
            .write_all(&self.tx)
            .map_err(|e| format!("sending PDU: {e}"))
    }

    /// Reads one PDU. Returns its header and leaves its data segment (without padding) in `self.rx`.
    fn recv(&mut self) -> Result<[u8; BHS_LEN]> {
        let mut bhs = [0u8; BHS_LEN];
        self.stream
            .read_exact(&mut bhs)
            .map_err(|e| format!("reading PDU header: {e}"))?;
        if self.negotiated.header_digest {
            let mut digest = [0u8; 4];
            self.stream
                .read_exact(&mut digest)
                .map_err(|e| format!("reading header digest: {e}"))?;
            if u32::from_le_bytes(digest) != crc32c(&bhs) {
                return Err("header digest mismatch".into());
            }
        }
        let len = pdu::data_len(&bhs);
        let padded = pdu::padded(len);
        if padded > RX_CAPACITY {
            return Err(format!(
                "target sent a {len}-byte data segment; this initiator accepts {RX_CAPACITY}"
            ));
        }
        self.rx.clear();
        self.rx.resize(padded, 0);
        if padded > 0 {
            self.stream
                .read_exact(&mut self.rx)
                .map_err(|e| format!("reading PDU data: {e}"))?;
            if self.negotiated.data_digest {
                let mut digest = [0u8; 4];
                self.stream
                    .read_exact(&mut digest)
                    .map_err(|e| format!("reading data digest: {e}"))?;
                if u32::from_le_bytes(digest) != crc32c(&self.rx) {
                    return Err("data digest mismatch".into());
                }
            }
        }
        self.rx.truncate(len);
        Ok(bhs)
    }

    /// Whether another command can be sent now: the in-flight limit is not reached and the target's
    /// command window (MaxCmdSN) has room.
    pub fn can_submit(&self) -> bool {
        self.inflight.len() < MAX_INFLIGHT
            && (self.max_cmd_sn.wrapping_sub(self.cmd_sn) as i32) >= 0
    }

    /// Commands in flight on this session.
    pub fn inflight(&self) -> usize {
        self.inflight.len()
    }

    /// Sends a SCSI command and returns without waiting. `buf` holds the data: the data to send for a
    /// write, and where read data lands for a read. It must stay valid until `poll` returns `tag`.
    ///
    /// # Safety
    /// `buf` must point to `len` bytes that the caller keeps alive and does not touch until the command
    /// completes.
    pub unsafe fn submit(
        &mut self,
        cdb: &[u8],
        tag: usize,
        buf: *mut u8,
        len: usize,
        write: bool,
    ) -> Result<()> {
        if !self.can_submit() {
            return Err("target's command window is closed".into());
        }
        let itt = self.next_itt();
        let command = pdu::ScsiCommand {
            lun: 0,
            itt,
            cmd_sn: self.cmd_sn,
            exp_stat_sn: self.exp_stat_sn,
            cdb,
            read: !write && len > 0,
            write,
            expected: len as u32,
        };
        self.cmd_sn = self.cmd_sn.wrapping_add(1);
        let header = command.encode()?;
        self.frame(&header, &[])?;
        self.inflight.push(Inflight {
            itt,
            tag,
            buf,
            len,
            received: 0,
            write,
            status: None,
            sense: [0; SENSE_CAPACITY],
            sense_len: 0,
        });
        Ok(())
    }

    /// Waits for the next PDU that finishes an in-flight command. Data-In for a read and R2T for a
    /// write are handled on the way. Commands may finish in any order.
    pub fn poll(&mut self) -> Result<Done> {
        loop {
            let bhs = self.recv()?;
            let itt = pdu::be32(&bhs, 16);
            let find = |s: &Self| {
                s.inflight
                    .iter()
                    .position(|i| i.itt == itt)
                    .ok_or_else(|| format!("PDU for task {itt}, which is not in flight"))
            };
            match bhs[0] & 0x3f {
                pdu::DATA_IN => {
                    let idx = find(self)?;
                    let offset = pdu::be32(&bhs, 40) as usize;
                    let len = self.rx.len();
                    if bhs[1] & 0x01 != 0 {
                        self.observe(pdu::sequence(&bhs));
                    }
                    let inf = &mut self.inflight[idx];
                    if inf.write {
                        return Err("Data-In for a write".into());
                    }
                    if offset != inf.received || offset + len > inf.len {
                        return Err(format!(
                            "Data-In at {offset} for {} of {} bytes",
                            inf.received, inf.len
                        ));
                    }
                    // SAFETY: `submit` guarantees `buf` holds `len` bytes, and offset + len is within them.
                    unsafe { std::slice::from_raw_parts_mut(inf.buf.add(offset), len) }
                        .copy_from_slice(&self.rx);
                    inf.received += len;
                    if bhs[1] & 0x01 != 0 {
                        inf.status = Some(bhs[3]);
                    }
                    if bhs[1] & 0x80 != 0 {
                        if let Some(status) = inf.status {
                            let tag = inf.tag;
                            self.inflight.swap_remove(idx);
                            return Ok(Done {
                                code: 0,
                                tag,
                                status,
                                unavailable: false,
                                sense: [0; SENSE_CAPACITY],
                                sense_len: 0,
                            });
                        }
                    }
                }
                pdu::SCSI_RESPONSE => {
                    let idx = find(self)?;
                    if bhs[2] != 0 {
                        return Err(format!(
                            "target could not complete the command (response {:#x})",
                            bhs[2]
                        ));
                    }
                    self.observe(pdu::sequence(&bhs));
                    let mut inf = self.inflight.swap_remove(idx);
                    if self.rx.len() >= 2 {
                        let n = u16::from_be_bytes([self.rx[0], self.rx[1]]) as usize;
                        let end = (2 + n).min(self.rx.len()).min(2 + SENSE_CAPACITY);
                        inf.sense[..end - 2].copy_from_slice(&self.rx[2..end]);
                        inf.sense_len = end - 2;
                    }
                    let unavailable = inf.sense_len >= 14
                        && inf.sense[12] == 0x04
                        && matches!(inf.sense[13], 0x0a..=0x0c);
                    return Ok(Done {
                        code: 0,
                        tag: inf.tag,
                        status: bhs[3],
                        unavailable,
                        sense: inf.sense,
                        sense_len: inf.sense_len,
                    });
                }
                pdu::R2T => {
                    let idx = find(self)?;
                    let r = pdu::decode_r2t(&bhs)?;
                    self.observe(r.seq);
                    let (buf, len, write) = {
                        let inf = &self.inflight[idx];
                        (inf.buf, inf.len, inf.write)
                    };
                    if !write {
                        return Err("target asked for write data for a read command".into());
                    }
                    self.send_write(buf, len, &r)?;
                }
                pdu::NOP_IN => {
                    let n = pdu::decode_nop_in(&bhs)?;
                    self.observe(n.seq);
                    if n.ttt != NO_TTT {
                        let reply = pdu::nop_out(0, NO_TTT, n.ttt, self.cmd_sn, self.exp_stat_sn);
                        self.frame(&reply, &[])?;
                    }
                }
                other => return Err(format!("unexpected PDU opcode {other:#x}")),
            }
        }
    }

    /// Sends the write data an R2T asks for, from the command's buffer, in PDUs the target accepts.
    fn send_write(&mut self, buf: *mut u8, len: usize, r2t: &pdu::R2t) -> Result<()> {
        let start = r2t.offset as usize;
        let end = start
            .checked_add(r2t.desired as usize)
            .filter(|&end| end <= len)
            .ok_or_else(|| {
                format!(
                    "target asked for bytes {start}..+{} of a {len}-byte write",
                    r2t.desired
                )
            })?;
        let mut offset = start;
        let mut data_sn = 0u32;
        while offset < end {
            let n = (end - offset).min(self.negotiated.max_recv);
            let header = pdu::data_out_header(
                r2t.itt,
                r2t.ttt,
                self.exp_stat_sn,
                data_sn,
                offset as u32,
                offset + n == end,
                n,
            );
            // SAFETY: `offset + n` is within the `len` bytes `submit` guarantees for this command.
            let data = unsafe { std::slice::from_raw_parts(buf.add(offset), n) };
            self.frame(&header, data)?;
            offset += n;
            data_sn += 1;
        }
        Ok(())
    }

    /// Runs one command and waits for it. Only for commands sent with nothing else in flight.
    pub fn command_into(
        &mut self,
        cdb: &[u8],
        read: &mut [u8],
        write: Option<&[u8]>,
    ) -> Result<u8> {
        if !self.inflight.is_empty() {
            return Err("a command is already in flight on this session".into());
        }
        let (buf, len) = match write {
            Some(data) => (data.as_ptr() as *mut u8, data.len()),
            None => (read.as_mut_ptr(), read.len()),
        };
        // SAFETY: `buf` and `len` describe `read` or `write`, which live until the wait below returns.
        let result =
            unsafe { self.submit(cdb, 0, buf, len, write.is_some()) }.and_then(|()| loop {
                let done = self.poll()?;
                if done.tag == 0 {
                    self.sense.clear();
                    self.sense.extend_from_slice(&done.sense[..done.sense_len]);
                    return Ok(done.status);
                }
            });
        // A failed command leaves nothing in flight. Otherwise every later command would report that
        // this one is still pending, which hides the error that actually happened.
        if result.is_err() {
            self.inflight.clear();
        }
        result
    }

    /// Runs a command and returns its data and status. Allocates; used for the metadata commands.
    pub fn command(&mut self, cdb: &[u8], read: u32, write: Option<&[u8]>) -> Result<Outcome> {
        let mut data = vec![0u8; read as usize];
        let status = self.command_into(cdb, &mut data, write)?;
        Ok(Outcome {
            status,
            sense: self.sense.clone(),
            data,
        })
    }

    /// Logs out and closes the session. A failed logout is returned, not ignored.
    pub fn logout(mut self) -> Result<()> {
        let itt = self.next_itt();
        let header = pdu::logout_request(itt, 0, self.cmd_sn, self.exp_stat_sn);
        self.frame(&header, &[])?;
        let response = self.recv()?;
        pdu::check_logout_response(&response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// Live test: needs a target that accepts AuthMethod=None for this initiator.
    /// Run: SPIT_ISCSI_PORTAL=103.187.7.170 SPIT_ISCSI_TARGET=iqn... SPIT_ISCSI_INITIATOR=iqn...
    ///      cargo test --release iscsi::session -- --ignored --nocapture
    /// Live test: writes a pattern to a few blocks of the LUN and reads it back over the session. The
    /// LUN is a disposable loop device. Run with the same environment as the login test.
    #[test]
    #[ignore]
    fn live_write_and_read_back() {
        use super::super::scsi;
        let portal = std::env::var("SPIT_ISCSI_PORTAL").expect("set SPIT_ISCSI_PORTAL");
        let target = std::env::var("SPIT_ISCSI_TARGET").expect("set SPIT_ISCSI_TARGET");
        let initiator = std::env::var("SPIT_ISCSI_INITIATOR").expect("set SPIT_ISCSI_INITIATOR");
        let mut session = login(&portal, &target, &initiator, None, "CRC32C,None").expect("login");
        let (_, block_len) = scsi::capacity(&mut session).expect("capacity");
        let mut data = vec![0u8; 8 * block_len as usize];
        crate::pattern::fill(&mut data, 0, 0x5eed);
        let status = session
            .command_into(&scsi::block_cdb(0x2a, 1000, 8), &mut [], Some(&data))
            .expect("write");
        assert_eq!(status, 0, "WRITE(10) status");
        let mut back = vec![0u8; data.len()];
        let status = session
            .command_into(&scsi::block_cdb(0x28, 1000, 8), &mut back, None)
            .expect("read");
        assert_eq!(status, 0, "READ(10) status");
        assert_eq!(back, data, "data read back differs from data written");
        session.logout().expect("logout");
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

/// Opens the TCP connection to a portal, with the session timeouts set.
fn connect_stream(portal: &str) -> Result<TcpStream> {
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
fn discovery_keys(initiator: &str) -> Vec<(String, String)> {
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
fn parse_targets(text: &[u8]) -> Vec<Discovered> {
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
