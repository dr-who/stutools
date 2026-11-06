//! The `pr` cases of how a reservation changes hands or survives (SPC-6 5.14.11): PREEMPT and PREEMPT
//! AND ABORT (5.14.11.2.4, 5.14.11.2.6), the holder's session reinstated with its ISID (RFC 7143
//! 6.3.5), a second path of the holder's host whose ISID differs only in its qualifier, as Windows MPIO
//! paths do, READ FULL STATUS, SPC-2 RELEASE beside a persistent reservation (5.14.3), unregistering,
//! and CLEAR (5.14.11.2.7).

use super::cases::{expect, Want};
use super::lun_access::{open, unit_attentions, Creds};
use super::ops::{self, persistent_reserve_in};
use super::path::Path;
use super::pr_commands::{hex, out, parse_full_status, read_holder, read_keys, FullStatus};
use super::pr_ports::{Run, ISID_A, ISID_A2, KEY_A, KEY_B, KEY_C, SECTION};
use super::session::Session;

impl Run<'_> {
    /// SPC-6 5.14.11.2.4: PREEMPT and PREEMPT AND ABORT, the way a clustering node takes a disk from a
    /// node that stopped answering.
    pub(super) fn preemption(
        &mut self,
        a: &mut Session,
        b: &mut Session,
        c: &mut Session,
        kind: u8,
    ) {
        let tag = format!("type {kind}");
        self.status(
            &format!("{tag}: A RESERVE"),
            &[Want::Good],
            out(a, ops::PR_RESERVE, kind, KEY_A, 0),
            true,
        );
        // Table 67: PREEMPT from a nexus that is not registered conflicts.
        self.status(
            &format!("{tag}: C PREEMPT from an unregistered port"),
            &[Want::Conflict],
            out(c, ops::PR_PREEMPT, kind, KEY_C, KEY_A),
            true,
        );
        self.status(
            &format!("{tag}: B PREEMPT A"),
            &[Want::Good],
            out(b, ops::PR_PREEMPT, kind, KEY_B, KEY_A),
            true,
        );
        // 5.14.11.2.4.3 e): the preempted nexus is told REGISTRATIONS PREEMPTED.
        self.attention(
            a,
            &format!("{tag}: A told it was preempted"),
            Some((0x2a, 0x05)),
            true,
        );
        self.keys_are(
            c,
            &format!("{tag}: only B registered after PREEMPT"),
            vec![KEY_B],
            true,
        );
        self.holder_is(
            c,
            &format!("{tag}: B holds after PREEMPT"),
            Some((KEY_B, kind)),
            true,
        );
        // SBC-5 table 13: A is no longer registered, so it may read and may not write.
        let write = self.write(a);
        self.status(
            &format!("{tag}: preempted A WRITE"),
            &[Want::Conflict],
            write,
            true,
        );
        let read = self.read(a);
        self.status(
            &format!("{tag}: preempted A READ"),
            &[Want::Good],
            read,
            true,
        );
        self.status(
            &format!("{tag}: A registers again (REGISTER AND IGNORE EXISTING KEY)"),
            &[Want::Good],
            out(a, ops::PR_REGISTER_AND_IGNORE, 0, 0, KEY_A),
            true,
        );
        // 5.14.11.2.6: PREEMPT AND ABORT preempts as PREEMPT does, and aborts B's commands.
        self.status(
            &format!("{tag}: A PREEMPT AND ABORT B"),
            &[Want::Good],
            out(a, ops::PR_PREEMPT_AND_ABORT, kind, KEY_A, KEY_B),
            false,
        );
        self.attention(
            b,
            &format!("{tag}: B told it was preempted"),
            Some((0x2a, 0x05)),
            false,
        );
        self.holder_is(
            c,
            &format!("{tag}: A holds after PREEMPT AND ABORT"),
            Some((KEY_A, kind)),
            false,
        );
        self.status(
            &format!("{tag}: B registers again"),
            &[Want::Good],
            out(b, ops::PR_REGISTER_AND_IGNORE, 0, 0, KEY_B),
            true,
        );
        self.status(
            &format!("{tag}: A RELEASE"),
            &[Want::Good],
            out(a, ops::PR_RELEASE, kind, KEY_A, 0),
            true,
        );
        self.settle(a, b, c, None, true);
    }

    /// Puts the logical unit in the state the next phase starts from, unrecorded, when a target that
    /// missed a step left it elsewhere: A holding `hold` (or nothing reserved) and, when `both`, A and
    /// B registered. One fault is then judged once, not again by every case after it. A target that
    /// did every step is already there and is sent nothing.
    pub(super) fn settle(
        &mut self,
        a: &mut Session,
        b: &mut Session,
        c: &mut Session,
        hold: Option<u8>,
        both: bool,
    ) {
        let want = hold.map(|kind| (KEY_A, kind));
        let keys = read_keys(c).map(|(_, k)| k).unwrap_or_default();
        let registered = !both || (keys.contains(&KEY_A) && keys.contains(&KEY_B));
        if read_holder(c).ok() != Some(want) || !registered {
            let _ = out(a, ops::PR_REGISTER_AND_IGNORE, 0, 0, KEY_A);
            let _ = out(a, ops::PR_CLEAR, 0, KEY_A, 0);
            // The CLEAR leaves RESERVATIONS PREEMPTED pending on the other nexuses; their next
            // command would only report it.
            for s in [&mut *a, &mut *b, &mut *c] {
                let _ = unit_attentions(s);
            }
            let _ = out(a, ops::PR_REGISTER_AND_IGNORE, 0, 0, KEY_A);
            let _ = out(b, ops::PR_REGISTER_AND_IGNORE, 0, 0, KEY_B);
            if let Some(kind) = hold {
                let _ = out(a, ops::PR_RESERVE, kind, KEY_A, 0);
            }
        }
        for s in [a, b, c] {
            let _ = unit_attentions(s);
        }
    }

    /// RFC 7143 6.3.5 and SPC-6 5.14.7: logging in again with A's ISID reinstates A's session, and the
    /// registration and reservation belong to the I_T nexus, so they survive. Windows does this when a
    /// path comes back. Returns the new session for A.
    pub(super) fn reinstatement(
        &mut self,
        path: &Path,
        initiator: &str,
        creds: Creds,
        mut a: Session,
        b: &mut Session,
    ) -> Option<Session> {
        self.status(
            "A RESERVE write exclusive",
            &[Want::Good],
            out(&mut a, ops::PR_RESERVE, ops::PR_WRITE_EXCLUSIVE, KEY_A, 0),
            true,
        );
        let mut again = match open(path, initiator, creds, ISID_A) {
            Ok(s) => s,
            Err(err) => {
                self.verdict(
                    "A logs in again with its ISID",
                    "login ok".into(),
                    err,
                    false,
                    true,
                );
                return None;
            }
        };
        drop(a);
        let _ = unit_attentions(&mut again);
        self.holder_is(
            &mut again,
            "reservation kept across A's reinstated session",
            Some((KEY_A, ops::PR_WRITE_EXCLUSIVE)),
            true,
        );
        let write = self.write(&mut again);
        self.status("reinstated A WRITE as holder", &[Want::Good], write, true);
        let write = self.write(b);
        self.status(
            "B WRITE while reinstated A holds",
            &[Want::Conflict],
            write,
            true,
        );
        Some(again)
    }

    /// A second path of A's host: same IQN, an ISID that differs only in the qualifier. SPC-6 5.14.7 d)
    /// B): the initiator port name (IQN and the whole ISID) names the nexus, so this is another I_T
    /// nexus and not the holder. Windows MSDSM registers the key on every path.
    pub(super) fn second_path(
        &mut self,
        path: &Path,
        initiator: &str,
        creds: Creds,
        a: &mut Session,
        b: &mut Session,
        c: &mut Session,
    ) {
        let mut p = match open(path, initiator, creds, ISID_A2) {
            Ok(s) => s,
            Err(err) => {
                self.verdict(
                    "A's second path logs in",
                    "login ok".into(),
                    err,
                    false,
                    true,
                );
                return;
            }
        };
        let _ = unit_attentions(&mut p);
        let write = self.write(&mut p);
        self.status(
            "A's second path (ISID differs in its qualifier) WRITE while A holds",
            &[Want::Conflict],
            write,
            false,
        );
        self.status(
            "A's second path REGISTER AND IGNORE EXISTING KEY",
            &[Want::Good],
            out(&mut p, ops::PR_REGISTER_AND_IGNORE, 0, 0, KEY_A),
            true,
        );
        // 6.16.2: a key registered by two I_T nexuses is listed twice.
        self.keys_are(
            c,
            "READ KEYS lists A's key once per path",
            vec![KEY_A, KEY_A, KEY_B],
            true,
        );
        let write = self.write(&mut p);
        self.status(
            "A's registered second path WRITE while A holds",
            &[Want::Conflict],
            write,
            false,
        );
        self.full_status(c);
        // 5.14.11.2.3: unregistering a nexus that is not the holder leaves the reservation alone.
        self.status(
            "A's second path unregisters",
            &[Want::Good],
            out(&mut p, ops::PR_REGISTER, 0, KEY_A, 0),
            false,
        );
        self.keys_are(
            c,
            "READ KEYS after the second path unregisters",
            vec![KEY_A, KEY_B],
            false,
        );
        self.holder_is(
            b,
            "A still holds",
            Some((KEY_A, ops::PR_WRITE_EXCLUSIVE)),
            false,
        );
        let _ = a;
        let _ = p.logout();
    }

    /// SPC-6 6.14.5: READ FULL STATUS describes every registered nexus with its TransportID; the holder
    /// is A's port, named by its IQN and ISID (7.6.4.5).
    pub(super) fn full_status(&mut self, s: &mut Session) {
        let o = persistent_reserve_in(s, ops::PR_IN_READ_FULL_STATUS, 4096);
        let (passed, o) = expect(
            self.cases,
            SECTION,
            format!("{} READ FULL STATUS", self.name),
            &[Want::Good],
            o,
        );
        let Some(o) = o.filter(|_| passed) else {
            return;
        };
        let list = parse_full_status(&o.data);
        let a_port = format!(",i,0x{}", hex(&ISID_A));
        let holders: Vec<&FullStatus> = list.iter().filter(|d| d.holder).collect();
        let holder_ok = holders.len() == 1
            && holders[0].key == KEY_A
            && holders[0].transport.to_lowercase().contains(&a_port);
        let iscsi = list.iter().all(|d| d.protocol == 5);
        let covered: usize = list.iter().filter(|d| !d.all_ports).count();
        self.verdict(
            "READ FULL STATUS names the holder's port",
            format!("one holder, key {KEY_A:#x}, TransportID ending {a_port}"),
            holders
                .iter()
                .map(|d| format!("key {:#x} {}", d.key, d.transport))
                .collect::<Vec<_>>()
                .join("; "),
            holder_ok,
            false,
        );
        self.verdict(
            "READ FULL STATUS lists each registered nexus as iSCSI",
            "3 descriptors, protocol 5".into(),
            format!("{} descriptors ({covered} single-port)", list.len()),
            iscsi && (list.len() == 3 || list.iter().any(|d| d.all_ports)),
            false,
        );
    }

    /// SPC-6 5.14.3: with CRH set, RELEASE(6) from the holder returns GOOD and releases nothing.
    pub(super) fn spc2_release(&mut self, a: &mut Session, c: &mut Session) {
        self.status(
            "RELEASE(6) from the holder (CRH)",
            &[Want::Good],
            a.command(&ops::RELEASE_6, 0, None),
            false,
        );
        self.holder_is(
            c,
            "RELEASE(6) leaves the reservation",
            Some((KEY_A, ops::PR_WRITE_EXCLUSIVE)),
            false,
        );
    }

    /// SPC-6 5.14.11.2.3 and 5.14.10: when the holder unregisters, a write exclusive reservation goes
    /// with it.
    pub(super) fn unregistering(&mut self, a: &mut Session, c: &mut Session) {
        self.status(
            "holder A unregisters",
            &[Want::Good],
            out(a, ops::PR_REGISTER, 0, KEY_A, 0),
            false,
        );
        self.holder_is(c, "no reservation once the holder unregisters", None, false);
    }

    /// SPC-6 5.14.11.2.7: CLEAR releases the reservation, removes every registration, and tells every
    /// other registered nexus RESERVATIONS PREEMPTED.
    pub(super) fn clearing(&mut self, a: &mut Session, b: &mut Session, c: &mut Session) {
        self.status(
            "A registers again",
            &[Want::Good],
            out(a, ops::PR_REGISTER_AND_IGNORE, 0, 0, KEY_A),
            true,
        );
        self.status(
            "A RESERVE write exclusive before CLEAR",
            &[Want::Good],
            out(a, ops::PR_RESERVE, ops::PR_WRITE_EXCLUSIVE, KEY_A, 0),
            true,
        );
        self.status(
            "B CLEAR",
            &[Want::Good],
            out(b, ops::PR_CLEAR, 0, KEY_B, 0),
            true,
        );
        self.attention(a, "A told of the CLEAR", Some((0x2a, 0x03)), true);
        self.keys_are(c, "no keys after CLEAR", vec![], true);
        self.holder_is(c, "no reservation after CLEAR", None, true);
        let write = self.write(c);
        self.status("C WRITE after CLEAR", &[Want::Good], write, true);
    }
}
