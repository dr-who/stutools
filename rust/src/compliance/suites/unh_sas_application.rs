//! UNH-IOL SAS Application Layer Test Suite v1.02, as spit runs it: each of the suite's tests by its own ID, the spit cases that
//! decide it, and what spit does not yet run. Built from the suites inventory; a test spit
//! starts running gets its cases and coverage here.

use super::super::suite::{Coverage::*, Ref, Style, Suite, Test};

pub const UNH_SAS_APPLICATION: Suite = Suite {
    title: "UNH-IOL SAS Application Layer Test Suite v1.02",
    publisher: "UNH-IOL",
    source: "https://www.iol.unh.edu/testing/storage/sas",
    style: Style::Unh,
    transports: &["SAS"],
    tests: &[
        Test::new("10.1.1", "SCSI CDB – TEST UNIT READY", Covered, &[Ref::new("spc", "TEST UNIT READY")], ""),
        Test::new("10.1.2", "SCSI CDB – INQUIRY", Covered, &[Ref::new("disk", "")], ""),
        Test::new("10.1.3", "SCSI CDB – START STOP", Covered, &[Ref::new("disk", "START STOP UNIT")], ""),
        Test::new("10.1.4", "SCSI CDB – MODE SENSE", Partial, &[Ref::new("disk", "MODE SENSE (6) page 02h alone"), Ref::new("disk", "MODE SENSE (10) page 02h alone")], "page 02h is read on its own with MODE SENSE (6) and (10) and compared with page 3Fh; its SPL format (page length 0Eh) is not judged"),
        Test::new("10.1.5", "SCSI CDB – MODE SELECT", Covered, &[Ref::new("disk", "MODE SELECT (10) * WCE"), Ref::new("disk", "MODE SENSE (10) after MODE SELECT (10)"), Ref::new("disk", "MODE SELECT (6) * WCE again"), Ref::new("disk", "MODE SELECT (10) of a WCE that is not changeable refused"), Ref::new("disk", "Caching page current and saved values written back")], ""),
        Test::new("10.1.6", "SCSI CDB – READ CAPACITY", Covered, &[Ref::new("disk", "")], ""),
        Test::new("10.1.7", "SCSI CDB – WRITE", Covered, &[Ref::new("io", "")], ""),
        Test::new("10.1.8", "SCSI CDB – READ", Covered, &[Ref::new("io", "READ (10) returns what WRITE (10) wrote")], ""),
        Test::new("10.1.9", "SCSI CDB – LOG SENSE", Covered, &[Ref::new("disk", "")], ""),
        Test::new("10.2.1", "STP – IDENTIFY DEVICE", NotApplicable, &[], "the DUT is an expander/STP bridge with a SATA drive. spit lists sbc SAT ATA PASS-THROUGH (12)/(16) as out of scope"),
        Test::new("10.2.2", "STP – SET FEATURES", NotApplicable, &[], "expander/STP DUT"),
        Test::new("10.2.3", "STP – IDLE", NotApplicable, &[], "expander/STP DUT"),
        Test::new("10.2.4", "STP – SET MULTIPLE MODE", NotApplicable, &[], "expander/STP DUT"),
        Test::new("10.2.5", "STP – WRITE SECTORS", NotApplicable, &[], "expander/STP DUT"),
        Test::new("10.2.6", "STP – READ SECTORS", NotApplicable, &[], "expander/STP DUT"),
        Test::new("10.2.7", "STP – WRITE MULTIPLE", NotApplicable, &[], "expander/STP DUT"),
        Test::new("10.2.8", "STP – READ MULTIPLE", NotApplicable, &[], "expander/STP DUT"),
        Test::new("10.2.9", "STP – WRITE DMA", NotApplicable, &[], "expander/STP DUT"),
        Test::new("10.2.10", "STP – READ DMA", NotApplicable, &[], "expander/STP DUT"),
    ],
};
