//! The sections of `--compliance` in the order they run, each with the standard it tests against
//! and, where Windows depends on it, the Windows component that sends those commands. Every case
//! names one of these sections; the score and the report list them in this order.

/// (section, standard and clause it tests).
pub const SECTIONS: [(&str, &str); 26] = [
    (
        "login",
        "RFC 7143 6.3 and 13: login phase and key negotiation",
    ),
    (
        "identity",
        "SPC-6 INQUIRY, VPD 80h and 83h: one LUN on every path",
    ),
    (
        "scsi",
        "SPC-6, SBC-5: identity, capacity and target port groups",
    ),
    ("chap", "RFC 7143 12.1: CHAP authentication"),
    ("session", "RFC 7143: sessions, NOP-Out and reinstatement"),
    (
        "spc",
        "SPC-6 Table 88: mandatory commands and invalid opcodes",
    ),
    (
        "sbc",
        "SBC-3/4/5 command tables: the block command set (--sbc)",
    ),
    (
        "features",
        "SPC-6 Annex A, SBC-4 Annex H, SBC-5 Annex A: feature sets",
    ),
    (
        "disk",
        "SPC-6, SBC-5: what Windows and Linux ask of a new disk",
    ),
    (
        "io",
        "SBC-5: reads, writes, FUA, VERIFY, cache flush (NTFS)",
    ),
    (
        "unmap",
        "SBC-5 4.7: logical block provisioning, UNMAP (TRIM)",
    ),
    ("ats", "SBC-5 5.3: COMPARE AND WRITE"),
    ("odx", "SBC-5 4.28, SPC-6: ROD token copy (Windows ODX)"),
    (
        "pr",
        "SPC-6 5.14: persistent reservations (Failover Clustering)",
    ),
    (
        "tmf",
        "SAM-6 7, RFC 7143 11.5: task management (MPIO recovery)",
    ),
    (
        "mpio",
        "SPC-6 5.18: ALUA target port groups (Windows MSDSM)",
    ),
    ("nvme-disc", "NVMe Base 2.1 5.1.12.3: discovery log page"),
    (
        "nvme-conn",
        "NVMe Base 2.1 Fabrics: Connect and controller IDs",
    ),
    (
        "nvme-prop",
        "NVMe Base 2.1: CAP, VS, CC and CSTS properties",
    ),
    (
        "nvme-ident",
        "NVMe Base 2.1 5.1.13: Identify, the same IDs on every path",
    ),
    (
        "nvme-admin",
        "NVMe Base 2.1: log pages, features, Abort, Keep Alive",
    ),
    (
        "nvme-io",
        "NVM Command Set 1.1: Read, Write, FUA, Flush, ONCS commands",
    ),
    (
        "nvme-ana",
        "NVMe Base 2.1 8.1.1: asymmetric namespace access",
    ),
    (
        "nvme-resv",
        "NVMe Base 2.1 8.1.22: reservations (Failover Clustering)",
    ),
    ("nvme-sess", "NVMe Base 2.1 Fabrics: repeated associations"),
    (
        "windows",
        "What Windows MPIO, NTFS and clustering need of a disk",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_section_is_named_once_and_cites_a_standard() {
        let mut names: Vec<&str> = SECTIONS.iter().map(|(s, _)| *s).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), SECTIONS.len());
        assert!(SECTIONS.iter().all(|(_, spec)| !spec.is_empty()));
    }
}
