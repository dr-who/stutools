//! The nvme-resv cases of `--compliance`: every reservation command (`reservation`) and type, with
//! four hosts. Windows Failover
//! Clustering arbitrates for a disk with SCSI persistent reservations; on NVMe, stornvme turns them
//! into these commands, so a namespace a cluster shares must implement them.
//!
//! Reservations belong to a host, named by its 128-bit host identifier, not to a controller: two
//! associations with one host identifier are one host on two paths (A and A2 here), and both may do
//! what the holder may. The cases use four hosts: A holds, A2 is A's second path (through the other
//! address when there is one), B registers, C never does. Writes put back the data the block already
//! holds, so the cases change no data. When ONCS bit 5 is clear the commands must be refused.
//!
//! Base 2.1: the commands 7.5 to 7.8 (Figures 569 to 584), the rules 8.1.22 (access by type Figures
//! 645 and 646, registering 8.1.22.3, acquiring 8.1.22.5, releasing 8.1.22.6, preempting 8.1.22.7,
//! clearing 8.1.22.8), the notification log page 5.1.12.1.32 (Figure 290). SCSI-NVMe 1.5 4.13 maps
//! Windows' PERSISTENT RESERVE onto them: SCSI type 5h (Write Exclusive, Registrants Only, which
//! clustering takes) is RTYPE 3h, REGISTER AND IGNORE EXISTING KEY is Register with IEKEY.

use super::compliance::Target;
use super::controller;
use super::host::Host;
use super::identify_cases::Facts;
use super::io_cases::AREA_FROM_END;
use super::link::Data;
use super::nvm;
use super::reservation::{
    allowed, register, report, two_keys, ACQUIRE, CLEAR, FEAT_RESV_MASK, FEAT_RESV_PERSIST,
    LOG_RESERVATION, ONCS_RESERVATIONS, PREEMPT, PREEMPT_AND_ABORT, REGISTER, RELEASE, REPLACE,
    TYPES, UNREGISTER,
};
use super::reservation_hosts::{
    Hosts, A, A2, B, C, HOST_A, HOST_B, HOST_C, KEY_A, KEY_B, KEY_C, NAMES,
};
use super::revisions as rev;
use super::sqe::{self, Command};
use super::verdict::{
    status_case, HOST_ID_INCONSISTENT, INVALID_FIELD, INVALID_OPCODE, RESERVATION_CONFLICT, SUCCESS,
};
use crate::compliance::{case, Case};
use crate::Result;

/// The type Windows clustering takes: Write Exclusive, Registrants Only.
const WERO: u8 = 3;

/// A replacement key for B, and a key no host registers.
const KEY_B2: u64 = 0x5350_4954_4e56_00b3;
const KEY_WRONG: u64 = 0x5350_4954_4e56_00ee;

/// The outcome for the windows section.
pub struct Verdict {
    /// ONCS lists reservations.
    pub supported: bool,
    /// Every reservation case passed and Write Exclusive, Registrants Only is supported.
    pub clustering: bool,
}

/// Runs the nvme-resv cases on the first namespace in `facts`.
pub fn run(t: &Target, name: &str, facts: &Facts, cases: &mut Vec<Case>) -> Verdict {
    let Some((nsid, ns)) = facts.namespaces.first().copied() else {
        return Verdict {
            supported: false,
            clustering: false,
        };
    };
    let label = format!("{name} ns {nsid} {}", rev::RESERVATIONS.name());
    // Base 2.1 8.1.22: the controller (ONCS bit 5) and the namespace (RESCAP non-zero) must both
    // support reservations.
    let supported = facts.ctrl.oncs & ONCS_RESERVATIONS != 0 && ns.rescap != 0;
    let mut ids = [[0u8; 16]; 4];
    for id in &mut ids {
        match controller::random_hostid() {
            Ok(v) => *id = v,
            Err(e) => {
                cases.push(case(
                    "nvme-resv",
                    format!("{label} host IDs"),
                    "made".into(),
                    e,
                    false,
                ));
                return Verdict {
                    supported,
                    clustering: false,
                };
            }
        }
    }
    ids[A2] = ids[A];
    let other = *t.addrs.get(1).unwrap_or(&t.addrs[0]);
    let plan = [
        (t.addrs[0], HOST_A),
        (other, HOST_A),
        (t.addrs[0], HOST_B),
        (t.addrs[0], HOST_C),
    ];
    let mut hosts = Vec::new();
    for (i, (addr, nqn)) in plan.iter().enumerate() {
        match Host::open(*addr, &t.subnqn, nqn, ids[i]) {
            Ok(h) => hosts.push(h),
            Err(e) => {
                cases.push(case(
                    "nvme-resv",
                    format!("{label} host {} at {addr}", NAMES[i]),
                    "connected".into(),
                    e,
                    false,
                ));
                return Verdict {
                    supported,
                    clustering: false,
                };
            }
        }
    }

    // Base 2.1 8.1.22: unless the controller and the namespace both support reservations, the four
    // commands are Invalid Command Opcode.
    if !supported {
        let mut buf = vec![0u8; 4096];
        let done = hosts[A].io(report(nsid, buf.len(), true), Data::In(&mut buf));
        status_case(
            cases,
            "nvme-resv",
            format!(
                "{label} Reservation Report (ONCS bit 5 {}, RESCAP {:#04x})",
                facts.ctrl.oncs >> 5 & 1,
                ns.rescap
            ),
            &[INVALID_OPCODE],
            "Invalid Command Opcode",
            &done,
        );
        let done = hosts[A].send(register(nsid, REGISTER, false), &two_keys(0, KEY_A));
        status_case(
            cases,
            "nvme-resv",
            format!(
                "{label} Reservation Register (ONCS bit 5 {}, RESCAP {:#04x})",
                facts.ctrl.oncs >> 5 & 1,
                ns.rescap
            ),
            &[INVALID_OPCODE],
            "Invalid Command Opcode",
            &done,
        );
        return Verdict {
            supported,
            clustering: false,
        };
    }

    let len = ns.block_len as usize;
    let lba = ns.blocks - AREA_FROM_END + 7;
    let mut original = vec![0u8; len];
    let read = hosts[C].io(nvm::read(nsid, lba, 1), Data::In(&mut original));
    let mut r = Hosts {
        hosts,
        ids,
        nsid,
        lba,
        original,
        label,
        all: true,
        cases,
    };
    if !matches!(&read, Ok(d) if d.ok()) {
        r.check("read the test block", &[SUCCESS], "success", &read);
        return Verdict {
            supported,
            clustering: false,
        };
    }

    // Start from nothing: an earlier run that stopped part way may have left registrations. Clear
    // removes every registration, whichever host made it.
    let reg = r.register(A, REGISTER, true, 0, KEY_A);
    let clear = r.release(A, CLEAR, 0, KEY_A);
    if !matches!((&reg, &clear), (Ok(x), Ok(y)) if x.ok() && y.ok()) {
        r.check(
            "clean slate: A Register and Clear",
            &[SUCCESS],
            "success",
            if reg.as_ref().is_ok_and(|d| d.ok()) {
                &clear
            } else {
                &reg
            },
        );
    }

    // Base 2.1 8.1.22 and Figure 32 note 4: with a non-zero RESCAP, both namespace features are
    // mandatory, and Get Features names the namespace.
    for (fid, feature) in [
        (FEAT_RESV_MASK, rev::FEAT_RESV_MASK),
        (FEAT_RESV_PERSIST, rev::FEAT_RESV_PERSIST),
    ] {
        let mut cmd = Command::new(sqe::OP_GET_FEATURES, 0);
        cmd.set_nsid(nsid);
        cmd.set_u32(40, fid as u32);
        let done = r.hosts[A].admin.submit(cmd, Data::None);
        r.check(
            &format!("Get Features {}", feature.item),
            &[SUCCESS],
            "success",
            &done,
        );
    }

    r.expect_report(
        &format!("{} with no registrants", rev::RESERVATION_EDS.name()),
        0,
        &[],
        &[],
    );
    // Base 2.1 7.8: a host with a 128-bit Host Identifier (every fabrics host) that clears EDS is
    // refused with Host Identifier Inconsistent Format.
    let mut buf = vec![0u8; 4096];
    let done = r.hosts[A].io(report(nsid, buf.len(), false), Data::In(&mut buf));
    r.check(
        "Report without the extended structure",
        &[HOST_ID_INCONSISTENT],
        "Host Identifier Inconsistent Format",
        &done,
    );

    let done = r.register(A, REGISTER, false, 0, KEY_A);
    r.check("A Register", &[SUCCESS], "success", &done);
    // Windows registers with REGISTER AND IGNORE EXISTING KEY, which is IEKEY here.
    let done = r.register(B, REGISTER, true, KEY_WRONG, KEY_B);
    r.check(
        "B Register ignoring the existing key (IEKEY)",
        &[SUCCESS],
        "success",
        &done,
    );
    let done = r.register(C, REPLACE, false, KEY_C, KEY_C);
    r.check(
        "C Replace without a registration",
        &[RESERVATION_CONFLICT],
        "Reservation Conflict",
        &done,
    );
    let done = r.register(B, REPLACE, false, KEY_B, KEY_B2);
    r.check("B Replace its key", &[SUCCESS], "success", &done);
    let done = r.register(B, REPLACE, false, KEY_B2, KEY_B);
    r.check("B Replace its key back", &[SUCCESS], "success", &done);
    let done = r.register(B, REPLACE, false, KEY_WRONG, KEY_B2);
    r.check(
        "B Replace with a wrong current key",
        &[RESERVATION_CONFLICT],
        "Reservation Conflict",
        &done,
    );
    r.expect_report("Report lists A and B", 0, &[A, B], &[]);

    let rescap = ns.rescap;
    for (rtype, type_name) in TYPES {
        if rescap & (1 << rtype) == 0 {
            let done = r.acquire(A, ACQUIRE, rtype, KEY_A, 0);
            r.check(
                &format!("A Acquire {type_name} (RESCAP says unsupported)"),
                &[INVALID_FIELD],
                "Invalid Field in Command",
                &done,
            );
            continue;
        }
        let done = r.acquire(A, ACQUIRE, rtype, KEY_A, 0);
        if !r.check(
            &format!("A Acquire {type_name}"),
            &[SUCCESS],
            "success",
            &done,
        ) {
            continue;
        }
        let all_registrants = rtype >= 5;
        let holding: &[usize] = if all_registrants { &[A, B] } else { &[A] };
        r.expect_report(
            &format!("Report shows {type_name}"),
            rtype,
            &[A, B],
            holding,
        );
        for who in [A, A2, B, C] {
            let (read, write) = if who == A || who == A2 || (who == B && all_registrants) {
                (true, true)
            } else {
                allowed(rtype, who == B)
            };
            r.access(type_name, who, false, read);
            r.access(type_name, who, true, write);
        }
        // 8.1.22.5: a second Acquire is a conflict, except by a holder for the same type, and every
        // registrant holds an All Registrants reservation (Figure 645).
        let done = r.acquire(B, ACQUIRE, rtype, KEY_B, 0);
        if all_registrants {
            r.check(
                &format!("{type_name}: B Acquire as a holder"),
                &[SUCCESS],
                "success",
                &done,
            );
        } else {
            r.check(
                &format!("{type_name}: B Acquire while A holds"),
                &[RESERVATION_CONFLICT],
                "Reservation Conflict",
                &done,
            );
        }
        // 8.1.22.6: Release naming another type completes with Invalid Field in Command (Figure 576
        // words it as a should).
        let wrong = rtype % 6 + 1;
        let done = r.release(A, RELEASE, wrong, KEY_A);
        r.check_should(
            &format!("{type_name}: A Release naming type {wrong}"),
            &[INVALID_FIELD],
            "Invalid Field in Command",
            &done,
        );
        let done = r.release(A, RELEASE, rtype, KEY_A);
        r.check(
            &format!("{type_name}: A Release"),
            &[SUCCESS],
            "success",
            &done,
        );
        r.expect_report(&format!("{type_name}: released"), 0, &[A, B], &[]);
    }

    // Preempt, as a cluster node takes a disk from a node that stopped answering.
    let held = if rescap & (1 << WERO) != 0 {
        Some(WERO)
    } else {
        TYPES.iter().map(|t| t.0).find(|t| rescap & (1 << t) != 0)
    };
    if let Some(rtype) = held {
        let done = r.acquire(A, ACQUIRE, rtype, KEY_A, 0);
        r.check(
            &format!("type {rtype}: A Acquire to be preempted"),
            &[SUCCESS],
            "success",
            &done,
        );
        let done = r.acquire(C, PREEMPT, rtype, KEY_C, KEY_A);
        r.check(
            "C Preempt without a registration",
            &[RESERVATION_CONFLICT],
            "Reservation Conflict",
            &done,
        );
        let done = r.acquire(B, PREEMPT, rtype, KEY_B, KEY_A);
        r.check("B Preempt A", &[SUCCESS], "success", &done);
        r.expect_report("Report after B preempts A", rtype, &[B], &[B]);
        r.access("after the preempt", A, true, false);
        r.access("after the preempt", A2, true, false);
        let done = r.register(A, REGISTER, true, 0, KEY_A);
        r.check("A Register again", &[SUCCESS], "success", &done);
        let done = r.acquire(A, PREEMPT_AND_ABORT, rtype, KEY_A, KEY_B);
        r.check("A Preempt and Abort B", &[SUCCESS], "success", &done);
        r.expect_report("Report after A preempts B", rtype, &[A], &[A]);

        // Base 2.1 8.1.22.7: B was unregistered by A's preempt, so B's controller has a registration
        // preempted notification (5.1.12.1.32, Figure 290: type 1h; 2h and 3h are released and
        // preempted). It is read now, before B's association closes.
        let mut log = [0u8; 64];
        let done = r.hosts[B].admin.log_page(LOG_RESERVATION, 0, 0, &mut log);
        if r.check(
            &format!("B {}", rev::LOG_RESERVATION.name()),
            &[SUCCESS],
            "success",
            &done,
        ) {
            let kind = log[8];
            r.fact(
                "B notification",
                "type 1 to 3 (registration preempted, reservation released or preempted)".into(),
                format!("type {kind}, {} more pages", log[9]),
                (1..=3).contains(&kind),
            );
        }

        // A reservation belongs to the host: it survives the host's associations closing, so a path
        // that comes back finds it (as MPIO does after a path failure). Base 2.1 8.1.22: registrations
        // and reservations persist across Controller Level Resets; Figure 583 CNTLID FFFFh reports a
        // registrant with no controller.
        let done = r.register(B, REGISTER, false, 0, KEY_B);
        r.check("B Register again", &[SUCCESS], "success", &done);
        let reopened = (|| -> Result<()> {
            let (first, second) = (t.addrs[0], other);
            r.hosts.truncate(0);
            r.hosts.push(Host::open(first, &t.subnqn, HOST_A, ids[A])?);
            r.hosts.push(Host::open(second, &t.subnqn, HOST_A, ids[A])?);
            r.hosts.push(Host::open(first, &t.subnqn, HOST_B, ids[B])?);
            r.hosts.push(Host::open(first, &t.subnqn, HOST_C, ids[C])?);
            Ok(())
        })();
        r.fact(
            "every host reconnects",
            "connected".into(),
            match &reopened {
                Ok(()) => "connected".into(),
                Err(e) => e.clone(),
            },
            reopened.is_ok(),
        );
        if reopened.is_err() {
            return Verdict {
                supported,
                clustering: false,
            };
        }
        r.expect_report("Report after every host reconnects", rtype, &[A, B], &[A]);
        r.access("after reconnecting", A, true, true);
    }

    let done = r.register(B, UNREGISTER, false, KEY_B, 0);
    r.check("B Unregister", &[SUCCESS], "success", &done);
    let done = r.release(A, CLEAR, 0, KEY_A);
    r.check("A Clear", &[SUCCESS], "success", &done);
    r.expect_report("Report after Clear", 0, &[], &[]);

    Verdict {
        supported,
        clustering: r.all && rescap & (1 << WERO) != 0,
    }
}
