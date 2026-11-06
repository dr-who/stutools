//! The explicit ALUA transition case of the `mpio` section (SPC-6 5.18.2.6, 6.43): SET TARGET PORT
//! GROUPS moves a group that does not hold the answering port to another state it supports, every other
//! I_T nexus is told ASYMMETRIC ACCESS STATE CHANGED (2Ah/06h), REPORT TARGET PORT GROUPS shows the new
//! state, and the group is moved back to where it was.

use super::alua::SECTION;
use super::cases::{expect, record, Want};
use super::login;
use super::lun_access::{open, show_attentions, unit_attentions, Lun};
use super::ops;
use super::port::Port;
use super::port_groups::{parse_groups, rtpg, state_bit, Group};
use super::scsi;
use crate::compliance::Case;

pub(super) fn current_state(s: &mut Port, id: u16) -> Option<u8> {
    let o = rtpg(s, false).ok().filter(|o| o.status == 0)?;
    parse_groups(&o.data)
        .1
        .into_iter()
        .find(|g| g.id == id)
        .map(|g| g.state)
}

/// A real explicit transition of a group that does not hold the answering port, to another supported
/// primary state and back. SPC-6 5.18.2.6: every other I_T nexus is told ASYMMETRIC ACCESS STATE
/// CHANGED (2Ah/06h); the nexus that sent SET TARGET PORT GROUPS is not.
pub(super) fn transition(
    s: &mut Port,
    lun: Lun,
    name: &str,
    own: &Group,
    groups: &[Group],
    cases: &mut Vec<Case>,
) {
    let Lun {
        path,
        initiator,
        creds,
    } = lun;
    let label = |what: &str| format!("{name} {what}");
    let Some((g, to)) = groups.iter().filter(|g| g.id != own.id).find_map(|g| {
        let target = match g.state {
            0x2 => [0x1, 0x0]
                .into_iter()
                .find(|&t| g.supported & state_bit(t) != 0),
            0x0 | 0x1 => (g.supported & state_bit(0x2) != 0).then_some(0x2),
            _ => None,
        };
        target.map(|t| (g, t))
    }) else {
        record(
            cases,
            SECTION,
            label("explicit transition of another group"),
            "a second group to move",
            "none to move",
            true,
        );
        return;
    };
    let Ok(mut watcher) = open(path, initiator, creds, login::random_isid()) else {
        record(
            cases,
            SECTION,
            label("a second nexus to watch the transition"),
            "login ok",
            "refused",
            false,
        );
        return;
    };
    let _ = unit_attentions(&mut watcher);
    let a = expect(
        cases,
        SECTION,
        label(&format!(
            "SET TARGET PORT GROUPS group {} from {} to {}",
            g.id,
            scsi::state_name(g.state),
            scsi::state_name(to)
        )),
        &[Want::Good],
        ops::set_port_group_state(s, g.id, to),
    );
    if a.0 {
        let now = current_state(s, g.id);
        record(
            cases,
            SECTION,
            label("REPORT TARGET PORT GROUPS shows the new state"),
            scsi::state_name(to),
            now.map_or("missing", scsi::state_name),
            now == Some(to),
        );
        let seen_by_watcher = unit_attentions(&mut watcher);
        record(
            cases,
            SECTION,
            label("the other nexus is told ASYMMETRIC ACCESS STATE CHANGED"),
            "2Ah/06h",
            show_attentions(&seen_by_watcher),
            seen_by_watcher.contains(&(0x2a, 0x06)),
        );
        let mine = unit_attentions(s);
        record(
            cases,
            SECTION,
            label("the nexus that sent it is not told"),
            "no 2Ah/06h",
            show_attentions(&mine),
            !mine.contains(&(0x2a, 0x06)),
        );
        let back = expect(
            cases,
            SECTION,
            label(&format!(
                "SET TARGET PORT GROUPS group {} back to {}",
                g.id,
                scsi::state_name(g.state)
            )),
            &[Want::Good],
            ops::set_port_group_state(s, g.id, g.state),
        );
        let restored = current_state(s, g.id);
        record(
            cases,
            SECTION,
            label("group back in its first state"),
            scsi::state_name(g.state),
            restored.map_or("missing", scsi::state_name),
            back.0 && restored == Some(g.state),
        );
    }
    let _ = unit_attentions(s);
    let _ = watcher.logout();
}
