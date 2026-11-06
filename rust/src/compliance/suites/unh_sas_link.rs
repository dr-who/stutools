//! UNH-IOL SAS SPL Link Layer Test Suite v1.4, as spit runs it: each of the suite's tests by its own ID, the spit cases that
//! decide it, and what spit does not yet run. Built from the suites inventory; a test spit
//! starts running gets its cases and coverage here.

use super::super::suite::{Coverage::*, Style, Suite, Test};

pub const UNH_SAS_LINK: Suite = Suite {
    title: "UNH-IOL SAS SPL Link Layer Test Suite v1.4",
    publisher: "UNH-IOL",
    source: "https://www.iol.unh.edu/testing/storage/sas",
    style: Style::Unh,
    transports: &["SAS"],
    tests: &[
        Test::new("6.1.1", "Link Reset – Repeat Phy Sequence if no Identify", NotApplicable, &[], "N/A: frame-level (analyzer)"),
        Test::new("6.1.2", "Link Reset – Ignore Additional Identify", NotApplicable, &[], "N/A: frame-level (analyzer)"),
        Test::new("6.1.3", "Link Reset – HARD_RESET Received", Missing, &[], "a host can trigger it (/sys/class/sas_phy/<phy>/hard_reset, or SMP PHY CONTROL HARD RESET), but sees only the result (path back, UA 29h), not the COMINIT. See gaps"),
        Test::new("6.1.4", "Connections – OPEN_ACCEPT", NotApplicable, &[], "N/A: frame-level (analyzer)"),
        Test::new("6.1.5", "Connections – 2 SOAF", NotApplicable, &[], "N/A: frame-level (analyzer)"),
        Test::new("6.1.6", "OPEN_REJECT Rate Not Supported", NotApplicable, &[], "N/A: frame-level (analyzer)"),
        Test::new("6.1.7", "OPEN_REJECT Protocol Not Supported", NotApplicable, &[], "N/A: frame-level (analyzer)"),
        Test::new("6.1.8", "OPEN_REJECT Wrong Destination", NotApplicable, &[], "N/A: frame-level (analyzer)"),
        Test::new("6.1.9", "OPEN_REJECT Retry", NotApplicable, &[], "N/A: frame-level (analyzer)"),
        Test::new("6.1.10", "SSP Interlocked Frame", NotApplicable, &[], "N/A: frame-level (analyzer)"),
        Test::new("6.1.11", "No ACK for Interlocked Frame", NotApplicable, &[], "N/A: frame-level (analyzer)"),
        Test::new("6.1.12", "SSP Multiple ACKs", Missing, &[], "the load is host-driven. The criterion needs an analyzer; a host proxy is the phy error counters (LOG SENSE page 18h, /sys/class/sas_phy/*/…_count). See gaps"),
        Test::new("6.1.13", "SSP Frames – RRDY", NotApplicable, &[], "N/A: frame-level (analyzer)"),
        Test::new("6.1.14", "Close – DONE (NORMAL)", NotApplicable, &[], "N/A: frame-level (analyzer)"),
        Test::new("6.1.15", "Close – DONE (ACK/NAK TIMEOUT)", NotApplicable, &[], "N/A: frame-level (analyzer)"),
        Test::new("6.1.16", "Close – DONE (CREDIT TIMEOUT)", NotApplicable, &[], "N/A: frame-level (analyzer)"),
        Test::new("6.1.17", "Close – CLOSE (NORMAL)", NotApplicable, &[], "N/A: frame-level (analyzer)"),
        Test::new("6.1.18", "BREAK by the Testing Station", NotApplicable, &[], "N/A: frame-level (analyzer)"),
        Test::new("6.1.19", "BREAK by the DUT", NotApplicable, &[], "N/A: frame-level (analyzer)"),
        Test::new("6.1.20", "Through Expanders – Open SSP Target", NotApplicable, &[], "N/A: frame-level (analyzer)"),
        Test::new("6.1.21", "BREAK – OpenTimeout", NotApplicable, &[], "N/A: frame-level (analyzer)"),
        Test::new("6.1.22", "CLOSE (CLEAR AFFILIATION)", NotApplicable, &[], "N/A: frame-level (analyzer)"),
        Test::new("6.1.23", "CLOSE (RESERVED 0)", NotApplicable, &[], "N/A: frame-level (analyzer)"),
        Test::new("6.1.24", "CLOSE (RESERVED 1)", NotApplicable, &[], "N/A: frame-level (analyzer)"),
        Test::new("6.1.25", "HARD_RESET After Identify", NotApplicable, &[], "N/A: frame-level (analyzer); an HBA sends HARD_RESET only in place of IDENTIFY"),
        Test::new("6.1.26", "Interlocked Frame / NAK", NotApplicable, &[], "N/A: frame-level (analyzer)"),
        Test::new("6.2.1–6.2.24", "Link layer for initiators (24 tests: link reset ×3, SMP discover, expander OPEN_REJECT retry, OPEN_ACCEPT, SOAF, OPEN_REJECT ×4, multiple ACKs, RRDY, DONE ×3, CLOSE, BREAK ×2, through-expander SSP/SMP opens, SSP/SMP open timeouts, CLOSE (RESERVED 1))", NotApplicable, &[], "initiator DUT; frame-level (analyzer)"),
        Test::new("6.3.1–6.3.29", "Link layer for expanders (29 tests: link reset ×3, SMP OPEN_ACCEPT/SOAF, OPEN_REJECT NO/BAD DESTINATION, STP open, STP RESOURCES BUSY, SATA_HOLD, SATA_R_IP, STP open timeout, BREAK forwarding ×2, AIP ×3, AWT ×3, PATHWAY BLOCKED, SMP request/response/close format, CLOSE variants ×3, HARD_RESET after identify, CRC NAK)", NotApplicable, &[], "expander DUT; frame-level (analyzer)"),
    ],
};
