//! UNH-IOL NVMe/TCP Transport Conformance Test Suite v26.0, as spit runs it: each of the suite's tests by its own ID, the spit cases that
//! decide it, and what spit does not yet run. Built from the suites inventory; a test spit
//! starts running gets its cases and coverage here.

use super::super::suite::{Coverage::*, Ref, Style, Suite, Test};

pub const UNH_NVME_TCP: Suite = Suite {
    title: "UNH-IOL NVMe/TCP Transport Conformance Test Suite v26.0",
    publisher: "UNH-IOL",
    source: "https://www.iol.unh.edu/testing/storage/nvme",
    style: Style::Unh,
    transports: &["NVMe/TCP"],
    tests: &[
        Test::new("1.1", "Connect to Discovery Service (M)", Partial, &[Ref::new("nvme-disc", "")], "nvme-disc: log page (Connect, enable and Get Log Page 70h succeed). Gap: 1.1.2–1.1.5, 1.1.8"),
        Test::new("1.2", "Connect to NVM Subsystem (M)", Partial, &[Ref::new("nvme-conn", "")], "nvme-conn: connect, a second association. Gap: RECFMT; HOSTID 0; SQSIZE bounds"),
        Test::new("1.3", "Property Get (FYI; 1.3.2 M)", Partial, &[Ref::new("nvme-prop", "CAP")], "nvme-prop: CAP, VS, enable (CSTS). Gap: CC read; DSTRD=0, CQR=1; 1.3.2 (mandatory); 1.3.3–1.3.7"),
        Test::new("1.4", "Disconnect (FYI)", Missing, &[], "spit closes TCP connections and never sends Disconnect"),
        Test::new("1.5", "Connection Loss (FYI)", Missing, &[], "Only weakly judgeable (race); FYI"),
        Test::new("2.1", "Identify Controller Fabrics Fields (M)", Partial, &[Ref::new("nvme-ident", "NVMe 1.2.1 IOCCSZ and IORCSZ")], "nvme-ident: NVMe 1.2.1 IOCCSZ and IORCSZ (1028, 1). Gap: FCATT reserved bits; reserved bytes 2047:1804; ICDOFF/MSDBD reported"),
        Test::new("2.2", "Association after Reset (FYI)", Missing, &[], "No reset in spit"),
        Test::new("2.3", "Admin Command Support (FYI)", Partial, &[Ref::new("nvme-admin", "")], "nvme-admin cases cover all but DST (nvmet: no DST)"),
        Test::new("2.4", "Authentication (FYI)", Missing, &[], "spit has no in-band authentication; N/A on a target without AUTHREQ"),
        Test::new("2.5", "Shutdown (FYI)", Missing, &[], "See NVM 4.12/4.16"),
        Test::new("2.6", "SGLs within the Command Capsule (FYI)", Missing, &[], "spit sends one descriptor"),
        Test::new("2.7", "Rapid Path Failure Recovery, CCR (FYI)", Missing, &[], "2.7.3 and 2.7.4 are cheap consistency checks"),
        Test::new("3.1", "Initialize Discovery Controller (M)", Partial, &[Ref::new("nvme-disc", "")], "nvme-disc: log page, record <nqn> (TCP, address, port, subtype). Gap: Identify on the DC; ADRFAM; ASQSZ ≥ 32; TREQ; TSAS; GENCTR; PORTID"),
        Test::new("3.2", "Keep Alive Timeout (M)", Missing, &[], "spit keeps sessions alive (host.rs) but never lets KATO expire, and never checks TBKAS (CTRATT bit 6)"),
        Test::new("4.1", "ICReq Errors (FYI)", Missing, &[], "spit sends only a valid ICReq (pdu.rs icreq) and never parses a C2HTermReq FES as a verdict"),
        Test::new("4.2", "Connect Errors (FYI)", Missing, &[], "All five are cheap"),
        Test::new("4.3", "Queue Depth Exceeded (FYI)", Missing, &[], "spit runs one command at a time on compliance queues"),
        Test::new("4.4", "TLS Support (FYI)", Missing, &[], "spit parses the record but not TSAS"),
        Test::new("4.5", "Digest Support (FYI)", Partial, &[], "spit asks for no digests and fails any PDU with digest flags (pdu.rs). That is an error path, not a case. Gap: 4.5.3/4.5.4 (spit cannot negotiate digests)"),
        Test::new("5.1", "Centralized Discovery Commands (FYI)", NotApplicable, &[], "Mostly CDC. 5.1.10 (DUPRETINFO only for SUBTYPE 3h) and 5.1.11 (TDLPL=0 for the basic format) apply to any DDC and are cheap"),
        Test::new("6.1–6.3", "Automated Discovery: mDNS, zoning, kickstart (FYI)", NotApplicable, &[], "CDC/DDC registration; not a plain target"),
    ],
};
