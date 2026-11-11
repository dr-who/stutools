//! A connection in full feature phase driven PDU by PDU, for the cases that judge the fields of every
//! PDU (RFC 7143 11) or send what `session` never sends: a NOP-Out without the immediate bit, a
//! Logout for another CID, a Text Request. It logs in as `login::establish` does, with the
//! `SessionKeys` the case asks for, then sends hand-built headers with the negotiated digests and
//! reads one PDU at a time, each
//! within a wait the caller gives: an answer is a PDU, the connection closed, a digest the target got
//! wrong, or silence. Every PDU received is kept in `log`, in order, so a case can judge StatSN over
//! the whole connection. It runs one task at a time; a target NOP-In ping is answered when the
//! caller asks (`answer_ping`).

use super::commands::Commands;
use super::crc32c::crc32c;
use super::login::{self, Established, Mutual, Negotiated, Plan, SessionKeys};
use super::lun_access::Creds;
use super::path::Path;
use super::pdu::{self, BHS_LEN, NO_TTT};
use super::session::Outcome;
use crate::Result;
use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// How long a PDU is waited for unless a case says otherwise.
pub(super) const PDU_WAIT: Duration = Duration::from_secs(10);

/// The error `drain_attentions` returns for a port in the standby state.
pub(super) const STANDBY: &str = "target port is in the standby state";

/// The largest data segment read: spit never declares more than `login::MAX_OWN_RECV`; a target that
/// sends more is reported by the cases, so up to 16 MiB is read.
const MAX_SEGMENT: usize = 1 << 24;

/// One Data-Out PDU's fields: task tag, target transfer tag (FFFFFFFFh for unsolicited data),
/// DataSN, buffer offset, the F bit, and whether its data digest is to be corrupted.
pub(super) struct DataOut {
    pub(super) itt: u32,
    pub(super) ttt: u32,
    pub(super) data_sn: u32,
    pub(super) offset: u32,
    pub(super) last: bool,
    pub(super) bad_digest: bool,
}

/// One PDU from the target: its header and its data segment without padding.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Pdu {
    pub bhs: [u8; BHS_LEN],
    pub data: Vec<u8>,
}

impl Pdu {
    pub(super) fn opcode(&self) -> u8 {
        self.bhs[0] & 0x3f
    }
    pub(super) fn flags(&self) -> u8 {
        self.bhs[1]
    }
    pub(super) fn itt(&self) -> u32 {
        pdu::be32(&self.bhs, 16)
    }
    pub(super) fn ttt(&self) -> u32 {
        pdu::be32(&self.bhs, 20)
    }
    pub(super) fn stat_sn(&self) -> u32 {
        pdu::be32(&self.bhs, 24)
    }
    pub(super) fn exp_cmd_sn(&self) -> u32 {
        pdu::be32(&self.bhs, 28)
    }
    pub(super) fn max_cmd_sn(&self) -> u32 {
        pdu::be32(&self.bhs, 32)
    }
    /// DataSN of a Data-In, R2TSN of an R2T, ExpDataSN of a SCSI Response (RFC 7143 11.4, 11.7, 11.8).
    pub(super) fn sn36(&self) -> u32 {
        pdu::be32(&self.bhs, 36)
    }
    /// BufferOffset of a Data-In or R2T.
    pub(super) fn offset(&self) -> u32 {
        pdu::be32(&self.bhs, 40)
    }
    /// Residual Count of a SCSI Response or a Data-In with S=1; DDTL of an R2T.
    pub(super) fn word44(&self) -> u32 {
        pdu::be32(&self.bhs, 44)
    }
    /// The SCSI status of a SCSI Response, or of a Data-In with S=1.
    pub(super) fn status(&self) -> u8 {
        self.bhs[3]
    }
    /// The sense data a SCSI Response carries (RFC 7143 11.4.7), without its length.
    pub(super) fn sense(&self) -> Vec<u8> {
        if self.data.len() < 2 {
            return Vec::new();
        }
        let n = u16::from_be_bytes([self.data[0], self.data[1]]) as usize;
        self.data[2..(2 + n).min(self.data.len())].to_vec()
    }
}

/// What one read brought.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Received {
    Pdu(Pdu),
    /// The target closed or reset the connection.
    Closed,
    /// Nothing within the wait.
    Silent,
    /// A PDU whose header or data digest did not match what the target sent.
    BadDigest(&'static str),
}

/// A connection in full feature phase.
pub(super) struct PduConnection {
    stream: TcpStream,
    pub negotiated: Negotiated,
    pub cmd_sn: u32,
    pub exp_stat_sn: u32,
    pub max_cmd_sn: u32,
    itt: u32,
    pub lun: u64,
    /// Every PDU received, in order.
    pub log: Vec<Pdu>,
}

impl PduConnection {
    /// Logs in to the logical unit `path` names, offering `keys`.
    pub(super) fn open(
        path: &Path,
        initiator: &str,
        creds: Creds,
        keys: &SessionKeys,
    ) -> Result<PduConnection> {
        Self::open_as(path, initiator, creds, keys, login::random_isid(), None)
    }

    /// `open` as the initiator port `isid`, with mutual CHAP when `mutual` is given.
    pub(super) fn open_as(
        path: &Path,
        initiator: &str,
        creds: Creds,
        keys: &SessionKeys,
        isid: [u8; 6],
        mutual: Option<Mutual>,
    ) -> Result<PduConnection> {
        let plan = Plan {
            isid,
            chap: creds,
            chap_offer: "5,6,7,8",
            mutual,
            keys: keys.offer(initiator, &path.target)?,
        };
        let e = login::establish(&path.portal, &path.target, initiator, plan)?;
        Ok(Self::from_established(e, path.lun_field()))
    }

    pub(super) fn from_established(e: Established, lun: u64) -> PduConnection {
        PduConnection {
            stream: e.stream,
            negotiated: e.negotiated,
            cmd_sn: e.cmd_sn,
            exp_stat_sn: e.exp_stat_sn,
            max_cmd_sn: e.max_cmd_sn,
            itt: login::LOGIN_ITT,
            lun,
            log: Vec::new(),
        }
    }

    /// A fresh initiator task tag.
    pub(super) fn next_itt(&mut self) -> u32 {
        self.itt = self.itt.wrapping_add(1);
        if self.itt == NO_TTT {
            self.itt = 1;
        }
        self.itt
    }

    /// Sends one PDU: `header` (the BHS and any AHS), its digest, `data` padded to 4 bytes and its
    /// digest, each digest as negotiated.
    pub(super) fn send(&mut self, header: &[u8], data: &[u8]) -> Result<()> {
        crate::watchdog::beat(header);
        let mut out = Vec::with_capacity(header.len() + data.len() + 16);
        out.extend_from_slice(header);
        if self.negotiated.header_digest {
            out.extend_from_slice(&crc32c(header).to_le_bytes());
        }
        if !data.is_empty() {
            let start = out.len();
            out.extend_from_slice(data);
            out.resize(start + pdu::padded(data.len()), 0);
            if self.negotiated.data_digest {
                let d = crc32c(&out[start..]);
                out.extend_from_slice(&d.to_le_bytes());
            }
        }
        self.stream
            .write_all(&out)
            .map_err(|e| format!("sending PDU: {e}"))
    }

    /// Reads the next PDU within `wait`, records its sequence numbers, and keeps it in `log`.
    pub(super) fn recv(&mut self, wait: Duration) -> Received {
        crate::watchdog::beat(&[]);
        let _ = self.stream.set_read_timeout(Some(wait));
        let mut bhs = [0u8; BHS_LEN];
        if let Err(e) = self.stream.read_exact(&mut bhs) {
            return match e.kind() {
                ErrorKind::WouldBlock | ErrorKind::TimedOut => Received::Silent,
                _ => Received::Closed,
            };
        }
        let _ = self.stream.set_read_timeout(Some(PDU_WAIT));
        let ahs = bhs[4] as usize * 4;
        let mut skip = vec![0u8; ahs];
        if self.stream.read_exact(&mut skip).is_err() {
            return Received::Closed;
        }
        if self.negotiated.header_digest {
            let mut d = [0u8; 4];
            if self.stream.read_exact(&mut d).is_err() {
                return Received::Closed;
            }
            let mut covered = bhs.to_vec();
            covered.extend_from_slice(&skip);
            if u32::from_le_bytes(d) != crc32c(&covered) {
                return Received::BadDigest("header digest");
            }
        }
        let len = pdu::data_len(&bhs);
        if len > MAX_SEGMENT {
            return Received::Closed;
        }
        let mut data = vec![0u8; pdu::padded(len)];
        if self.stream.read_exact(&mut data).is_err() {
            return Received::Closed;
        }
        if len > 0 && self.negotiated.data_digest {
            let mut d = [0u8; 4];
            if self.stream.read_exact(&mut d).is_err() {
                return Received::Closed;
            }
            if u32::from_le_bytes(d) != crc32c(&data) {
                return Received::BadDigest("data digest");
            }
        }
        data.truncate(len);
        let p = Pdu { bhs, data };
        self.observe(&p);
        self.log.push(p.clone());
        Received::Pdu(p)
    }

    /// Takes ExpStatSN and MaxCmdSN from a PDU that carries a valid StatSN (RFC 7143 11.7.4: a Data-In
    /// only with S=1; an R2T and a NOP-In with no task tag carry the next StatSN, not a new one).
    fn observe(&mut self, p: &Pdu) {
        let carries_status = match p.opcode() {
            pdu::DATA_IN => p.flags() & 0x01 != 0,
            pdu::R2T => false,
            pdu::NOP_IN => p.itt() != NO_TTT,
            _ => true,
        };
        if carries_status {
            self.exp_stat_sn = p.stat_sn().wrapping_add(1);
        }
        if (p.max_cmd_sn().wrapping_sub(p.exp_cmd_sn()) as i32) >= -1 {
            self.max_cmd_sn = p.max_cmd_sn();
        }
    }

    /// Answers a NOP-In ping that asks for a reply (TTT not FFFFFFFFh, RFC 7143 11.18.3).
    pub(super) fn answer_ping(&mut self, nop_in: &Pdu) -> Result<()> {
        if nop_in.ttt() == NO_TTT {
            return Ok(());
        }
        let reply = pdu::nop_out(
            self.lun,
            NO_TTT,
            nop_in.ttt(),
            self.cmd_sn,
            self.exp_stat_sn,
        );
        self.send(&reply, &[])
    }

    /// A SCSI Command header for this LUN with the next task tag and CmdSN (advanced unless `immediate`):
    /// the R and W bits, EDTL, `data_len` bytes of immediate data, and the F bit when no unsolicited
    /// Data-Out follows. Returns the header and its task tag.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn command_header(
        &mut self,
        cdb: &[u8],
        edtl: u32,
        read: bool,
        write: bool,
        data_len: usize,
        final_bit: bool,
        immediate: bool,
    ) -> Result<([u8; BHS_LEN], u32)> {
        let itt = self.next_itt();
        let mut bhs = pdu::ScsiCommand {
            lun: self.lun,
            itt,
            cmd_sn: self.cmd_sn,
            exp_stat_sn: self.exp_stat_sn,
            cdb,
            read,
            write,
            expected: edtl,
        }
        .encode()?;
        if !final_bit {
            bhs[1] &= !0x80;
        }
        if immediate {
            bhs[0] |= 0x40;
        } else {
            self.cmd_sn = self.cmd_sn.wrapping_add(1);
        }
        pdu::set_data_len(&mut bhs, data_len);
        Ok((bhs, itt))
    }

    /// A NOP-Out ping header: a task tag, TTT FFFFFFFFh, `len` bytes of data, and the I bit when
    /// `immediate` (a non-immediate ping takes a CmdSN, RFC 7143 11.18).
    pub(super) fn ping_header(&mut self, len: usize, immediate: bool) -> ([u8; BHS_LEN], u32) {
        let itt = self.next_itt();
        let mut bhs = pdu::nop_out(self.lun, itt, NO_TTT, self.cmd_sn, self.exp_stat_sn);
        if !immediate {
            bhs[0] &= !0x40;
            self.cmd_sn = self.cmd_sn.wrapping_add(1);
        }
        pdu::set_data_len(&mut bhs, len);
        (bhs, itt)
    }

    /// Waits for the PDU that finishes task `itt` (a SCSI Response, or a Data-In with S=1 and F=1),
    /// answering pings on the way. Returns every PDU for the task, then how it ended.
    pub(super) fn finish(&mut self, itt: u32, wait: Duration) -> (Vec<Pdu>, Received) {
        let mut seen = Vec::new();
        loop {
            match self.recv(wait) {
                Received::Pdu(p) if p.opcode() == pdu::NOP_IN && p.itt() == NO_TTT => {
                    let _ = self.answer_ping(&p);
                }
                Received::Pdu(p) => {
                    let done = p.itt() == itt
                        && (p.opcode() == pdu::SCSI_RESPONSE
                            || (p.opcode() == pdu::DATA_IN && p.flags() & 0x81 == 0x81));
                    if done {
                        return (seen, Received::Pdu(p));
                    }
                    seen.push(p);
                }
                other => return (seen, other),
            }
        }
    }

    /// Sends a SCSI command that moves no data or reads `read` bytes, and returns its Data-In PDUs
    /// and how it ended.
    pub(super) fn read_command(&mut self, cdb: &[u8], read: u32) -> Result<(Vec<Pdu>, Received)> {
        let (bhs, itt) = self.command_header(cdb, read, read > 0, false, 0, true, false)?;
        let mut header = bhs.to_vec();
        header.extend_from_slice(&pdu::extended_cdb(cdb));
        self.send(&header, &[])?;
        Ok(self.finish(itt, PDU_WAIT))
    }

    /// The error `drain_attentions` returns for a port in the standby state.
    pub(super) const STANDBY: &str = "target port is in the standby state";

    /// TEST UNIT READY until it is GOOD, at most eight times: a new nexus may have unit attentions.
    pub(super) fn drain_attentions(&mut self) -> Result<()> {
        for _ in 0..8 {
            let (_, end) = self.read_command(&[0; 6], 0)?;
            match end {
                Received::Pdu(p) if p.status() == 0 => return Ok(()),
                // NOT READY, LOGICAL UNIT NOT ACCESSIBLE, TARGET PORT IN STANDBY STATE (04h, 0Bh): a standby
                // port refuses TEST UNIT READY by design (SPC-3 5.8.2.4.4), so there is nothing to wait for.
                Received::Pdu(p) if p.sense().get(12..14) == Some(&[0x04, 0x0b]) => {
                    return Err(STANDBY.into())
                }
                Received::Pdu(_) => continue,
                other => return Err(format!("TEST UNIT READY: {other:?}")),
            }
        }
        Err("TEST UNIT READY never GOOD".into())
    }

    /// A Data-Out PDU carrying `payload` as `d` describes. When `d.bad_digest`, the data digest field
    /// is flipped in its lowest bit, as a one-bit NIC corruption would (RFC 7143 7.8): the only place
    /// this module sends a digest the data does not match.
    pub(super) fn data_out(&mut self, d: DataOut, payload: &[u8]) -> Result<()> {
        let header = pdu::data_out_header(
            d.itt,
            d.ttt,
            self.exp_stat_sn,
            d.data_sn,
            d.offset,
            d.last,
            payload.len(),
        );
        if !d.bad_digest {
            return self.send(&header, payload);
        }
        let mut out = Vec::with_capacity(payload.len() + 64);
        out.extend_from_slice(&header);
        if self.negotiated.header_digest {
            out.extend_from_slice(&crc32c(&header).to_le_bytes());
        }
        let start = out.len();
        out.extend_from_slice(payload);
        out.resize(start + pdu::padded(payload.len()), 0);
        if self.negotiated.data_digest {
            out.extend_from_slice(&(crc32c(&out[start..]) ^ 1).to_le_bytes());
        }
        self.stream
            .write_all(&out)
            .map_err(|e| format!("sending Data-Out: {e}"))
    }

    /// Runs a WRITE the way `form` says over `data`: `immediate` bytes in the command PDU, `unsolicited`
    /// bytes at once as Data-Out with TTT FFFFFFFFh, the rest on the target's R2Ts. The Data-Out named
    /// by `bad_data_out` (counted from 0) has a wrong data digest. Returns every PDU seen and how the
    /// task ended.
    pub(super) fn write(
        &mut self,
        cdb: &[u8],
        data: &[u8],
        form: &WriteForm,
    ) -> Result<(Vec<Pdu>, Received)> {
        let edtl = form.edtl.unwrap_or(data.len() as u32);
        let imm = form.immediate.min(data.len());
        // The command PDU's F bit is set unless unsolicited Data-Out PDUs follow it (RFC 7143 11.3.1).
        let final_bit = form.unsolicited == 0;
        let (bhs, itt) = self.command_header(cdb, edtl, false, true, imm, final_bit, false)?;
        let mut header = bhs.to_vec();
        header.extend_from_slice(&pdu::extended_cdb(cdb));
        self.send(&header, &data[..imm])?;
        let mut data_out_count = 0usize;
        let bad = |n: usize, form: &WriteForm| form.bad_data_out == Some(n);
        // Unsolicited Data-Out (TTT FFFFFFFFh) for the bytes after the immediate part.
        if form.unsolicited > 0 {
            let seg = form.segment.unwrap_or(self.negotiated.max_recv).max(1);
            let mut off = imm;
            let end = (imm + form.unsolicited).min(data.len());
            let mut sn = 0u32;
            while off < end {
                let n = (end - off).min(seg);
                let last = off + n >= end;
                let d = DataOut {
                    itt,
                    ttt: NO_TTT,
                    data_sn: sn,
                    offset: off as u32,
                    last,
                    bad_digest: bad(data_out_count, form),
                };
                self.data_out(d, &data[off..off + n])?;
                data_out_count += 1;
                sn += 1;
                off += n;
            }
        }
        // The rest on the target's R2Ts. Every R2T and Reject seen is kept for the caller to judge.
        let mut seen = Vec::new();
        loop {
            match self.recv(PDU_WAIT) {
                Received::Pdu(p) if p.opcode() == pdu::NOP_IN && p.itt() == NO_TTT => {
                    let _ = self.answer_ping(&p);
                }
                Received::Pdu(p) if p.opcode() == pdu::R2T && p.itt() == itt => {
                    let start = p.offset() as usize;
                    let want = p.word44() as usize;
                    let end = (start + want).min(data.len());
                    let seg = form.segment.unwrap_or(self.negotiated.max_recv).max(1);
                    let mut off = start;
                    let mut sn = 0u32;
                    while off < end {
                        let n = (end - off).min(seg);
                        let last = off + n >= end;
                        let extra = if last { form.solicited_extra } else { 0 };
                        let slice_end = (off + n + extra).min(data.len());
                        let d = DataOut {
                            itt,
                            ttt: p.ttt(),
                            data_sn: sn,
                            offset: off as u32,
                            last,
                            bad_digest: bad(data_out_count, form),
                        };
                        self.data_out(d, &data[off..slice_end])?;
                        data_out_count += 1;
                        sn += 1;
                        off += n;
                    }
                    seen.push(p);
                }
                Received::Pdu(p) if p.itt() == itt && p.opcode() == pdu::SCSI_RESPONSE => {
                    return Ok((seen, Received::Pdu(p)));
                }
                Received::Pdu(p) if p.opcode() == pdu::REJECT => {
                    // A Reject ends nothing by itself; keep reading for the task's status or close.
                    seen.push(p);
                    let (more, end) = self.collect_until_status(itt);
                    seen.extend(more);
                    return Ok((seen, end));
                }
                Received::Pdu(p) => seen.push(p),
                other => return Ok((seen, other)),
            }
        }
    }

    /// After a Reject, reads on for the task's SCSI Response or a close.
    fn collect_until_status(&mut self, itt: u32) -> (Vec<Pdu>, Received) {
        let mut seen = Vec::new();
        loop {
            match self.recv(PDU_WAIT) {
                Received::Pdu(p) if p.opcode() == pdu::NOP_IN && p.itt() == NO_TTT => {
                    let _ = self.answer_ping(&p);
                }
                Received::Pdu(p) if p.itt() == itt && p.opcode() == pdu::SCSI_RESPONSE => {
                    return (seen, Received::Pdu(p))
                }
                Received::Pdu(p) if p.opcode() == pdu::R2T && p.itt() == itt => {
                    seen.push(p); // a recovery R2T; the caller judges it
                    return (seen, Received::Silent);
                }
                Received::Pdu(p) => seen.push(p),
                other => return (seen, other),
            }
        }
    }

    /// Logs out (reason 0, close the session) and waits for the response.
    pub(super) fn logout(mut self) -> Result<()> {
        let itt = self.next_itt();
        let bhs = pdu::logout_request(itt, 0, self.cmd_sn, self.exp_stat_sn);
        self.send(&bhs, &[])?;
        loop {
            match self.recv(PDU_WAIT) {
                Received::Pdu(p) if p.opcode() == 0x26 => {
                    return pdu::check_logout_response(&p.bhs)
                }
                Received::Pdu(_) => continue,
                other => return Err(format!("logout: {other:?}")),
            }
        }
    }
}

/// How a WRITE's data travels (RFC 7143 4.2.5.2, 11.7): the immediate part in the command PDU, an
/// unsolicited part at once, the rest on R2Ts, with options for the error cases.
#[derive(Debug, Clone, Default)]
pub(super) struct WriteForm {
    /// Bytes carried in the command PDU as immediate data.
    pub immediate: usize,
    /// Bytes sent at once as Data-Out with target transfer tag FFFFFFFFh (needs InitialR2T=No).
    pub unsolicited: usize,
    /// The Data-Out at this index (counted from 0) is sent with a wrong data digest.
    pub bad_data_out: Option<usize>,
    /// Bytes added past what an R2T's last Data-Out should carry (too much solicited data).
    pub solicited_extra: usize,
    /// Data-Out segments no longer than this; the target's MaxRecvDataSegmentLength when None.
    pub segment: Option<usize>,
    /// The command PDU's EDTL, when it is not the data length.
    pub edtl: Option<u32>,
}

/// The data a command read: the Data-In segments laid at their offsets.
pub(super) fn gather(data_ins: &[Pdu], len: usize) -> Vec<u8> {
    let mut out = vec![0u8; len];
    for p in data_ins.iter().filter(|p| p.opcode() == pdu::DATA_IN) {
        let at = p.offset() as usize;
        let end = (at + p.data.len()).min(len);
        if at < end {
            out[at..end].copy_from_slice(&p.data[..end - at]);
        }
    }
    out
}

impl Commands for PduConnection {
    /// A command as `Session::command` runs it, for the metadata commands the cases need (READ
    /// CAPACITY, a read-back): reads only; a write goes through the cases' own Data-Out.
    fn command(&mut self, cdb: &[u8], read: u32, write: Option<&[u8]>) -> Result<Outcome> {
        if write.is_some() {
            return Err("PduConnection::command does not write".into());
        }
        let (mut pdus, end) = self.read_command(cdb, read)?;
        let last = match end {
            Received::Pdu(p) => p,
            other => return Err(format!("no status: {other:?}")),
        };
        let sense = if last.opcode() == pdu::SCSI_RESPONSE {
            last.sense()
        } else {
            Vec::new()
        };
        let status = last.status();
        pdus.push(last);
        let received = pdus
            .iter()
            .filter(|p| p.opcode() == pdu::DATA_IN)
            .map(|p| p.data.len())
            .sum();
        Ok(Outcome {
            status,
            sense,
            data: gather(&pdus, read as usize),
            received,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_in(offset: u32, data: &[u8]) -> Pdu {
        let mut bhs = [0u8; BHS_LEN];
        bhs[0] = pdu::DATA_IN;
        bhs[40..44].copy_from_slice(&offset.to_be_bytes());
        Pdu {
            bhs,
            data: data.to_vec(),
        }
    }

    #[test]
    fn gathered_data_lands_at_each_offset() {
        let pdus = [data_in(4, b"efgh"), data_in(0, b"abcd")];
        assert_eq!(gather(&pdus, 8), b"abcdefgh");
        assert_eq!(gather(&pdus, 6), b"abcdef");
    }

    #[test]
    fn sense_is_read_from_its_length() {
        let p = Pdu {
            bhs: [0; BHS_LEN],
            data: vec![0, 3, 0x70, 0, 5, 9],
        };
        assert_eq!(p.sense(), vec![0x70, 0, 5]);
        assert!(Pdu {
            bhs: [0; BHS_LEN],
            data: vec![]
        }
        .sense()
        .is_empty());
    }
}
