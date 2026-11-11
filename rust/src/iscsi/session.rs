//! A user-space iSCSI session: TCP connect, login, SCSI commands (reads with Data-In, writes with
//! R2T and Data-Out), header and data digests when negotiated, NOP replies, and logout.

use super::crc32c::crc32c;
use super::login::{Negotiated, LOGIN_ITT};
use super::path::Path;
use super::pdu::{self, BHS_LEN, NO_TTT};
use crate::paths::{Done, SENSE_CAPACITY};
use crate::Result;
use std::{
    io::{Read, Write},
    net::TcpStream,
    time::{Duration, Instant},
};

/// How long a synchronous command may take before the answer is given up on, however many PDUs the target
/// sends meanwhile (a target that keeps answering with NOP-In would otherwise hold the command forever).
const COMMAND_LIMIT: Duration = Duration::from_secs(10);

/// How a target answered a task management request: a response code (RFC 7143 11.6.1: 0 function
/// complete, 1 task does not exist, 5 not supported, 255 rejected), or a Reject PDU with its reason.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TaskAnswer {
    Response(u8),
    Rejected(u8),
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
    /// The 8-byte LUN field every command carries (`Path::lun_field`); 0 unless `for_lun` set it.
    lun: u64,
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
    /// Data-In bytes of the last command that finished.
    last_received: usize,
}

/// The start of the error a command gets when the target rejects its PDU, followed by the reason.
pub const REJECTED: &str = "target rejected the PDU, reason";

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
    /// The read buffer, as long as was asked for.
    pub data: Vec<u8>,
    /// How many bytes of it the target sent (Data-In), which is less than asked for when the target had
    /// less (SPC-6 4.2.5.6).
    pub received: usize,
}

impl Session {
    /// A session that login has just brought to full feature phase on `stream`, with the command
    /// numbers the final login response gave.
    pub(super) fn established(
        stream: TcpStream,
        negotiated: Negotiated,
        cmd_sn: u32,
        exp_stat_sn: u32,
        max_cmd_sn: u32,
    ) -> Session {
        Session {
            access_tier: None,
            stream,
            tx: Vec::with_capacity(BHS_LEN + RX_CAPACITY + 16),
            rx: Vec::with_capacity(RX_CAPACITY),
            sense: Vec::with_capacity(256),
            inflight: Vec::with_capacity(MAX_INFLIGHT),
            last_received: 0,
            negotiated,
            itt: LOGIN_ITT + 1,
            cmd_sn,
            exp_stat_sn,
            max_cmd_sn,
            lun: 0,
        }
    }

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
    fn frame(&mut self, header: &[u8], data: &[u8]) -> Result<()> {
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
            lun: self.lun,
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
        if cdb.len() > 16 {
            let mut long = header.to_vec();
            long.extend_from_slice(&pdu::extended_cdb(cdb));
            self.frame(&long, &[])?;
        } else {
            self.frame(&header, &[])?;
        }
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
        self.poll_until(None)
    }

    /// `poll`, giving up once `deadline` has passed.
    fn poll_until(&mut self, deadline: Option<Instant>) -> Result<Done> {
        loop {
            if deadline.is_some_and(|d| Instant::now() > d) {
                return Err(format!(
                    "no answer to the command within {} s",
                    COMMAND_LIMIT.as_secs()
                ));
            }
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
                            self.last_received = inf.received;
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
                    self.last_received = inf.received;
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
                pdu::NOP_IN => self.answer_nop(&bhs)?,
                // RFC 7143 11.17: the target refuses a PDU it cannot take (reason 05h, command not
                // supported; 09h, invalid PDU field). The command it carried is over.
                pdu::REJECT => {
                    self.observe(pdu::sequence(&bhs));
                    let rejected = pdu::be32(&self.rx, 16);
                    self.inflight.retain(|i| i.itt != rejected);
                    return Err(format!("{REJECTED} {:#04x}", bhs[2]));
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
        crate::watchdog::beat(cdb);
        if !self.inflight.is_empty() {
            return Err("a command is already in flight on this session".into());
        }
        let (buf, len) = match write {
            Some(data) => (data.as_ptr() as *mut u8, data.len()),
            None => (read.as_mut_ptr(), read.len()),
        };
        // SAFETY: `buf` and `len` describe `read` or `write`, which live until the wait below returns.
        let deadline = Instant::now() + COMMAND_LIMIT;
        let result =
            unsafe { self.submit(cdb, 0, buf, len, write.is_some()) }.and_then(|()| loop {
                let done = self.poll_until(Some(deadline))?;
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
            received: self.last_received,
        })
    }

    /// Answers a NOP-In that asks for a reply (it carries a target transfer tag), and records its
    /// sequence numbers.
    fn answer_nop(&mut self, bhs: &[u8; BHS_LEN]) -> Result<()> {
        let n = pdu::decode_nop_in(bhs)?;
        self.observe(n.seq);
        if n.ttt != NO_TTT {
            let reply = pdu::nop_out(0, NO_TTT, n.ttt, self.cmd_sn, self.exp_stat_sn);
            self.frame(&reply, &[])?;
        }
        Ok(())
    }

    /// Sends a task management function for the session's logical unit and waits for the answer. `referenced` is the task
    /// tag ABORT TASK names, with `ref_cmd_sn` its CmdSN; other functions ignore both. Only for a session
    /// with nothing in flight.
    pub fn task_management(
        &mut self,
        function: u8,
        referenced: u32,
        ref_cmd_sn: u32,
    ) -> Result<TaskAnswer> {
        if !self.inflight.is_empty() {
            return Err("a command is in flight on this session".into());
        }
        let itt = self.next_itt();
        // ABORT TASK and QUERY TASK name a task (RFC 7143 11.5.1, RFC 7144 6.3).
        let rtt = if matches!(function, pdu::TMF_ABORT_TASK | pdu::TMF_QUERY_TASK) {
            referenced
        } else {
            NO_TTT
        };
        let header = pdu::task_management_request(
            function,
            self.lun,
            itt,
            rtt,
            self.cmd_sn,
            self.exp_stat_sn,
            ref_cmd_sn,
        );
        self.frame(&header, &[])?;
        loop {
            let bhs = self.recv()?;
            match bhs[0] & 0x3f {
                pdu::TASK_MANAGEMENT_RESPONSE if pdu::be32(&bhs, 16) == itt => {
                    self.observe(pdu::sequence(&bhs));
                    return Ok(TaskAnswer::Response(bhs[2]));
                }
                pdu::REJECT => return Ok(TaskAnswer::Rejected(bhs[2])),
                pdu::NOP_IN => self.answer_nop(&bhs)?,
                other => return Err(format!("unexpected PDU opcode {other:#x}")),
            }
        }
    }

    /// This session addressing the logical unit `path` names.
    pub(super) fn for_lun(mut self, path: &Path) -> Session {
        self.lun = path.lun_field();
        self
    }

    /// The CmdSN the next command will carry, which ABORT TASK uses to name a task.
    pub fn cmd_sn(&self) -> u32 {
        self.cmd_sn
    }

    /// Sends a NOP-Out ping carrying `data` and returns the data of the NOP-In that answers it
    /// (RFC 7143 11.18: the target echoes the ping's data under its task tag).
    pub fn ping(&mut self, data: &[u8]) -> Result<Vec<u8>> {
        if !self.inflight.is_empty() {
            return Err("a command is in flight on this session".into());
        }
        let itt = self.next_itt();
        let mut header = pdu::nop_out(0, itt, NO_TTT, self.cmd_sn, self.exp_stat_sn);
        pdu::set_data_len(&mut header, data.len());
        self.frame(&header, data)?;
        loop {
            let bhs = self.recv()?;
            if bhs[0] & 0x3f != pdu::NOP_IN {
                return Err(format!("unexpected PDU opcode {:#x}", bhs[0] & 0x3f));
            }
            if pdu::be32(&bhs, 16) == itt {
                self.observe(pdu::sequence(&bhs));
                return Ok(self.rx.clone());
            }
            self.answer_nop(&bhs)?;
        }
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
    use super::super::login::login;
    use super::*;

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
}
