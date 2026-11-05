//! The `pr` cases of `--compliance`: SPC-5 persistent reservations, which Windows Failover Clustering
//! uses to own a disk (REGISTER AND IGNORE EXISTING KEY, RESERVE or PREEMPT with Write Exclusive –
//! Registrants Only, READ KEYS, READ RESERVATION, RELEASE, CLEAR). Three initiator ports log in with
//! ISIDs that differ in their first bytes: A holds the reservation, B is registered and C is not. Each
//! of the six types is checked against the SBC-5 table 13 read and write rules for all three; then
//! preemption, reinstatement of A's session, a second path of A whose ISID differs only in its
//! qualifier (the way one host's MPIO paths differ), READ FULL STATUS, unregistering and CLEAR. Every
//! write sends the block's own data back, so the LUN is unchanged. Returns whether the commands Windows
//! clustering relies on all behaved.

use super::cases::{
    allowed, expect, expect_should, last_block, open, read_block, read_cmd, record,
    show_attentions, unit_attentions, write_cmd, Creds, Want,
};
use super::ops::{self, persistent_reserve_in, persistent_reserve_out, pr_parameters};
use super::report::Path;
use super::scsi;
use super::session::{Outcome, Session};
use crate::compliance::Case;
use crate::Result;

const SECTION: &str = "pr";

/// Keys for the tests: any distinct values, marked so a stray key is easy to spot.
const KEY_A: u64 = 0x5350_4954_0000_00a1;
const KEY_B: u64 = 0x5350_4954_0000_00b2;
const KEY_B2: u64 = 0x5350_4954_0000_00b3;
const KEY_C: u64 = 0x5350_4954_0000_00c3;
const KEY_WRONG: u64 = 0x5350_4954_0000_00ee;

/// Fixed ISIDs (random format, T = 10b) so a later run can CLEAR what an earlier one registered. A, B
/// and C differ in byte 1; A2 is A with another qualifier (bytes 4 and 5), a second path of one host.
const ISID_A: [u8; 6] = [0x80, 0xa1, 0x53, 0x50, 0x00, 0x01];
const ISID_A2: [u8; 6] = [0x80, 0xa1, 0x53, 0x50, 0x00, 0x02];
const ISID_B: [u8; 6] = [0x80, 0xb2, 0x53, 0x50, 0x00, 0x01];
const ISID_C: [u8; 6] = [0x80, 0xc3, 0x53, 0x50, 0x00, 0x01];

/// The six reservation types (SPC-6 6.14.3.3) and their names.
const TYPES: [(u8, &str); 6] = [
    (1, "write exclusive"),
    (3, "exclusive access"),
    (5, "write exclusive, registrants only"),
    (6, "exclusive access, registrants only"),
    (7, "write exclusive, all registrants"),
    (8, "exclusive access, all registrants"),
];

/// What REPORT CAPABILITIES says (SPC-6 6.14.4, table 179).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(super) struct Capabilities {
    pub crh: bool,
    pub sip_c: bool,
    pub atp_c: bool,
    pub ptpl_c: bool,
    pub tmv: bool,
    pub allow_commands: u8,
    /// The type mask, bytes 4 and 5.
    pub mask: u16,
}

impl Capabilities {
    pub(super) fn parse(d: &[u8]) -> Self {
        let at = |i: usize| d.get(i).copied().unwrap_or(0);
        Self {
            crh: at(2) & 0x10 != 0,
            sip_c: at(2) & 0x08 != 0,
            atp_c: at(2) & 0x04 != 0,
            ptpl_c: at(2) & 0x01 != 0,
            tmv: at(3) & 0x80 != 0,
            allow_commands: (at(3) >> 4) & 0x7,
            mask: u16::from_be_bytes([at(4), at(5)]),
        }
    }

    /// Whether the device server supports reservation type `t` (table 181). Without TMV the mask says
    /// nothing, so every type is tried.
    pub(super) fn supports(&self, t: u8) -> bool {
        let bit = match t {
            1 => 0x0200,
            3 => 0x0800,
            5 => 0x2000,
            6 => 0x4000,
            7 => 0x8000,
            8 => 0x0001,
            _ => 0,
        };
        !self.tmv || self.mask & bit != 0
    }

    fn describe(&self) -> String {
        let types: Vec<String> = TYPES
            .iter()
            .filter(|(t, _)| self.supports(*t))
            .map(|(t, _)| t.to_string())
            .collect();
        format!(
            "types {}{}, CRH {}, SIP_C {}, ATP_C {}, PTPL_C {}",
            types.join(","),
            if self.tmv { "" } else { " (no TMV)" },
            self.crh as u8,
            self.sip_c as u8,
            self.atp_c as u8,
            self.ptpl_c as u8
        )
    }
}

/// What the reservation run found, for the Windows cases.
pub(super) struct Findings {
    /// Every case Windows clustering relies on passed.
    pub cluster: bool,
    pub capabilities: Option<Capabilities>,
}

fn out(s: &mut Session, action: u8, kind: u8, key: u64, service_key: u64) -> Result<Outcome> {
    persistent_reserve_out(s, action, kind, &pr_parameters(key, service_key))
}

/// READ KEYS (SPC-6 6.14.2): the generation and every registered key, once per I_T nexus.
/// PERSISTENT RESERVE IN past any unit attentions pending for the nexus, which are checked by their own
/// cases: a state query reports the state.
fn reserve_in(s: &mut Session, action: u8, alloc: u16) -> Result<Outcome> {
    let mut o = persistent_reserve_in(s, action, alloc)?;
    for _ in 0..4 {
        if !(o.status == 2 && super::cases::sense(&o.sense).0 == 0x6) {
            break;
        }
        o = persistent_reserve_in(s, action, alloc)?;
    }
    Ok(o)
}

fn read_keys(s: &mut Session) -> Result<(u32, Vec<u64>)> {
    let o = reserve_in(s, ops::PR_IN_READ_KEYS, 1024)?;
    scsi::check_status("PERSISTENT RESERVE IN (read keys)", o.status, &o.sense)?;
    Ok(parse_keys(&o.data))
}

fn parse_keys(d: &[u8]) -> (u32, Vec<u64>) {
    let generation = scsi::be32(d, 0).unwrap_or(0);
    let len = scsi::be32(d, 4).unwrap_or(0) as usize;
    let keys = d
        .get(8..(8 + len).min(d.len()))
        .unwrap_or(&[])
        .chunks_exact(8)
        .map(|k| u64::from_be_bytes(k.try_into().unwrap()))
        .collect();
    (generation, keys)
}

/// READ RESERVATION (SPC-6 6.14.3): the holder's key and the type, or None when there is none.
fn read_holder(s: &mut Session) -> Result<Option<(u64, u8)>> {
    let o = reserve_in(s, ops::PR_IN_READ_RESERVATIONS, 512)?;
    scsi::check_status(
        "PERSISTENT RESERVE IN (read reservation)",
        o.status,
        &o.sense,
    )?;
    Ok(parse_holder(&o.data))
}

fn parse_holder(d: &[u8]) -> Option<(u64, u8)> {
    let len = scsi::be32(d, 4).unwrap_or(0);
    if len < 16 || d.len() < 22 {
        return None;
    }
    let key = u64::from_be_bytes(d[8..16].try_into().unwrap());
    Some((key, d[21] & 0x0f))
}

/// One full status descriptor (SPC-6 6.14.5, table 183).
#[derive(Debug, Clone, PartialEq)]
struct FullStatus {
    key: u64,
    holder: bool,
    all_ports: bool,
    protocol: u8,
    /// The TransportID's name and ISID as text, e.g. `iqn...,i,0x80a153500001`.
    transport: String,
}

fn parse_full_status(d: &[u8]) -> Vec<FullStatus> {
    let total = scsi::be32(d, 4).unwrap_or(0) as usize;
    let end = (8 + total).min(d.len());
    let mut out = Vec::new();
    let mut i = 8;
    while i + 24 <= end {
        let len = scsi::be32(d, i + 20).unwrap_or(0) as usize;
        let id = d.get(i + 24..(i + 24 + len).min(end)).unwrap_or(&[]);
        let transport = id
            .get(4..)
            .map(|t| {
                String::from_utf8_lossy(t)
                    .trim_end_matches('\0')
                    .to_string()
            })
            .unwrap_or_default();
        out.push(FullStatus {
            key: u64::from_be_bytes(d[i..i + 8].try_into().unwrap()),
            holder: d[i + 12] & 0x01 != 0,
            all_ports: d[i + 12] & 0x02 != 0,
            protocol: id.first().copied().unwrap_or(0xff) & 0x0f,
            transport,
        });
        i += 24 + len;
    }
    out
}

fn hex(isid: &[u8; 6]) -> String {
    isid.iter().map(|b| format!("{b:02x}")).collect()
}

fn show_holder(h: &Result<Option<(u64, u8)>>) -> String {
    match h {
        Ok(Some((k, t))) => format!("key {k:#x}, type {t}"),
        Ok(None) => "none".into(),
        Err(err) => err.clone(),
    }
}

fn show_keys(k: &Result<(u32, Vec<u64>)>) -> String {
    match k {
        Ok((_, keys)) if keys.is_empty() => "none".into(),
        Ok((_, keys)) => {
            let all: Vec<String> = keys.iter().map(|k| format!("{k:#x}")).collect();
            all.join(", ")
        }
        Err(err) => err.clone(),
    }
}

/// Registers on the LUN as initiator port `isid` with REGISTER AND IGNORE EXISTING KEY, which works
/// whatever key it had (SPC-6 5.14.7, table 69), then CLEARs (5.14.11.2.7).
fn clear_as(path: &Path, initiator: &str, creds: Creds, isid: [u8; 6]) -> Result<()> {
    let mut s = open(path, initiator, creds, isid)?;
    let _ = unit_attentions(&mut s);
    let register = out(&mut s, ops::PR_REGISTER_AND_IGNORE, 0, 0, KEY_A)?;
    scsi::check_status(
        "REGISTER AND IGNORE EXISTING KEY",
        register.status,
        &register.sense,
    )?;
    let clear = out(&mut s, ops::PR_CLEAR, 0, KEY_A, 0)?;
    scsi::check_status("CLEAR", clear.status, &clear.sense)?;
    let _ = s.logout();
    Ok(())
}

/// The ISIDs this initiator's registrations carry, from READ FULL STATUS TransportIDs
/// (`IQN,i,0xISID`, SPC-6 7.6.4.5).
fn registered_isids(path: &Path, initiator: &str, creds: Creds) -> Vec<[u8; 6]> {
    let Ok(mut s) = open(path, initiator, creds, ISID_C) else {
        return Vec::new();
    };
    let _ = unit_attentions(&mut s);
    let list = reserve_in(&mut s, ops::PR_IN_READ_FULL_STATUS, 8192)
        .ok()
        .filter(|o| o.status == 0)
        .map(|o| parse_full_status(&o.data))
        .unwrap_or_default();
    let _ = s.logout();
    let prefix = format!("{},i,0x", initiator.to_lowercase());
    list.iter()
        .filter_map(|d| {
            let hex = d
                .transport
                .to_lowercase()
                .strip_prefix(&prefix)?
                .to_string();
            let bytes: Vec<u8> = (0..6)
                .filter_map(|i| u8::from_str_radix(hex.get(2 * i..2 * i + 2)?, 16).ok())
                .collect();
            bytes.try_into().ok()
        })
        .collect()
}

/// Clears every registration and reservation on the LUN, so a run that stopped part way cannot make
/// the next one fail. CLEAR is tried from A's port first; when a reservation held through another
/// initiator port of this IQN refuses it (a target that treats that port as not registered), it is
/// tried from each registered port READ FULL STATUS names. Records a case only when all fail.
pub(super) fn clean_slate(
    path: &Path,
    initiator: &str,
    creds: Creds,
    name: &str,
    cases: &mut Vec<Case>,
) {
    let Err(first) = clear_as(path, initiator, creds, ISID_A) else {
        return;
    };
    for isid in registered_isids(path, initiator, creds) {
        if clear_as(path, initiator, creds, isid).is_ok() {
            return;
        }
    }
    record(
        cases,
        SECTION,
        format!("{name} clean slate"),
        "GOOD",
        first,
        false,
    );
}

/// Runs every reservation case on one LUN.
pub(super) fn run(
    path: &Path,
    initiator: &str,
    creds: Creds,
    name: &str,
    cases: &mut Vec<Case>,
) -> Findings {
    let mut findings = Findings {
        cluster: false,
        capabilities: None,
    };
    let (mut a, mut b, mut c) = match (
        open(path, initiator, creds, ISID_A),
        open(path, initiator, creds, ISID_B),
        open(path, initiator, creds, ISID_C),
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
    let block = last_block(&mut a).and_then(|(lba, len)| {
        let target = lba.saturating_sub(1);
        read_block(&mut a, target, len).map(|data| (target, data))
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
    };
    findings.capabilities = t.capabilities(&mut a);
    let caps = findings.capabilities.unwrap_or_default();
    t.registration(&mut a, &mut b, &mut c);
    for (kind, label) in TYPES {
        t.reservation_type(&mut a, &mut b, &mut c, &caps, kind, label);
    }
    let preempt_type = if caps.supports(5) { 5 } else { 1 };
    t.preemption(&mut a, &mut b, &mut c, preempt_type);
    if let Some(new_a) = t.reinstatement(path, initiator, creds, a, &mut b) {
        a = new_a;
        t.second_path(path, initiator, creds, &mut a, &mut b, &mut c);
        if caps.crh {
            t.spc2_release(&mut a, &mut c);
        }
        t.unregistering(&mut a, &mut c);
        t.clearing(&mut a, &mut b, &mut c);
        let _ = a.logout();
    }
    findings.cluster = t.cluster;
    let _ = b.logout();
    let _ = c.logout();
    clean_slate(path, initiator, creds, name, cases);
    findings
}

/// The state of one reservation run: where it writes and whether the clustering cases still pass.
struct Run<'a> {
    cases: &'a mut Vec<Case>,
    name: &'a str,
    target: u32,
    original: Vec<u8>,
    cluster: bool,
}

impl Run<'_> {
    /// Records a status case. `cluster` marks the cases Windows clustering depends on.
    fn status(
        &mut self,
        label: &str,
        wants: &[Want],
        result: Result<Outcome>,
        cluster: bool,
    ) -> bool {
        let (passed, _) = expect(
            self.cases,
            SECTION,
            format!("{} {label}", self.name),
            wants,
            result,
        );
        if cluster {
            self.cluster &= passed;
        }
        passed
    }

    /// SBC-5 table 13: an allowed command "should complete normally" (a miss warns); a conflicting one
    /// "shall" get RESERVATION CONFLICT (a miss fails). Windows clustering needs both either way.
    fn access(&mut self, label: &str, may: bool, result: Result<Outcome>, cluster: bool) -> bool {
        let name = format!("{} {label}", self.name);
        let (passed, _) = if may {
            expect_should(self.cases, SECTION, name, &[Want::Good], result)
        } else {
            expect(self.cases, SECTION, name, &[Want::Conflict], result)
        };
        if cluster {
            self.cluster &= passed;
        }
        passed
    }

    fn verdict(
        &mut self,
        label: &str,
        expected: String,
        actual: String,
        passed: bool,
        cluster: bool,
    ) {
        record(
            self.cases,
            SECTION,
            format!("{} {label}", self.name),
            expected,
            actual,
            passed,
        );
        if cluster {
            self.cluster &= passed;
        }
    }

    fn write(&self, s: &mut Session) -> Result<Outcome> {
        write_cmd(s, self.target, &self.original)
    }

    fn read(&self, s: &mut Session) -> Result<Outcome> {
        read_cmd(s, self.target, self.original.len())
    }

    fn holder_is(&mut self, s: &mut Session, label: &str, want: Option<(u64, u8)>, cluster: bool) {
        let holder = read_holder(s);
        let expected = match want {
            Some((k, t)) => format!("key {k:#x}, type {t}"),
            None => "none".into(),
        };
        let passed = matches!(&holder, Ok(h) if *h == want);
        self.verdict(label, expected, show_holder(&holder), passed, cluster);
    }

    fn keys_are(&mut self, s: &mut Session, label: &str, mut want: Vec<u64>, cluster: bool) {
        let keys = read_keys(s);
        want.sort_unstable();
        let expected = if want.is_empty() {
            "none".to_string()
        } else {
            let all: Vec<String> = want.iter().map(|k| format!("{k:#x}")).collect();
            all.join(", ")
        };
        let passed =
            matches!(&keys, Ok((_, k)) if { let mut k = k.clone(); k.sort_unstable(); k == want });
        self.verdict(label, expected, show_keys(&keys), passed, cluster);
    }

    /// Checks the unit attentions pending for `s` against SPC-5: `want` is the one that must be there,
    /// or None when there must be none. Drains them either way.
    fn attention(&mut self, s: &mut Session, label: &str, want: Option<(u8, u8)>, cluster: bool) {
        let seen = unit_attentions(s);
        let (expected, passed) = match want {
            Some((asc, ascq)) => (format!("{asc:02x}/{ascq:02x}"), seen.contains(&(asc, ascq))),
            None => (
                "none".to_string(),
                !seen.iter().any(|&(asc, _)| asc == 0x2a),
            ),
        };
        self.verdict(label, expected, show_attentions(&seen), passed, cluster);
    }

    /// SPC-6 6.14.4: REPORT CAPABILITIES returns 8 bytes with LENGTH 8.
    fn capabilities(&mut self, a: &mut Session) -> Option<Capabilities> {
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

    /// SPC-6 5.14.7: registering, tables 68 and 69, and the PRgeneration count (6.16.2).
    fn registration(&mut self, a: &mut Session, b: &mut Session, c: &mut Session) {
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
    fn reservation_type(
        &mut self,
        a: &mut Session,
        b: &mut Session,
        c: &mut Session,
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
            ("C (not registered)", !exclusive_access, false),
        ];
        for (i, (who, may_read, may_write)) in rules.into_iter().enumerate() {
            let s: &mut Session = match i {
                0 => &mut *a,
                1 => &mut *b,
                _ => &mut *c,
            };
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

    /// SPC-6 5.14.11.2.4: PREEMPT and PREEMPT AND ABORT, the way a clustering node takes a disk from a
    /// node that stopped answering.
    fn preemption(&mut self, a: &mut Session, b: &mut Session, c: &mut Session, kind: u8) {
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
        for s in [a, b, c] {
            let _ = unit_attentions(s);
        }
    }

    /// RFC 7143 6.3.5 and SPC-6 5.14.7: logging in again with A's ISID reinstates A's session, and the
    /// registration and reservation belong to the I_T nexus, so they survive. Windows does this when a
    /// path comes back. Returns the new session for A.
    fn reinstatement(
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
    fn second_path(
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
    fn full_status(&mut self, s: &mut Session) {
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
    fn spc2_release(&mut self, a: &mut Session, c: &mut Session) {
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
    fn unregistering(&mut self, a: &mut Session, c: &mut Session) {
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
    fn clearing(&mut self, a: &mut Session, b: &mut Session, c: &mut Session) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_decode_the_type_mask() {
        // CRH and PTPL_C; TMV; WR_EX, EX_AC, WR_EX_RO, EX_AC_RO, WR_EX_AR and EX_AC_AR.
        let caps = Capabilities::parse(&[0, 8, 0x11, 0x80, 0xea, 0x01, 0, 0]);
        assert!(caps.crh && caps.ptpl_c && caps.tmv && !caps.sip_c);
        for t in [1, 3, 5, 6, 7, 8] {
            assert!(caps.supports(t), "type {t}");
        }
        let only_we = Capabilities::parse(&[0, 8, 0, 0x80, 0x02, 0, 0, 0]);
        assert!(only_we.supports(1) && !only_we.supports(5) && !only_we.supports(8));
        let no_tmv = Capabilities::parse(&[0, 8, 0, 0, 0, 0, 0, 0]);
        assert!(no_tmv.supports(5));
    }

    #[test]
    fn keys_and_holder_parse() {
        let mut d = vec![0, 0, 0, 7, 0, 0, 0, 16];
        d.extend_from_slice(&KEY_A.to_be_bytes());
        d.extend_from_slice(&KEY_B.to_be_bytes());
        assert_eq!(parse_keys(&d), (7, vec![KEY_A, KEY_B]));
        let mut r = vec![0, 0, 0, 1, 0, 0, 0, 16];
        r.extend_from_slice(&KEY_A.to_be_bytes());
        r.extend_from_slice(&[0, 0, 0, 0, 0, 0x05, 0, 0]);
        assert_eq!(parse_holder(&r), Some((KEY_A, 5)));
        assert_eq!(parse_holder(&[0, 0, 0, 1, 0, 0, 0, 0]), None);
    }

    #[test]
    fn full_status_reads_holder_and_transport_id() {
        let name = b"iqn.x,i,0x80a153500001\0\0";
        let mut id = vec![0x45, 0, 0, name.len() as u8];
        id.extend_from_slice(name);
        let mut desc = KEY_A.to_be_bytes().to_vec();
        desc.extend_from_slice(&[0, 0, 0, 0, 0x01, 0x01, 0, 0, 0, 0, 0, 1]);
        desc.extend_from_slice(&(id.len() as u32).to_be_bytes());
        desc.extend_from_slice(&id);
        let mut d = vec![0, 0, 0, 3];
        d.extend_from_slice(&(desc.len() as u32).to_be_bytes());
        d.extend_from_slice(&desc);
        let list = parse_full_status(&d);
        assert_eq!(list.len(), 1);
        assert!(list[0].holder && !list[0].all_ports);
        assert_eq!(list[0].protocol, 5);
        assert_eq!(list[0].transport, "iqn.x,i,0x80a153500001");
        assert_eq!(hex(&ISID_A), "80a153500001");
    }

    #[test]
    fn isids_differ_where_intended() {
        assert_eq!(ISID_A[..4], ISID_A2[..4], "same host part");
        assert_ne!(ISID_A[4..], ISID_A2[4..], "different qualifier");
        assert_ne!(ISID_A[1], ISID_B[1]);
        assert_ne!(ISID_B[1], ISID_C[1]);
        assert!([ISID_A, ISID_A2, ISID_B, ISID_C]
            .iter()
            .all(|i| i[0] >> 6 == 0b10));
    }
}
