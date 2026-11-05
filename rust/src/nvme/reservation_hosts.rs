//! The four hosts the nvme-resv cases use (A, A2 on A's second path, B and C), on one namespace and
//! test block, and the checks they make: issue a reservation command and judge its status, read the
//! extended Reservation Report and compare it with who should be registered and who should hold, and
//! read or write the test block to check what the reservation type allows. Writes put back the data
//! the block already holds. `all` stays true while every case passes.

use super::host::Host;
use super::link::Data;
use super::nvm;
use super::reservation::{acquire, parse_report, register, release, report, two_keys, Report};
use super::sqe::Completion;
use super::verdict::{status_case, status_should, RESERVATION_CONFLICT, SUCCESS};
use crate::compliance::{case, Case};
use crate::Result;

/// Each host's key: distinct, and marked so a stray key is easy to spot.
pub const KEY_A: u64 = 0x5350_4954_4e56_00a1;
pub const KEY_B: u64 = 0x5350_4954_4e56_00b2;
pub const KEY_C: u64 = 0x5350_4954_4e56_00c4;

/// The host NQN of each host. A and A2 share one, as two paths of one host do.
pub const HOST_A: &str = "nqn.2026-11.com.stutools:spit-resv-a";
pub const HOST_B: &str = "nqn.2026-11.com.stutools:spit-resv-b";
pub const HOST_C: &str = "nqn.2026-11.com.stutools:spit-resv-c";

/// Indexes of the hosts.
pub const A: usize = 0;
pub const A2: usize = 1;
pub const B: usize = 2;
pub const C: usize = 3;
pub const NAMES: [&str; 4] = ["A", "A2", "B", "C"];

/// The hosts, the namespace and block the cases use, and whether every case so far passed.
pub struct Hosts<'a> {
    pub hosts: Vec<Host>,
    pub ids: [[u8; 16]; 4],
    pub nsid: u32,
    pub lba: u64,
    pub original: Vec<u8>,
    pub label: String,
    pub all: bool,
    pub cases: &'a mut Vec<Case>,
}

impl Hosts<'_> {
    pub fn check(
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

    /// `check` for a clause that says "should": a miss is a warning, and does not count against
    /// the clustering verdict.
    pub fn check_should(
        &mut self,
        name: &str,
        want: &[(u8, u8)],
        expected: &str,
        done: &Result<Completion>,
    ) -> bool {
        status_should(
            self.cases,
            "nvme-resv",
            format!("{} {name}", self.label),
            want,
            expected,
            done,
        )
    }

    pub fn fact(&mut self, name: &str, expected: String, actual: String, passed: bool) {
        self.all &= passed;
        self.cases.push(case(
            "nvme-resv",
            format!("{} {name}", self.label),
            expected,
            actual,
            passed,
        ));
    }

    pub fn register(
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

    pub fn acquire(
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

    pub fn release(&mut self, who: usize, action: u8, rtype: u8, key: u64) -> Result<Completion> {
        let nsid = self.nsid;
        self.hosts[who].send(release(nsid, action, rtype), &key.to_le_bytes())
    }

    pub fn report(&mut self, who: usize) -> Result<Report> {
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
    pub fn show(&self, r: &Report) -> String {
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
    pub fn expect_report(
        &mut self,
        name: &str,
        rtype: u8,
        registered: &[usize],
        holding: &[usize],
    ) {
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
    pub fn access(&mut self, kind: &str, who: usize, write: bool, allowed: bool) {
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
