//! The `mpio` cases of `--compliance`: asymmetric logical unit access (SPC-6 5.18), which Windows
//! MSDSM, Linux dm-multipath (scsi_dh_alua) and VMware use to choose paths. The TPGS field of standard
//! INQUIRY must agree with REPORT TARGET PORT GROUPS; VPD 83h must name the answering port and its group
//! (relative target port and target port group designators) as RTPG does; each group's state must be
//! one its supported-state bits allow; the extended header format must list the same groups. SET TARGET
//! PORT GROUPS works only with explicit ALUA: a no-op on the answering port's group, a reserved state
//! refused, and a real transition of another group and back, which tells other nexuses ASYMMETRIC ACCESS
//! STATE CHANGED. Through a standby port, the commands SPC-6 5.18.2.5.4 lists must work and TEST UNIT
//! READY and READ must report the standby state.

use super::cases::{
    be16, be32, expect, expect_should, open, record, show_attentions, unit_attentions, vpd, Creds,
    Seen, Want, REFUSED, REFUSED_SA,
};
use super::disk::parse_designations;
use super::ops;
use super::report::Path;
use super::scsi;
use super::session::{self, Outcome, Session};
use crate::compliance::Case;
use crate::Result;

const SECTION: &str = "mpio";
const KEY_STANDBY: u64 = 0x5350_4954_0000_005b;

/// One target port group descriptor (SPC-6 table 272).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Group {
    pub id: u16,
    pub state: u8,
    pub supported: u8,
    pub ports: Vec<u16>,
}

/// REPORT TARGET PORT GROUPS, in the length only format or, with `extended`, asking for the extended
/// header (PARAMETER DATA FORMAT 001b, SPC-6 table 268).
fn rtpg(s: &mut Session, extended: bool) -> Result<Outcome> {
    let a = 4096u32.to_be_bytes();
    let cdb = [
        0xa3,
        if extended { 0x2a } else { 0x0a },
        0,
        0,
        0,
        0,
        a[0],
        a[1],
        a[2],
        a[3],
        0,
        0,
    ];
    s.command(&cdb, 4096, None)
}

/// The groups in RTPG data. The extended header (RTPG_FMT 001b in byte 4) is 8 bytes, the length only
/// header 4 (SPC-5 tables 271, 272).
pub(super) fn parse_groups(d: &[u8]) -> (bool, Vec<Group>) {
    let extended = d.get(4).is_some_and(|b| (b >> 4) & 0x7 == 1);
    let end = (4 + be32(d, 0) as usize).min(d.len());
    let mut i = if extended { 8 } else { 4 };
    let mut out = Vec::new();
    while i + 8 <= end {
        let count = d[i + 7] as usize;
        out.push(Group {
            id: be16(d, i + 2),
            state: d[i] & 0x0f,
            supported: d[i + 1],
            ports: (0..count).map(|p| be16(d, i + 8 + 4 * p + 2)).collect(),
        });
        i += 8 + 4 * count;
    }
    (extended, out)
}

/// The supported-state bit for a state (SPC-6 table 272): AO_SUP, AN_SUP, S_SUP, U_SUP, LBD_SUP, O_SUP,
/// T_SUP.
pub(super) fn state_bit(state: u8) -> u8 {
    match state {
        0x0 => 0x01,
        0x1 => 0x02,
        0x2 => 0x04,
        0x3 => 0x08,
        0x4 => 0x10,
        0xe => 0x40,
        0xf => 0x80,
        _ => 0,
    }
}

/// What the ALUA cases found, for the Windows cases.
pub(super) struct Findings {
    /// TPGS is set and REPORT TARGET PORT GROUPS and the VPD 83h port designators agree.
    pub usable: bool,
    pub tpgs: u8,
    pub own_state: Option<u8>,
}

/// The ALUA cases through an active port.
pub(super) fn run(
    s: &mut Session,
    path: &Path,
    initiator: &str,
    creds: Creds,
    name: &str,
    tpgs: u8,
    cases: &mut Vec<Case>,
    seen: &mut Seen,
) -> Findings {
    let label = |what: &str| format!("{name} {what}");
    let mut findings = Findings {
        usable: false,
        tpgs,
        own_state: None,
    };
    // SPC-6 6.34: REPORT TARGET PORT GROUPS is supported when TPGS is not 00b, and refused otherwise.
    let wants: Vec<Want> = if tpgs != 0 {
        vec![Want::Good]
    } else {
        REFUSED_SA.to_vec()
    };
    let answer = expect(
        cases,
        SECTION,
        label(&format!("REPORT TARGET PORT GROUPS (TPGS {tpgs:02b}b)")),
        &wants,
        rtpg(s, false),
    );
    seen.answer("REPORT TARGET PORT GROUPS", SECTION, &answer);
    let Some(o) = answer.1.filter(|o| o.status == 0) else {
        return findings;
    };
    let (_, groups) = parse_groups(&o.data);
    if groups.is_empty() {
        record(
            cases,
            SECTION,
            label("REPORT TARGET PORT GROUPS lists a group"),
            "one or more",
            "none",
            false,
        );
        return findings;
    }
    // SPC-6 7.7.6, 7.7.6.8: with ALUA, VPD 83h names the relative target port (type 4h) and its
    // target port group (type 5h), association 1 (the target port).
    let ids = vpd(s, 0x83, 4096)
        .ok()
        .filter(|o| o.status == 0)
        .map(|o| parse_designations(&o.data).0)
        .unwrap_or_default();
    let port = ids
        .iter()
        .find(|d| d.kind == 4 && d.association == 1)
        .map(|d| be16(&d.value, 2));
    let group = ids
        .iter()
        .find(|d| d.kind == 5 && d.association == 1)
        .map(|d| be16(&d.value, 2));
    let listed = match (port, group) {
        (Some(p), Some(g)) => groups
            .iter()
            .find(|x| x.id == g)
            .filter(|x| x.ports.contains(&p)),
        _ => None,
    };
    record(
        cases,
        SECTION,
        label("VPD 83h port and group agree with REPORT TARGET PORT GROUPS"),
        "the group lists the port",
        format!("port {port:?}, group {group:?}"),
        listed.is_some(),
    );
    findings.own_state = listed.map(|g| g.state);
    findings.usable = tpgs != 0 && listed.is_some();
    // SPC-6 6.34: each group's state is one of the states it says it supports.
    for g in &groups {
        record(
            cases,
            SECTION,
            label(&format!("group {} state is a supported state", g.id)),
            "its *_SUP bit set",
            format!(
                "{} with supported bits {:08b}b",
                scsi::state_name(g.state),
                g.supported
            ),
            g.supported & state_bit(g.state) != 0,
        );
    }
    // SPC-6 6.34: PARAMETER DATA FORMAT 001b asks for the extended header; the groups are the same.
    let ext = rtpg(s, true);
    let a = expect(
        cases,
        SECTION,
        label("REPORT TARGET PORT GROUPS, extended header"),
        &[Want::Good],
        ext,
    );
    if let Some(o) = a.1.filter(|o| o.status == 0) {
        let (extended, again) = parse_groups(&o.data);
        let same = again.iter().map(|g| (g.id, g.state)).collect::<Vec<_>>()
            == groups.iter().map(|g| (g.id, g.state)).collect::<Vec<_>>();
        record(
            cases,
            SECTION,
            label("extended header lists the same groups"),
            "RTPG_FMT 001b, same groups",
            format!(
                "RTPG_FMT {}, {}",
                if extended { "001b" } else { "000b" },
                if same {
                    "same groups"
                } else {
                    "different groups"
                }
            ),
            extended && same,
        );
    }
    let explicit = tpgs & 0x2 != 0;
    let own = listed.cloned().unwrap_or_else(|| groups[0].clone());
    if !explicit {
        // SPC-6 6.43: SET TARGET PORT GROUPS is for explicit ALUA; without it the command is refused.
        let a = expect(
            cases,
            SECTION,
            label("SET TARGET PORT GROUPS without explicit ALUA"),
            &[Want::Key(0x5)],
            ops::set_port_group_state(s, own.id, own.state),
        );
        seen.answer("SET TARGET PORT GROUPS", SECTION, &a);
        return findings;
    }
    if own.state == 0xf {
        record(
            cases,
            SECTION,
            label("SET TARGET PORT GROUPS no-op"),
            "skipped",
            "group is transitioning",
            true,
        );
    } else {
        // SPC-6 6.43: setting a group to its own state changes nothing.
        let a = expect(
            cases,
            SECTION,
            label(&format!(
                "SET TARGET PORT GROUPS group {} to its own state",
                own.id
            )),
            &[Want::Good],
            ops::set_port_group_state(s, own.id, own.state),
        );
        seen.answer("SET TARGET PORT GROUPS", SECTION, &a);
        let after = current_state(s, own.id);
        record(
            cases,
            SECTION,
            label("state unchanged by the no-op"),
            scsi::state_name(own.state),
            after.map_or("group missing", scsi::state_name),
            after == Some(own.state),
        );
    }
    // SPC-6 6.43: an unsupported state (5h is reserved) is an invalid field in the parameter list.
    let a = expect(
        cases,
        SECTION,
        label("SET TARGET PORT GROUPS to reserved state 5h refused"),
        &[Want::Ascq(0x5, 0x26, 0x00)],
        ops::set_port_group_state(s, own.id, 0x5),
    );
    seen.answer("SET TARGET PORT GROUPS", SECTION, &a);
    let _ = unit_attentions(s);
    transition(s, path, initiator, creds, name, &own, &groups, cases);
    findings
}

fn current_state(s: &mut Session, id: u16) -> Option<u8> {
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
fn transition(
    s: &mut Session,
    path: &Path,
    initiator: &str,
    creds: Creds,
    name: &str,
    own: &Group,
    groups: &[Group],
    cases: &mut Vec<Case>,
) {
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
    let Ok(mut watcher) = open(path, initiator, creds, session::random_isid()) else {
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

/// Through a port in the standby state: SPC-6 5.18.2.5.4 lists the commands that must still work
/// (here INQUIRY, REPORT LUNS, REPORT TARGET PORT GROUPS, MODE SENSE, REQUEST SENSE, LOG SENSE,
/// PERSISTENT RESERVE IN and OUT), and TEST UNIT READY and READ report LOGICAL UNIT NOT ACCESSIBLE,
/// TARGET PORT IN STANDBY STATE (04h/0Bh).
pub(super) fn standby(s: &mut Session, name: &str, cases: &mut Vec<Case>) {
    let label = |what: &str| format!("{name} standby: {what}");
    let _ = unit_attentions(s);
    let not_ready = [Want::Ascq(0x2, 0x04, 0x0b)];
    // SPC-6 5.18.2.5.4: other commands shall be terminated; TEST UNIT READY "should" be.
    expect_should(
        cases,
        SECTION,
        label("TEST UNIT READY reports the standby state"),
        &not_ready,
        s.command(&[0, 0, 0, 0, 0, 0], 0, None),
    );
    expect(
        cases,
        SECTION,
        label("READ (10) reports the standby state"),
        &not_ready,
        s.command(&scsi::block_cdb(0x28, 0, 1), 4096, None),
    );
    expect(
        cases,
        SECTION,
        label("INQUIRY"),
        &[Want::Good],
        s.command(&[0x12, 0, 0, 0, 96, 0], 96, None),
    );
    expect(
        cases,
        SECTION,
        label("REPORT LUNS"),
        &[Want::Good],
        s.command(&[0xa0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0], 256, None),
    );
    expect(
        cases,
        SECTION,
        label("REPORT TARGET PORT GROUPS"),
        &[Want::Good],
        rtpg(s, false),
    );
    expect(
        cases,
        SECTION,
        label("MODE SENSE (10)"),
        &[Want::Good],
        s.command(&[0x5a, 0x08, 0x08, 0, 0, 0, 0, 0x01, 0, 0], 256, None),
    );
    expect(
        cases,
        SECTION,
        label("REQUEST SENSE"),
        &[Want::Good],
        s.command(&[0x03, 0, 0, 0, 252, 0], 252, None),
    );
    let mut log = vec![Want::Good];
    log.extend(REFUSED);
    expect(
        cases,
        SECTION,
        label("LOG SENSE"),
        &log,
        s.command(&[0x4d, 0, 0x40, 0, 0, 0, 0, 0x01, 0, 0], 256, None),
    );
    expect(
        cases,
        SECTION,
        label("PERSISTENT RESERVE IN, READ KEYS"),
        &[Want::Good],
        ops::persistent_reserve_in(s, ops::PR_IN_READ_KEYS, 512),
    );
    // Windows clustering registers its key on every path, the standby ones too.
    let register = ops::persistent_reserve_out(
        s,
        ops::PR_REGISTER_AND_IGNORE,
        0,
        &ops::pr_parameters(0, KEY_STANDBY),
    );
    if expect(
        cases,
        SECTION,
        label("PERSISTENT RESERVE OUT, REGISTER AND IGNORE EXISTING KEY"),
        &[Want::Good],
        register,
    )
    .0
    {
        let unregister = ops::persistent_reserve_out(
            s,
            ops::PR_REGISTER_AND_IGNORE,
            0,
            &ops::pr_parameters(0, 0),
        );
        expect(
            cases,
            SECTION,
            label("PERSISTENT RESERVE OUT, unregister"),
            &[Want::Good],
            unregister,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(id: u16, state: u8, supported: u8, ports: &[u16]) -> Vec<u8> {
        let mut d = vec![
            state,
            supported,
            (id >> 8) as u8,
            id as u8,
            0,
            0,
            0,
            ports.len() as u8,
        ];
        for p in ports {
            d.extend_from_slice(&[0, 0, (p >> 8) as u8, *p as u8]);
        }
        d
    }

    #[test]
    fn groups_parse_in_both_header_formats() {
        let mut body = descriptor(0, 0x0, 0x0f, &[1]);
        body.extend(descriptor(1, 0x2, 0x0f, &[2, 3]));
        let mut short = ((body.len()) as u32).to_be_bytes().to_vec();
        short.extend_from_slice(&body);
        let (ext, groups) = parse_groups(&short);
        assert!(!ext);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[1].ports, vec![2, 3]);
        let mut long = ((body.len() + 4) as u32).to_be_bytes().to_vec();
        long.extend_from_slice(&[0x10, 5, 0, 0]);
        long.extend_from_slice(&body);
        let (ext, again) = parse_groups(&long);
        assert!(ext);
        assert_eq!(again, groups);
    }

    #[test]
    fn supported_state_bits_follow_table_273() {
        assert_eq!(state_bit(0x0), 0x01);
        assert_eq!(state_bit(0x2), 0x04);
        assert_eq!(state_bit(0xe), 0x40, "offline is O_SUP");
        assert_eq!(state_bit(0xf), 0x80, "transitioning is T_SUP");
        assert_eq!(state_bit(0x5), 0);
    }
}
