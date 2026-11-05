//! The `mpio` cases of `--compliance`: asymmetric logical unit access (SPC-6 5.18), which Windows
//! MSDSM, Linux dm-multipath (scsi_dh_alua) and VMware use to choose paths. The TPGS field of standard
//! INQUIRY must agree with REPORT TARGET PORT GROUPS; VPD 83h must name the answering port and its group
//! (relative target port and target port group designators) as RTPG does; each group's state must be
//! one its supported-state bits allow; the extended header format must list the same groups. SET TARGET
//! PORT GROUPS works only with explicit ALUA: a no-op on the answering port's group, a reserved state
//! refused, and a real transition of another group and back, which tells other nexuses ASYMMETRIC ACCESS
//! STATE CHANGED. Through a standby port, the commands SPC-6 5.18.2.5.4 lists must work and TEST UNIT
//! READY and READ must report the standby state.

use super::alua_transition::{current_state, transition};
use super::cases::{expect, record, Want, REFUSED_SA};
use super::lun_access::{unit_attentions, vpd, Creds};
use super::ops;
use super::path::Path;
use super::port_groups::{parse_groups, rtpg, state_bit};
use super::scsi;
use super::scsi_fields::be16;
use super::seen::Seen;
use super::session::Session;
use super::vpd_pages::parse_designations;
use crate::compliance::Case;

pub(super) const SECTION: &str = "mpio";

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
