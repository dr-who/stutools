//! Microsoft Failover Cluster validation: Storage, as spit runs it: each of the suite's tests by its own ID, the spit cases that
//! decide it, and what spit does not yet run. Built from the suites inventory; a test spit
//! starts running gets its cases and coverage here.

use super::super::suite::{Coverage::*, Ref, Style, Suite, Test};

pub const CLUSTER_VALIDATION: Suite = Suite {
    title: "Microsoft Failover Cluster validation: Storage",
    publisher: "Microsoft",
    source: "https://learn.microsoft.com/en-us/windows-server/failover-clustering/",
    style: Style::Cluster,
    transports: &["iSCSI", "SAS", "SCSI"],
    tests: &[
        Test::new("CL-LIST.1", "List Disks", Covered, &[Ref::new("disk", "standard INQUIRY"), Ref::new("disk", "VPD 80h"), Ref::new("disk", "VPD 83h")], ""),
        Test::new("CL-LIST.2", "List Disks To Be Validated", Covered, &[Ref::new("identity", "")], ""),
        Test::new("CL-LIST.3", "Disk with a PR is skipped", NotApplicable, &[], "host-side; spit's clean_slate clears PRs before its own run"),
        Test::new("CL-VPD.1", "VPD 83h from every node", Covered, &[Ref::new("identity", "+ * *")], ""),
        Test::new("CL-VPD.2", "Uniqueness across disks", Covered, &[Ref::new("identity", "+ * + *")], ""),
        Test::new("CL-LAT.1", "Validate Disk Access Latency", Missing, &[], "spit times no compliance I/O"),
        Test::new("CL-PR.1", "REGISTER AND IGNORE EXISTING from node 1", Covered, &[Ref::new("pr", "B REGISTER AND IGNORE EXISTING KEY")], ""),
        Test::new("CL-PR.2", "RESERVE type 5 from node 1", Covered, &[Ref::new("pr", "type 5 (*): A RESERVE")], ""),
        Test::new("CL-PR.3", "READ RESERVATION from node 1", Covered, &[Ref::new("pr", "type 5 (*): READ RESERVATION")], ""),
        Test::new("CL-PR.4", "REGISTER AND IGNORE EXISTING from node 2", Covered, &[Ref::new("pr", "B REGISTER AND IGNORE EXISTING KEY")], ""),
        Test::new("CL-PR.5", "RESERVE from node 2 while node 1 holds", Covered, &[Ref::new("pr", "type 5 (*): B RESERVE the same type")], ""),
        Test::new("CL-PR.6", "PREEMPT from an unregistered node", Covered, &[Ref::new("pr", "type 5: C PREEMPT from an unregistered port")], ""),
        Test::new("CL-PR.7", "RESERVE after (re)registering and preempting", Covered, &[Ref::new("pr", "type 5: B PREEMPT A"), Ref::new("pr", "B holds after PREEMPT")], ""),
        Test::new("CL-PR.8", "Unregister cleanup on every path", Covered, &[Ref::new("pr", "holder A unregisters"), Ref::new("pr", "A's second path unregisters")], ""),
        Test::new("CL-PR.9", "PR command time", Missing, &[], "spit records no PR latency"),
        Test::new("CL-PR.10", "Separate node identities", Partial, &[], "spit's B and C share one initiator name; separate-name coverage depends on the run's second IQN (spit-initiator-b)"),
        Test::new("CL-ARB.1", "Validate Disk Arbitration", Partial, &[Ref::new("pr", "B PREEMPT A"), Ref::new("pr", "A PREEMPT AND ABORT B"), Ref::new("pr", "A holds after PREEMPT AND ABORT")], "preempt mechanics (,, ); the arbitration timing and the defence sequence are not run"),
        Test::new("CL-ARB.2", "Validate Multiple Arbitration", Missing, &[Ref::new("ats", "two hosts race 100 rounds")], "spit has a two-host race for COMPARE AND WRITE only (ats \"two hosts race 100 rounds\"); none for PREEMPT"),
        Test::new("CL-FO.1", "Validate Disk Failover", Partial, &[Ref::new("coherence", "")], "A writes, B reads) and preempt run separately; never data written under A's reservation then read by B after it takes over"),
        Test::new("CL-FO.2", "Validate Simultaneous Failover", Missing, &[], "pr runs LUN by LUN"),
        Test::new("CL-FS.1", "Validate File System", NotApplicable, &[], "file system, host-side"),
        Test::new("CL-MPIO.1", "Validate Microsoft MPIO-based disks", Partial, &[Ref::new("identity", "")], "target side (one per LUN on every path) covered by; DSM checks are host-side"),
        Test::new("CL-CSV.1", "Validate CSV Network Bindings / Settings", NotApplicable, &[], "not storage protocol"),
        Test::new("CL-SSPR.1", "No PR present before the test", Covered, &[Ref::new("pr", "READ KEYS lists A and B")], ""),
        Test::new("CL-SSPR.2", "Node 1 REGISTER AND IGNORE EXISTING KEY RK 0 SARK 0xa", Covered, &[Ref::new("pr", "B REGISTER AND IGNORE EXISTING KEY")], ""),
        Test::new("CL-SSPR.3", "Node 1 RESERVE with key 0xa", Covered, &[Ref::new("pr", "type 5 (*): A RESERVE")], ""),
        Test::new("CL-SSPR.4", "Node 2 REGISTER AND IGNORE EXISTING KEY SARK 0x100aa", Covered, &[Ref::new("pr", "B REGISTER AND IGNORE EXISTING KEY")], ""),
        Test::new("CL-SSPR.5", "Holder changes its key while holding", Missing, &[], "only non-holder B changes its key (\"B REGISTER changes its key\")"),
        Test::new("CL-SSPR.6", "Non-holder changes its key", Covered, &[Ref::new("pr", "B REGISTER changes its key"), Ref::new("pr", "READ KEYS shows B's new key")], ""),
        Test::new("CL-SSPR.7", "Holder re-registers the same key", Missing, &[], "as WHCP.CA.5"),
        Test::new("CL-SSPR.8", "Reservation present after the holder's key change", Missing, &[], "as CL-SSPR.5"),
    ],
};
