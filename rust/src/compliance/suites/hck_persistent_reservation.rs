//! Microsoft HCK Persistent Reservation Test (LOGO), as spit runs it: each of the suite's tests by its own ID, the spit cases that
//! decide it, and what spit does not yet run. Built from the suites inventory; a test spit
//! starts running gets its cases and coverage here.

use super::super::suite::{Coverage::*, Ref, Style, Suite, Test};

pub const HCK_PERSISTENT_RESERVATION: Suite = Suite {
    title: "Microsoft HCK Persistent Reservation Test (LOGO)",
    publisher: "Microsoft",
    source: "https://learn.microsoft.com/en-us/previous-versions/windows/hardware/hck/jj124920(v=vs.85)",
    style: Style::Hlk,
    transports: &["iSCSI", "SAS", "SCSI"],
    tests: &[
        Test::new("WHCP-PR-1", "PR IN READ KEYS (00h)", Covered, &[Ref::new("pr", "READ KEYS lists A and B")], ""),
        Test::new("WHCP-PR-2", "PR IN READ RESERVATION (01h)", Partial, &[Ref::new("pr", "type *: READ RESERVATION")], "judges key and type; SCOPE not judged"),
        Test::new("WHCP-PR-3", "PR OUT RESERVE (01h), LU_SCOPE, type 5h", Covered, &[Ref::new("pr", "type 5 (write exclusive, registrants only): A RESERVE")], ""),
        Test::new("WHCP-PR-4", "PR OUT RELEASE (02h)", Covered, &[Ref::new("pr", "type *: A RELEASE")], ""),
        Test::new("WHCP-PR-5", "PR OUT CLEAR (03h)", Covered, &[Ref::new("pr", "B CLEAR"), Ref::new("pr", "no keys after CLEAR")], ""),
        Test::new("WHCP-PR-6", "PR OUT PREEMPT (04h)", Covered, &[Ref::new("pr", "type 5: B PREEMPT A"), Ref::new("pr", "A told it was preempted")], ""),
        Test::new("WHCP-PR-7", "PR OUT REGISTER AND IGNORE EXISTING KEY (06h)", Covered, &[Ref::new("pr", "B REGISTER AND IGNORE EXISTING KEY")], ""),
        Test::new("WHCP-PR-8", "PR OUT REGISTER (00h)", Covered, &[Ref::new("pr", "A REGISTER"), Ref::new("pr", "B REGISTER changes its key"), Ref::new("pr", "B REGISTER with the wrong reservation key")], ""),
        Test::new("WHCP-PR-9", "Scope and type of an existing reservation cannot be changed", Partial, &[], "pr \"type N: A RESERVE type M while holding\""),
        Test::new("WHCP-PR-10", "Re-register an already registered nexus", Covered, &[Ref::new("pr", "A registers again"), Ref::new("pr", "B registers again")], ""),
        Test::new("WHCP-PR-11", "Re-reserve by the holder (5.6.9)", Missing, &[], "the holder never repeats its own RESERVE"),
        Test::new("WHCP-PR-12", "PREEMPT / PREEMPT AND ABORT with an invalid SERVICE ACTION RESERVATION KEY", Missing, &[], "only \"C PREEMPT from an unregistered port\"; a registered nexus naming an unknown key is never sent"),
        Test::new("WHCP-PR-13", "ADDITIONAL LENGTH zero with no reservation (6.11.3.2)", Partial, &[Ref::new("pr", "no reservation after RELEASE")], "treats any length < 16 as none; 0 not required"),
        Test::new("WHCP-PR-14", "Register and unregister on all paths (MSDSM)", Covered, &[Ref::new("pr", "READ KEYS lists A's key once per path"), Ref::new("pr", "A's second path unregisters")], ""),
        Test::new("WHCP-PR-15", "Failover within 5 s for up to 100 LUNs; normal I/O within 25 s", Missing, &[], "no timing of PREEMPT handover or of 100 LUNs"),
    ],
};
