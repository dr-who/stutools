//! One pool over any mix of transports. A path names an iSCSI portal or an NVMe-oF controller, and the pool
//! runs a queue's requests across all of them with the same policies. So `--iscsi` and `--nvmeof` paths can
//! serve one LUN or namespace together, and the pool checks that every path reports the same geometry.

use super::{Done, Fabric, Geometry, Io, Pool, Transport};
use crate::iscsi::{
    connection::IscsiFabric, report::Path as IscsiPath, session::Session as IscsiSession,
};
use crate::nvme::session::{NvmeFabric, NvmePath, NvmeSession};
use crate::paths::Policy;
use crate::Result;

/// One path as the user named it.
#[derive(Clone)]
pub enum PathSpec {
    Iscsi(IscsiPath),
    Nvmeof(NvmePath),
}

/// How each transport opens its paths. A pool uses both, one for each kind of path it holds.
pub struct PathFabric {
    pub iscsi: IscsiFabric,
    pub nvme: NvmeFabric,
}

/// An open path of either transport.
pub enum AnySession {
    Iscsi(IscsiSession),
    Nvmeof(NvmeSession),
}

/// The pool a queue runs over, whatever mix of transports it holds.
pub type MultiPool = Pool<PathFabric>;

/// Opens every path. iSCSI paths log in and NVMe-oF paths connect; the pool checks the geometry.
pub fn open_paths(
    paths: &[PathSpec],
    iscsi: IscsiFabric,
    nvme: NvmeFabric,
    policy: Policy,
    seed: u64,
) -> Result<MultiPool> {
    Pool::open(PathFabric { iscsi, nvme }, paths, policy, seed)
}

impl Fabric for PathFabric {
    type Path = PathSpec;
    type Session = AnySession;

    fn label(&self, path: &PathSpec) -> String {
        match path {
            PathSpec::Iscsi(p) => self.iscsi.label(p),
            PathSpec::Nvmeof(p) => self.nvme.label(p),
        }
    }

    fn open(&self, path: &PathSpec) -> Result<(AnySession, Geometry)> {
        match path {
            PathSpec::Iscsi(p) => {
                let (session, geometry) = self.iscsi.open(p)?;
                Ok((AnySession::Iscsi(session), geometry))
            }
            PathSpec::Nvmeof(p) => {
                let (session, geometry) = self.nvme.open(p)?;
                Ok((AnySession::Nvmeof(session), geometry))
            }
        }
    }
}

impl Transport for AnySession {
    fn can_submit(&self) -> bool {
        match self {
            AnySession::Iscsi(s) => Transport::can_submit(s),
            AnySession::Nvmeof(s) => Transport::can_submit(s),
        }
    }

    fn inflight(&self) -> usize {
        match self {
            AnySession::Iscsi(s) => Transport::inflight(s),
            AnySession::Nvmeof(s) => Transport::inflight(s),
        }
    }

    fn tier(&self) -> Option<u8> {
        match self {
            AnySession::Iscsi(s) => Transport::tier(s),
            AnySession::Nvmeof(s) => Transport::tier(s),
        }
    }

    fn check(&self, io: Io) -> Result<()> {
        match self {
            AnySession::Iscsi(s) => Transport::check(s, io),
            AnySession::Nvmeof(s) => Transport::check(s, io),
        }
    }

    unsafe fn submit(&mut self, tag: usize, io: Io, buf: *mut u8, len: usize) -> Result<()> {
        match self {
            // SAFETY: the pool keeps `buf` valid until `poll` returns `tag`.
            AnySession::Iscsi(s) => unsafe { Transport::submit(s, tag, io, buf, len) },
            // SAFETY: as above.
            AnySession::Nvmeof(s) => unsafe { Transport::submit(s, tag, io, buf, len) },
        }
    }

    fn poll(&mut self) -> Result<Done> {
        match self {
            AnySession::Iscsi(s) => Transport::poll(s),
            AnySession::Nvmeof(s) => Transport::poll(s),
        }
    }

    fn tick(&mut self) -> Result<()> {
        match self {
            AnySession::Iscsi(s) => Transport::tick(s),
            AnySession::Nvmeof(s) => Transport::tick(s),
        }
    }

    fn failure(&self, io: Io, done: &Done) -> String {
        match self {
            AnySession::Iscsi(s) => Transport::failure(s, io, done),
            AnySession::Nvmeof(s) => Transport::failure(s, io, done),
        }
    }

    fn unavailable(&self, done: &Done) -> String {
        match self {
            AnySession::Iscsi(s) => Transport::unavailable(s, done),
            AnySession::Nvmeof(s) => Transport::unavailable(s, done),
        }
    }
}
