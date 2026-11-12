//! One NVMe/TCP connection. Each queue (the admin queue and every I/O queue) is its own TCP connection
//! after ICReq/ICResp. `submit` runs one command to completion; a pipelining session uses `send` and
//! `read` directly and matches completions to commands itself.

use std::{
    net::{SocketAddr, TcpStream},
    time::Duration,
};

use super::{
    pdu::{self, Pdu},
    sqe::{Command, Completion},
    Result,
};

const TIMEOUT: Duration = Duration::from_secs(30);
/// The largest H2CData PDU spit sends when the controller names none.
const DEFAULT_H2C: u32 = 64 * 1024;
/// Room reserved up front for the frames and data of one link, so the hot path does not allocate.
pub const FRAME_CAPACITY: usize = 1 << 20;

/// Where the data of one command goes or comes from.
pub enum Data<'a> {
    None,
    /// The controller returns `buf.len()` bytes (C2HData).
    In(&'a mut [u8]),
    /// The host sends `data` when the controller asks for it (R2T and H2CData).
    Out(&'a [u8]),
    /// The host sends `data` in the capsule itself.
    Inline(&'a [u8]),
}

/// A TCP connection to one NVMe/TCP queue, after the ICReq/ICResp exchange.
pub struct Link {
    stream: TcpStream,
    max_h2c: u32,
    /// Reused for every frame this link sends, so the hot path does not allocate.
    tx: Vec<u8>,
    /// Reused for every PDU this link reads.
    rx: Pdu,
}

impl Link {
    /// Opens the TCP connection and exchanges ICReq and ICResp.
    pub fn open(addr: SocketAddr) -> Result<Self> {
        let stream = TcpStream::connect_timeout(&addr, TIMEOUT)
            .map_err(|e| format!("NVMe/TCP: connecting to {addr}: {e}"))?;
        stream
            .set_nodelay(true)
            .and_then(|_| stream.set_read_timeout(Some(TIMEOUT)))
            .and_then(|_| stream.set_write_timeout(Some(TIMEOUT)))
            .map_err(|e| format!("NVMe/TCP: socket setup: {e}"))?;
        let mut link = Self {
            stream,
            max_h2c: DEFAULT_H2C,
            tx: Vec::with_capacity(FRAME_CAPACITY),
            rx: Pdu::with_capacity(pdu::ICREQ_LEN, FRAME_CAPACITY),
        };
        link.send(&pdu::icreq())?;
        let resp = pdu::parse_icresp(&link.read()?)?;
        if resp.max_h2c_data != 0 {
            link.max_h2c = resp.max_h2c_data.min(DEFAULT_H2C);
        }
        Ok(link)
    }

    /// Writes bytes that are already framed as PDUs.
    pub fn send(&mut self, bytes: &[u8]) -> Result<()> {
        pdu::write(&mut self.stream, bytes)
    }

    /// Reads the next PDU from the controller.
    pub fn read(&mut self) -> Result<Pdu> {
        pdu::read(&mut self.stream)
    }

    /// Reads the next PDU into `p`, reusing its buffers.
    pub fn read_into(&mut self, p: &mut Pdu) -> Result<()> {
        pdu::read_into(&mut self.stream, p)
    }

    /// Sends a CapsuleCmd carrying `sqe` and `data`, framed in the link's reused buffer.
    pub fn send_capsule(&mut self, sqe: &[u8; 64], data: &[u8]) -> Result<()> {
        pdu::frame_into(
            &mut self.tx,
            pdu::CAPSULE_CMD,
            0,
            pdu::CAPSULE_CMD_LEN,
            |h| h[8..].copy_from_slice(sqe),
            data,
        );
        pdu::write(&mut self.stream, &self.tx)
    }

    /// Sends one command with `cid` and waits for its completion, moving data as `data` describes.
    pub fn submit(&mut self, cid: u16, mut cmd: Command, data: Data) -> Result<Completion> {
        cmd.0[2..4].copy_from_slice(&cid.to_le_bytes());
        let inline: &[u8] = match &data {
            Data::None | Data::In(_) | Data::Out(_) => &[],
            Data::Inline(b) => b,
        };
        match &data {
            Data::None => cmd.set_data(0, false),
            Data::In(b) => cmd.set_data(b.len(), false),
            Data::Out(b) => cmd.set_data(b.len(), false),
            Data::Inline(b) => cmd.set_data(b.len(), true),
        }
        self.send_capsule(&cmd.0, inline)?;
        let (mut sink, source): (Option<&mut [u8]>, Option<&[u8]>) = match data {
            Data::None | Data::Inline(_) => (None, None),
            Data::In(b) => (Some(b), None),
            Data::Out(b) => (None, Some(b)),
        };
        loop {
            let mut p = std::mem::replace(&mut self.rx, Pdu::empty());
            self.read_into(&mut p)?;
            let result = self.handle_sync(cid, &mut sink, source, &p);
            self.rx = p;
            if let Some(done) = result? {
                return Ok(done);
            }
        }
    }

    /// Handles one PDU received while a synchronous command is in flight: returns the completion when
    /// the command is done, and None while it still waits for more.
    fn handle_sync(
        &mut self,
        cid: u16,
        sink: &mut Option<&mut [u8]>,
        source: Option<&[u8]>,
        p: &Pdu,
    ) -> Result<Option<Completion>> {
        match p.kind {
            pdu::C2H_DATA => {
                let c2h = pdu::parse_c2h(p)?;
                let buf = sink
                    .as_deref_mut()
                    .ok_or("NVMe/TCP: controller sent data for a command with none")?;
                if c2h.cccid != cid {
                    return Err(format!(
                        "NVMe/TCP: data for command {} while {cid} is in flight",
                        c2h.cccid
                    ));
                }
                let (start, end) = (c2h.datao as usize, c2h.datao as usize + c2h.datal as usize);
                buf.get_mut(start..end)
                    .ok_or("NVMe/TCP: C2HData outside the command's buffer")?
                    .copy_from_slice(&p.data);
                if c2h.success {
                    return Ok(Some(Completion {
                        dw0: 0,
                        wide: 0,
                        sq_head: 0,
                        cid,
                        status: 0,
                    }));
                }
                Ok(None)
            }
            pdu::R2T => {
                let r2t = pdu::parse_r2t(p)?;
                let buf = source
                    .ok_or("NVMe/TCP: controller asked for data this command does not send")?;
                self.send_h2c(cid, r2t, buf)?;
                Ok(None)
            }
            pdu::CAPSULE_RESP => {
                let done = Completion::parse(&pdu::parse_capsule_resp(p)?);
                if done.cid != cid {
                    return Err(format!(
                        "NVMe/TCP: completion for command {} while {cid} is in flight",
                        done.cid
                    ));
                }
                Ok(Some(done))
            }
            pdu::TERM_C2H | pdu::TERM_H2C => Err(pdu::describe_term(p)),
            other => Err(format!(
                "NVMe/TCP: unexpected PDU type {other:#x} on a synchronous command"
            )),
        }
    }

    /// Answers one R2T with the host data it asks for, in pieces no larger than the controller allows.
    pub fn send_h2c(&mut self, cid: u16, r2t: pdu::R2t, buf: &[u8]) -> Result<()> {
        let start = r2t.r2to as usize;
        let end = start + r2t.r2tl as usize;
        let src = buf
            .get(start..end)
            .ok_or("NVMe/TCP: R2T asks for data outside the command's buffer")?;
        let mut sent = 0usize;
        while sent < src.len() {
            let n = (src.len() - sent).min(self.max_h2c as usize);
            let last = sent + n == src.len();
            pdu::frame_into(
                &mut self.tx,
                pdu::H2C_DATA,
                if last { pdu::FLAG_LAST } else { 0 },
                pdu::DATA_HEADER_LEN,
                |h| {
                    h[8..10].copy_from_slice(&cid.to_le_bytes());
                    h[10..12].copy_from_slice(&r2t.ttag.to_le_bytes());
                    h[12..16].copy_from_slice(&(r2t.r2to + sent as u32).to_le_bytes());
                    h[16..20].copy_from_slice(&(n as u32).to_le_bytes());
                },
                &src[sent..sent + n],
            );
            pdu::write(&mut self.stream, &self.tx)?;
            sent += n;
        }
        Ok(())
    }
}
