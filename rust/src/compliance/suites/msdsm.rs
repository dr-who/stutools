//! Microsoft MPIO and MSDSM requirements for ALUA targets, as spit runs it: each of the suite's tests by its own ID, the spit cases that
//! decide it, and what spit does not yet run. Built from the suites inventory; a test spit
//! starts running gets its cases and coverage here.

use super::super::suite::{Coverage::*, Ref, Style, Suite, Test};

pub const MSDSM: Suite = Suite {
    title: "Microsoft MPIO and MSDSM requirements for ALUA targets",
    publisher: "Microsoft",
    source: "https://learn.microsoft.com/en-us/windows-server/storage/",
    style: Style::Hlk,
    transports: &["iSCSI", "SAS", "SCSI"],
    tests: &[
        Test::new(
            "MSDSM.1",
            "SPC-3 compliance",
            Covered,
            &[
                Ref::new("sbc", ""),
                Ref::new("disk", "standard INQUIRY VERSION"),
            ],
            "",
        ),
        Test::new(
            "MSDSM.2",
            "Unique hardware identifier",
            Covered,
            &[
                Ref::new("identity", ""),
                Ref::new(
                    "windows",
                    "MPIO and Failover Clustering: unique LUN ID in VPD 83h",
                ),
            ],
            "",
        ),
        Test::new(
            "MSDSM.3",
            "Claim by VID/PID",
            Covered,
            &[Ref::new(
                "disk",
                "vendor, product and revision are printable ASCII",
            )],
            "",
        ),
        Test::new(
            "MSDSM.4",
            "TPGS reported",
            Covered,
            &[Ref::new("mpio", "REPORT TARGET PORT GROUPS (TPGS 11b)")],
            "",
        ),
        Test::new(
            "MSDSM.5",
            "Explicit ALUA preferred",
            Covered,
            &[Ref::new("mpio", "SET TARGET PORT GROUPS *")],
            "",
        ),
        Test::new(
            "MSDSM.6",
            "Access states",
            Covered,
            &[
                Ref::new("mpio", "group 0 state is a supported state"),
                Ref::new("mpio", "group 1 state is a supported state"),
            ],
            "",
        ),
        Test::new(
            "MSDSM.7",
            "TPG id, PREF, relative port id agree",
            Covered,
            &[Ref::new(
                "mpio",
                "VPD 83h port and group agree with REPORT TARGET PORT GROUPS",
            )],
            "",
        ),
        Test::new(
            "MSDSM.8",
            "RTPG answered within the DSM timeout",
            Missing,
            &[],
            "spit times no RTPG",
        ),
        Test::new(
            "MSDSM.9",
            "RTPG retried after CHECK CONDITION",
            Partial,
            &[],
            "spit's unit_attentions drain UAs; RTPG right after a state change is not checked",
        ),
        Test::new(
            "MSDSM.10",
            "Standby path commands",
            Covered,
            &[
                Ref::new("mpio", "standby: INQUIRY"),
                Ref::new("mpio", "standby: REPORT TARGET PORT GROUPS"),
                Ref::new("mpio", "standby: TEST UNIT READY reports the standby state"),
                Ref::new("mpio", "standby: PERSISTENT RESERVE IN, READ KEYS"),
                Ref::new(
                    "mpio",
                    "standby: PERSISTENT RESERVE OUT, REGISTER AND IGNORE EXISTING KEY",
                ),
                Ref::new("mpio", "standby: MODE SENSE (10)"),
            ],
            "",
        ),
        Test::new(
            "MSDSM.11",
            "PR IN on every path",
            Partial,
            &[],
            "PR IN from A, B, C and A's second path; not every portal",
        ),
        Test::new(
            "MSDSM.12",
            "START STOP UNIT causes no failover",
            Covered,
            &[
                Ref::new("disk", "START STOP UNIT, STOP"),
                Ref::new("disk", "START STOP UNIT, START"),
                Ref::new("windows", "START STOP UNIT changes no target port group"),
            ],
            "",
        ),
        Test::new(
            "MSDSM.13",
            "Path recovery within PDORemovePeriod",
            Missing,
            &[],
            "as MPIO.2/MPIO.3",
        ),
        Test::new(
            "MSDSM.14",
            "ALUA state-change UA 2Ah/06h",
            Partial,
            &[
                Ref::new(
                    "mpio",
                    "the other nexus is told ASYMMETRIC ACCESS STATE CHANGED",
                ),
                Ref::new("mpio", "the nexus that sent it is not told"),
            ],
            "LIO fails),; MSDSM's use is not published",
        ),
        Test::new("MSDSM.15", "32 paths", NotApplicable, &[], "host limit"),
        Test::new(
            "MSDSM.16",
            "Default load-balance policy",
            NotApplicable,
            &[],
            "host-side (sources conflict)",
        ),
        Test::new(
            "MSDSM.17",
            "ODX retried after MPIO failover",
            NotApplicable,
            &[],
            "host-side",
        ),
    ],
};
