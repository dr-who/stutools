//! The SCSI operations cases of `--scsitest`: UNMAP, COMPARE AND WRITE (ATS) and PERSISTENT RESERVE.
//! Each case states what SBC-3 or SPC-4 requires, then checks the target against it. Every case
//! that changes data writes to the last block of the LUN, and the original block is written back.

use super::ops::{self, CHECK_CONDITION, RESERVATION_CONFLICT};
use super::report::Path;
use super::scsi;
use super::scsitest::{case, Case};
use super::session::{self, Outcome, Session};
use crate::Result;

/// Credentials for one target: the CHAP user and secret, when the target requires CHAP.
pub(super) type Creds<'a> = Option<(&'a str, &'a [u8])>;

/// Keys for the reservation tests: any distinct values, marked so a stray key is easy to spot.
const KEY_A: u64 = 0x5350_4954_0000_00a1;
const KEY_B: u64 = 0x5350_4954_0000_00b2;
const KEY_WRONG: u64 = 0x5350_4954_0000_00c3;

/// Fixed ISIDs for the reservation sessions. LIO keeps reservations across sessions and keys them by
/// ISID, so a fixed ISID lets a later run CLEAR what an earlier run registered. T=10b (random format).
const ISID_A: [u8; 6] = [0x80, 0x53, 0x50, 0x49, 0x54, 0x0a];
const ISID_B: [u8; 6] = [0x80, 0x53, 0x50, 0x49, 0x54, 0x0b];

const DIGEST: &str = "CRC32C,None";
const CHAP_OFFER: &str = "5,6,7,8";

/// Runs the three sections for every target. A target that cannot be logged in to fails each case
/// in its section, so the refusal shows up in the score.
/// `standby` is true for a target whose LUN the lab expects in the standby state: its data commands
/// are refused, so only the refusal and the port group commands are checked there.
pub(super) fn run(targets: &[(&Path, Creds, bool)], initiator: &str, cases: &mut Vec<Case>) {
    for (path, creds, standby) in targets {
        let name = path
            .target
            .rsplit(':')
            .next()
            .unwrap_or(&path.target)
            .to_string();
        let mut s = match open(path, initiator, *creds, super::session::random_isid()) {
            Ok(s) => s,
            Err(err) => {
                for section in ["spc", "unmap", "ats", "pr", "mpio"] {
                    cases.push(case(
                        section,
                        format!("{name} login"),
                        "login ok".into(),
                        format!("refused ({err})"),
                        false,
                    ));
                }
                continue;
            }
        };
        if !*standby {
            // Reservations belong to the LUN, not the session, so the LUN starts from nothing.
            match open(path, initiator, *creds, ISID_A) {
                Ok(mut a) => {
                    clear_reservations(&mut a, cases, &name);
                    let _ = a.logout();
                }
                Err(err) => cases.push(case(
                    "pr",
                    format!("{name} clean slate"),
                    "login ok".into(),
                    err.to_string(),
                    false,
                )),
            }
        }
        if *standby {
            standby_refused(&mut s, &name, cases);
        } else {
            spc_cases(&mut s, &name, cases);
            unmap_cases(&mut s, &name, cases);
            ats_cases(&mut s, &name, cases);
        }
        mpio_cases(&mut s, &name, cases);
        let _ = s.logout();
        if !*standby {
            pr_cases(path, initiator, *creds, &name, cases);
        }
    }
}

fn open(path: &Path, initiator: &str, creds: Creds, isid: [u8; 6]) -> Result<Session> {
    session::login_as(
        &path.portal,
        &path.target,
        initiator,
        isid,
        creds,
        DIGEST,
        CHAP_OFFER,
    )
}

/// One line that says what a SCSI answer was, for the expected and actual columns.
fn describe(outcome: &Outcome) -> String {
    match outcome.status {
        0 => "GOOD".into(),
        RESERVATION_CONFLICT => "RESERVATION CONFLICT".into(),
        CHECK_CONDITION => {
            let (key, asc) = sense_key_asc(&outcome.sense);
            format!("CHECK CONDITION key {key:#x} asc {asc:#04x}")
        }
        other => format!("status {other:#04x}"),
    }
}

/// The sense key and additional sense code of fixed-format sense data.
fn sense_key_asc(sense: &[u8]) -> (u8, u8) {
    (
        sense.get(2).copied().unwrap_or(0) & 0x0f,
        sense.get(12).copied().unwrap_or(0),
    )
}

/// Records one case: the expected description, the actual one, and whether the status matched.
fn outcome_case(
    cases: &mut Vec<Case>,
    section: &'static str,
    name: String,
    expected: &str,
    result: Result<Outcome>,
    passes: impl Fn(&Outcome) -> bool,
) -> Option<Outcome> {
    match result {
        Ok(outcome) => {
            let passed = passes(&outcome);
            cases.push(case(
                section,
                name,
                expected.into(),
                describe(&outcome),
                passed,
            ));
            Some(outcome)
        }
        Err(err) => {
            cases.push(case(section, name, expected.into(), err.to_string(), false));
            None
        }
    }
}

/// SPC-4 commands every active LUN answers: TEST UNIT READY, REQUEST SENSE, INQUIRY VPD 0x00, REPORT LUNS,
/// MODE SENSE(6), and the refusal of an unknown opcode (SPC-4 4.4, invalid command operation code).
fn spc_cases(s: &mut Session, name: &str, cases: &mut Vec<Case>) {
    outcome_case(
        cases,
        "spc",
        format!("{name} TEST UNIT READY"),
        "GOOD",
        s.command(&[0x00, 0, 0, 0, 0, 0], 0, None),
        |o| o.status == 0,
    );

    // SPC-4 6.33: fixed-format sense is 0x70 (current) or 0x72 (descriptor), and REQUEST SENSE returns it.
    outcome_case(
        cases,
        "spc",
        format!("{name} REQUEST SENSE"),
        "GOOD, response 0x70 or 0x72",
        s.command(&[0x03, 0, 0, 0, 18, 0], 18, None),
        |o| o.status == 0 && matches!(o.data.first(), Some(0x70 | 0x72)),
    );

    // SPC-4 7.8.6: VPD 0x00 lists the supported pages in ascending order. Every LUN has 0x00, 0x80 and 0x83.
    outcome_case(
        cases,
        "spc",
        format!("{name} INQUIRY VPD 0x00 supported pages"),
        "GOOD, sorted, has 0x80 and 0x83",
        vpd(s, 0x00, 255),
        |o| {
            let len = o.data.get(3).copied().unwrap_or(0) as usize;
            let pages = o.data.get(4..4 + len).unwrap_or(&[]);
            o.status == 0
                && pages.windows(2).all(|w| w[0] < w[1])
                && pages.contains(&0x80)
                && pages.contains(&0x83)
        },
    );

    // SPC-4 6.27: REPORT LUNS returns an 8-byte header, then 8 bytes per LUN. A single-LUN target lists LUN 0.
    outcome_case(
        cases,
        "spc",
        format!("{name} REPORT LUNS"),
        "GOOD, length multiple of 8, LUN 0 listed",
        s.command(&[0xa0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0], 256, None),
        |o| {
            let len = scsi::be32(&o.data, 0).unwrap_or(u32::MAX) as usize;
            o.status == 0 && len % 8 == 0 && len >= 8 && o.data.get(8..16) == Some(&[0u8; 8])
        },
    );

    // SPC-4 7.12: MODE SENSE(6) for all pages. The MODE DATA LENGTH byte excludes itself.
    outcome_case(
        cases,
        "spc",
        format!("{name} MODE SENSE(6) all pages"),
        "GOOD, mode data length consistent",
        s.command(&[0x1a, 0, 0x3f, 0, 255, 0], 255, None),
        |o| {
            o.status == 0
                && o.data
                    .first()
                    .is_some_and(|&l| l as usize + 1 <= 255 && l >= 3)
        },
    );

    // SPC-4 4.4: an opcode the device does not implement is refused with ILLEGAL REQUEST, invalid
    // command operation code (ASC 20h).
    outcome_case(
        cases,
        "spc",
        format!("{name} unknown opcode refused"),
        "CHECK CONDITION key 0x5 asc 0x20",
        s.command(&[0x7f, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 0, None),
        |o| o.status == CHECK_CONDITION && sense_key_asc(&o.sense) == (0x5, 0x20),
    );
}

/// SPC-4 and ALUA: a LUN in the standby state refuses data commands with NOT READY, ASC 04/0b.
/// Commands for the port groups still work, so SET TPG is checked on these targets too.
fn standby_refused(s: &mut Session, name: &str, cases: &mut Vec<Case>) {
    let (lba, block_len) = match last_block(s) {
        Ok(v) => v,
        Err(err) => {
            cases.push(case(
                "mpio",
                format!("{name} capacity"),
                "readable".into(),
                err.to_string(),
                false,
            ));
            return;
        }
    };
    let outcome = s.command(
        &super::scsi::block_cdb(0x28, lba.saturating_sub(1), 1),
        block_len as u32,
        None,
    );
    outcome_case(
        cases,
        "mpio",
        format!("{name} READ refused while standby"),
        "CHECK CONDITION key 0x2 asc 0x04",
        outcome,
        |o| o.status == CHECK_CONDITION && sense_key_asc(&o.sense) == (0x2, 0x04),
    );
}

fn vpd(s: &mut Session, page: u8, len: u16) -> Result<Outcome> {
    let l = len.to_be_bytes();
    s.command(&[0x12, 0x01, page, l[0], l[1], 0], len as u32, None)
}

/// Reads the last block of the LUN, which every test here uses.
fn last_block(s: &mut Session) -> Result<(u32, usize)> {
    let (last, block_len) = scsi::capacity(s)?;
    let lba = u32::try_from(last).map_err(|_| "LUN is larger than 32-bit LBAs")?;
    Ok((lba, block_len as usize))
}

fn read_block(s: &mut Session, lba: u32, len: usize) -> Result<Vec<u8>> {
    let o = s.command(&super::scsi::block_cdb(0x28, lba, 1), len as u32, None)?;
    scsi::check_status("READ(10)", o.status, &o.sense)?;
    Ok(o.data)
}

fn write_block(s: &mut Session, lba: u32, data: &[u8]) -> Result<()> {
    let o = s.command(&super::scsi::block_cdb(0x2a, lba, 1), 0, Some(data))?;
    scsi::check_status("WRITE(10)", o.status, &o.sense)
}

fn unmap_cases(s: &mut Session, name: &str, cases: &mut Vec<Case>) {
    let (lba, block_len) = match last_block(s) {
        Ok(v) => v,
        Err(err) => {
            cases.push(case(
                "unmap",
                format!("{name} capacity"),
                "readable".into(),
                err.to_string(),
                false,
            ));
            return;
        }
    };
    let target = lba.saturating_sub(1);
    let original = match read_block(s, target, block_len) {
        Ok(d) => d,
        Err(err) => {
            cases.push(case(
                "unmap",
                format!("{name} read original"),
                "GOOD".into(),
                err.to_string(),
                false,
            ));
            return;
        }
    };

    // SBC-3 Block Limits (0xb0): the unmap descriptor count is 0 when UNMAP is not supported.
    // Logical Block Provisioning (0xb2): LBPU (byte 5, bit 7) says whether unmapping is allowed.
    let limits = vpd(s, 0xb0, 64);
    let (max_descriptors, max_lba) = match &limits {
        Ok(o) if o.status == 0 && o.data.len() >= 28 => (
            u32::from_be_bytes(o.data[24..28].try_into().unwrap()),
            u32::from_be_bytes(o.data[20..24].try_into().unwrap()),
        ),
        _ => (0, 0),
    };
    let provisioning = vpd(s, 0xb2, 8);
    let lbpu = matches!(&provisioning, Ok(o) if o.status == 0 && o.data.len() > 5 && o.data[5] & 0x80 != 0);
    outcome_case(
        cases,
        "unmap",
        format!("{name} VPD 0xb0 block limits"),
        "GOOD, 28+ bytes",
        limits,
        |o| o.status == 0 && o.data.len() >= 28,
    );
    outcome_case(
        cases,
        "unmap",
        format!("{name} VPD 0xb2 provisioning"),
        "GOOD",
        provisioning,
        |o| o.status == 0,
    );
    let supported = max_descriptors > 0 && lbpu;
    let _ = max_lba;

    // SBC-3 5.25: UNMAP succeeds when advertised. When it is not, the target must refuse it with an
    // ILLEGAL REQUEST check condition (sense key 5), not complete it silently.
    let unmapped = outcome_case(
        cases,
        "unmap",
        format!("{name} UNMAP one block"),
        if supported {
            "GOOD"
        } else {
            "CHECK CONDITION key 0x5"
        },
        ops::unmap(s, target as u64, 1),
        |o| {
            if supported {
                o.status == 0
            } else {
                o.status == CHECK_CONDITION && sense_key_asc(&o.sense).0 == 0x5
            }
        },
    )
    .is_some_and(|o| o.status == 0);

    if unmapped {
        // Both zeros and the old data are legal after UNMAP when LBPRZ is not known. Anything else
        // is data the target invented.
        let after = read_block(s, target, block_len);
        let legal = matches!(&after, Ok(d) if d.iter().all(|&b| b == 0) || *d == original);
        cases.push(case(
            "unmap",
            format!("{name} read after UNMAP"),
            "zeros or the original data".into(),
            match &after {
                Ok(d) if d.iter().all(|&b| b == 0) => "zeros".into(),
                Ok(d) if *d == original => "original data".into(),
                Ok(_) => "other data".into(),
                Err(err) => err.to_string(),
            },
            legal,
        ));
    }
    let restored = write_block(s, target, &original);
    if let Err(err) = restored {
        cases.push(case(
            "unmap",
            format!("{name} restore block"),
            "written back".into(),
            err.to_string(),
            false,
        ));
    }
}

fn ats_cases(s: &mut Session, name: &str, cases: &mut Vec<Case>) {
    let (lba, block_len) = match last_block(s) {
        Ok(v) => v,
        Err(err) => {
            cases.push(case(
                "ats",
                format!("{name} capacity"),
                "readable".into(),
                err.to_string(),
                false,
            ));
            return;
        }
    };
    let target = lba.saturating_sub(1);
    let original = match read_block(s, target, block_len) {
        Ok(d) => d,
        Err(err) => {
            cases.push(case(
                "ats",
                format!("{name} read original"),
                "GOOD".into(),
                err.to_string(),
                false,
            ));
            return;
        }
    };
    let a = vec![0xa5u8; block_len];
    let b = vec![0x5au8; block_len];
    let c = vec![0x3cu8; block_len];
    if let Err(err) = write_block(s, target, &a) {
        cases.push(case(
            "ats",
            format!("{name} set up block A"),
            "GOOD".into(),
            err.to_string(),
            false,
        ));
        return;
    }

    // SBC-3 5.3: the compare matches, so the write runs and the target answers GOOD.
    let mut matching = a.clone();
    matching.extend_from_slice(&b);
    outcome_case(
        cases,
        "ats",
        format!("{name} COMPARE AND WRITE, compare matches"),
        "GOOD",
        ops::compare_and_write(s, target as u64, 1, &matching),
        |o| o.status == 0,
    );
    let now = read_block(s, target, block_len);
    cases.push(case(
        "ats",
        format!("{name} block holds the written data"),
        "B".into(),
        match &now {
            Ok(d) if *d == b => "B".into(),
            Ok(_) => "other".into(),
            Err(err) => err.to_string(),
        },
        matches!(&now, Ok(d) if *d == b),
    ));

    // SBC-3 5.3: the block is B now, so comparing with A miscompares. The target reports MISCOMPARE
    // (sense key 0xe, ASC 0x1d) and must not write C.
    let mut mismatching = a.clone();
    mismatching.extend_from_slice(&c);
    outcome_case(
        cases,
        "ats",
        format!("{name} COMPARE AND WRITE, compare fails"),
        "CHECK CONDITION key 0xe asc 0x1d",
        ops::compare_and_write(s, target as u64, 1, &mismatching),
        |o| o.status == CHECK_CONDITION && sense_key_asc(&o.sense) == (0xe, 0x1d),
    );
    let after = read_block(s, target, block_len);
    cases.push(case(
        "ats",
        format!("{name} failed compare writes nothing"),
        "B".into(),
        match &after {
            Ok(d) if *d == b => "B".into(),
            Ok(d) if *d == c => "C (written)".into(),
            Ok(_) => "other".into(),
            Err(err) => err.to_string(),
        },
        matches!(&after, Ok(d) if *d == b),
    ));

    let restored = write_block(s, target, &original);
    if let Err(err) = restored {
        cases.push(case(
            "ats",
            format!("{name} restore block"),
            "written back".into(),
            err.to_string(),
            false,
        ));
    }
}

/// SET TARGET PORT GROUPS (SPC-4 6.36). Setting a group to the state it is already in changes
/// nothing, so it is safe on a lab target. A reserved state value must be refused.
fn mpio_cases(s: &mut Session, name: &str, cases: &mut Vec<Case>) {
    let groups = match scsi::port_groups(s) {
        Ok(g) if !g.is_empty() => g,
        Ok(_) => {
            cases.push(case(
                "mpio",
                format!("{name} REPORT TPG"),
                "one or more groups".into(),
                "none".into(),
                false,
            ));
            return;
        }
        Err(err) => {
            cases.push(case(
                "mpio",
                format!("{name} REPORT TPG"),
                "GOOD".into(),
                err.to_string(),
                false,
            ));
            return;
        }
    };
    let group = &groups[0];
    if group.state == 0x0e {
        cases.push(case(
            "mpio",
            format!("{name} SET TPG no-op"),
            "skipped".into(),
            "group is transitioning".into(),
            true,
        ));
    } else {
        outcome_case(
            cases,
            "mpio",
            format!("{name} SET TPG group {} to its own state", group.id),
            "GOOD",
            ops::set_port_group_state(s, group.id, group.state),
            |o| o.status == 0,
        );
        let after = scsi::port_groups(s);
        cases.push(case(
            "mpio",
            format!("{name} state unchanged by no-op SET TPG"),
            scsi::state_name(group.state).into(),
            match &after {
                Ok(g) => g
                    .iter()
                    .find(|x| x.id == group.id)
                    .map_or("group missing".into(), |x| scsi::state_name(x.state).into()),
                Err(err) => err.to_string(),
            },
            matches!(&after, Ok(g) if g.iter().any(|x| x.id == group.id && x.state == group.state)),
        ));
    }
    outcome_case(
        cases,
        "mpio",
        format!("{name} SET TPG reserved state refused"),
        "CHECK CONDITION key 0x5",
        ops::set_port_group_state(s, group.id, 0x5),
        |o| o.status == CHECK_CONDITION && sense_key_asc(&o.sense).0 == 0x5,
    );
}

/// Clears every registration and reservation on the LUN, so a run that stopped part way cannot make
/// the next one fail. Registers KEY_A on this nexus first, because CLEAR needs a matching key.
/// A REGISTER from a nexus that is not registered must still succeed while another nexus holds the
/// reservation (SPC-4 REGISTER is not a conflicting service action); this step checks that too.
fn clear_reservations(s: &mut Session, cases: &mut Vec<Case>, name: &str) {
    let register =
        ops::persistent_reserve_out(s, ops::PR_REGISTER, 0, &ops::pr_parameters(0, KEY_A));
    let clear = ops::persistent_reserve_out(s, ops::PR_CLEAR, 0, &ops::pr_parameters(KEY_A, 0));
    let ok = |r: &Result<Outcome>| matches!(r, Ok(o) if o.status == 0);
    if !ok(&register) || !ok(&clear) {
        cases.push(case(
            "pr",
            format!("{name} clean slate"),
            "GOOD".into(),
            match (&register, &clear) {
                (Err(err), _) | (_, Err(err)) => err.to_string(),
                (Ok(r), _) if r.status != 0 => describe(r),
                (_, Ok(c)) => describe(c),
            },
            false,
        ));
    }
}

/// Reads the registered keys (READ KEYS) as a list.
fn read_keys(s: &mut Session) -> Result<Vec<u64>> {
    let o = ops::persistent_reserve_in(s, ops::PR_IN_READ_KEYS, 512)?;
    scsi::check_status("PERSISTENT RESERVE IN (read keys)", o.status, &o.sense)?;
    let len = scsi::be32(&o.data, 4).unwrap_or(0) as usize;
    Ok(o.data[8..(8 + len).min(o.data.len())]
        .chunks_exact(8)
        .map(|k| u64::from_be_bytes(k.try_into().unwrap()))
        .collect())
}

/// Reads the reservation holder (READ RESERVATIONS): the key and type, or None when unreserved.
fn read_holder(s: &mut Session) -> Result<Option<(u64, u8)>> {
    let o = ops::persistent_reserve_in(s, ops::PR_IN_READ_RESERVATIONS, 512)?;
    scsi::check_status(
        "PERSISTENT RESERVE IN (read reservations)",
        o.status,
        &o.sense,
    )?;
    let len = scsi::be32(&o.data, 4).unwrap_or(0);
    if len == 0 || o.data.len() < 22 {
        return Ok(None);
    }
    let key = u64::from_be_bytes(o.data[8..16].try_into().unwrap());
    Ok(Some((key, o.data[21] & 0x0f)))
}

/// Runs the reservation scenario with two initiator ports, A and B (two ISIDs, the same IQN).
fn pr_cases(path: &Path, initiator: &str, creds: Creds, name: &str, cases: &mut Vec<Case>) {
    let (mut a, mut b) = match (
        open(path, initiator, creds, ISID_A),
        open(path, initiator, creds, ISID_B),
    ) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(err), _) | (_, Err(err)) => {
            cases.push(case(
                "pr",
                format!("{name} two sessions"),
                "two logins ok".into(),
                err.to_string(),
                false,
            ));
            return;
        }
    };
    let (lba, block_len) = match last_block(&mut a) {
        Ok(v) => v,
        Err(err) => {
            cases.push(case(
                "pr",
                format!("{name} capacity"),
                "readable".into(),
                err.to_string(),
                false,
            ));
            return;
        }
    };
    let target = lba.saturating_sub(1);
    let block = vec![0x77u8; block_len];
    let original = read_block(&mut a, target, block_len).ok();

    // SPC-4 6.15.3: REPORT CAPABILITIES returns a fixed 8-byte answer.
    let caps = ops::persistent_reserve_in(&mut a, ops::PR_IN_REPORT_CAPABILITIES, 8);
    outcome_case(
        cases,
        "pr",
        format!("{name} REPORT CAPABILITIES length"),
        "GOOD, length 8",
        caps,
        |o| o.status == 0 && u16::from_be_bytes([o.data[0], o.data[1]]) == 8,
    );

    // Both ports register. READ KEYS from A must list both keys and nothing else.
    let reg_a =
        ops::persistent_reserve_out(&mut a, ops::PR_REGISTER, 0, &ops::pr_parameters(0, KEY_A));
    outcome_case(
        cases,
        "pr",
        format!("{name} A REGISTER"),
        "GOOD",
        reg_a,
        |o| o.status == 0,
    );
    let reg_b =
        ops::persistent_reserve_out(&mut b, ops::PR_REGISTER, 0, &ops::pr_parameters(0, KEY_B));
    outcome_case(
        cases,
        "pr",
        format!("{name} B REGISTER"),
        "GOOD",
        reg_b,
        |o| o.status == 0,
    );
    let keys = read_keys(&mut a);
    cases.push(case(
        "pr",
        format!("{name} READ KEYS lists A and B"),
        format!("{KEY_A:#x}, {KEY_B:#x}"),
        match &keys {
            Ok(k) => format!("{k:x?}"),
            Err(err) => err.to_string(),
        },
        matches!(&keys, Ok(k) if k.len() == 2 && k.contains(&KEY_A) && k.contains(&KEY_B)),
    ));

    // A takes write-exclusive. B sees A as holder, cannot write or reserve, and can still read.
    let reserve = ops::persistent_reserve_out(
        &mut a,
        ops::PR_RESERVE,
        ops::PR_WRITE_EXCLUSIVE,
        &ops::pr_parameters(KEY_A, 0),
    );
    outcome_case(
        cases,
        "pr",
        format!("{name} A RESERVE write-exclusive"),
        "GOOD",
        reserve,
        |o| o.status == 0,
    );
    let holder = read_holder(&mut b);
    cases.push(case(
        "pr",
        format!("{name} READ RESERVATIONS shows A"),
        format!("key {KEY_A:#x}, type 0x1"),
        match &holder {
            Ok(Some((k, t))) => format!("key {k:#x}, type {t:#x}"),
            Ok(None) => "unreserved".into(),
            Err(err) => err.to_string(),
        },
        matches!(&holder, Ok(Some((k, 1))) if *k == KEY_A),
    ));
    let write_b = s_write(&mut b, target, &block);
    outcome_case(
        cases,
        "pr",
        format!("{name} B WRITE while A holds it"),
        "RESERVATION CONFLICT",
        write_b,
        |o| o.status == RESERVATION_CONFLICT,
    );
    let read_b = read_block(&mut b, target, block_len);
    cases.push(case(
        "pr",
        format!("{name} B READ while A holds it (write-exclusive)"),
        "GOOD".into(),
        match &read_b {
            Ok(_) => "GOOD".into(),
            Err(err) => err.to_string(),
        },
        read_b.is_ok(),
    ));
    let reserve_b = ops::persistent_reserve_out(
        &mut b,
        ops::PR_RESERVE,
        ops::PR_WRITE_EXCLUSIVE,
        &ops::pr_parameters(KEY_B, 0),
    );
    outcome_case(
        cases,
        "pr",
        format!("{name} B RESERVE while A holds it"),
        "RESERVATION CONFLICT",
        reserve_b,
        |o| o.status == RESERVATION_CONFLICT,
    );

    // SPC-4 6.16.3: RELEASE from an I_T nexus that does not hold the reservation has no effect.
    let release_b = ops::persistent_reserve_out(
        &mut b,
        ops::PR_RELEASE,
        ops::PR_WRITE_EXCLUSIVE,
        &ops::pr_parameters(KEY_B, 0),
    );
    outcome_case(
        cases,
        "pr",
        format!("{name} B RELEASE by non-holder"),
        "GOOD",
        release_b,
        |o| o.status == 0,
    );
    let still = read_holder(&mut b);
    cases.push(case(
        "pr",
        format!("{name} A still holds after B's RELEASE"),
        format!("key {KEY_A:#x}"),
        match &still {
            Ok(Some((k, _))) => format!("key {k:#x}"),
            Ok(None) => "unreserved".into(),
            Err(err) => err.to_string(),
        },
        matches!(&still, Ok(Some((k, _))) if *k == KEY_A),
    ));

    // The holder writes; after A releases, B writes too.
    let write_a = s_write(&mut a, target, &block);
    outcome_case(
        cases,
        "pr",
        format!("{name} A WRITE as holder"),
        "GOOD",
        write_a,
        |o| o.status == 0,
    );
    let release_a = ops::persistent_reserve_out(
        &mut a,
        ops::PR_RELEASE,
        ops::PR_WRITE_EXCLUSIVE,
        &ops::pr_parameters(KEY_A, 0),
    );
    outcome_case(
        cases,
        "pr",
        format!("{name} A RELEASE"),
        "GOOD",
        release_a,
        |o| o.status == 0,
    );
    let write_b2 = s_write(&mut b, target, &block);
    outcome_case(
        cases,
        "pr",
        format!("{name} B WRITE after release"),
        "GOOD",
        write_b2,
        |o| o.status == 0,
    );

    // SPC-4 6.16.4: REGISTER with a reservation key that is not the registered one conflicts.
    let wrong = ops::persistent_reserve_out(
        &mut b,
        ops::PR_REGISTER,
        0,
        &ops::pr_parameters(KEY_WRONG, KEY_B),
    );
    outcome_case(
        cases,
        "pr",
        format!("{name} REGISTER with wrong reservation key"),
        "RESERVATION CONFLICT",
        wrong,
        |o| o.status == RESERVATION_CONFLICT,
    );

    // SPC-4 6.16.5: PREEMPT by B takes the reservation from A. Its key is the reservation key and A's
    // key is the service action reservation key.
    let _ = ops::persistent_reserve_out(&mut a, ops::PR_REGISTER, 0, &ops::pr_parameters(0, KEY_A));
    let reserve_a = ops::persistent_reserve_out(
        &mut a,
        ops::PR_RESERVE,
        ops::PR_WRITE_EXCLUSIVE,
        &ops::pr_parameters(KEY_A, 0),
    );
    outcome_case(
        cases,
        "pr",
        format!("{name} A RESERVE again"),
        "GOOD",
        reserve_a,
        |o| o.status == 0,
    );
    let preempt = ops::persistent_reserve_out(
        &mut b,
        ops::PR_PREEMPT,
        ops::PR_WRITE_EXCLUSIVE,
        &ops::pr_parameters(KEY_B, KEY_A),
    );
    outcome_case(
        cases,
        "pr",
        format!("{name} B PREEMPT A"),
        "GOOD",
        preempt,
        |o| o.status == 0,
    );
    // SPC-4: the nexus whose registration was preempted gets a unit attention (ASC 2A/05, registrations
    // preempted) on its next command. The preempter's own commands are not affected.
    let a_after = ops::persistent_reserve_in(&mut a, ops::PR_IN_READ_RESERVATIONS, 512);
    outcome_case(
        cases,
        "pr",
        format!("{name} A told its registration was preempted"),
        "CHECK CONDITION key 0x6 asc 0x2a",
        a_after,
        |o| o.status == CHECK_CONDITION && sense_key_asc(&o.sense) == (0x6, 0x2a),
    );
    let taken = read_holder(&mut b);
    cases.push(case(
        "pr",
        format!("{name} B holds after PREEMPT"),
        format!("key {KEY_B:#x}"),
        match &taken {
            Ok(Some((k, _))) => format!("key {k:#x}"),
            Ok(None) => "unreserved".into(),
            Err(err) => err.to_string(),
        },
        matches!(&taken, Ok(Some((k, _))) if *k == KEY_B),
    ));

    // Clean up: CLEAR removes every registration and reservation, so the next run starts from nothing.
    let clear =
        ops::persistent_reserve_out(&mut b, ops::PR_CLEAR, 0, &ops::pr_parameters(KEY_B, 0));
    outcome_case(cases, "pr", format!("{name} B CLEAR"), "GOOD", clear, |o| {
        o.status == 0
    });
    let empty = read_keys(&mut b);
    cases.push(case(
        "pr",
        format!("{name} no keys after CLEAR"),
        "none".into(),
        match &empty {
            Ok(k) => format!("{} keys", k.len()),
            Err(err) => err.to_string(),
        },
        matches!(&empty, Ok(k) if k.is_empty()),
    ));

    if let Some(data) = original {
        if let Err(err) = write_block(&mut a, target, &data) {
            cases.push(case(
                "pr",
                format!("{name} restore block"),
                "written back".into(),
                err.to_string(),
                false,
            ));
        }
    }
    let _ = a.logout();
    let _ = b.logout();
}

/// WRITE(10) of one block, returned as an outcome so the caller can judge the status.
fn s_write(s: &mut Session, lba: u32, data: &[u8]) -> Result<Outcome> {
    s.command(&super::scsi::block_cdb(0x2a, lba, 1), 0, Some(data))
}
