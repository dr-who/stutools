//! The NVMe/TCP cases of `--compliance`. Every `--nvme` HOST is discovered (the discovery log page) and
//! each NVM subsystem it reports is tested; HOST/NQN tests that one. Nothing is written: the I/O cases
//! read. The expectations come from the NVMe base and NVMe-oF specifications:
//! - nvme-disc: the discovery controller connects, enables and returns a log page whose records name
//!   a transport, an address and a subsystem;
//! - nvme-conn: Fabrics Connect returns a controller ID, and a second association gets another one;
//! - nvme-prop: CAP reports queue entries and the NVM command set, VS is 1.0 or later, enabling sets
//!   CSTS.RDY;
//! - nvme-ident: Identify Controller names the subsystem connected to and has a serial and model;
//!   each active namespace has blocks of 512 bytes or more, a power of two;
//! - nvme-admin: Keep Alive succeeds and a reserved admin opcode is Invalid Command Opcode;
//! - nvme-io: an I/O queue connects, the first and last blocks read, Flush succeeds, a read past the
//!   end is LBA Out of Range and a read of an inactive namespace is Invalid Namespace or Format;
//! - nvme-sess: three associations in turn connect and enable.

use super::controller::{self, Association, Controller};
use super::link::Data;
use super::sqe::{self, Command, Completion};
use super::{discover_subsystems, Discovered, DEFAULT_HOSTNQN};
use crate::compliance::{case, Case};
use crate::Result;

/// SUBTYPE in a discovery log record: an NVM subsystem (2), not a discovery controller (1 or 3).
const SUBTYPE_NVM: u8 = 2;

/// Status codes of the generic command status type (SCT 0).
const SC_INVALID_OPCODE: u8 = 0x01;
const SC_INVALID_NAMESPACE: u8 = 0x0b;
const SC_LBA_OUT_OF_RANGE: u8 = 0x80;

/// An admin opcode the base specification reserves.
const RESERVED_ADMIN_OPCODE: u8 = 0x03;

/// The VS property: the specification version the controller implements.
const VS: u32 = 0x08;

/// Adds the NVMe/TCP cases for the `--nvme` targets to `cases`.
pub fn cases(entries: &[String], cases: &mut Vec<Case>) -> Result<()> {
    let mut subsystems = Vec::new();
    for entry in entries {
        if entry.contains('/') {
            let path = super::parse_path(entry)?;
            subsystems.push((path.addr, path.subnqn));
            continue;
        }
        let records = discovery(entry, cases);
        for r in records.into_iter().filter(|r| r.subtype == SUBTYPE_NVM) {
            let host = if r.traddr.contains(':') {
                format!("[{}]:{}", r.traddr, r.trsvcid)
            } else {
                format!("{}:{}", r.traddr, r.trsvcid)
            };
            subsystems.push((super::host_address(&host)?, r.subnqn));
        }
    }
    for (addr, subnqn) in &subsystems {
        subsystem(*addr, subnqn, cases);
    }
    Ok(())
}

/// The discovery cases for one host, and the records its log page holds.
fn discovery(host: &str, cases: &mut Vec<Case>) -> Vec<Discovered> {
    match discover_subsystems(host) {
        Err(e) => {
            cases.push(case(
                "nvme-disc",
                format!("{host} log page"),
                "read".into(),
                e,
                false,
            ));
            Vec::new()
        }
        Ok(list) => {
            cases.push(case(
                "nvme-disc",
                format!("{host} log page"),
                "read".into(),
                format!("{} records", list.len()),
                true,
            ));
            for d in &list {
                let well_formed =
                    d.transport == "tcp" && !d.traddr.is_empty() && !d.trsvcid.is_empty();
                cases.push(case(
                    "nvme-disc",
                    format!("{host} record {}", d.subnqn),
                    "tcp, an address, a port and a subtype".into(),
                    format!(
                        "{} {}:{} subtype {}",
                        d.transport, d.traddr, d.trsvcid, d.subtype
                    ),
                    well_formed && !d.subnqn.is_empty() && (1..=3).contains(&d.subtype),
                ));
            }
            list
        }
    }
}

/// Every case for one subsystem.
fn subsystem(addr: std::net::SocketAddr, subnqn: &str, cases: &mut Vec<Case>) {
    let name = format!("{} at {addr}", subnqn.rsplit(':').next().unwrap_or(subnqn));
    let hostid = match controller::random_hostid() {
        Ok(id) => id,
        Err(e) => {
            cases.push(case(
                "nvme-conn",
                format!("{name} host ID"),
                "made".into(),
                e,
                false,
            ));
            return;
        }
    };
    let assoc = Association {
        addr,
        subnqn,
        hostnqn: DEFAULT_HOSTNQN,
        hostid,
    };

    // Connect, and a second association alongside it.
    let mut c = match Controller::connect(&assoc) {
        Ok(c) => c,
        Err(e) => {
            cases.push(case(
                "nvme-conn",
                format!("{name} connect"),
                "a controller ID".into(),
                e,
                false,
            ));
            return;
        }
    };
    cases.push(case(
        "nvme-conn",
        format!("{name} connect"),
        "a controller ID".into(),
        format!("{:#06x}", c.cntlid()),
        c.cntlid() != 0xffff,
    ));
    let second = Controller::connect(&assoc);
    cases.push(case(
        "nvme-conn",
        format!("{name} a second association"),
        "another controller ID".into(),
        match &second {
            Ok(s) => format!("{:#06x}", s.cntlid()),
            Err(e) => e.clone(),
        },
        second.as_ref().is_ok_and(|s| s.cntlid() != c.cntlid()),
    ));
    drop(second);

    // Properties, and enabling the controller.
    let cap = c.property_get(controller::CAP, true);
    cases.push(case(
        "nvme-prop",
        format!("{name} CAP"),
        "MQES > 0, the NVM command set".into(),
        match &cap {
            Ok(v) => format!("MQES {}, CSS {:#x}", v & 0xffff, (v >> 37) & 0xff),
            Err(e) => e.clone(),
        },
        cap.as_ref()
            .is_ok_and(|v| v & 0xffff > 0 && (v >> 37) & 1 == 1),
    ));
    let vs = c.property_get(VS, false);
    cases.push(case(
        "nvme-prop",
        format!("{name} VS"),
        "1.0 or later".into(),
        match &vs {
            Ok(v) => format!("{}.{}", v >> 16, (v >> 8) & 0xff),
            Err(e) => e.clone(),
        },
        vs.as_ref().is_ok_and(|v| v >> 16 >= 1),
    ));
    let enabled = c.enable();
    cases.push(case(
        "nvme-prop",
        format!("{name} enable"),
        "CSTS.RDY".into(),
        match &enabled {
            Ok(()) => "ready".into(),
            Err(e) => e.clone(),
        },
        enabled.is_ok(),
    ));
    if enabled.is_err() {
        return;
    }

    // Identify the controller and every active namespace.
    let info = c
        .identify(controller::CNS_CONTROLLER, 0)
        .map(|d| controller::parse_controller(&d));
    cases.push(case(
        "nvme-ident",
        format!("{name} controller"),
        format!("SUBNQN {subnqn}, a serial and model"),
        match &info {
            Ok(i) => format!(
                "SUBNQN {}, serial {:?}, model {:?}",
                i.nqn, i.serial, i.model
            ),
            Err(e) => e.clone(),
        },
        info.as_ref()
            .is_ok_and(|i| i.nqn == subnqn && !i.serial.is_empty() && !i.model.is_empty()),
    ));
    let active = c
        .identify(controller::CNS_ACTIVE_NAMESPACES, 0)
        .map(|d| controller::parse_active_namespaces(&d));
    cases.push(case(
        "nvme-ident",
        format!("{name} active namespaces"),
        "listed".into(),
        match &active {
            Ok(list) => format!("{list:?}"),
            Err(e) => e.clone(),
        },
        active.is_ok(),
    ));
    let active = active.unwrap_or_default();
    let mut namespaces = Vec::new();
    for &nsid in &active {
        let ns = c
            .identify(controller::CNS_NAMESPACE, nsid)
            .map(|d| controller::parse_namespace(&d));
        let ok = ns
            .as_ref()
            .is_ok_and(|n| n.blocks > 0 && n.block_len >= 512 && n.block_len.is_power_of_two());
        cases.push(case(
            "nvme-ident",
            format!("{name} namespace {nsid}"),
            "blocks > 0 of 512 B or more, a power of two".into(),
            match &ns {
                Ok(n) => format!("{} blocks of {} B", n.blocks, n.block_len),
                Err(e) => e.clone(),
            },
            ok,
        ));
        if let Ok(n) = ns {
            if ok {
                namespaces.push((nsid, n));
            }
        }
    }

    // Admin commands.
    let kept = c.keep_alive();
    cases.push(case(
        "nvme-admin",
        format!("{name} Keep Alive"),
        "success".into(),
        match &kept {
            Ok(()) => "success".into(),
            Err(e) => e.clone(),
        },
        kept.is_ok(),
    ));
    let reserved = c.submit(Command::new(RESERVED_ADMIN_OPCODE, 0), Data::None);
    status_case(
        cases,
        "nvme-admin",
        format!("{name} reserved admin opcode {RESERVED_ADMIN_OPCODE:#04x}"),
        &[SC_INVALID_OPCODE],
        "Invalid Command Opcode",
        &reserved,
    );

    // I/O, on one queue of this association: reads only.
    let cntlid = c.cntlid();
    let mut io = match controller::connect_io_queue(&assoc, cntlid, 1, 31) {
        Ok(link) => {
            cases.push(case(
                "nvme-io",
                format!("{name} I/O queue"),
                "connected".into(),
                "connected".into(),
                true,
            ));
            link
        }
        Err(e) => {
            cases.push(case(
                "nvme-io",
                format!("{name} I/O queue"),
                "connected".into(),
                e,
                false,
            ));
            sessions(&assoc, &name, cases);
            return;
        }
    };
    let mut cid = 1u16;
    let mut next = || {
        cid = cid.wrapping_add(1).max(1);
        cid
    };
    for (nsid, ns) in &namespaces {
        let mut buf = vec![0u8; ns.block_len as usize];
        for (label, lba) in [("first", 0), ("last", ns.blocks - 1)] {
            let done = io.submit(next(), read(*nsid, lba), Data::In(&mut buf));
            status_case(
                cases,
                "nvme-io",
                format!("{name} ns {nsid} read the {label} block"),
                &[0],
                "success",
                &done,
            );
        }
        let done = io.submit(next(), read(*nsid, ns.blocks), Data::In(&mut buf));
        status_case(
            cases,
            "nvme-io",
            format!("{name} ns {nsid} read past the end"),
            &[SC_LBA_OUT_OF_RANGE],
            "LBA Out of Range",
            &done,
        );
        let mut flush = Command::new(sqe::OP_FLUSH, 0);
        flush.set_nsid(*nsid);
        let done = io.submit(next(), flush, Data::None);
        status_case(
            cases,
            "nvme-io",
            format!("{name} ns {nsid} Flush"),
            &[0],
            "success",
            &done,
        );
    }
    if let Some((_, ns)) = namespaces.first() {
        let inactive = (1..=u32::MAX - 1)
            .find(|n| !active.contains(n))
            .unwrap_or(u32::MAX - 1);
        let mut buf = vec![0u8; ns.block_len as usize];
        let done = io.submit(next(), read(inactive, 0), Data::In(&mut buf));
        status_case(
            cases,
            "nvme-io",
            format!("{name} read of inactive namespace {inactive}"),
            &[SC_INVALID_NAMESPACE],
            "Invalid Namespace or Format",
            &done,
        );
    }
    drop(io);
    drop(c);
    sessions(&assoc, &name, cases);
}

/// Three associations in turn, each connected and enabled, then closed.
fn sessions(assoc: &Association, name: &str, cases: &mut Vec<Case>) {
    for k in 1..=3 {
        let result = Controller::connect(assoc).and_then(|mut c| c.enable().map(|()| c.cntlid()));
        cases.push(case(
            "nvme-sess",
            format!("{name} association {k}"),
            "connect and enable".into(),
            match &result {
                Ok(id) => format!("controller {id:#06x}"),
                Err(e) => e.clone(),
            },
            result.is_ok(),
        ));
    }
}

/// A Read of one block at `lba` of namespace `nsid`.
fn read(nsid: u32, lba: u64) -> Command {
    let mut cmd = Command::new(sqe::OP_READ, 0);
    cmd.set_nsid(nsid);
    cmd.set_u64(40, lba);
    cmd.set_u16(48, 0); // NLB, 0-based: one block
    cmd
}

/// A case that passes when a command completed with one of `codes` in the generic status type (0 is
/// success).
fn status_case(
    cases: &mut Vec<Case>,
    section: &'static str,
    name: String,
    codes: &[u8],
    expected: &str,
    done: &Result<Completion>,
) {
    let (actual, passed) = match done {
        Ok(c) if c.ok() => ("success".to_string(), codes.contains(&0)),
        Ok(c) => {
            let (sct, sc) = c.code();
            (
                format!("status type {sct:#x} code {sc:#04x}"),
                sct == 0 && codes.contains(&sc),
            )
        }
        Err(e) => (e.clone(), false),
    };
    cases.push(case(section, name, expected.into(), actual, passed));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_read_names_its_block_and_namespace() {
        let c = read(7, 0x0102_0304_0506);
        assert_eq!(c.0[0], sqe::OP_READ);
        assert_eq!(&c.0[4..8], &7u32.to_le_bytes());
        assert_eq!(&c.0[40..48], &0x0102_0304_0506u64.to_le_bytes());
        assert_eq!(&c.0[48..50], &[0, 0]);
    }

    #[test]
    fn status_cases_match_the_generic_codes() {
        let mut cases = Vec::new();
        let mut b = [0u8; 16];
        b[14..16].copy_from_slice(&((SC_LBA_OUT_OF_RANGE as u16) << 1).to_le_bytes());
        let done = Ok(Completion::parse(&b));
        status_case(
            &mut cases,
            "nvme-io",
            "past".into(),
            &[SC_LBA_OUT_OF_RANGE],
            "LBA Out of Range",
            &done,
        );
        status_case(&mut cases, "nvme-io", "ok".into(), &[0], "success", &done);
        assert!(cases[0].passed);
        assert!(!cases[1].passed);
    }
}
