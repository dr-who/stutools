//! NVMe reservations (NVMe base specification, Reservations, and the NVM Command Set's Reservation
//! Register, Report, Acquire and Release) and the nvme-resv cases of `--compliance`. Windows Failover
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

use super::compliance::{
    status_case, Target, HOST_ID_INCONSISTENT, INVALID_FIELD, INVALID_OPCODE, RESERVATION_CONFLICT,
    SUCCESS,
};
use super::controller;
use super::host::Host;
use super::identify_cases::Facts;
use super::io_cases::AREA_FROM_END;
use super::link::Data;
use super::nvm;
use super::revisions as rev;
use super::sqe::{self, Command, Completion};
use crate::compliance::{case, Case};
use crate::Result;

/// ONCS bit 5: the controller supports reservations.
const ONCS_RESERVATIONS: u16 = 1 << 5;
/// The reservation notification log page.
const LOG_RESERVATION: u8 = 0x80;
/// The namespace features a controller with reservations supports (Base 2.1 8.1.22, Figures 32 and
/// 385).
const FEAT_RESV_MASK: u8 = 0x82;
const FEAT_RESV_PERSIST: u8 = 0x83;

/// Register actions (RREGA), acquire actions (RACQA) and release actions (RRELA).
const REGISTER: u8 = 0;
const UNREGISTER: u8 = 1;
const REPLACE: u8 = 2;
const ACQUIRE: u8 = 0;
const PREEMPT: u8 = 1;
const PREEMPT_AND_ABORT: u8 = 2;
const RELEASE: u8 = 0;
const CLEAR: u8 = 1;

/// Reservation types and their names. RESCAP bit N says type N is supported.
const TYPES: [(u8, &str); 6] = [
    (1, "Write Exclusive"),
    (2, "Exclusive Access"),
    (3, "Write Exclusive, Registrants Only"),
    (4, "Exclusive Access, Registrants Only"),
    (5, "Write Exclusive, All Registrants"),
    (6, "Exclusive Access, All Registrants"),
];
/// The type Windows clustering takes: Write Exclusive, Registrants Only.
const WERO: u8 = 3;

/// Keys for the cases: distinct, and marked so a stray key is easy to spot.
const KEY_A: u64 = 0x5350_4954_4e56_00a1;
const KEY_B: u64 = 0x5350_4954_4e56_00b2;
const KEY_B2: u64 = 0x5350_4954_4e56_00b3;
const KEY_C: u64 = 0x5350_4954_4e56_00c4;
const KEY_WRONG: u64 = 0x5350_4954_4e56_00ee;

/// The host NQN of each host. A and A2 share one, as two paths of one host do.
const HOST_A: &str = "nqn.2026-11.com.stutools:spit-resv-a";
const HOST_B: &str = "nqn.2026-11.com.stutools:spit-resv-b";
const HOST_C: &str = "nqn.2026-11.com.stutools:spit-resv-c";

/// Indexes of the hosts.
const A: usize = 0;
const A2: usize = 1;
const B: usize = 2;
const C: usize = 3;
const NAMES: [&str; 4] = ["A", "A2", "B", "C"];

/// The outcome for the windows section.
pub struct Verdict {
    /// ONCS lists reservations.
    pub supported: bool,
    /// Every reservation case passed and Write Exclusive, Registrants Only is supported.
    pub clustering: bool,
}

/// Reservation Register: `action` is RREGA; the data is `register_data`.
pub fn register(nsid: u32, action: u8, ignore_key: bool) -> Command {
    let mut cmd = Command::new(sqe::OP_RESV_REGISTER, 0);
    cmd.set_nsid(nsid);
    cmd.set_u32(40, action as u32 | (ignore_key as u32) << 3);
    cmd
}

/// Reservation Acquire: `action` is RACQA and `rtype` the reservation type; the data is two keys.
pub fn acquire(nsid: u32, action: u8, rtype: u8) -> Command {
    let mut cmd = Command::new(sqe::OP_RESV_ACQUIRE, 0);
    cmd.set_nsid(nsid);
    cmd.set_u32(40, action as u32 | (rtype as u32) << 8);
    cmd
}

/// Reservation Release: `action` is RRELA and `rtype` the type held; the data is the current key.
pub fn release(nsid: u32, action: u8, rtype: u8) -> Command {
    let mut cmd = Command::new(sqe::OP_RESV_RELEASE, 0);
    cmd.set_nsid(nsid);
    cmd.set_u32(40, action as u32 | (rtype as u32) << 8);
    cmd
}

/// Reservation Report of `len` bytes; `extended` asks for the extended data structure (EDS), which
/// carries 128-bit host identifiers.
pub fn report(nsid: u32, len: usize, extended: bool) -> Command {
    let mut cmd = Command::new(sqe::OP_RESV_REPORT, 0);
    cmd.set_nsid(nsid);
    cmd.set_u32(40, (len / 4 - 1) as u32);
    cmd.set_u32(44, extended as u32);
    cmd
}

/// Two keys as the 16 bytes Register (current, new) and Acquire (current, preempt) take.
pub fn two_keys(first: u64, second: u64) -> [u8; 16] {
    let mut d = [0u8; 16];
    d[0..8].copy_from_slice(&first.to_le_bytes());
    d[8..16].copy_from_slice(&second.to_le_bytes());
    d
}

/// One registrant in an extended report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registrant {
    pub cntlid: u16,
    /// RCSTS bit 0: this host holds the reservation.
    pub holds: bool,
    pub key: u64,
    pub hostid: [u8; 16],
}

/// A Reservation Report with the extended data structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub generation: u32,
    /// RTYPE: the reservation type held, 0 for none.
    pub rtype: u8,
    pub registrants: Vec<Registrant>,
}

/// Parses an extended Reservation Report: a 64-byte header (generation at 0, RTYPE at 4, the
/// registrant count REGCTL at 5-6), then a 64-byte entry per registrant (CNTLID at 0, RCSTS at 2,
/// the key at 8, the 128-bit host identifier at 16).
pub fn parse_report(d: &[u8]) -> Report {
    let count = u16::from_le_bytes([d[5], d[6]]) as usize;
    let registrants = (0..count)
        .filter_map(|i| d.get(64 + 64 * i..128 + 64 * i))
        .map(|e| Registrant {
            cntlid: u16::from_le_bytes([e[0], e[1]]),
            holds: e[2] & 1 != 0,
            key: u64::from_le_bytes(e[8..16].try_into().unwrap()),
            hostid: e[16..32].try_into().unwrap(),
        })
        .collect();
    Report {
        generation: u32::from_le_bytes(d[0..4].try_into().unwrap()),
        rtype: d[4],
        registrants,
    }
}

/// Whether a host that does not hold the reservation may read and write under `rtype`, when it is
/// registered and when it is not (the base specification's command behaviour table).
pub fn allowed(rtype: u8, registered: bool) -> (bool, bool) {
    match rtype {
        1 => (true, false),
        2 => (false, false),
        3 | 5 => (true, registered),
        4 | 6 => (registered, registered),
        _ => (true, true),
    }
}

/// The hosts, the namespace and block the cases use, and whether every case so far passed.
struct Run<'a> {
    hosts: Vec<Host>,
    ids: [[u8; 16]; 4],
    nsid: u32,
    lba: u64,
    original: Vec<u8>,
    label: String,
    all: bool,
    cases: &'a mut Vec<Case>,
}

impl Run<'_> {
    fn check(
        &mut self,
        name: &str,
        want: &[(u8, u8)],
        expected: &str,
        done: &Result<Completion>,
    ) -> bool {
        let passed = status_case(
            self.cases,
            "nvme-resv",
            format!("{} {name}", self.label),
            want,
            expected,
            done,
        );
        self.all &= passed;
        passed
    }

    fn fact(&mut self, name: &str, expected: String, actual: String, passed: bool) {
        self.all &= passed;
        self.cases.push(case(
            "nvme-resv",
            format!("{} {name}", self.label),
            expected,
            actual,
            passed,
        ));
    }

    fn register(
        &mut self,
        who: usize,
        action: u8,
        ignore: bool,
        current: u64,
        new: u64,
    ) -> Result<Completion> {
        let nsid = self.nsid;
        self.hosts[who].send(register(nsid, action, ignore), &two_keys(current, new))
    }

    fn acquire(
        &mut self,
        who: usize,
        action: u8,
        rtype: u8,
        current: u64,
        preempt: u64,
    ) -> Result<Completion> {
        let nsid = self.nsid;
        self.hosts[who].send(acquire(nsid, action, rtype), &two_keys(current, preempt))
    }

    fn release(&mut self, who: usize, action: u8, rtype: u8, key: u64) -> Result<Completion> {
        let nsid = self.nsid;
        self.hosts[who].send(release(nsid, action, rtype), &key.to_le_bytes())
    }

    fn report(&mut self, who: usize) -> Result<Report> {
        let mut buf = vec![0u8; 4096];
        let nsid = self.nsid;
        let done = self.hosts[who].io(report(nsid, buf.len(), true), Data::In(&mut buf))?;
        if !done.ok() {
            let (sct, sc) = done.code();
            return Err(format!(
                "Reservation Report: status type {sct:#x} code {sc:#04x}"
            ));
        }
        Ok(parse_report(&buf))
    }

    /// The registrants as (host, key) pairs by host letter, and the hosts that hold.
    fn show(&self, r: &Report) -> String {
        let who = |id: &[u8; 16]| {
            [A, B, C]
                .into_iter()
                .find(|&i| &self.ids[i] == id)
                .map_or("another host", |i| NAMES[i])
        };
        let regs: Vec<String> = r
            .registrants
            .iter()
            .map(|g| {
                format!(
                    "{}{}",
                    who(&g.hostid),
                    if g.holds { " (holds)" } else { "" }
                )
            })
            .collect();
        format!("type {}, registrants [{}]", r.rtype, regs.join(", "))
    }

    /// Checks a report against the expected type, registered hosts and holding hosts. Hosts are
    /// compared as a set: a host with two controllers may be listed once or once per controller.
    fn expect_report(&mut self, name: &str, rtype: u8, registered: &[usize], holding: &[usize]) {
        let r = self.report(C);
        let expected = format!(
            "type {rtype}, registrants [{}]",
            registered
                .iter()
                .map(|&i| format!(
                    "{}{}",
                    NAMES[i],
                    if holding.contains(&i) { " (holds)" } else { "" }
                ))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let (actual, passed) = match &r {
            Ok(r) => {
                let mut hosts: Vec<(usize, bool, u64)> = Vec::new();
                let mut unknown = false;
                for g in &r.registrants {
                    match [A, B, C].into_iter().find(|&i| self.ids[i] == g.hostid) {
                        Some(i) if !hosts.iter().any(|h| h.0 == i) => {
                            hosts.push((i, g.holds, g.key))
                        }
                        Some(_) => {}
                        None => unknown = true,
                    }
                }
                hosts.sort();
                let want_key = |i: usize| match i {
                    A => KEY_A,
                    B => KEY_B,
                    _ => KEY_C,
                };
                let ok = !unknown
                    && r.rtype == rtype
                    && hosts.len() == registered.len()
                    && hosts.iter().all(|&(i, holds, key)| {
                        registered.contains(&i)
                            && holds == holding.contains(&i)
                            && key == want_key(i)
                    });
                (self.show(r), ok)
            }
            Err(e) => (e.clone(), false),
        };
        self.fact(name, expected, actual, passed);
    }

    /// A read or a write (of the block's own data) by host `who`, which must be allowed or must
    /// conflict.
    fn access(&mut self, kind: &str, who: usize, write: bool, allowed: bool) {
        let (nsid, lba) = (self.nsid, self.lba);
        let original = self.original.clone();
        let done = if write {
            self.hosts[who].send(nvm::write(nsid, lba, 1), &original)
        } else {
            let mut buf = vec![0u8; original.len()];
            self.hosts[who].io(nvm::read(nsid, lba, 1), Data::In(&mut buf))
        };
        let op = if write { "write" } else { "read" };
        let role = match who {
            A => "host A",
            A2 => "host A on its second path",
            B => "host B",
            _ => "host C (never registered)",
        };
        self.check(
            &format!("{kind}: {role} {op}"),
            if allowed {
                &[SUCCESS]
            } else {
                &[RESERVATION_CONFLICT]
            },
            if allowed {
                "success"
            } else {
                "Reservation Conflict"
            },
            &done,
        );
    }
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
    let mut r = Run {
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
        r.check(&feature.name(), &[SUCCESS], "success", &done);
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
        r.check(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_put_the_action_type_and_flags_in_cdw10() {
        let c = register(1, REPLACE, true);
        assert_eq!(c.0[0], sqe::OP_RESV_REGISTER);
        assert_eq!(&c.0[40..44], &(2u32 | 8).to_le_bytes());
        let c = acquire(1, PREEMPT_AND_ABORT, WERO);
        assert_eq!(&c.0[40..44], &(2u32 | 3 << 8).to_le_bytes());
        let c = release(1, CLEAR, 0);
        assert_eq!(c.0[0], sqe::OP_RESV_RELEASE);
        assert_eq!(&c.0[40..44], &1u32.to_le_bytes());
        let c = report(1, 4096, true);
        assert_eq!(&c.0[40..44], &1023u32.to_le_bytes());
        assert_eq!(&c.0[44..48], &1u32.to_le_bytes());
        assert_eq!(&two_keys(1, 2)[8..16], &2u64.to_le_bytes());
    }

    #[test]
    fn an_extended_report_lists_each_registrant() {
        let mut d = vec![0u8; 256];
        d[0..4].copy_from_slice(&7u32.to_le_bytes());
        d[4] = WERO;
        d[5] = 2;
        d[64] = 1;
        d[66] = 1;
        d[72..80].copy_from_slice(&KEY_A.to_le_bytes());
        d[80..96].copy_from_slice(&[0xaa; 16]);
        d[136..144].copy_from_slice(&KEY_B.to_le_bytes());
        let r = parse_report(&d);
        assert_eq!(r.generation, 7);
        assert_eq!(r.rtype, WERO);
        assert_eq!(r.registrants.len(), 2);
        assert!(r.registrants[0].holds && !r.registrants[1].holds);
        assert_eq!(r.registrants[0].hostid, [0xaa; 16]);
        assert_eq!(r.registrants[1].key, KEY_B);
    }

    #[test]
    fn access_follows_the_reservation_type() {
        assert_eq!(allowed(1, true), (true, false));
        assert_eq!(allowed(2, true), (false, false));
        assert_eq!(allowed(3, true), (true, true));
        assert_eq!(allowed(3, false), (true, false));
        assert_eq!(allowed(6, false), (false, false));
    }
}
