//! Microsoft HLK SCSI Compliance Fuzz Test (LOGO), as spit runs it: each of the suite's tests by its own ID, the spit cases that
//! decide it, and what spit does not yet run. Built from the suites inventory; a test spit
//! starts running gets its cases and coverage here.

use super::super::suite::{Coverage::*, Ref, Style, Suite, Test};

pub const HLK_FUZZ: Suite = Suite {
    title: "Microsoft HLK SCSI Compliance Fuzz Test (LOGO)",
    publisher: "Microsoft",
    source: "https://learn.microsoft.com/en-us/windows-hardware/test/hlk/testref/4f42dc87-2019-4423-bacb-2ed09da96685",
    style: Style::Hlk,
    transports: &["iSCSI", "SAS", "SCSI"],
    tests: &[
        Test::new("FUZZ-1", "SCSI Compliance Fuzz Test", Partial, &[Ref::new("sbc", ""), Ref::new("spc", "unimplemented operation code refused")], "probes every opcode once, well formed, and; no reserved-bit, length or field fuzzing and no liveness check after it"),
    ],
};
