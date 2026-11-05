//! One host as the compliance tests see it: an association (admin queue, enabled) and one I/O queue,
//! under a host NQN and host identifier the caller picks. Two `Host`s with the same NQN and identifier
//! are one host on two paths, which is how reservations and multipath tell hosts apart. Commands run
//! one at a time; Keep Alive is sent on the admin queue when the I/O queue has been busy for a while,
//! so a long test does not outlive the keep-alive timeout.

use std::{
    net::SocketAddr,
    time::{Duration, Instant},
};

use super::{
    controller::{self, Association, Controller},
    identify,
    link::{Data, Link},
    sqe::{Command, Completion},
    Result,
};

/// Keep Alive goes out when this long has passed since the last one: well inside `KATO_MS`.
const KEEP_ALIVE_EVERY: Duration = Duration::from_millis(controller::KATO_MS as u64 / 4);
/// The I/O queue size, 0-based, capped by the controller's MQES.
const IO_SQSIZE: u16 = 31;

pub struct Host {
    pub admin: Controller,
    io: Link,
    cid: u16,
    last_keep_alive: Instant,
    /// Write data up to this many bytes travels in the capsule (the controller's in-capsule size).
    inline: usize,
}

impl Host {
    /// Connects and enables an association to `subnqn` at `addr` as `hostnqn`/`hostid`, then connects
    /// its first I/O queue.
    pub fn open(addr: SocketAddr, subnqn: &str, hostnqn: &str, hostid: [u8; 16]) -> Result<Self> {
        let assoc = Association {
            addr,
            subnqn,
            hostnqn,
            hostid,
        };
        let mut admin = Controller::connect(&assoc)?;
        let cap = admin.property_get(controller::CAP, true)?;
        admin.enable()?;
        let inline =
            identify::parse_controller(&admin.identify(identify::CNS_CONTROLLER, 0)?).inline_bytes;
        let sqsize = IO_SQSIZE.min((cap & 0xffff) as u16);
        let io = controller::connect_io_queue(&assoc, admin.cntlid(), 1, sqsize)?;
        Ok(Self {
            admin,
            io,
            cid: 1,
            last_keep_alive: Instant::now(),
            inline,
        })
    }

    /// Runs one I/O command to completion.
    pub fn io(&mut self, cmd: Command, data: Data) -> Result<Completion> {
        if self.last_keep_alive.elapsed() >= KEEP_ALIVE_EVERY {
            self.admin.keep_alive()?;
            self.last_keep_alive = Instant::now();
        }
        self.cid = self.cid.wrapping_add(1).max(1);
        self.io.submit(self.cid, cmd, data)
    }

    /// Runs one I/O command that sends `data`: in the capsule when it fits, else when the controller
    /// asks for it (R2T).
    pub fn send(&mut self, cmd: Command, data: &[u8]) -> Result<Completion> {
        let how = if data.len() <= self.inline {
            Data::Inline(data)
        } else {
            Data::Out(data)
        };
        self.io(cmd, how)
    }
}
