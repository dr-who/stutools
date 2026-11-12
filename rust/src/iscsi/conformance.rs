//! The driver of the SCSI sections of `--compliance`: for each target it runs, in order, the `spc`
//! (with REPORT LUNS and the LUNs it does not list), `disk` (with the Linux scan replay, the INQUIRY and
//! VPD framing and the MODE SELECT that changes the write cache), `io` (with VERIFY, READ and WRITE (6),
//! the 1000-command runs and the 12 MiB cache runs), `unmap`, `ats`, `odx`, `mpio` (with the Windows
//! online replay), `pr`, `tmf`, `sbc` (with SPC-2 reservations across two nexuses), `features` and
//! `windows` sections, each in its own module, sharing what they find through `Seen` and the `Disk` the
//! `disk` section reads, and ends with the START STOP UNIT cases, which stop a spinning disk. A target
//! that cannot be logged in to fails one case in each section. A LUN reached through a standby port
//! runs only the standby cases of `mpio`. Every data-changing case writes the original data back.

use super::cases::{no_failures, record};
use super::login;
use super::lun_access::{open, short_name, unit_attentions, Creds, Lun};
use super::path::Path;
use super::seen::Seen;
use super::{
    alua, ats_race, block_commands, block_io, bulk_integrity, cache_integrity, cdb_lengths,
    coherence, compare_and_write, data_path, disk, feature_sets, header_digest, host_traces,
    inquiry_framing, mode_block_descriptors, mode_select_change, mode_select_pages, odx,
    optional_commands, pr_command_set, pr_empty_state, pr_ports, pr_single_nexus, provisioning,
    report_luns_cases, reservations, six_byte_io, spc2_reservations, spc_commands, standby_cases,
    start_stop_cases, task_management, unlisted_lun, verify_cases, vpd_framing, windows,
    windows_device_rules,
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
        let luns = report_luns_cases::run(&mut s, &name, t.path.lun == 0, cases);
        let disk = disk::run(&mut s, &name, levels, cases, &mut seen);
        unlisted_lun::run(lun, &name, &luns, disk.block_len, cases);
        let same_lun: Vec<&Path> = targets
            .iter()
            .filter(|o| !o.standby && o.path.target == t.path.target && o.path.lun == t.path.lun)
            .map(|o| o.path)
            .collect();
        let ports = inquiry_framing::target_ports(&same_lun, initiator, t.creds);
        inquiry_framing::run(&mut s, &name, &disk, ports, cases);
        vpd_framing::run(&mut s, &name, &disk, cases);
        mode_select_change::run(&mut s, lun, second, &name, &disk, cases);
        mode_select_pages::run(&mut s, &name, cases);
        mode_block_descriptors::run(&mut s, &name, &disk, cases);
        cdb_lengths::run(&mut s, &name, &disk, cases);
        optional_commands::run(&mut s, &name, cases);
        host_traces::linux(&mut s, &name, &disk, cases);
        let io = block_io::run(&mut s, &name, &disk, cases, &mut seen);
        verify_cases::run(&mut s, &name, &disk, cases);
        let (reads6, writes6) = six_byte_io::run(&mut s, &name, &disk, cases);
        bulk_integrity::run(&mut s, &name, &disk, reads6 && writes6, cases);
        cache_integrity::run(&mut s, &name, &disk, cases);
        coherence::run(t.path, initiator, second, t.creds, &name, cases);
        data_path::run(t.path, initiator, second, t.creds, &name, &disk, cases);
        let thin = provisioning::run(&mut s, &name, &disk, cases, &mut seen);
        compare_and_write::run(&mut s, &name, &disk, cases, &mut seen);
        if seen
            .probe("COMPARE AND WRITE")
            .is_some_and(|p| p.supported && p.passed)
        {
            ats_race::run(t.path, initiator, second, t.creds, &name, &disk, cases);
        }
        let first = cases.len();
        let odx = odx::run(&mut s, &name, &disk, cases, &mut seen);
        let odx_clean = no_failures(&cases[first..]);
        let alua = alua::run(&mut s, lun, &name, disk.tpgs(), cases, &mut seen);
        let windows_online = host_traces::windows(&mut s, &name, &disk, cases);
        pr_empty_state::run(&mut s, &name, cases);
        pr_single_nexus::run(&mut s, &name, cases);
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
        let compare_and_write = seen.probe("COMPARE AND WRITE").is_some_and(|p| p.supported);
        if let Ok(lba) = u32::try_from(disk.last_lba.saturating_sub(1)) {
            spc2_reservations::run(
                lun,
                second,
                &name,
                lba,
                disk.block_len as usize,
                compare_and_write,
                cases,
            );
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
        windows_device_rules::run(&name, &disk, &luns, !t.path.is_local(), cases);
        // Last: a STOP spins a disk down.
        match open(t.path, initiator, t.creds, login::random_isid()) {
            Ok(mut s) => {
                start_stop_cases::run(&mut s, &name, disk.tpgs(), disk.block_len, cases);
                let _ = s.logout();
            }
            Err(err) => {
                record(
                    cases,
                    "disk",
                    format!("{name} login for START STOP UNIT"),
                    "login ok",
                    err,
                    false,
                );
            }
        }
        // After everything else: a target that closes the connection on a variable length CDB loses only
        // this session.
        match open(t.path, initiator, t.creds, login::random_isid()) {
            Ok(mut s) => {
                cdb_lengths::run_variable_length(&mut s, &name, &disk, cases);
                let _ = s.logout();
            }
            Err(err) => {
                record(
                    cases,
                    "disk",
                    format!("{name} login for READ (32) and WRITE (32)"),
                    "login ok",
                    err,
                    false,
                );
            }
        }
    }
}
