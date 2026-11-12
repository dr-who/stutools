//! The MODE SELECT cases of the `disk` section that change a parameter, as Windows does when "Enable
//! write caching on the device" is changed and Linux does through sysfs `cache_type` (SPC-6 6.12;
//! SPC-5 6.12 for MODE SELECT (6), which SPC-6 made obsolete; SBC-5 6.5.6): MODE SELECT (6) and (10)
//! with only a header; when WCE of the Caching page is changeable, MODE SELECT (10) flips it, MODE
//! SENSE reads it back, and MODE SELECT (6) flips it back; every other I_T nexus is told MODE
//! PARAMETERS CHANGED (2Ah/01h) and the nexus that sent it is not, unless the Mode Page Policy VPD
//! page says the page is per I_T nexus (SPC-6 7.5.3, 7.7.9); a non-changeable bit, a WCE that is not
//! changeable, and a wrong PAGE LENGTH are each refused with INVALID FIELD IN PARAMETER LIST and change
//! nothing; and when the page is saveable, a MODE SELECT with SP changes the saved values. The page's
//! current and saved values are written back at the end, whatever happened before.

use super::cases::{describe, expect, expect_should, record, record_info, record_should, Want};
use super::disk::{label, Disk, SECTION};
use super::local_nexus;
use super::lun_access::{open, show_attentions, unit_attentions, Lun};
use super::port::Port;
use super::pr_ports::ISID_A2;
use super::write_cache::{caching_page, select, wce, with_wce, CHANGEABLE, CURRENT, SAVED, WCE};
use crate::compliance::Case;

/// MODE PARAMETERS CHANGED (SPC-6 table 61).
const CHANGED: (u8, u8) = (0x2a, 0x01);

/// The mode page policy of the Caching page from the Mode Page Policy VPD page (SPC-6 table 562):
/// its own descriptor, or the first descriptor's when that one covers every page (3Fh/FFh).
pub(super) fn caching_policy(page_87: &[u8]) -> Option<u8> {
    let end = (4 + super::scsi_fields::be16(page_87, 2) as usize).min(page_87.len());
    let descriptors: Vec<&[u8]> = page_87.get(4..end)?.chunks_exact(4).collect();
    descriptors
        .iter()
        .find(|d| d[0] & 0x3f == 0x08 && d[1] == 0)
        .or_else(|| {
            descriptors
                .first()
                .filter(|d| d[0] & 0x3f == 0x3f && d[1] == 0xff)
        })
        .map(|d| d[2] & 0x03)
}

/// The first bit of `page` past its header that the changeable mask `mask` marks not changeable, as
/// (byte, bit mask).
pub(super) fn fixed_bit(page: &[u8], mask: &[u8]) -> Option<(usize, u8)> {
    (2..page.len().min(mask.len())).find_map(|i| {
        (0..8)
            .map(|b| 1u8 << b)
            .find(|m| mask[i] & m == 0)
            .map(|m| (i, m))
    })
}

/// The other nexuses that watch for the unit attention: B, when there is a second initiator name, and
/// A's second port.
fn watchers(
    lun: Lun,
    second: Option<&str>,
    name: &str,
    cases: &mut Vec<Case>,
) -> Vec<(&'static str, Port)> {
    let mut out = Vec::new();
    let mut wanted = vec![(lun.initiator, ISID_A2, "A's second port")];
    // A local disk with one initiator port has no second nexus: its "second port" would be the very nexus
    // that sends MODE SELECT, which the device rightly does not tell. Only a second nexus can be judged.
    if lun.path.is_local() && local_nexus::initiator_ports(&lun.path.target).map_or(true, |n| n < 2)
    {
        wanted.remove(0);
        record_info(
            cases,
            SECTION,
            label(name, "SPC-6 6.12 A's second port is told MODE PARAMETERS CHANGED"),
            "not run: the local disk has one initiator port, so there is no second I_T nexus to tell",
        );
    }
    if let Some(b) = second {
        wanted.insert(0, (b, super::pr_ports::ISID_B, "B"));
    }
    for (who, isid, label) in wanted {
        if let Ok(mut p) = open(lun.path, who, lun.creds, isid) {
            let _ = unit_attentions(&mut p);
            out.push((label, p));
        }
    }
    out
}

pub(super) fn run(
    s: &mut Port,
    lun: Lun,
    second: Option<&str>,
    name: &str,
    disk: &Disk,
    cases: &mut Vec<Case>,
) {
    let at = |clause: &str, what: &str| label(name, &format!("{clause} {what}"));
    // SPC-6 6.12, SPC-5 6.12: a parameter list of only a header (no block descriptors, no pages) sets
    // nothing and is not an error.
    let header10 = [0u8; 8];
    expect(
        cases,
        SECTION,
        at("SPC-6 6.12", "MODE SELECT (10) with only a header"),
        &[Want::Good],
        s.command(&[0x55, 0x10, 0, 0, 0, 0, 0, 0, 8, 0], 0, Some(&header10)),
    );
    let header6 = [0u8; 4];
    expect(
        cases,
        SECTION,
        at("SPC-5 6.12", "MODE SELECT (6) with only a header"),
        &[Want::Good, Want::Ascq(0x5, 0x20, 0x00)],
        s.command(&[0x15, 0x10, 0, 0, 4, 0], 0, Some(&header6)),
    );
    let (Ok(Some(current)), Ok(Some(mask))) =
        (caching_page(s, CURRENT), caching_page(s, CHANGEABLE))
    else {
        return;
    };
    let saved = caching_page(s, SAVED).ok().flatten();
    change_wce(s, lun, second, name, disk, &current, &mask, cases);
    refusals(s, name, &current, &mask, cases);
    if let Some(saved) = &saved {
        if current[0] & 0x80 != 0 && mask.get(2).is_some_and(|m| m & WCE != 0) {
            save_wce(s, name, saved, cases);
        }
    } else {
        // SPC-6 6.12: SP set when the logical unit has no saved pages is an invalid field in the CDB.
        expect(
            cases,
            SECTION,
            at(
                "SPC-6 6.12",
                "MODE SELECT (10) with SP when saved values are not supported",
            ),
            &[Want::Ascq(0x5, 0x24, 0x00)],
            select(s, &current, true, true),
        );
    }
    // Whatever happened above: the saved values, then the current ones, as they were.
    if let Some(saved) = &saved {
        if current[0] & 0x80 != 0 {
            let _ = select(s, saved, true, true);
        }
    }
    let _ = select(s, &current, true, false);
    let now = caching_page(s, CURRENT).ok().flatten();
    let then = caching_page(s, SAVED).ok().flatten();
    let restored = now.as_deref() == Some(&current[..]) && then == saved;
    record(
        cases,
        SECTION,
        at(
            "SBC-5 6.5.6",
            "Caching page current and saved values written back",
        ),
        "as before",
        if restored { "as before" } else { "different" },
        restored,
    );
}

/// Flips WCE with MODE SELECT (10), reads it back, flips it back with MODE SELECT (6), and judges the
/// unit attentions; or, when WCE is not changeable, judges the refusal.
#[allow(clippy::too_many_arguments)]
fn change_wce(
    s: &mut Port,
    lun: Lun,
    second: Option<&str>,
    name: &str,
    disk: &Disk,
    current: &[u8],
    mask: &[u8],
    cases: &mut Vec<Case>,
) {
    let at = |clause: &str, what: &str| label(name, &format!("{clause} {what}"));
    let on = wce(current);
    let flipped = with_wce(current, !on);
    let to = if on { "clears" } else { "sets" };
    if mask.get(2).is_none_or(|m| m & WCE == 0) {
        // SPC-6 6.12 a): a field reported as not changeable set to another value is refused, and nothing
        // changes.
        expect(
            cases,
            SECTION,
            at(
                "SPC-6 6.12",
                "MODE SELECT (10) of a WCE that is not changeable refused",
            ),
            &[Want::Ascq(0x5, 0x26, 0x00)],
            select(s, &flipped, true, false),
        );
        return;
    }
    let mut others = watchers(lun, second, name, cases);
    let _ = unit_attentions(s);
    let (good, _) = expect(
        cases,
        SECTION,
        at("SPC-6 6.12", &format!("MODE SELECT (10) {to} WCE")),
        &[Want::Good],
        select(s, &flipped, true, false),
    );
    // SPC-6 6.13.2: the current values are those the last MODE SELECT set.
    let after = caching_page(s, CURRENT).ok().flatten();
    record(
        cases,
        SECTION,
        at(
            "SPC-6 6.13",
            &format!("MODE SENSE (10) after MODE SELECT (10) {to} WCE"),
        ),
        format!("WCE {}", u8::from(!on)),
        after.as_deref().map_or("no page".to_string(), |p| {
            format!("WCE {}", u8::from(wce(p)))
        }),
        after.as_deref().is_some_and(|p| wce(p) != on),
    );
    if good {
        attentions(s, &mut others, name, disk, "MODE SELECT (10)", cases);
    }
    // Back with MODE SELECT (6), when the device has it.
    let back = select(s, current, false, false);
    let (_, answer) = expect(
        cases,
        SECTION,
        at(
            "SPC-5 6.12",
            &format!(
                "MODE SELECT (6) {} WCE again",
                if on { "sets" } else { "clears" }
            ),
        ),
        &[Want::Good, Want::Ascq(0x5, 0x20, 0x00)],
        back,
    );
    let six = answer.is_some_and(|o| o.status == 0);
    if six {
        let six_page = s
            .command(&[0x1a, 0x08, 0x08, 0, 255, 0], 255, None)
            .ok()
            .filter(|o| o.status == 0)
            .and_then(|o| super::mode_page_cases::mode_page_of(&o.data, false).map(<[u8]>::to_vec));
        record(
            cases,
            SECTION,
            at("SPC-5 6.14", "MODE SENSE (6) after MODE SELECT (6)"),
            format!("WCE {}", u8::from(on)),
            six_page.as_deref().map_or("no page".to_string(), |p| {
                format!("WCE {}", u8::from(wce(p)))
            }),
            six_page.as_deref().is_some_and(|p| wce(p) == on),
        );
    } else {
        let _ = select(s, current, true, false);
    }
    // The flip back told them again; a later section's nexus with the same ISID starts clean.
    for (_, mut p) in others {
        let _ = unit_attentions(&mut p);
        let _ = p.logout();
    }
}

/// Judges the unit attentions a MODE SELECT that changed a parameter raised: MODE PARAMETERS CHANGED on
/// every other nexus, once, and not on the one that sent it, for a page shared by the nexuses or kept
/// per target port; none for a page kept per I_T nexus (SPC-6 6.12, 7.5.3).
fn attentions(
    s: &mut Port,
    others: &mut [(&'static str, Port)],
    name: &str,
    disk: &Disk,
    sent: &str,
    cases: &mut Vec<Case>,
) {
    let policy = disk.page(0x87).and_then(caching_policy);
    let per_nexus = policy == Some(0b11);
    let policy_text = match policy {
        Some(0) => "shared",
        Some(1) => "per target port",
        Some(3) => "per I_T nexus",
        Some(_) => "obsolete",
        None => "not reported, so taken as shared",
    };
    for (who, p) in others.iter_mut() {
        let seen = unit_attentions(p);
        let n = seen.iter().filter(|&&a| a == CHANGED).count();
        let what = label(
            name,
            &format!("SPC-6 6.12 {who} is told MODE PARAMETERS CHANGED after {sent}"),
        );
        let expected = if per_nexus {
            "no 2Ah/01h (the Caching page is per I_T nexus)".to_string()
        } else {
            format!("2Ah/01h once (Caching page policy {policy_text})")
        };
        let passed = if per_nexus { n == 0 } else { n == 1 };
        // Without the Mode Page Policy VPD page the policy is a guess.
        if policy.is_some() {
            record(
                cases,
                SECTION,
                what,
                expected,
                show_attentions(&seen),
                passed,
            );
        } else {
            record_should(
                cases,
                SECTION,
                what,
                expected,
                show_attentions(&seen),
                passed,
            );
        }
    }
    let own = unit_attentions(s);
    record(
        cases,
        SECTION,
        label(
            name,
            &format!("SPC-6 6.12 the nexus that sent {sent} is not told"),
        ),
        "no 2Ah/01h",
        show_attentions(&own),
        !own.contains(&CHANGED),
    );
}

/// The refusals that change nothing (SPC-6 6.12 a) and c)): a bit the changeable mask marks fixed, set
/// to another value; and the page sent with a PAGE LENGTH other than the one MODE SENSE returned.
fn refusals(s: &mut Port, name: &str, current: &[u8], mask: &[u8], cases: &mut Vec<Case>) {
    let at = |what: &str| label(name, &format!("SPC-6 6.12 {what}"));
    if let Some((byte, bit)) = fixed_bit(current, mask) {
        let mut page = current.to_vec();
        page[byte] ^= bit;
        expect(
            cases,
            SECTION,
            at(&format!(
                "MODE SELECT (10) of a non-changeable Caching page bit (byte {byte}, {bit:02x}h) refused"
            )),
            &[Want::Ascq(0x5, 0x26, 0x00)],
            select(s, &page, true, false),
        );
        unchanged(s, name, current, "the non-changeable bit", cases);
    }
    if current.len() > 3 {
        let mut page = current[..current.len() - 1].to_vec();
        page[1] -= 1;
        expect(
            cases,
            SECTION,
            at("MODE SELECT (10) of the Caching page one byte short refused"),
            &[Want::Ascq(0x5, 0x26, 0x00)],
            select(s, &page, true, false),
        );
        unchanged(s, name, current, "the short page", cases);
    }
}

fn unchanged(s: &mut Port, name: &str, current: &[u8], what: &str, cases: &mut Vec<Case>) {
    let now = caching_page(s, CURRENT).ok().flatten();
    record(
        cases,
        SECTION,
        label(
            name,
            &format!("SPC-6 6.12 Caching page unchanged by {what}"),
        ),
        "unchanged",
        if now.as_deref() == Some(current) {
            "unchanged"
        } else {
            "changed"
        },
        now.as_deref() == Some(current),
    );
}

/// SPC-6 6.12, 6.13.5: a MODE SELECT with SP saves the page, so the saved values change.
fn save_wce(s: &mut Port, name: &str, saved: &[u8], cases: &mut Vec<Case>) {
    let at = |what: &str| label(name, &format!("SPC-6 6.12 {what}"));
    let flipped = with_wce(saved, !wce(saved));
    let (good, _) = expect_should(
        cases,
        SECTION,
        at("MODE SELECT (10) with SP flips the saved WCE"),
        &[Want::Good],
        select(s, &flipped, true, true),
    );
    if !good {
        return;
    }
    let now = caching_page(s, SAVED).ok().flatten();
    record(
        cases,
        SECTION,
        at("MODE SENSE (10) saved values follow MODE SELECT with SP"),
        format!("saved WCE {}", u8::from(!wce(saved))),
        now.as_deref().map_or("no page".to_string(), |p| {
            format!("saved WCE {}", u8::from(wce(p)))
        }),
        now.as_deref().is_some_and(|p| wce(p) != wce(saved)),
    );
    let o = select(s, saved, true, true);
    if let Ok(o) = &o {
        if o.status != 0 {
            record(
                cases,
                SECTION,
                at("saved Caching page written back"),
                "GOOD",
                describe(o),
                false,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_caching_policy_is_its_own_or_the_default_descriptor() {
        let shared_default = [0, 0x87, 0, 8, 0x3f, 0xff, 0x00, 0, 0x08, 0x00, 0x03, 0];
        assert_eq!(caching_policy(&shared_default), Some(3));
        let only_default = [0, 0x87, 0, 4, 0x3f, 0xff, 0x01, 0];
        assert_eq!(caching_policy(&only_default), Some(1));
        let other = [0, 0x87, 0, 4, 0x0a, 0x00, 0x03, 0];
        assert_eq!(caching_policy(&other), None);
    }

    #[test]
    fn a_fixed_bit_is_the_first_the_mask_clears() {
        let page = [0x88, 0x12, 0x04, 0x00];
        let mask = [0x08, 0x12, 0xff, 0xf7];
        assert_eq!(fixed_bit(&page, &mask), Some((3, 0x08)));
        assert_eq!(fixed_bit(&page, &[0x08, 0x12, 0xff, 0xff]), None);
    }
}
