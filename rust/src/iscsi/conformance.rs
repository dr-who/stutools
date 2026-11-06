//! The driver of the SCSI sections of `--compliance`: for each target it runs, in order, the `spc`,
//! `disk` (with the Linux scan replay), `io`, `unmap`, `ats`, `odx`, `mpio` (with the Windows online
//! replay), `pr`, `tmf`, `sbc`, `features` and `windows` sections, each in its own module, sharing what
//! they find through `Seen` and the `Disk` the `disk` section reads. A target that cannot be logged in
//! to fails one case in each section. A LUN reached through a standby port runs only the standby cases
//! of `mpio`. Every data-changing case writes the original data back.

use super::cases::{no_failures, record};
use super::login;
use super::lun_access::{open, short_name, unit_attentions, Creds, Lun};
use super::path::Path;
use super::seen::Seen;
use super::{
    alua, block_commands, block_io, coherence, compare_and_write, disk, feature_sets,
    header_digest, host_traces, odx, pr_command_set, pr_ports, provisioning, reservations,
    spc_commands, standby_cases, task_management, windows,
};
use crate::compliance::{Case, ScsiLevel};

const SECTIONS: [&str; 13] = [
    "spc",
    "disk",
    "io",
    "coherence",
    "unmap",
    "ats",
    "odx",
    "mpio",
    "pr",
    "tmf",
    "sbc",
    "features",
    "windows",
];

/// One target to test: its path, credentials, whether the path is standby, and the iSCSIProtocolLevel
/// it negotiated.
pub(super) struct Target<'a> {
    pub path: &'a Path,
    pub creds: Creds<'a>,
    pub standby: bool,
    pub protocol_level: Option<u32>,
}

pub(super) fn run(
    targets: &[Target],
    initiator: &str,
    second: Option<&str>,
    levels: &[ScsiLevel],
    cases: &mut Vec<Case>,
) {
    for t in targets {
        let name = short_name(t.path);
        let lun = Lun {
            path: t.path,
            initiator,
            creds: t.creds,
        };
        // Before this target's session opens, so the unit attentions a CLEAR raises are not its first
        // answers.
        if !t.standby {
            pr_ports::clean_slate(t.path, initiator, t.creds, &name, cases);
        }
        let mut s = match open(t.path, initiator, t.creds, login::random_isid()) {
            Ok(s) => s,
            Err(err) => {
                for section in SECTIONS {
                    record(
                        cases,
                        section,
                        format!("{name} login"),
                        "login ok",
                        format!("refused ({err})"),
                        false,
                    );
                }
                continue;
            }
        };
        let _ = unit_attentions(&mut s);
        if t.standby {
            standby_cases::run(&mut s, &name, cases);
            let _ = s.logout();
            continue;
        }
        let mut seen = Seen::default();
        spc_commands::run(&mut s, &name, cases, &mut seen);
        let disk = disk::run(&mut s, &name, levels, cases, &mut seen);
        host_traces::linux(&mut s, &name, &disk, cases);
        let io = block_io::run(&mut s, &name, &disk, cases, &mut seen);
        coherence::run(t.path, initiator, second, t.creds, &name, cases);
        let thin = provisioning::run(&mut s, &name, &disk, cases, &mut seen);
        compare_and_write::run(&mut s, &name, &disk, cases, &mut seen);
        let first = cases.len();
        let odx = odx::run(&mut s, &name, &disk, cases, &mut seen);
        let odx_clean = no_failures(&cases[first..]);
        let alua = alua::run(&mut s, lun, &name, disk.tpgs(), cases, &mut seen);
        let windows_online = host_traces::windows(&mut s, &name, &disk, cases);
        // The reservation and reset cases use sessions of their own; this one would sit idle meanwhile.
        let _ = s.logout();
        let first = cases.len();
        let pr = reservations::run(t.path, initiator, second, t.creds, &name, cases);
        pr_command_set::run(t.path, initiator, second, t.creds, &name, cases);
        let pr_ok = no_failures(&cases[first..]);
        seen.note("PERSISTENT RESERVE IN", "pr", true, pr_ok, "see pr");
        seen.note("PERSISTENT RESERVE OUT", "pr", true, pr_ok, "see pr");
        // Task management and header digests are iSCSI's; a local disk has neither here.
        if !t.path.is_local() {
            task_management::run(
                t.path,
                initiator,
                t.creds,
                &name,
                t.protocol_level,
                cases,
                &mut seen,
            );
            header_digest::run(t.path, initiator, t.creds, &name, disk.block_len, cases);
        }
        match open(t.path, initiator, t.creds, login::random_isid()) {
            Ok(mut s) => {
                let _ = unit_attentions(&mut s);
                block_commands::run(&mut s, lun, &name, &disk, levels, &mut seen, cases);
                let _ = s.logout();
            }
            Err(err) => {
                record(
                    cases,
                    "sbc",
                    format!("{name} login"),
                    "login ok",
                    err,
                    false,
                );
            }
        }
        feature_sets::run(&name, &disk, &seen, cases);
        let mode_sense_6 = seen
            .probe("MODE SENSE (6)")
            .is_some_and(|p| p.supported && p.passed);
        windows::run(
            &name,
            &windows::Facts {
                unique_id: disk.unique_id,
                enumeration: windows_online,
                cluster: pr.cluster,
                flush: io.flush,
                alua,
                trim: thin.trim,
                provisioning_clean: thin.clean,
                odx,
                odx_clean,
                write_cache: disk.write_cache(),
                dpofua: disk.dpofua(),
                mode_sense_6,
            },
            cases,
        );
    }
}
