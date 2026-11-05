//! The NVMe/TCP cases of `--compliance`. Every `--nvme` HOST is discovered (the discovery log page) and
//! each NVM subsystem it reports is tested, through the first address the log gives for it and, for
//! identity, ANA and multipath reservations, through every other one; HOST/NQN tests that one. Each
//! case names the NVMe revision that defines what it tests (`revisions`) and is judged against the
//! revision the controller's VS property reports. The sections:
//! - nvme-disc: the discovery controller connects, enables and returns a log page whose records name
//!   a transport, an address and a subsystem;
//! - nvme-conn: Fabrics Connect returns a controller ID, and a second association gets another one;
//! - nvme-prop: CAP reports queue entries and the NVM command set, VS is 1.0 or later, enabling sets
//!   CSTS.RDY;
//! - nvme-ident (`identify_cases`): Identify Controller and Namespace, identification descriptors,
//!   and the same identity on every path;
//! - nvme-admin (`admin_cases`): log pages, features, Abort, Keep Alive, Asynchronous Event Request;
//! - nvme-io (`io_cases`): reads and writes, FUA, errors, MDTS, Flush, Compare, Write Zeroes and
//!   Dataset Management, changing only blocks it writes back;
//! - nvme-ana (`ana`): the ANA log page through every address;
//! - nvme-resv (`reservations`): every reservation command and type with four hosts;
//! - nvme-sess: three associations in turn connect and enable;
//! - windows: what Windows needs of the subsystem, from the results above.

use std::net::SocketAddr;

use super::controller::{self, Association, Controller};
use super::revisions::Rev;
use super::sqe::Completion;
use super::{
    admin_cases, ana, discover_subsystems, identify_cases, io_cases, reservations, Discovered,
    DEFAULT_HOSTNQN,
};
use crate::compliance::{case, Case};
use crate::Result;

/// SUBTYPE in a discovery log record: an NVM subsystem (2), not a discovery controller (1 or 3).
const SUBTYPE_NVM: u8 = 2;

/// Status codes as (status code type, status code). SCT 0 is the generic command status (Base 2.1
/// Figure 102; NVM 1.1 Figure 17 for 80h).
pub const SUCCESS: (u8, u8) = (0, 0x00);
pub const INVALID_OPCODE: (u8, u8) = (0, 0x01);
pub const INVALID_FIELD: (u8, u8) = (0, 0x02);
pub const INVALID_NAMESPACE: (u8, u8) = (0, 0x0b);
pub const HOST_ID_INCONSISTENT: (u8, u8) = (0, 0x18);
pub const LBA_OUT_OF_RANGE: (u8, u8) = (0, 0x80);
pub const RESERVATION_CONFLICT: (u8, u8) = (0, 0x83);
/// SCT 1, command specific: Invalid Log Page (Base 2.1 Figure 103, Get Log Page).
pub const INVALID_LOG_PAGE: (u8, u8) = (1, 0x09);
/// SCT 2, media and data integrity errors: Compare Failure (NVM 1.1 Figure 19).
pub const COMPARE_FAILURE: (u8, u8) = (2, 0x85);

/// The VS property: the specification version the controller implements.
const VS: u32 = 0x08;

/// One NVM subsystem under test: its NQN and every address the discovery log gives for it, the
/// first of which runs every case.
pub struct Target {
    pub subnqn: String,
    pub addrs: Vec<SocketAddr>,
}

/// Adds the NVMe/TCP cases for the `--nvme` targets to `cases`.
pub fn cases(entries: &[String], cases: &mut Vec<Case>) -> Result<()> {
    let mut targets: Vec<Target> = Vec::new();
    let mut add =
        |subnqn: String, addr: SocketAddr| match targets.iter_mut().find(|t| t.subnqn == subnqn) {
            Some(t) if !t.addrs.contains(&addr) => t.addrs.push(addr),
            Some(_) => {}
            None => targets.push(Target {
                subnqn,
                addrs: vec![addr],
            }),
        };
    for entry in entries {
        if entry.contains('/') {
            let path = super::parse_path(entry)?;
            add(path.subnqn, path.addr);
            continue;
        }
        let records = discovery(entry, cases);
        for r in records.into_iter().filter(|r| r.subtype == SUBTYPE_NVM) {
            let host = if r.traddr.contains(':') {
                format!("[{}]:{}", r.traddr, r.trsvcid)
            } else {
                format!("{}:{}", r.traddr, r.trsvcid)
            };
            add(r.subnqn, super::host_address(&host)?);
        }
    }
    for t in &targets {
        subsystem(t, cases);
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
fn subsystem(t: &Target, cases: &mut Vec<Case>) {
    let addr = t.addrs[0];
    let subnqn = t.subnqn.as_str();
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

    let Some(facts) = identify_cases::run(
        &mut c,
        &name,
        t,
        *cap.as_ref().unwrap_or(&0),
        vs.ok(),
        cases,
    ) else {
        sessions(&assoc, &name, cases);
        return;
    };
    admin_cases::run(&mut c, &name, hostid, &facts, cases);
    drop(c);
    let io = io_cases::run(t, &name, &facts, cases);
    let ana = ana::run(t, &name, &facts, cases);
    let resv = reservations::run(t, &name, &facts, cases);
    sessions(&assoc, &name, cases);
    admin_cases::async_event(addr, subnqn, &name, cases);
    windows(&name, t, &facts, &io, ana.as_deref(), &resv, cases);
}

/// What Windows needs of this subsystem to use it as a disk on more than one path and in a cluster.
/// Windows sends SCSI commands, which stornvme translates as SCSI-NVMe 1.5 describes: the Device
/// Identification page MPIO matches paths by comes from the namespace IDs (6.1.4), SYNCHRONIZE CACHE
/// from Flush (5.5), PERSISTENT RESERVE from the reservation commands (4.13), UNMAP from Dataset
/// Management with the deallocate attribute (5.6), and the caching mode page's WCE from the Volatile
/// Write Cache feature (6.3.3.2).
fn windows(
    name: &str,
    t: &Target,
    facts: &identify_cases::Facts,
    io: &io_cases::IoFacts,
    ana: Option<&[String]>,
    resv: &reservations::Verdict,
    cases: &mut Vec<Case>,
) {
    let paths = t.addrs.len() + 1;
    cases.push(case(
        "windows",
        format!(
            "{name} MPIO: Device Identification VPD from the namespace IDs (SCSI-NVMe 1.5 6.1.4)"
        ),
        format!("a non-zero EUI-64, NGUID or UUID, the same on all {paths} paths"),
        if facts.unique_ids {
            "the same".into()
        } else {
            "missing or different".into()
        },
        facts.unique_ids,
    ));
    cases.push(case(
        "windows",
        format!("{name} NTFS: SYNCHRONIZE CACHE as Flush (SCSI-NVMe 1.5 5.5)"),
        "success on every namespace".into(),
        if io.flush {
            "success".into()
        } else {
            "failed".into()
        },
        io.flush,
    ));
    cases.push(case(
        "windows",
        format!(
            "{name} Failover Clustering: PERSISTENT RESERVE as reservations (SCSI-NVMe 1.5 4.13)"
        ),
        "reservations with RTYPE 3h (SCSI type 5h), every case passed".into(),
        if !resv.supported {
            format!(
                "reservations off (ONCS bit 5 {}, RESCAP {:#04x})",
                facts.ctrl.oncs >> 5 & 1,
                facts.namespaces.first().map_or(0, |n| n.1.rescap)
            )
        } else if resv.clustering {
            "supported, every case passed".into()
        } else {
            "supported, but a case failed or the type is missing".into()
        },
        resv.clustering,
    ));
    cases.push(case(
        "windows",
        format!("{name} MPIO path states (ANA)"),
        "reported on every path, or not reported (every path is then used alike)".into(),
        match ana {
            Some(states) if !states.is_empty() => states.join(", "),
            Some(_) => "reported, no namespace listed".into(),
            None => "not reported".into(),
        },
        ana.map_or(true, |s| !s.is_empty()),
    ));
    cases.push(case(
        "windows",
        format!("{name} TRIM: UNMAP as Dataset Management deallocate (SCSI-NVMe 1.5 5.6)"),
        "works, or is not offered".into(),
        match io.trim {
            Some(true) => "works".into(),
            Some(false) => "offered but failed".into(),
            None => "not offered (ONCS bit 2 clear)".into(),
        },
        io.trim != Some(false),
    ));
    let cache = facts.ctrl.vwc & 1 != 0;
    cases.push(case(
        "windows",
        format!("{name} write cache: caching mode page WCE (SCSI-NVMe 1.5 6.3.3.2)"),
        "reported; flushed when present".into(),
        if cache {
            "volatile write cache, Flush works"
        } else {
            "no volatile write cache"
        }
        .into(),
        !cache || io.flush,
    ));
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

/// A case that passes when a command completed with one of `want`, each a (status code type, status
/// code) pair; (0, 0) is success. Returns whether it passed.
pub fn status_case(
    cases: &mut Vec<Case>,
    section: &'static str,
    name: String,
    want: &[(u8, u8)],
    expected: &str,
    done: &Result<Completion>,
) -> bool {
    let (actual, passed) = match done {
        Ok(c) if c.ok() => ("success".to_string(), want.contains(&SUCCESS)),
        Ok(c) => {
            let got = c.code();
            (
                format!("status type {:#x} code {:#04x}", got.0, got.1),
                want.contains(&got),
            )
        }
        Err(e) => (e.clone(), false),
    };
    cases.push(case(section, name, expected.into(), actual, passed));
    passed
}

/// A case for `rev`, judged against the revision the controller implements (`vs`). When that revision
/// defines it, the command must end with one of `want`. When it does not, a refusal (Invalid Command
/// Opcode, Invalid Field, Invalid Log Page) passes too: the controller predates the capability.
#[allow(clippy::too_many_arguments)]
pub fn rev_case(
    cases: &mut Vec<Case>,
    section: &'static str,
    prefix: &str,
    rev: &Rev,
    vs: u32,
    want: &[(u8, u8)],
    expected: &str,
    done: &Result<Completion>,
) -> bool {
    let name = format!("{prefix} {}", rev.name());
    if rev.defined_in(vs) {
        return status_case(cases, section, name, want, expected, done);
    }
    let refused = matches!(done, Ok(c) if !c.ok() && [INVALID_OPCODE, INVALID_FIELD, INVALID_LOG_PAGE].contains(&c.code()));
    if refused {
        cases.push(case(
            section,
            name,
            format!("{expected}, or refused"),
            rev.absent(vs),
            true,
        ));
        return true;
    }
    status_case(
        cases,
        section,
        name,
        want,
        &format!("{expected}, or refused"),
        done,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_cases_match_type_and_code() {
        let mut cases = Vec::new();
        let mut b = [0u8; 16];
        b[14..16].copy_from_slice(&((LBA_OUT_OF_RANGE.1 as u16) << 1).to_le_bytes());
        let done = Ok(Completion::parse(&b));
        assert!(status_case(
            &mut cases,
            "nvme-io",
            "past".into(),
            &[LBA_OUT_OF_RANGE],
            "LBA Out of Range",
            &done
        ));
        assert!(!status_case(
            &mut cases,
            "nvme-io",
            "ok".into(),
            &[SUCCESS],
            "success",
            &done
        ));
        // The same code in another status code type is another status.
        b[14..16].copy_from_slice(&((1u16 << 9 | 0x80) << 1).to_le_bytes());
        let done = Ok(Completion::parse(&b));
        assert!(!status_case(
            &mut cases,
            "nvme-io",
            "type".into(),
            &[LBA_OUT_OF_RANGE],
            "LBA Out of Range",
            &done
        ));
        assert!(!cases[1].passed);
    }

    #[test]
    fn a_capability_newer_than_the_controller_may_be_refused() {
        let mut cases = Vec::new();
        let mut b = [0u8; 16];
        b[14..16].copy_from_slice(
            &((INVALID_LOG_PAGE.0 as u16) << 9 | (INVALID_LOG_PAGE.1 as u16) << 1).to_le_bytes(),
        );
        let refused = Ok(Completion::parse(&b));
        let ana = super::super::revisions::LOG_ANA;
        assert!(rev_case(
            &mut cases,
            "nvme-ana",
            "x",
            &ana,
            0x0001_0300,
            &[SUCCESS],
            "success",
            &refused
        ));
        assert_eq!(
            cases[0].actual,
            "not in NVMe 1.3, which the controller implements"
        );
        assert!(!rev_case(
            &mut cases,
            "nvme-ana",
            "x",
            &ana,
            0x0001_0400,
            &[SUCCESS],
            "success",
            &refused
        ));
        assert_eq!(cases[1].name, "x NVMe 1.4 ANA log page");
    }
}
