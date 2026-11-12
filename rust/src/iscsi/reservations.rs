//! The `pr` cases of `--compliance`: SPC-5 persistent reservations, which Windows Failover Clustering
//! uses to own a disk (REGISTER AND IGNORE EXISTING KEY, RESERVE or PREEMPT with Write Exclusive –
//! Registrants Only, READ KEYS, READ RESERVATION, RELEASE, CLEAR). Three initiator ports log in with
//! ISIDs that differ in their first bytes: A holds the reservation, B is registered and C is not. Each
//! of the six types is checked against the SBC-5 table 13 read and write rules for all three; then
//! preemption, reinstatement of A's session, a second path of A whose ISID differs only in its
//! qualifier (the way one host's MPIO paths differ), READ FULL STATUS, unregistering and CLEAR. Every
//! write sends the block's own data back, so the LUN is unchanged. Returns whether the commands Windows
//! clustering relies on all behaved.

use super::cases::{allowed, record, Want};
use super::lun_access::{last_block64, open, read_block64, unit_attentions, Creds};
use super::ops::{self, persistent_reserve_in};
use super::path::Path;
use super::port::Port;
use super::pr_commands::{out, read_keys, Capabilities, TYPES};
use super::pr_ports::{
    clean_slate, Run, ISID_A, ISID_B, ISID_C, ISID_SIBLING, KEY_A, KEY_B, KEY_B2, KEY_C, KEY_WRONG,
    SECTION,
};
use super::scsi;
use crate::compliance::Case;

/// What the reservation run found, for the Windows cases.
pub(super) struct Findings {
    /// Every case Windows clustering relies on passed; None when they could not be run (one
    /// initiator name, on a target that does not tell its ports apart).
    pub cluster: Option<bool>,
    pub capabilities: Option<Capabilities>,
}

/// Runs every reservation case on one LUN. Port A logs in as `initiator`; B and C as `second` when
/// given, so they are another initiator as a second cluster node is, else as `initiator` too.
pub(super) fn run(
    path: &Path,
    initiator: &str,
    second: Option<&str>,
    creds: Creds,
    name: &str,
    cases: &mut Vec<Case>,
) -> Findings {
    let mut findings = Findings {
        cluster: Some(false),
        capabilities: None,
    };
    let other = second.unwrap_or(initiator);
    // Each initiator port goes through its own portal of the target where the target has several: A, B and C
    // are then three I_T nexuses on three target ports (SPC-3 5.6.1, LU scope). A target with one portal
    // gives all three the same path, as before.
    let portals = target_portals(path, initiator);
    let on = |i: usize| Path {
        portal: portals
            .get(i)
            .cloned()
            .unwrap_or_else(|| path.portal.clone()),
        target: path.target.clone(),
        lun: path.lun,
    };
    let (pa, pb, pc) = (on(0), on(1), on(2));
    let (mut a, mut b, mut c) = match (
        open(&pa, initiator, creds, ISID_A),
        open(&pb, other, creds, ISID_B),
        open(&pc, other, creds, ISID_C),
    ) {
        (Ok(a), Ok(b), Ok(c)) => (a, b, c),
        (Err(err), _, _) | (_, Err(err), _) | (_, _, Err(err)) => {
            record(
                cases,
                SECTION,
                format!("{name} three initiator ports"),
                "three logins",
                err,
                false,
            );
            return findings;
        }
    };
    for s in [&mut a, &mut b, &mut c] {
        let _ = unit_attentions(s);
    }
    let block = last_block64(&mut a).and_then(|(lba, len)| {
        let target = lba.saturating_sub(1);
        read_block64(&mut a, target, len).map(|data| (target, data))
    });
    let (target, original) = match block {
        Ok(v) => v,
        Err(err) => {
            record(
                cases,
                SECTION,
                format!("{name} read the test block"),
                "GOOD",
                err,
                false,
            );
            return findings;
        }
    };
    let mut t = Run {
        cases,
        name,
        target,
        original,
        cluster: true,
        c_name: if second.is_some() {
            "C (not registered, B's initiator name)"
        } else {
            "C (not registered)"
        },
        separate: false,
        local: path.is_local(),
    };
    findings.capabilities = t.capabilities(&mut a);
    let caps = findings.capabilities.unwrap_or_default();
    let separate = match open(path, initiator, creds, ISID_SIBLING) {
        Ok(mut sibling) => {
            let _ = unit_attentions(&mut sibling);
            let separate = t.ports_of_one_initiator(&mut a, &mut sibling);
            let _ = sibling.logout();
            separate
        }
        Err(err) => {
            t.verdict(
                "a second port of A's initiator",
                "login ok".into(),
                err,
                false,
                false,
            );
            false
        }
    };
    t.separate = separate;
    if second.is_none() && !separate {
        super::cases::record_info(
            cases,
            SECTION,
            format!("{name} reservations between initiator ports"),
            "not tested: this target takes ports of one IQN for one I_T nexus; give a second name \
             it admits with --iscsi-initiator IQN,IQN2",
        );
        findings.cluster = None;
        let _ = (a.logout(), b.logout(), c.logout());
        clean_slate(path, initiator, creds, name, cases);
        return findings;
    }
    t.registration(&mut a, &mut b, &mut c);
    for (kind, label) in TYPES {
        t.reservation_type(&mut a, &mut b, &mut c, &caps, kind, label);
    }
    let preempt_type = if caps.supports(5) { 5 } else { 1 };
    t.preemption(&mut a, &mut b, &mut c, preempt_type);
    if let Some(new_a) = t.reinstatement(path, initiator, creds, a, &mut b) {
        a = new_a;
        t.settle(&mut a, &mut b, &mut c, Some(ops::PR_WRITE_EXCLUSIVE), true);
        t.second_path(path, initiator, creds, &mut a, &mut b, &mut c);
        t.settle(&mut a, &mut b, &mut c, Some(ops::PR_WRITE_EXCLUSIVE), true);
        if caps.crh {
            t.spc2_release(&mut a, &mut c);
        }
        t.unregistering(&mut a, &mut c);
        // The holder has just unregistered: nothing reserved, A need not be registered.
        t.settle(&mut a, &mut b, &mut c, None, false);
        t.clearing(&mut a, &mut b, &mut c);
        let _ = a.logout();
    }
    findings.cluster = Some(t.cluster);
    let _ = b.logout();
    let _ = c.logout();
    clean_slate(path, initiator, creds, name, cases);
    findings
}

impl Run<'_> {
    /// SPC-6 6.14.4: REPORT CAPABILITIES returns 8 bytes with LENGTH 8.
    pub(super) fn capabilities(&mut self, a: &mut Port) -> Option<Capabilities> {
        let o = persistent_reserve_in(a, ops::PR_IN_REPORT_CAPABILITIES, 8);
        let (passed, o) = match o {
            Ok(o) => {
                let ok = o.status == 0 && scsi::be32(&[0, 0, o.data[0], o.data[1]], 0) == Some(8);
                (ok, Some(o))
            }
            Err(err) => {
                self.verdict(
                    "REPORT CAPABILITIES",
                    "GOOD, LENGTH 8".into(),
                    err,
                    false,
                    true,
                );
                return None;
            }
        };
        let o = o?;
        if super::cases::invalid_opcode(&o) {
            self.verdict(
                "REPORT CAPABILITIES",
                "GOOD, LENGTH 8".into(),
                super::cases::NOT_IMPLEMENTED.into(),
                false,
                true,
            );
            return None;
        }
        let caps = Capabilities::parse(&o.data);
        let actual = if o.status == 0 {
            caps.describe()
        } else {
            super::cases::describe(&o)
        };
        self.verdict(
            "REPORT CAPABILITIES",
            "GOOD, LENGTH 8".into(),
            actual,
            passed,
            true,
        );
        passed.then_some(caps)
    }

    /// Two initiator ports of one iSCSI initiator are two I_T nexuses: an iSCSI initiator port is named
    /// by the initiator name and the ISID together (RFC 7143 4.2.7.1, 11.12.5), and persistent
    /// reservations are held and registered per I_T nexus (SPC-6 5.14.6). So B, registered but not
    /// holding, may RELEASE while A holds, and gets GOOD (SPC-6 5.14.11.2.2). A target that takes B for
    /// A's nexus, or for an unregistered one because it shares A's name, answers otherwise; every
    /// case between the two would then show that instead. Leaves no registration behind.
    pub(super) fn ports_of_one_initiator(&mut self, a: &mut Port, sibling: &mut Port) -> bool {
        let b = sibling;
        let setup = [
            out(a, ops::PR_REGISTER, 0, 0, KEY_A),
            out(b, ops::PR_REGISTER, 0, 0, KEY_B),
            out(a, ops::PR_RESERVE, 1, KEY_A, 0),
        ];
        let ready = setup.iter().all(|r| matches!(r, Ok(o) if o.status == 0));
        let passed = ready
            && self.status(
                "two ports of one initiator are two I_T nexuses: its second port RELEASE while A holds",
                &[Want::Good],
                out(b, ops::PR_RELEASE, 1, KEY_B, 0),
                // Windows' MPIO paths differ only in the ISID qualifier (see second_path), which this
                // port does not model, so it does not decide the clustering verdict.
                false,
            );
        let _ = out(a, ops::PR_CLEAR, 0, KEY_A, 0);
        let _ = unit_attentions(a);
        let _ = unit_attentions(b);
        passed
    }

    /// SPC-6 5.14.7: registering, tables 68 and 69, and the PRgeneration count (6.16.2).
    pub(super) fn registration(&mut self, a: &mut Port, b: &mut Port, c: &mut Port) {
        self.keys_are(c, "READ KEYS after CLEAR", vec![], true);
        let before = read_keys(c).map(|(g, _)| g).unwrap_or(0);
        // Table 68: an unregistered I_T nexus that names a non-zero RESERVATION KEY conflicts.
        self.status(
            "C REGISTER from an unregistered port with a non-zero key",
            &[Want::Conflict],
            out(c, ops::PR_REGISTER, 0, KEY_C, KEY_C),
            false,
        );
        self.status(
            "A REGISTER",
            &[Want::Good],
            out(a, ops::PR_REGISTER, 0, 0, KEY_A),
            true,
        );
        // Table 69: REGISTER AND IGNORE EXISTING KEY ignores the RESERVATION KEY field.
        self.status(
            "B REGISTER AND IGNORE EXISTING KEY",
            &[Want::Good],
            out(b, ops::PR_REGISTER_AND_IGNORE, 0, KEY_WRONG, KEY_B),
            true,
        );
        self.keys_are(c, "READ KEYS lists A and B", vec![KEY_A, KEY_B], true);
        let after = read_keys(c).map(|(g, _)| g);
        self.verdict(
            "PRgeneration counts the two registrations",
            format!("{}", before.wrapping_add(2)),
            match &after {
                Ok(g) => g.to_string(),
                Err(err) => err.clone(),
            },
            matches!(after, Ok(g) if g == before.wrapping_add(2)),
            false,
        );
        // Table 68: a registered I_T nexus naming a key that is not its own conflicts.
        self.status(
            "B REGISTER with the wrong reservation key",
            &[Want::Conflict],
            out(b, ops::PR_REGISTER, 0, KEY_WRONG, KEY_B2),
            false,
        );
        // Table 68: with its own key, REGISTER changes the key.
        self.status(
            "B REGISTER changes its key",
            &[Want::Good],
            out(b, ops::PR_REGISTER, 0, KEY_B, KEY_B2),
            false,
        );
        self.keys_are(c, "READ KEYS shows B's new key", vec![KEY_A, KEY_B2], false);
        self.status(
            "B REGISTER changes its key back",
            &[Want::Good],
            out(b, ops::PR_REGISTER, 0, KEY_B2, KEY_B),
            false,
        );
        // 5.14.7: any service action but REGISTER from an unregistered I_T nexus conflicts.
        self.status(
            "C RESERVE from an unregistered port",
            &[Want::Conflict],
            out(c, ops::PR_RESERVE, ops::PR_WRITE_EXCLUSIVE, KEY_C, 0),
            false,
        );
    }

    /// One reservation type: A reserves it, then SBC-5 table 13 decides what B (registered) and C (not
    /// registered) may read and write; then RESERVE and RELEASE by holder and non-holder (SPC-6 5.14.9,
    /// 5.14.11.2.2).
    pub(super) fn reservation_type(
        &mut self,
        a: &mut Port,
        b: &mut Port,
        c: &mut Port,
        caps: &Capabilities,
        kind: u8,
        label: &str,
    ) {
        let cluster = kind == ops::PR_WRITE_EXCLUSIVE_REGISTRANTS;
        let tag = format!("type {kind} ({label})");
        if !caps.supports(kind) {
            // SPC-6 6.15.2: a type the device server does not support is an invalid field in the CDB.
            self.status(
                &format!("{tag}: RESERVE of an unsupported type refused"),
                &[Want::Asc(0x5, 0x24)],
                out(a, ops::PR_RESERVE, kind, KEY_A, 0),
                cluster,
            );
            return;
        }
        if !self.status(
            &format!("{tag}: A RESERVE"),
            &[Want::Good],
            out(a, ops::PR_RESERVE, kind, KEY_A, 0),
            cluster,
        ) {
            return;
        }
        let all_registrants = matches!(kind, 7 | 8);
        let registrants = matches!(kind, 5..=8);
        let exclusive_access = matches!(kind, 3 | 6 | 8);
        // 5.14.10: an all registrants reservation reports key zero.
        let key = if all_registrants { 0 } else { KEY_A };
        self.holder_is(
            c,
            &format!("{tag}: READ RESERVATION"),
            Some((key, kind)),
            cluster,
        );
        // SBC-5 table 13: a registered nexus is like any other under the first two types, and may read
        // and write under the registrants types. A nexus that is not registered may read except under
        // the exclusive access types, and never write.
        let rules = [
            ("A (holder)", true, true),
            (
                "B (registered)",
                registrants || !exclusive_access,
                registrants,
            ),
            (self.c_name, !exclusive_access, false),
        ];
        for (i, (who, may_read, may_write)) in rules.into_iter().enumerate() {
            let s: &mut Port = match i {
                0 => &mut *a,
                1 => &mut *b,
                _ => &mut *c,
            };
            // A cluster's unregistered node has a name of its own; C shares one, so its answers count
            // for clustering only on a target that tells ports of one name apart.
            let cluster = cluster && (i < 2 || self.separate);
            let read = self.read(s);
            self.access(&format!("{tag}: {who} READ"), may_read, read, cluster);
            let write = self.write(s);
            self.access(&format!("{tag}: {who} WRITE"), may_write, write, cluster);
        }
        // 5.14.9: a holder reserving the same type gets GOOD; anyone else conflicts. Under the all
        // registrants types B is a holder too.
        self.status(
            &format!("{tag}: B RESERVE the same type"),
            &[allowed(all_registrants)],
            out(b, ops::PR_RESERVE, kind, KEY_B, 0),
            cluster,
        );
        if let Some(&(other, _)) = TYPES.iter().find(|(t, _)| *t != kind && caps.supports(*t)) {
            // 5.14.9: a holder that tries to change the type conflicts.
            self.status(
                &format!("{tag}: A RESERVE type {other} while holding"),
                &[Want::Conflict],
                out(a, ops::PR_RESERVE, other, KEY_A, 0),
                false,
            );
            // 5.14.11.2.2: a holder's RELEASE of another type is an invalid release.
            self.status(
                &format!("{tag}: A RELEASE of type {other}"),
                &[Want::Ascq(0x5, 0x26, 0x04)],
                out(a, ops::PR_RELEASE, other, KEY_A, 0),
                false,
            );
        }
        if !all_registrants {
            // 5.14.11.2.2: a RELEASE from a registered non-holder does nothing and returns GOOD.
            self.status(
                &format!("{tag}: B RELEASE by a non-holder"),
                &[Want::Good],
                out(b, ops::PR_RELEASE, kind, KEY_B, 0),
                false,
            );
            self.holder_is(
                c,
                &format!("{tag}: A still holds after B's RELEASE"),
                Some((KEY_A, kind)),
                false,
            );
        }
        self.status(
            &format!("{tag}: A RELEASE"),
            &[Want::Good],
            out(a, ops::PR_RELEASE, kind, KEY_A, 0),
            cluster,
        );
        // 5.14.11.2.2: releasing a registrants only or all registrants reservation tells every other
        // registered nexus RESERVATIONS RELEASED; releasing the other types tells nobody, and a nexus that
        // is not registered is never told.
        self.attention(
            b,
            &format!("{tag}: B told of the release"),
            registrants.then_some((0x2a, 0x04)),
            cluster,
        );
        self.attention(
            c,
            &format!("{tag}: C (not registered) not told of the release"),
            None,
            false,
        );
        self.holder_is(
            c,
            &format!("{tag}: no reservation after RELEASE"),
            None,
            cluster,
        );
        let _ = unit_attentions(a);
    }
}

/// The portals the target reports through SendTargets, the one given first. A target that reports no
/// other portal has only the one.
fn target_portals(path: &Path, initiator: &str) -> Vec<String> {
    let mut out = vec![path.portal.clone()];
    if let Ok(found) = super::send_targets::discover_targets(&path.portal, initiator) {
        for t in found.iter().filter(|t| t.target == path.target) {
            for address in &t.addresses {
                if !out.contains(address) {
                    out.push(address.clone());
                }
            }
        }
    }
    out
}
