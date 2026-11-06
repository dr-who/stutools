//! UNH-IOL NVMe RDMA/TCP Interoperability Test Suite v26.0, as spit runs it: each of the suite's tests by its own ID, the spit cases that
//! decide it, and what spit does not yet run. Built from the suites inventory; a test spit
//! starts running gets its cases and coverage here.

use super::super::suite::{Coverage::*, Ref, Style, Suite, Test};

pub const UNH_NVME_INTEROP: Suite = Suite {
    title: "UNH-IOL NVMe RDMA/TCP Interoperability Test Suite v26.0",
    publisher: "UNH-IOL",
    source: "https://www.iol.unh.edu/testing/storage/nvme",
    style: Style::Unh,
    transports: &["NVMe/TCP"],
    tests: &[
        Test::new("1.1", "Initiator to Target verification in a Fabric (M)", Partial, &[Ref::new("nvme-disc", "")], "through spit's own initiator. UNH needs ≥2 OS/HBA partners and ≥2 switches"),
        Test::new("1.2", "Initiator to Target traffic (M)", Partial, &[], "spit's workload mode (--nvme with verify) can do this, but --compliance has no timed-integrity case; nvme-io writes 8 blocks"),
        Test::new("1.3", "Traffic with Link Pull (M)", NotApplicable, &[], "Physical. A software stand-in is a TCP RST in mid-I/O, then reconnect and verify (see TCP 1.5)"),
        Test::new("1.4", "Traffic from Power-up (M)", NotApplicable, &[], "Physical"),
        Test::new("1.5", "Keep Alive Timeout (FYI)", Partial, &[Ref::new("nvme-admin", "Keep Alive Timer feature")], "nvme-admin: Keep Alive Timer feature, NVMe 1.2.1 Keep Alive"),
    ],
};
