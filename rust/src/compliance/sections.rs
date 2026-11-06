//! The sections of `--compliance` in the order they run. Each names the chapter of the report it
//! belongs to (`Part`), the standard and clause it tests, written against the latest editions held
//! (a report at an older SCSI level cites that level's, `citation`), and what it does to the device,
//! in plain words for the report. Every case names one of these sections; the score and the report
//! list them in this order.

use super::parts::Part;

/// One section.
pub struct Section {
    pub name: &'static str,
    pub part: Part,
    pub standard: &'static str,
    pub tests: &'static str,
}

const fn s(name: &'static str, part: Part, standard: &'static str, tests: &'static str) -> Section {
    Section {
        name,
        part,
        standard,
        tests,
    }
}

pub const SECTIONS: [Section; 26] = [
    s(
        "login",
        Part::Iscsi,
        "RFC 7143 6.3 and 13: login phase and key negotiation",
        "Logs in to every portal of every target, offering the keys Windows Server 2022 and Linux open-iscsi offer and others, and checks each answer: digests, segment and burst lengths, R2T and immediate data, error recovery level, and refusals of what the target does not allow.",
    ),
    s(
        "identity",
        Part::Spc,
        "SPC-6 7.7: INQUIRY, VPD 80h and 83h, one logical unit on every path",
        "Reads the standard INQUIRY data and the Unit Serial Number and Device Identification pages through every path to each target, and checks that every path names the same logical unit, which Windows MPIO and Linux dm-multipath depend on.",
    ),
    s(
        "scsi",
        Part::Spc,
        "SPC-6, SBC-5: identity, capacity and target port groups on every path",
        "Compares the capacity, block size and target port groups each path reports, and finds the paths in the standby state, which later sections test for what a standby port must take.",
    ),
    s(
        "chap",
        Part::Iscsi,
        "RFC 7143 12.1: CHAP authentication",
        "Logs in with the right CHAP secret, a wrong one and none, and checks that only the right one is let in.",
    ),
    s(
        "session",
        Part::Iscsi,
        "RFC 7143 11.2.3: sessions, digests, NOP-Out and reinstatement",
        "Opens sessions side by side, pings with NOP-Out, reinstates a session with the same ISID, and sends a 32-byte command (whose CDB travels in an additional header segment) on a session with CRC32C header digests.",
    ),
    s(
        "spc",
        Part::Spc,
        "SPC-6 table 88: the commands every device takes, and invalid ones",
        "Sends the commands every SCSI device must take (TEST UNIT READY, REPORT LUNS, INQUIRY, REQUEST SENSE) and an operation code no standard assigns, which must be refused with ILLEGAL REQUEST.",
    ),
    s(
        "sbc",
        Part::Sbc,
        "SBC-5 table 34: the block command set, row by row",
        "Walks the block device command table of the level's SBC edition. Each command is sent in a form that changes nothing, or, where it would destroy data, asked about with REPORT SUPPORTED OPERATION CODES. A mandatory command, or one a host sends, must work; an optional one must work or be refused cleanly; one the edition does not have only informs.",
    ),
    s(
        "features",
        Part::Sbc,
        "SPC-6 annex A, SBC-5 annex A: feature sets",
        "Checks each feature set the device claims in its SCSI Feature Sets VPD page against the set's requirements, one requirement at a time, and reports what an unclaimed set would miss. SPC-3, SPC-4 and SBC-3 define no feature sets.",
    ),
    s(
        "disk",
        Part::Sbc,
        "SPC-6 7.7, SBC-5 6.6: what Windows and Linux ask of a new disk",
        "Reads what a host's disk driver reads when a disk appears: INQUIRY at the allocation lengths hosts use, every VPD page the device lists (Block Limits, Block Device Characteristics, Logical Block Provisioning and the others) for its code, length and fields, the mode pages, READ CAPACITY, and START STOP UNIT.",
    ),
    s(
        "io",
        Part::Sbc,
        "SBC-5: reads, writes, FUA, VERIFY and cache flush (NTFS)",
        "Writes patterns and reads them back with READ and WRITE (10) and (16), FUA, VERIFY and SYNCHRONIZE CACHE, at the last block and past it, as NTFS, ext4 and XFS do; only blocks it writes back are changed.",
    ),
    s(
        "unmap",
        Part::Sbc,
        "SBC-5 4.7: logical block provisioning, UNMAP (TRIM)",
        "Checks the thin provisioning the device reports: UNMAP, WRITE SAME with UNMAP, GET LBA STATUS, and that unmapped blocks read back as the device says they will. Windows and Linux send these for TRIM and discard.",
    ),
    s(
        "ats",
        Part::Sbc,
        "SBC-5 5.3: COMPARE AND WRITE",
        "Atomic test and set, which VMware ESXi uses for its locks: a matching compare writes, a mismatch writes nothing and reports MISCOMPARE with the offset.",
    ),
    s(
        "odx",
        Part::Sbc,
        "SBC-5 4.28, SPC-6 6.25: ROD token copy (Windows ODX)",
        "Offloaded copy the way Windows ODX does it: POPULATE TOKEN, RECEIVE ROD TOKEN INFORMATION and WRITE USING TOKEN between blocks of the device, or a clean refusal.",
    ),
    s(
        "pr",
        Part::Spc,
        "SPC-6 5.14, 6.14, 6.15: persistent reservations (Failover Clustering)",
        "Registers, reserves, releases, preempts and clears with two initiator names and several ports, for every reservation type, and checks who may read and write under each. Windows Failover Clustering, VMware and Linux clusters rely on these.",
    ),
    s(
        "tmf",
        Part::Iscsi,
        "SAM-6 7, RFC 7143 11.5 and 11.6: task management (MPIO recovery)",
        "Sends each task management function, ABORT TASK, ABORT TASK SET, CLEAR TASK SET, LOGICAL UNIT RESET, the QUERY functions and an undefined one, and checks every response, as MPIO uses them to recover a path.",
    ),
    s(
        "mpio",
        Part::Spc,
        "SPC-6 5.18: ALUA target port groups (Windows MSDSM)",
        "Reads and changes the target port group states, checks the unit attention the other paths must get, and what a port in the standby state takes and refuses, as Windows MSDSM and dm-multipath use them.",
    ),
    s(
        "nvme-disc",
        Part::Nvme,
        "NVMe Base 2.1 5.1.12.3: discovery log page",
        "Reads the discovery log page of every --nvme host and checks each record.",
    ),
    s(
        "nvme-conn",
        Part::Nvme,
        "NVMe Base 2.1 Fabrics: Connect and controller IDs",
        "Connects to the admin and I/O queues and checks the controller IDs and the refusals of a wrong host NQN or subsystem.",
    ),
    s(
        "nvme-prop",
        Part::Nvme,
        "NVMe Base 2.1: CAP, VS, CC and CSTS properties",
        "Reads the controller properties and enables the controller as a host does.",
    ),
    s(
        "nvme-ident",
        Part::Nvme,
        "NVMe Base 2.1 5.1.13: Identify, the same IDs on every path",
        "Identify Controller, Namespace, the active namespace list and the identification descriptors, through every address, which must agree.",
    ),
    s(
        "nvme-admin",
        Part::Nvme,
        "NVMe Base 2.1: log pages, features, Abort, Keep Alive",
        "The mandatory log pages and features, Abort, Keep Alive and Asynchronous Event Request.",
    ),
    s(
        "nvme-io",
        Part::Nvme,
        "NVM Command Set 1.1: Read, Write, FUA, Flush, ONCS commands",
        "Reads and writes with FUA, past and across the end, Flush, and the optional commands the controller reports (Compare, Write Zeroes, Dataset Management).",
    ),
    s(
        "nvme-ana",
        Part::Nvme,
        "NVMe Base 2.1 8.1.1: asymmetric namespace access",
        "Reads the ANA log page through every address and checks its groups and states.",
    ),
    s(
        "nvme-resv",
        Part::Nvme,
        "NVMe Base 2.1 8.1.22: reservations (Failover Clustering)",
        "Every reservation command and type with four hosts, and who may read and write under each.",
    ),
    s(
        "nvme-sess",
        Part::Nvme,
        "NVMe Base 2.1 Fabrics: repeated associations",
        "Connects and disconnects repeatedly and checks the controller comes back each time.",
    ),
    s(
        "windows",
        Part::Hosts,
        "What Windows MPIO, NTFS and clustering need of a disk",
        "Replays the commands Windows Server 2022 and Linux send bringing a disk online, taken from packet captures, and judges from the results above what MPIO, NTFS, TRIM and Failover Clustering need.",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_section_is_named_once_and_says_what_it_tests() {
        let mut names: Vec<&str> = SECTIONS.iter().map(|s| s.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), SECTIONS.len());
        assert!(SECTIONS
            .iter()
            .all(|s| !s.standard.is_empty() && !s.tests.is_empty()));
    }
}
