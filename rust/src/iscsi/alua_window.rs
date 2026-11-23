//! The transition window of an explicit ALUA transition (SPC-4 5.15.2.5): while SET TARGET PORT GROUPS
//! moves a group, a second I_T nexus on a port of that group keeps sending READ (10). Each answer is
//! recorded by its status and sense. The standard allows only these during the transition: GOOD (the
//! command completed before or after it), CHECK CONDITION NOT READY, LOGICAL UNIT NOT ACCESSIBLE,
//! ASYMMETRIC ACCESS STATE TRANSITION (2/04/0ah), CHECK CONDITION NOT READY, LOGICAL UNIT NOT ACCESSIBLE,
//! TARGET PORT IN STANDBY STATE (2/04/0bh, the state before it), BUSY, and the unit attention ASYMMETRIC ACCESS
//! STATE CHANGED (6/2ah/06h) that the other nexus is owed. Anything else fails the case.
//! Whether the window was seen at all is reported, since a target that changes state at once shows none.

use super::login;
use super::lun_access::{Creds, open, vpd};
use super::path::Path;
use super::port::Port;
use super::port_groups::{parse_groups, rtpg};
use super::scsi_fields::be16;
use super::vpd_pages::parse_designations;
use std::sync::atomic::{AtomicBool, Ordering};

/// What the reader saw while the transition ran.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Window {
    pub answers: Vec<String>,
    pub transition_answers: usize,
    pub unexpected: Vec<String>,
}

/// One answer as text: GOOD, BUSY, or the status and sense key/ASC/ASCQ.
pub(super) fn classify(out: &crate::Result<super::session::Outcome>) -> String {
    match out {
        Ok(o) if o.status == 0 => "GOOD".into(),
        Ok(o) if o.status == 0x08 => "BUSY".into(),
        Ok(o) if o.status == 0x02 && o.sense.len() >= 14 => format!(
            "CHECK CONDITION {:x}/{:02x}/{:02x}",
            o.sense[2] & 0x0f,
            o.sense[12],
            o.sense[13]
        ),
        Ok(o) => format!("status {:02x}", o.status),
        Err(e) => format!("no answer: {e}"),
    }
}

/// Whether an answer is one the standard allows while a transition runs (or before and after it).
pub(super) fn allowed(answer: &str) -> bool {
    matches!(
        answer,
        "GOOD"
            | "BUSY"
            | "CHECK CONDITION 2/04/0a"
            | "CHECK CONDITION 2/04/0b"
            // SPC-4 5.15.2.5: the other I_T nexus is told of an explicit change by a unit attention.
            | "CHECK CONDITION 6/2a/06"
    )
}

/// The relative target port id a session's own portal answers as, from VPD 83h (type 4h, association 1).
fn port_of(s: &mut Port) -> Option<u16> {
    let o = vpd(s, 0x83, 4096).ok().filter(|o| o.status == 0)?;
    parse_designations(&o.data)
        .0
        .iter()
        .find(|d| d.kind == 4 && d.association == 1)
        .map(|d| be16(&d.value, 2))
}

/// A path among `others` whose own port belongs to target port group `group`, so a READ through it
/// is answered by that group's state.
pub(super) fn path_in_group<'a>(
    others: &[&'a Path],
    initiator: &str,
    creds: Creds,
    group: u16,
) -> Option<&'a Path> {
    others.iter().copied().find(|p| {
        let Ok(mut s) = open(p, initiator, creds, login::random_isid()) else {
            return false;
        };
        let member = port_of(&mut s).and_then(|port| {
            let o = rtpg(&mut s, false).ok().filter(|o| o.status == 0)?;
            parse_groups(&o.data)
                .1
                .into_iter()
                .find(|g| g.ports.contains(&port))
                .map(|g| g.id)
        });
        let _ = s.logout();
        member == Some(group)
    })
}

/// Runs `set` while a second nexus on `path` reads LBA 0 without pause, and returns what `set` returned
/// and the reader's answers. The reader starts, and answers once, before `set` runs, and keeps reading
/// until it has read for a while after `set` returned, so the window is covered on both sides.
pub(super) fn during<R>(
    path: &Path,
    initiator: &str,
    creds: Creds,
    set: impl FnOnce() -> R,
) -> (R, Window) {
    let stop = AtomicBool::new(false);
    let ready = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let reader = scope.spawn(|| {
            let mut w = Window::default();
            let Ok(mut s) = open(path, initiator, creds, login::random_isid()) else {
                w.unexpected.push("reader could not log in".into());
                ready.store(true, Ordering::Release);
                return w;
            };
            let mut after = 0usize;
            loop {
                let a = classify(&s.command(&super::scsi::block_cdb(0x28, 0, 1), 4096, None));
                if a.starts_with("CHECK CONDITION 2/04/0a") || a == "BUSY" {
                    w.transition_answers += 1;
                }
                if !allowed(&a) && w.unexpected.len() < 5 {
                    w.unexpected.push(a.clone());
                }
                if w.answers.last() != Some(&a) {
                    w.answers.push(a);
                }
                ready.store(true, Ordering::Release);
                if stop.load(Ordering::Acquire) {
                    after += 1;
                    if after >= 200 {
                        break;
                    }
                }
            }
            let _ = s.logout();
            w
        });
        while !ready.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
        let out = set();
        stop.store(true, Ordering::Release);
        let w = reader.join().unwrap_or_default();
        (out, w)
    })
}
