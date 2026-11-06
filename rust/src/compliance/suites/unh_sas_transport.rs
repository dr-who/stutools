//! UNH-IOL SAS SPL Transport Layer Test Suite v1.14, as spit runs it: each of the suite's tests by its own ID, the spit cases that
//! decide it, and what spit does not yet run. Built from the suites inventory; a test spit
//! starts running gets its cases and coverage here.

use super::super::suite::{Coverage::*, Ref, Style, Suite, Test};

pub const UNH_SAS_TRANSPORT: Suite = Suite {
    title: "UNH-IOL SAS SPL Transport Layer Test Suite v1.14",
    publisher: "UNH-IOL",
    source: "https://www.iol.unh.edu/testing/storage/sas",
    style: Style::Unh,
    transports: &["SAS"],
    tests: &[
        Test::new("8.1.1", "SSP Frames – Hashed Address", NotApplicable, &[], "frame header field (analyzer)"),
        Test::new("8.1.2", "SSP Frames – Information Unit", NotApplicable, &[], "frame-level (analyzer)"),
        Test::new("8.1.3", "SSP Frames – Number of Fill Bytes", NotApplicable, &[], "frame-level (analyzer)"),
        Test::new("8.1.4", "SSP Command IU – Tag", NotApplicable, &[], "frame-level (analyzer); the HBA owns the tags"),
        Test::new("8.1.5", "SSP Command IU – TPTT", NotApplicable, &[], "frame-level (analyzer)"),
        Test::new("8.1.6", "SSP Data IU – Fill Bytes Non-Zero", NotApplicable, &[], "frame-level (analyzer)"),
        Test::new("8.1.7", "SSP Data IU – Fill Bytes Zero", NotApplicable, &[], "frame-level (analyzer)"),
        Test::new("8.1.8", "SSP Data IU – Data Offset", NotApplicable, &[], "frame-level (analyzer)"),
        Test::new("8.1.9", "XFER_RDY – Requested Offset", NotApplicable, &[], "frame-level (analyzer). Reading page 02h is part of the Application 10.1.4 gap"),
        Test::new("8.1.10", "XFER_RDY – Large Transfer", NotApplicable, &[], "frame-level (analyzer)"),
        Test::new("8.1.11", "XFER_RDY – Maximum Burst Size", NotApplicable, &[], "frame-level (analyzer)"),
        Test::new("8.1.12", "XFER_RDY – Write Data Length", NotApplicable, &[], "frame-level (analyzer)"),
        Test::new("8.1.13", "Response IU – No Data Present", NotApplicable, &[], "DATAPRES is not visible to the host. The GOOD is in spc TEST UNIT READY"),
        Test::new("8.1.14", "Response IU – Sense Data Present", Partial, &[Ref::new("tmf", ""), Ref::new("disk", "")], "path /dev/sdX unit attention after the reset is recorded as INFO (got 29h/03h) after SG_SCSI_RESET, but not judged. DATAPRES needs an analyzer; spit cannot power-cycle the"),
        Test::new("8.1.15", "Response IU – Sense/Response Not Present", NotApplicable, &[], "frame-level (analyzer)"),
        Test::new("8.1.16", "Response IU – Response Data Present", NotApplicable, &[], "no user-space TMF to an arbitrary LUN (SG_SCSI_RESET only resets an existing sdev)"),
        Test::new("8.1.17", "Error – Invalid Length", NotApplicable, &[], "forged frame (generator)"),
        Test::new("8.1.18", "Error – Invalid LUN", NotApplicable, &[], "the HBA does not pass TMF response codes back; no TMF to an unconfigured LUN"),
        Test::new("8.1.19", "Error – No ACK/NAK Received", NotApplicable, &[], "primitive control (generator)"),
        Test::new("8.1.20", "Error – Unknown Tag", NotApplicable, &[], "forged frame (generator)"),
        Test::new("8.2.1", "Initiator – Hashed Address", NotApplicable, &[], "initiator DUT"),
        Test::new("8.2.2", "Initiator – Information Unit", NotApplicable, &[], "initiator DUT"),
        Test::new("8.2.3", "Initiator – Fill Bytes", NotApplicable, &[], "initiator DUT"),
        Test::new("8.2.4", "Initiator – Tag", NotApplicable, &[], "initiator DUT"),
        Test::new("8.2.5", "Initiator – TPTT echo", NotApplicable, &[], "initiator DUT"),
        Test::new("8.2.6", "Initiator – TPTT First Burst", NotApplicable, &[], "initiator DUT"),
        Test::new("8.2.7", "Initiator – TPTT Non-Data", NotApplicable, &[], "initiator DUT"),
        Test::new("8.2.8", "Initiator – DATAPRES Reserved", NotApplicable, &[], "initiator DUT"),
        Test::new("8.2.9", "Initiator – Extra Response", NotApplicable, &[], "initiator DUT"),
    ],
};
