//! UNH-IOL iSCSI Interoperability Test Suite v3.0, as spit runs it: each of the suite's tests by its own ID, the spit cases that
//! decide it, and what spit does not yet run. Built from the suites inventory; a test spit
//! starts running gets its cases and coverage here.

use super::super::suite::{Coverage::*, Ref, Style, Suite, Test};

pub const UNH_ISCSI_INTEROP: Suite = Suite {
    title: "UNH-IOL iSCSI Interoperability Test Suite v3.0",
    publisher: "UNH-IOL",
    source: "https://www.iol.unh.edu/testing/storage/iscsi",
    style: Style::Unh,
    transports: &["iSCSI"],
    tests: &[
        Test::new("1.1", "Power On", NotApplicable, &[], "Needs power control of the target and host"),
        Test::new("1.2.1", "Disconnect Reconnect", Partial, &[Ref::new("session", ""), Ref::new("login", "")], "with the ISID of an open, the reinstated is closed. Missing: a TCP drop without Logout, then a re- after 10 s and after 60 s (the target has seen the reset), with I/O checked"),
        Test::new("1.2.2", "Enable Disable", Partial, &[], "As 1.2.1 (same stimulus at the target)"),
        Test::new("1.3.1", "Disconnect Reconnect Resume Traffic Flow", Partial, &[Ref::new("pr", "")], "reinstated A WRITE as holder, B WRITE while reinstated A holds show I/O after reinstatement. Missing: a drop with WRITEs in flight, then a re- and a re-issue whose data reads back"),
        Test::new("1.3.2", "Disable Enable Resume Traffic Flow", Partial, &[], "As 1.3.1"),
        Test::new("2.1", "Multiple Targets Found", NotApplicable, &[], "Not Applicable when the DUT is a target (plan)"),
        Test::new("2.2", "Target Removed (single initiator)", NotApplicable, &[], "Not Applicable when the DUT is a target (plan)"),
        Test::new("3.1", "Configure Multi Initiator System", Covered, &[Ref::new("login", "initiator not on the access list")], "stgt hides the target from an unauthorized initiator with status 0203; the case accepts 0201h, 0202h or 0203h, or admission by an open target"),
        Test::new("3.2", "Target Removed (multi initiator)", Partial, &[Ref::new("session", "")], "Two IQNs, and reinstatement for A ( …, reinstated A …). Missing: the unauthorized-IQN check after a reconnect, and I/O during the drop"),
    ],
};
