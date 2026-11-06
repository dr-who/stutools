//! SPC-2 reservations across two I_T nexuses, as Windows Server 2003 clusters and VMware ESXi's
//! fallback on LUNs without ATS use them (SPC-2 RESERVE (6), RELEASE (6), RESERVE (10), RELEASE (10);
//! obsolete since SPC-3 apart from SPC-6 5.14.3), in the `sbc` section beside the RESERVE (6) row, and
//! the reservation broken by LOGICAL UNIT RESET in the `tmf` section. With A holding: B's RESERVE (6),
//! READ (10), WRITE (10) and COMPARE AND WRITE get RESERVATION CONFLICT, B's INQUIRY, REPORT LUNS and
//! REQUEST SENSE are answered, B's TEST UNIT READY may be either, and B's RELEASE (6) is GOOD and leaves
//! A holding. After A releases, B reserves and A conflicts. Then B's LOGICAL UNIT RESET frees A's
//! reservation (B reads, and reserves) and tells A of the reset (29h). RESERVE (10) and RELEASE (10)
//! work the same, or are refused as unsupported. It runs only on a LUN with no persistent reservation
//! registrations, which make RESERVE (6) conflict (SPC-6 5.14.3), and releases everything it reserved.

use super::cases::{describe, expect, record, Want};
use super::lun_access::{open, read_cmd, show_attentions, unit_attentions, write_cmd, Lun};
use super::ops::{self, RELEASE_6, RESERVE_6};
use super::port::Port;
use super::pr_commands::read_keys;
use super::pr_ports::ISID_SIBLING;
use crate::compliance::Case;

/// ISIDs of their own, apart from the persistent reservation cases' ports.
const ISID_HOLDER: [u8; 6] = [0x80, 0x52, 0x32, 0x00, 0x00, 0x01];
const ISID_OTHER: [u8; 6] = [0x80, 0x52, 0x32, 0x00, 0x00, 0x02];

const RESERVE_10: [u8; 10] = [0x56, 0, 0, 0, 0, 0, 0, 0, 0, 0];
const RELEASE_10: [u8; 10] = [0x57, 0, 0, 0, 0, 0, 0, 0, 0, 0];

/// The two nexuses: A as the first initiator name; B as the second, or A's name on the disk's other
/// port when there is no second name.
fn nexuses(lun: Lun, second: Option<&str>) -> Option<(Port, Port)> {
    let mut a = open(lun.path, lun.initiator, lun.creds, ISID_HOLDER).ok()?;
    let mut b = match second {
        Some(name) => open(lun.path, name, lun.creds, ISID_OTHER).ok()?,
        None => open(lun.path, lun.initiator, lun.creds, ISID_SIBLING).ok()?,
    };
    let _ = unit_attentions(&mut a);
    let _ = unit_attentions(&mut b);
    Some((a, b))
}

pub(super) fn run(
    lun: Lun,
    second: Option<&str>,
    name: &str,
    lba: u32,
    len: usize,
    compare_and_write: bool,
    cases: &mut Vec<Case>,
) {
    let sbc = |what: &str| format!("{name} SPC-2 {what}");
    let Some((mut a, mut b)) = nexuses(lun, second) else {
        record(
            cases,
            "sbc",
            sbc("RESERVE (6): two nexuses"),
            "both open",
            "refused",
            false,
        );
        return;
    };
    match read_keys(&mut a) {
        Ok((_, keys)) if keys.is_empty() => {}
        other => {
            super::cases::record_info(
                cases,
                "sbc",
                sbc("RESERVE (6) across two nexuses"),
                match other {
                    Ok((_, keys)) => format!(
                        "not run: {} persistent reservation keys registered",
                        keys.len()
                    ),
                    Err(e) => format!("not run: READ KEYS failed ({e})"),
                },
            );
            let _ = (a.logout(), b.logout());
            return;
        }
    }
    let Ok(original) = super::lun_access::read_block(&mut a, lba, len) else {
        return;
    };
    let (_, o) = expect(
        cases,
        "sbc",
        sbc("A RESERVE (6)"),
        &[Want::Good, Want::Ascq(0x5, 0x20, 0x00)],
        a.command(&RESERVE_6, 0, None),
    );
    if o.is_none_or(|o| o.status != 0) {
        let _ = (a.logout(), b.logout());
        return;
    }
    let held = |what: &str| sbc(&format!("B {what} while A holds RESERVE (6)"));
    expect(
        cases,
        "sbc",
        held("RESERVE (6)"),
        &[Want::Conflict],
        b.command(&RESERVE_6, 0, None),
    );
    expect(
        cases,
        "sbc",
        held("READ (10)"),
        &[Want::Conflict],
        read_cmd(&mut b, lba, len),
    );
    expect(
        cases,
        "sbc",
        held("WRITE (10)"),
        &[Want::Conflict],
        write_cmd(&mut b, lba, &original),
    );
    if compare_and_write {
        let mut both = original.clone();
        both.extend_from_slice(&original);
        expect(
            cases,
            "sbc",
            held("COMPARE AND WRITE"),
            &[Want::Conflict],
            ops::compare_and_write(&mut b, u64::from(lba), 1, &both),
        );
    }
    expect(
        cases,
        "sbc",
        held("TEST UNIT READY"),
        &[Want::Good, Want::Conflict],
        b.command(&[0, 0, 0, 0, 0, 0], 0, None),
    );
    expect(
        cases,
        "sbc",
        held("INQUIRY"),
        &[Want::Good],
        b.command(&[0x12, 0, 0, 0, 36, 0], 36, None),
    );
    expect(
        cases,
        "sbc",
        held("REPORT LUNS"),
        &[Want::Good],
        b.command(&[0xa0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0], 256, None),
    );
    expect(
        cases,
        "sbc",
        held("REQUEST SENSE"),
        &[Want::Good],
        b.command(&[0x03, 0, 0, 0, 252, 0], 252, None),
    );
    // A RELEASE from a nexus that holds nothing is GOOD and releases nothing.
    expect(
        cases,
        "sbc",
        held("RELEASE (6)"),
        &[Want::Good],
        b.command(&RELEASE_6, 0, None),
    );
    expect(
        cases,
        "sbc",
        sbc("A still holds after B's RELEASE (6): B READ (10)"),
        &[Want::Conflict],
        read_cmd(&mut b, lba, len),
    );
    expect(
        cases,
        "sbc",
        sbc("A RELEASE (6)"),
        &[Want::Good],
        a.command(&RELEASE_6, 0, None),
    );
    expect(
        cases,
        "sbc",
        sbc("B RESERVE (6) after A's RELEASE (6)"),
        &[Want::Good],
        b.command(&RESERVE_6, 0, None),
    );
    expect(
        cases,
        "sbc",
        sbc("A RESERVE (6) while B holds"),
        &[Want::Conflict],
        a.command(&RESERVE_6, 0, None),
    );
    expect(
        cases,
        "sbc",
        sbc("A READ (10) while B holds"),
        &[Want::Conflict],
        read_cmd(&mut a, lba, len),
    );
    expect(
        cases,
        "sbc",
        sbc("B RELEASE (6)"),
        &[Want::Good],
        b.command(&RELEASE_6, 0, None),
    );

    // LOGICAL UNIT RESET frees an SPC-2 reservation (SPC-2, SAM-3) and tells the other nexuses (SAM-6
    // 6.3.3: a unit attention, 29h).
    let tmf = |what: &str| format!("{name} SPC-2 {what}");
    let reserved = a.command(&RESERVE_6, 0, None).is_ok_and(|o| o.status == 0);
    if reserved {
        let reset = b.logical_unit_reset();
        record(
            cases,
            "tmf",
            tmf("B LOGICAL UNIT RESET while A holds RESERVE (6)"),
            "function complete",
            reset
                .as_ref()
                .map_or_else(|e| e.clone(), |()| "function complete".to_string()),
            reset.is_ok(),
        );
        if reset.is_ok() {
            let _ = unit_attentions(&mut b);
            expect(
                cases,
                "tmf",
                tmf("RESERVE (6) released by LOGICAL UNIT RESET: B READ (10)"),
                &[Want::Good],
                read_cmd(&mut b, lba, len),
            );
            expect(
                cases,
                "tmf",
                tmf("B RESERVE (6) after the reset"),
                &[Want::Good],
                b.command(&RESERVE_6, 0, None),
            );
            let _ = b.command(&RELEASE_6, 0, None);
            let seen = unit_attentions(&mut a);
            record(
                cases,
                "tmf",
                tmf("A, which held RESERVE (6), is told of the LOGICAL UNIT RESET"),
                "a unit attention 29h/xx",
                show_attentions(&seen),
                seen.iter().any(|&(asc, _)| asc == 0x29),
            );
        }
    }
    let _ = a.command(&RELEASE_6, 0, None);

    // RESERVE (10) and RELEASE (10): the same function, or refused as unsupported.
    let (_, o) = expect(
        cases,
        "sbc",
        sbc("A RESERVE (10)"),
        &[Want::Good, Want::Ascq(0x5, 0x20, 0x00)],
        a.command(&RESERVE_10, 0, None),
    );
    if o.is_some_and(|o| o.status == 0) {
        expect(
            cases,
            "sbc",
            sbc("B READ (10) while A holds RESERVE (10)"),
            &[Want::Conflict],
            read_cmd(&mut b, lba, len),
        );
        expect(
            cases,
            "sbc",
            sbc("A RELEASE (10)"),
            &[Want::Good],
            a.command(&RELEASE_10, 0, None),
        );
        let o = read_cmd(&mut b, lba, len);
        record(
            cases,
            "sbc",
            sbc("B READ (10) after A's RELEASE (10)"),
            "GOOD",
            o.as_ref().map_or_else(|e| e.clone(), describe),
            o.is_ok_and(|o| o.status == 0),
        );
    }
    let _ = a.command(&RELEASE_10, 0, None);
    let _ = (a.logout(), b.logout());
}
