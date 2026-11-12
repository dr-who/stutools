//! The initiator ports of the `pr` cases and the checks they make. A, B and C are three I_T nexuses:
//! fixed ISIDs (so a later run can CLEAR what an earlier one left) and keys marked so a stray one is
//! easy to spot. `clean_slate` clears every registration on the LUN, from whichever port can. `Run` is
//! one LUN's run: where it writes (the second-to-last block, always written with its own data), the
//! checks a case makes (a status, an access, the holder, the keys, the unit attentions), and whether
//! the cases Windows clustering relies on still pass.

use super::cases::{expect, expect_should, record, Want};
use super::lun_access::{open, read_cmd64, show_attentions, unit_attentions, write_cmd64, Creds};
use super::ops::{self};
use super::path::Path;
use super::port::Port;
use super::pr_commands::{
    out, parse_full_status, read_holder, read_keys, reserve_in, show_holder, show_keys,
};
use super::scsi;
use super::session::Outcome;
use crate::compliance::Case;
use crate::Result;

pub(super) const SECTION: &str = "pr";

/// Keys for the tests: any distinct values, marked so a stray key is easy to spot.
pub(super) const KEY_A: u64 = 0x5350_4954_0000_00a1;

pub(super) const KEY_B: u64 = 0x5350_4954_0000_00b2;

pub(super) const KEY_B2: u64 = 0x5350_4954_0000_00b3;

pub(super) const KEY_C: u64 = 0x5350_4954_0000_00c3;

pub(super) const KEY_WRONG: u64 = 0x5350_4954_0000_00ee;

/// Fixed ISIDs (random format, T = 10b) so a later run can CLEAR what an earlier one registered. A, B
/// and C differ in byte 1; A2 is A with another qualifier (bytes 4 and 5), a second path of one host.
pub(super) const ISID_A: [u8; 6] = [0x80, 0xa1, 0x53, 0x50, 0x00, 0x01];

pub(super) const ISID_A2: [u8; 6] = [0x80, 0xa1, 0x53, 0x50, 0x00, 0x02];

pub(super) const ISID_B: [u8; 6] = [0x80, 0xb2, 0x53, 0x50, 0x00, 0x01];

pub(super) const ISID_C: [u8; 6] = [0x80, 0xc3, 0x53, 0x50, 0x00, 0x01];

/// The port that checks two ports of A's initiator are two I_T nexuses (`ports_of_one_initiator`).
pub(super) const ISID_SIBLING: [u8; 6] = [0x80, 0xd4, 0x53, 0x50, 0x00, 0x01];

/// Registers on the LUN as initiator port `isid` with REGISTER AND IGNORE EXISTING KEY, which works
/// whatever key it had (SPC-6 5.14.7, table 69), then CLEARs (5.14.11.2.7).
pub(super) fn clear_as(path: &Path, initiator: &str, creds: Creds, isid: [u8; 6]) -> Result<()> {
    let mut s = open(path, initiator, creds, isid)?;
    let _ = unit_attentions(&mut s);
    let register = out(&mut s, ops::PR_REGISTER_AND_IGNORE, 0, 0, KEY_A)?;
    if super::cases::invalid_opcode(&register) {
        return Err(super::cases::NOT_IMPLEMENTED.into());
    }
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
pub(super) fn registered_isids(path: &Path, initiator: &str, creds: Creds) -> Vec<[u8; 6]> {
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

/// The state of one reservation run: where it writes and whether the clustering cases still pass.
pub(super) struct Run<'a> {
    pub(super) cases: &'a mut Vec<Case>,
    pub(super) name: &'a str,
    pub(super) target: u64,
    pub(super) original: Vec<u8>,
    pub(super) cluster: bool,
    /// What C is called: it shares B's initiator name when a second name was given.
    pub(super) c_name: &'static str,
    /// The target told two ports of one initiator apart (`ports_of_one_initiator`); only then do C's
    /// cases, on a port that shares a registrant's name, count for clustering.
    pub(super) separate: bool,
    /// A local SAS disk: TransportIDs are SAS addresses (SPC-6 7.6.4), not iSCSI names.
    pub(super) local: bool,
}

impl Run<'_> {
    /// Records a status case. `cluster` marks the cases Windows clustering depends on.
    pub(super) fn status(
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
    pub(super) fn access(
        &mut self,
        label: &str,
        may: bool,
        result: Result<Outcome>,
        cluster: bool,
    ) -> bool {
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

    pub(super) fn verdict(
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

    pub(super) fn write(&self, s: &mut Port) -> Result<Outcome> {
        write_cmd64(s, self.target, &self.original)
    }

    pub(super) fn read(&self, s: &mut Port) -> Result<Outcome> {
        read_cmd64(s, self.target, self.original.len())
    }

    pub(super) fn holder_is(
        &mut self,
        s: &mut Port,
        label: &str,
        want: Option<(u64, u8)>,
        cluster: bool,
    ) {
        let holder = read_holder(s);
        let expected = match want {
            Some((k, t)) => format!("key {k:#x}, type {t}"),
            None => "none".into(),
        };
        let passed = matches!(&holder, Ok(h) if *h == want);
        self.verdict(label, expected, show_holder(&holder), passed, cluster);
    }

    pub(super) fn keys_are(
        &mut self,
        s: &mut Port,
        label: &str,
        mut want: Vec<u64>,
        cluster: bool,
    ) {
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
    pub(super) fn attention(
        &mut self,
        s: &mut Port,
        label: &str,
        want: Option<(u8, u8)>,
        cluster: bool,
    ) {
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
