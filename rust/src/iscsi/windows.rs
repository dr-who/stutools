//! The iSCSI `windows` cases of `--compliance`: what Windows needs of a disk, from what the other
//! sections found. Required: a unique LUN ID (MPIO groups paths by it and Failover Clustering names
//! disks by it), every enumeration command answering (disk.sys), the persistent reservation commands
//! clustering sends, and SYNCHRONIZE CACHE plus FUA writes (NTFS orders its log with them). Reported,
//! and passing when supported or cleanly refused: ALUA for MSDSM, TRIM (UNMAP), ODX and the write cache.
//! PR, ALUA, TRIM and ODX are not in the Windows Server 2022 trace (that host had no MPIO, cluster or
//! TRIM); the trace's own commands are replayed in `host_traces`.

use super::cases::record;
use super::scsi;
use crate::compliance::Case;

const SECTION: &str = "windows";

/// What the other sections found on one LUN.
pub(super) struct Facts {
    pub unique_id: bool,
    pub enumeration: bool,
    /// None: the reservation cases could not be run (see pr).
    pub cluster: Option<bool>,
    pub flush: bool,
    pub alua: super::alua::Findings,
    pub trim: bool,
    pub provisioning_clean: bool,
    pub odx: bool,
    pub odx_clean: bool,
    pub write_cache: Option<bool>,
    pub dpofua: bool,
    pub mode_sense_6: bool,
}

/// What Windows Failover Clustering needs of the persistent reservation commands.
const CLUSTERING: &str = "REGISTER AND IGNORE, RESERVE and PREEMPT type 5, READ KEYS, \
    READ RESERVATION, RELEASE, CLEAR, kept across reinstatement";

pub(super) fn run(name: &str, f: &Facts, cases: &mut Vec<Case>) {
    let label = |what: &str| format!("{name} {what}");
    record(
        cases,
        SECTION,
        label("MPIO and Failover Clustering: unique LUN ID in VPD 83h"),
        "NAA, EUI-64 or SCSI name, association 0",
        if f.unique_id { "present" } else { "absent" },
        f.unique_id,
    );
    record(
        cases,
        SECTION,
        label("disk.sys enumeration: the Windows Server 2022 online commands answer"),
        "every replayed command GOOD",
        if f.enumeration {
            "all GOOD"
        } else {
            "a replayed command failed (see windows above)"
        },
        f.enumeration,
    );
    // Windows sends MODE SENSE (6) only (the 2022 trace); a target that refuses it relies on Windows
    // falling back to MODE SENSE (10).
    record(
        cases,
        SECTION,
        label("MODE SENSE (6), the only form Windows sends"),
        "answers",
        if f.mode_sense_6 {
            "answers"
        } else {
            "refused or wrong"
        },
        f.mode_sense_6,
    );
    let clustering = label("Failover Clustering: SCSI-3 persistent reservations");
    match f.cluster {
        Some(all) => {
            let actual = if all {
                "all behave"
            } else {
                "a clustering case failed (see pr)"
            };
            record(cases, SECTION, clustering, CLUSTERING, actual, all);
        }
        None => super::cases::record_info(
            cases,
            SECTION,
            clustering,
            "not tested (see pr): needs a second initiator name",
        ),
    }
    record(
        cases,
        SECTION,
        label("NTFS: SYNCHRONIZE CACHE and FUA writes"),
        "both work",
        if f.flush {
            "both work"
        } else {
            "one failed (see io)"
        },
        f.flush,
    );
    let alua = match (f.alua.tpgs, f.alua.usable) {
        (0, _) => "not offered (MSDSM uses Failover Only)".to_string(),
        (t, true) => format!(
            "TPGS {t:02b}b, this path {}",
            f.alua.own_state.map_or("unknown", scsi::state_name)
        ),
        (t, false) => format!("TPGS {t:02b}b but REPORT TARGET PORT GROUPS and VPD 83h disagree"),
    };
    record(
        cases,
        SECTION,
        label("MSDSM: ALUA"),
        "offered and consistent, or not offered",
        alua,
        f.alua.tpgs == 0 || f.alua.usable,
    );
    record(
        cases,
        SECTION,
        label("TRIM (UNMAP)"),
        "supported, or cleanly not offered",
        match (f.trim, f.provisioning_clean) {
            (true, _) => "supported",
            (false, true) => "not offered",
            (false, false) => "not offered, and a provisioning case failed (see unmap)",
        },
        f.trim || f.provisioning_clean,
    );
    record(
        cases,
        SECTION,
        label("ODX (token copy)"),
        "supported, or cleanly not offered",
        if f.odx { "supported" } else { "not offered" },
        f.odx || f.odx_clean,
    );
    record(
        cases,
        SECTION,
        label("write cache"),
        "reported in the Caching mode page",
        match f.write_cache {
            Some(wce) => format!(
                "{} (WCE {}), DPOFUA {}",
                if wce { "write-back" } else { "write-through" },
                wce as u8,
                f.dpofua as u8
            ),
            None => "not reported".into(),
        },
        f.write_cache.is_some(),
    );
}
