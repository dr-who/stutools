//! The START STOP UNIT cases of the `disk` section, in the four forms Windows sends at power
//! transitions and shutdown (SBC-5 5.31, 4.20.4): STOP and START, each with IMMED 0 and 1, POWER
//! CONDITION 0h and LOEJ 0. After a STOP, TEST UNIT READY and a READ (10) are refused as LOGICAL UNIT
//! NOT READY, INITIALIZING COMMAND REQUIRED (2/04/02) while the logical unit is stopped; after START
//! with IMMED 0 it is ready at once, and after START with IMMED 1 it becomes ready (TEST UNIT READY is
//! polled for up to 60 s). The run always ends with START, IMMED 0, so the medium is ready for what
//! follows. Microsoft also requires that the target port groups do not change (no path failover);
//! that is judged in the `windows` section. A START STOP UNIT refused as unsupported ends the cases.
//! On a spinning disk the STOPs spin it down, so this runs last.

use super::block_io::cdb10;
use super::cases::{describe, expect, record, record_should, Want, REFUSED};
use super::disk::{label, SECTION};
use super::lun_access::{test_unit_ready, unit_attentions};
use super::port::Port;
use super::scsi;
use crate::compliance::Case;
use std::time::{Duration, Instant};

/// How long a START with IMMED may take to make the logical unit ready.
const READY_WITHIN: Duration = Duration::from_secs(60);

/// LOGICAL UNIT NOT READY, INITIALIZING COMMAND REQUIRED (SBC-5 4.20.4).
const STOPPED: Want = Want::Ascq(0x2, 0x04, 0x02);

fn cdb(immed: bool, start: bool) -> [u8; 6] {
    [0x1b, u8::from(immed), 0, 0, u8::from(start), 0]
}

fn form(immed: bool, start: bool) -> String {
    format!(
        "START STOP UNIT, {} (IMMED {})",
        if start { "START" } else { "STOP" },
        u8::from(immed)
    )
}

pub(super) fn run(s: &mut Port, name: &str, tpgs: u8, block_len: u32, cases: &mut Vec<Case>) {
    let at = |what: &str| label(name, &format!("SBC-5 5.31 {what}"));
    let _ = unit_attentions(s);
    let groups = (tpgs != 0).then(|| scsi::port_groups(s).ok()).flatten();
    let mut wants = vec![Want::Good];
    wants.extend(REFUSED);
    for (immed, start) in [(false, false), (false, true), (true, false), (true, true)] {
        let (_, o) = expect(
            cases,
            SECTION,
            at(&form(immed, start)),
            &wants,
            s.command(&cdb(immed, start), 0, None),
        );
        match o {
            Some(o) if o.status == 0 => {}
            _ => break,
        }
        if !start {
            // SBC-5 4.20.4: stopped (or stopping, with IMMED), media access needs a START.
            let what = format!("after the STOP, IMMED {}", u8::from(immed));
            expect(
                cases,
                SECTION,
                label(name, &format!("SBC-5 4.20.4 TEST UNIT READY {what}")),
                &[STOPPED],
                test_unit_ready(s),
            );
            expect(
                cases,
                SECTION,
                label(name, &format!("SBC-5 4.20.4 READ (10) {what}")),
                &[STOPPED],
                s.command(&cdb10(0x28, 0, 0, 1), block_len, None),
            );
        } else if !immed {
            // SBC-5 5.31: with IMMED 0 the status comes after the logical unit is started.
            expect(
                cases,
                SECTION,
                at("TEST UNIT READY at once after START (IMMED 0)"),
                &[Want::Good],
                test_unit_ready(s),
            );
        } else {
            let began = Instant::now();
            let mut last = String::new();
            let ready = loop {
                match test_unit_ready(s) {
                    Ok(o) if o.status == 0 => break true,
                    Ok(o) => last = describe(&o),
                    Err(e) => last = e,
                }
                if began.elapsed() > READY_WITHIN {
                    break false;
                }
                std::thread::sleep(Duration::from_millis(200));
            };
            record(
                cases,
                SECTION,
                at("TEST UNIT READY after START (IMMED 1) becomes GOOD"),
                format!("GOOD within {} s", READY_WITHIN.as_secs()),
                if ready {
                    format!("GOOD after {:.1} s", began.elapsed().as_secs_f64())
                } else {
                    last
                },
                ready,
            );
        }
    }
    // Ready for what follows, whatever happened above.
    let _ = s.command(&cdb(false, true), 0, None);
    let _ = unit_attentions(s);
    expect(
        cases,
        SECTION,
        at("READ (10) after the last START"),
        &[Want::Good],
        s.command(&cdb10(0x28, 0, 0, 1), block_len, None),
    );
    if let Some(before) = groups {
        let after = scsi::port_groups(s).ok();
        record_should(
            cases,
            "windows",
            label(
                name,
                "WHCP START STOP UNIT changes no target port group (no path failover)",
            ),
            "the same groups and states",
            if after.as_ref() == Some(&before) {
                "the same"
            } else {
                "changed"
            },
            after.as_ref() == Some(&before),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_four_forms_set_immed_and_start() {
        assert_eq!(cdb(false, false), [0x1b, 0, 0, 0, 0, 0]);
        assert_eq!(cdb(true, true), [0x1b, 1, 0, 0, 1, 0]);
        assert_eq!(form(true, false), "START STOP UNIT, STOP (IMMED 1)");
    }
}
