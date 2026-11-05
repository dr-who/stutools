//! The block device command set as a table: every command of SBC-5 table 34 (the same rows as SBC-4
//! table 34, plus the ones either edition dropped), with the SBC editions that list it, the SPC editions
//! that define it when SPC does, the clause, the hosts that send it, and whether spit sends it. `--sbc N`
//! picks the edition the `sbc` section judges against, and only through `Row::rule`; nothing else in
//! spit looks at the edition.
//!
//! Where the editions come from: the command tables of SBC-3 (table 32), SBC-4 and SBC-5 (table 34)
//! and SPC-3 (table 44) to SPC-6, read from the texts once and pasted here (`min` is the first held
//! edition that lists a command, `max` the last when a later one dropped it). SBC-3 and SPC-3 are the
//! oldest editions held. SPC-4 calls the copy commands "(LID1)" and "(LID4)"; those are mapped to the
//! names SPC-5 uses. PREVENT ALLOW MEDIUM REMOVAL moved from SPC-3 to SBC and is judged as an SBC row.
//! RESERVE (6) and RELEASE (6) are in no edition held (SPC-2). SPC rows are judged against SPC-6.

/// The latest SBC and SPC editions held.
pub(super) const SBC_LATEST: u8 = 5;
pub(super) const SPC_LATEST: u8 = 6;

/// The editions of one standard that define a command.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Span {
    /// The first edition that defines it.
    pub min: u8,
    /// The last edition that still defines it; None while the latest does.
    pub max: Option<u8>,
    /// The first edition in which it is mandatory, for the M rows.
    pub mandatory_from: Option<u8>,
}

impl Span {
    pub(super) fn includes(&self, edition: u8, latest: u8) -> bool {
        self.min <= edition && edition <= self.max.unwrap_or(latest)
    }
}

const fn span(min: u8, max: Option<u8>) -> Option<Span> {
    Some(Span {
        min,
        max,
        mandatory_from: None,
    })
}

const fn mandatory(min: u8) -> Option<Span> {
    Some(Span {
        min,
        max: None,
        mandatory_from: Some(min),
    })
}

/// Whether spit sends a command.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Sending {
    /// Sent, by the `sbc` section or by the section that checks it in depth.
    Yes,
    /// Never sent, for the reason given; only REPORT SUPPORTED OPERATION CODES is asked about it.
    Never(&'static str),
}

/// One row of the command set.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Row {
    pub name: &'static str,
    pub opcode: u8,
    pub service_action: Option<u16>,
    /// The SBC editions whose table 34 lists it.
    pub sbc: Option<Span>,
    /// The SPC editions that define it, for SPC commands.
    pub spc: Option<Span>,
    /// "SBC 5.18" (the same in SBC-4 and SBC-5) or "SPC-6 6.30".
    pub clause: &'static str,
    /// The hosts that send it, e.g. "Windows, macOS, Linux", or "" when no mainstream initiator does.
    pub hosts: &'static str,
    /// When a host sends it but copes without it: how.
    pub fallback: Option<&'static str>,
    pub send: Sending,
}

/// How a row is judged under one SBC edition.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Rule {
    /// Mandatory in the edition: it must work.
    Mandatory,
    /// A host sends it and has no fallback: it must work.
    Host,
    /// A host sends it and copes when it is refused: works, or a clean refusal (with how the host copes).
    HostFallback(&'static str),
    /// Optional: works correctly, or a clean refusal.
    Optional,
    /// Not in this edition (introduced later or obsolete): informational, works or a clean refusal.
    Informational(String),
    /// Never sent; REPORT SUPPORTED OPERATION CODES is recorded.
    NotSent(&'static str),
}

impl Row {
    /// Whether the SBC edition lists this row, and whether SPC still defines it.
    pub(super) fn in_sbc(&self, edition: u8) -> bool {
        self.sbc.is_some_and(|s| s.includes(edition, SBC_LATEST))
    }

    fn spc_current(&self) -> bool {
        self.spc.is_none_or(|s| s.includes(SPC_LATEST, SPC_LATEST))
    }

    /// The rule for this row under SBC edition `edition`.
    pub(super) fn rule(&self, edition: u8) -> Rule {
        if let Sending::Never(why) = self.send {
            return Rule::NotSent(why);
        }
        let mandatory = self.in_sbc(edition)
            && self
                .sbc
                .and_then(|s| s.mandatory_from)
                .is_some_and(|m| m <= edition);
        if mandatory {
            return Rule::Mandatory;
        }
        if !self.hosts.is_empty() {
            return match self.fallback {
                Some(how) => Rule::HostFallback(how),
                None => Rule::Host,
            };
        }
        if !self.in_sbc(edition) {
            return Rule::Informational(match self.sbc {
                Some(s) if s.min > edition => format!("introduced in SBC-{}", s.min),
                Some(s) => format!("obsolete after SBC-{}", s.max.unwrap_or(SBC_LATEST)),
                None => "not in SBC table 34".into(),
            });
        }
        if !self.spc_current() {
            return Rule::Informational(format!(
                "obsolete after SPC-{}",
                self.spc.and_then(|s| s.max).unwrap_or(SPC_LATEST)
            ));
        }
        Rule::Optional
    }

    /// A note for the case name when the edition does not list a command a host still sends.
    pub(super) fn edition_note(&self, edition: u8) -> Option<String> {
        if !self.hosts.is_empty() && (!self.in_sbc(edition) || !self.spc_current()) {
            Some(match (self.in_sbc(edition), self.spc) {
                (false, _) => format!("not in SBC-{edition}, still sent by hosts"),
                (true, Some(s)) => format!(
                    "obsolete after SPC-{}, still sent by hosts",
                    s.max.unwrap_or(SPC_LATEST)
                ),
                _ => "still sent by hosts".into(),
            })
        } else {
            None
        }
    }
}

const ALL: &str = "Windows, macOS, Linux";
const SANITIZING: &str = "destroys data";
const ZONED: &str = "a zoned block command (ZBC), out of scope";
const SCC: &str = "an SCC-2 storage array command, out of scope";
const STATE: &str = "changes device state";

macro_rules! row {
    ($name:expr, $op:expr, $sa:expr, $sbc:expr, $spc:expr, $clause:expr, $hosts:expr, $fallback:expr, $send:expr) => {
        Row {
            name: $name,
            opcode: $op,
            service_action: $sa,
            sbc: $sbc,
            spc: $spc,
            clause: $clause,
            hosts: $hosts,
            fallback: $fallback,
            send: $send,
        }
    };
}

use Sending::{Never, Yes};

/// The rows, in SBC-5 table 34 order, then the rows other editions had.
#[rustfmt::skip]
pub(super) const ROWS: &[Row] = &[
    row!("ATA PASS-THROUGH (12)", 0xa1, None, span(3, None), None, "SAT", "", None, Never("an ATA command set passthrough, out of scope") ),
    row!("ATA PASS-THROUGH (16)", 0x85, None, span(3, None), None, "SAT", "", None, Never("an ATA command set passthrough, out of scope") ),
    row!("BACKGROUND CONTROL", 0x9e, Some(0x15), span(4, None), None, "SBC 5.2", "", None, Yes ),
    row!("CHANGE ALIASES", 0xa4, Some(0x0b), span(3, None), span(3, None), "SPC-6 6.3", "", None, Never(STATE) ),
    row!("CLOSE ZONE", 0x94, Some(0x01), span(4, None), None, "ZBC", "", None, Never(ZONED) ),
    row!("COMPARE AND WRITE", 0x89, None, span(3, None), None, "SBC 5.3", "VMware ESXi", Some("ESXi falls back to SCSI-2 reservations without ATS"), Yes ),
    row!("COPY OPERATION ABORT", 0x83, Some(0x1c), span(4, None), span(4, None), "SPC-6 6.4", "", None, Never("needs a copy operation in progress") ),
    row!("EXTENDED COPY", 0x83, Some(0x01), span(4, None), span(4, None), "SPC-6 6.6", "", None, Never("copies data between devices") ),
    row!("FINISH ZONE", 0x94, Some(0x02), span(4, None), None, "ZBC", "", None, Never(ZONED) ),
    row!("FORMAT UNIT", 0x04, None, mandatory(3), None, "SBC 5.4", "", None, Never(SANITIZING) ),
    row!("FORMAT WITH PRESET", 0x38, None, span(4, None), None, "SBC 5.5", "", None, Never(SANITIZING) ),
    row!("GET LBA STATUS (16)", 0x9e, Some(0x12), span(3, None), None, "SBC 5.6", "Windows", Some("sent only when READ CAPACITY (16) sets LBPME"), Yes ),
    row!("GET LBA STATUS (32)", 0x7f, Some(0x0012), span(4, None), None, "SBC 5.7", "", None, Yes ),
    row!("GET PHYSICAL ELEMENT STATUS", 0x9e, Some(0x17), span(4, None), None, "SBC 5.8", "", None, Yes ),
    row!("GET STREAM STATUS", 0x9e, Some(0x16), span(4, None), None, "SBC 5.9", "", None, Yes ),
    row!("INQUIRY", 0x12, None, mandatory(3), span(3, None), "SPC-6 6.7", ALL, None, Yes ),
    row!("LOG SELECT", 0x4c, None, span(3, None), span(3, None), "SPC-6 6.8", "", None, Never("resets log parameters") ),
    row!("LOG SENSE", 0x4d, None, span(3, None), span(3, None), "SPC-6 6.9", "Windows, Linux", Some("only for health and provisioning pages"), Yes ),
    row!("MODE SELECT (10)", 0x55, None, span(3, None), span(3, None), "SPC-6 6.12", "Windows, Linux", Some("sent only when the write cache setting is changed"), Yes ),
    row!("MODE SENSE (10)", 0x5a, None, span(3, None), span(3, None), "SPC-6 6.13", ALL, None, Yes ),
    row!("OPEN ZONE", 0x94, Some(0x03), span(4, None), None, "ZBC", "", None, Never(ZONED) ),
    row!("ORWRITE (16)", 0x8b, None, span(3, None), None, "SBC 5.10", "", None, Yes ),
    row!("ORWRITE (32)", 0x7f, Some(0x000e), span(3, None), None, "SBC 5.11", "", None, Yes ),
    row!("PERSISTENT RESERVE IN", 0x5e, None, span(3, None), span(3, None), "SPC-6 6.14", "Windows, Linux, VMware ESXi", Some("only clustering and cluster file systems send it"), Yes ),
    row!("PERSISTENT RESERVE OUT", 0x5f, None, span(3, None), span(3, None), "SPC-6 6.15", "Windows, Linux, VMware ESXi", Some("only clustering and cluster file systems send it"), Yes ),
    row!("POPULATE TOKEN", 0x83, Some(0x10), span(3, None), None, "SBC 5.12", "Windows", Some("sent only when VPD 8Fh advertises ODX"), Yes ),
    row!("PRE-FETCH (10)", 0x34, None, span(3, None), None, "SBC 5.13", "", None, Yes ),
    row!("PRE-FETCH (16)", 0x90, None, span(3, None), None, "SBC 5.14", "", None, Yes ),
    row!("PREVENT ALLOW MEDIUM REMOVAL", 0x1e, None, span(3, None), None, "SBC 5.15", "macOS", Some("macOS ignores a refusal on a fixed disk"), Yes ),
    row!("READ (10)", 0x28, None, mandatory(3), None, "SBC 5.16", ALL, None, Yes ),
    row!("READ (12)", 0xa8, None, span(3, None), None, "SBC 5.17", "", None, Yes ),
    row!("READ (16)", 0x88, None, mandatory(3), None, "SBC 5.18", ALL, None, Yes ),
    row!("READ (32)", 0x7f, Some(0x0009), span(3, None), None, "SBC 5.19", "", None, Yes ),
    row!("READ ATTRIBUTE", 0x8c, None, span(3, None), span(3, None), "SPC-6 6.17", "", None, Yes ),
    row!("READ BUFFER (10)", 0x3c, None, span(3, None), span(3, None), "SPC-6 6.18", "", None, Yes ),
    row!("READ BUFFER (16)", 0x9b, None, span(4, None), span(5, None), "SPC-6 6.19", "", None, Yes ),
    row!("READ CAPACITY (10)", 0x25, None, mandatory(3), None, "SBC 5.20", ALL, None, Yes ),
    row!("READ CAPACITY (16)", 0x9e, Some(0x10), mandatory(3), None, "SBC 5.21", ALL, None, Yes ),
    row!("READ DEFECT DATA (10)", 0x37, None, span(3, None), None, "SBC 5.22", "", None, Yes ),
    row!("READ DEFECT DATA (12)", 0xb7, None, span(3, None), None, "SBC 5.23", "", None, Yes ),
    row!("READ MEDIA SERIAL NUMBER", 0xab, Some(0x01), span(4, None), span(3, None), "SPC-6 6.20", "", None, Yes ),
    row!("REASSIGN BLOCKS", 0x07, None, span(3, None), None, "SBC 5.24", "", None, Never("reallocates blocks") ),
    row!("RECEIVE COPY DATA", 0x84, Some(0x06), span(4, None), span(4, None), "SPC-6 6.22", "", None, Never("needs a copy operation") ),
    row!("RECEIVE COPY STATUS", 0x84, Some(0x05), span(4, None), span(4, None), "SPC-6 6.23", "", None, Never("needs a copy operation") ),
    row!("RECEIVE DIAGNOSTIC RESULTS", 0x1c, None, span(3, None), span(3, None), "SPC-6 6.24", "", None, Yes ),
    row!("RECEIVE ROD TOKEN INFORMATION", 0x84, Some(0x07), span(3, None), span(4, None), "SPC-6 6.25", "Windows", Some("sent only when VPD 8Fh advertises ODX"), Yes ),
    row!("REDUNDANCY GROUP IN", 0xba, None, span(3, None), None, "SCC-2", "", None, Never(SCC) ),
    row!("REDUNDANCY GROUP OUT", 0xbb, None, span(3, None), None, "SCC-2", "", None, Never(SCC) ),
    row!("REMOVE ELEMENT AND MODIFY ZONES", 0x9e, Some(0x1a), span(5, None), None, "ZBC", "", None, Never(ZONED) ),
    row!("REMOVE ELEMENT AND TRUNCATE", 0x9e, Some(0x18), span(4, None), None, "SBC 5.26", "", None, Never(SANITIZING) ),
    row!("REMOVE I_T NEXUS", 0xa4, Some(0x0c), span(3, None), span(4, None), "SPC-6 6.26", "", None, Never("drops other sessions") ),
    row!("REPORT ALIASES", 0xa3, Some(0x0b), span(3, None), span(3, None), "SPC-6 6.27", "", None, Yes ),
    row!("REPORT ALL ROD TOKENS", 0x84, Some(0x08), span(4, None), span(4, None), "SPC-6 6.28", "", None, Yes ),
    row!("REPORT IDENTIFYING INFORMATION", 0xa3, Some(0x05), span(3, None), span(4, None), "SPC-6 6.29", "", None, Yes ),
    row!("REPORT LUNS", 0xa0, None, mandatory(3), span(3, None), "SPC-6 6.30", ALL, None, Yes ),
    row!("REPORT PRIORITY", 0xa3, Some(0x0e), span(3, None), span(3, None), "SPC-6 6.31", "", None, Yes ),
    row!("REPORT PROVISIONING INITIALIZATION PATTERN", 0xa3, Some(0x1d), span(4, None), None, "SBC 5.27", "", None, Yes ),
    row!("REPORT REALMS", 0x95, Some(0x06), span(4, None), None, "ZBC", "", None, Never(ZONED) ),
    row!("REPORT REFERRALS", 0x9e, Some(0x13), span(3, None), None, "SBC 5.28", "", None, Yes ),
    row!("REPORT SUPPORTED OPERATION CODES", 0xa3, Some(0x0c), span(3, None), span(3, None), "SPC-6 6.32", "Windows, Linux", Some("hosts probe the commands instead"), Yes ),
    row!("REPORT SUPPORTED TASK MANAGEMENT FUNCTIONS", 0xa3, Some(0x0d), span(3, None), span(3, None), "SPC-6 6.33", "", None, Yes ),
    row!("REPORT TARGET PORT GROUPS", 0xa3, Some(0x0a), span(3, None), span(3, None), "SPC-6 6.34", "Windows, Linux, VMware ESXi", Some("sent only when TPGS is set"), Yes ),
    row!("REPORT TIMESTAMP", 0xa3, Some(0x0f), span(4, None), span(3, None), "SPC-6 6.35", "", None, Yes ),
    row!("REPORT ZONE DOMAINS", 0x95, Some(0x07), span(4, None), None, "ZBC", "", None, Never(ZONED) ),
    row!("REPORT ZONES", 0x95, Some(0x00), span(4, None), None, "ZBC", "", None, Never(ZONED) ),
    row!("REQUEST SENSE", 0x03, None, mandatory(3), span(3, None), "SPC-6 6.36", ALL, None, Yes ),
    row!("RESET WRITE POINTER", 0x94, Some(0x04), span(4, None), None, "ZBC", "", None, Never(ZONED) ),
    row!("RESTORE ELEMENTS AND REBUILD", 0x9e, Some(0x19), span(4, None), None, "SBC 5.29", "", None, Never(SANITIZING) ),
    row!("SANITIZE", 0x48, None, span(3, None), None, "SBC 5.30", "", None, Never(SANITIZING) ),
    row!("SECURITY PROTOCOL IN", 0xa2, None, span(3, None), span(4, None), "SPC-6 6.37", "Windows, Linux", Some("sent to find self-encrypting drives; refused means none"), Yes ),
    row!("SECURITY PROTOCOL OUT", 0xb5, None, span(3, None), span(4, None), "SPC-6 6.38", "", None, Never(STATE) ),
    row!("SEND DIAGNOSTIC", 0x1d, None, span(3, None), span(3, None), "SPC-6 6.39", "", None, Never("starts a self-test") ),
    row!("SET IDENTIFYING INFORMATION", 0xa4, Some(0x06), span(3, None), span(4, None), "SPC-6 6.41", "", None, Never(STATE) ),
    row!("SET PRIORITY", 0xa4, Some(0x0e), span(3, None), span(3, None), "SPC-6 6.42", "", None, Never(STATE) ),
    row!("SET TARGET PORT GROUPS", 0xa4, Some(0x0a), span(3, None), span(3, None), "SPC-6 6.43", "Windows, Linux", Some("sent only for explicit ALUA"), Yes ),
    row!("SET TIMESTAMP", 0xa4, Some(0x0f), span(4, None), span(3, None), "SPC-6 6.44", "", None, Never(STATE) ),
    row!("SPARE IN", 0xbc, None, span(3, None), None, "SCC-2", "", None, Never(SCC) ),
    row!("SPARE OUT", 0xbd, None, span(3, None), None, "SCC-2", "", None, Never(SCC) ),
    row!("START STOP UNIT", 0x1b, None, span(3, None), None, "SBC 5.31", ALL, Some("hosts ignore a refusal on a fixed disk"), Yes ),
    row!("STREAM CONTROL", 0x9e, Some(0x14), span(4, None), None, "SBC 5.32", "", None, Never("opens and closes streams") ),
    row!("SYNCHRONIZE CACHE (10)", 0x35, None, span(3, None), None, "SBC 5.33", ALL, None, Yes ),
    row!("SYNCHRONIZE CACHE (16)", 0x91, None, span(3, None), None, "SBC 5.34", "Linux", Some("Linux uses it only past 2 TiB"), Yes ),
    row!("TEST UNIT READY", 0x00, None, mandatory(3), span(3, None), "SPC-6 6.46", ALL, None, Yes ),
    row!("UNMAP", 0x42, None, span(3, None), None, "SBC 5.35", "Windows, Linux, VMware ESXi", Some("sent only when VPD B2h sets LBPU"), Yes ),
    row!("VERIFY (10)", 0x2f, None, span(3, None), None, "SBC 5.36", "Windows", Some("only surface scans (chkdsk /r) send it"), Yes ),
    row!("VERIFY (12)", 0xaf, None, span(3, None), None, "SBC 5.37", "", None, Yes ),
    row!("VERIFY (16)", 0x8f, None, span(3, None), None, "SBC 5.38", "Windows", Some("only surface scans (chkdsk /r) send it"), Yes ),
    row!("VERIFY (32)", 0x7f, Some(0x000a), span(3, None), None, "SBC 5.39", "", None, Yes ),
    row!("VOLUME SET IN", 0xbe, None, span(3, None), None, "SCC-2", "", None, Never(SCC) ),
    row!("VOLUME SET OUT", 0xbf, None, span(3, None), None, "SCC-2", "", None, Never(SCC) ),
    row!("WRITE (10)", 0x2a, None, span(3, None), None, "SBC 5.40", ALL, None, Yes ),
    row!("WRITE (12)", 0xaa, None, span(3, None), None, "SBC 5.41", "", None, Yes ),
    row!("WRITE (16)", 0x8a, None, span(3, None), None, "SBC 5.42", ALL, None, Yes ),
    row!("WRITE (32)", 0x7f, Some(0x000b), span(3, None), None, "SBC 5.43", "", None, Yes ),
    row!("WRITE AND VERIFY (10)", 0x2e, None, span(3, None), None, "SBC 5.44", "", None, Yes ),
    row!("WRITE AND VERIFY (12)", 0xae, None, span(3, None), None, "SBC 5.45", "", None, Yes ),
    row!("WRITE AND VERIFY (16)", 0x8e, None, span(3, None), None, "SBC 5.46", "", None, Yes ),
    row!("WRITE AND VERIFY (32)", 0x7f, Some(0x000c), span(3, None), None, "SBC 5.47", "", None, Yes ),
    row!("WRITE ATOMIC (16)", 0x9c, None, span(4, None), None, "SBC 5.48", "Linux 6.11+", Some("sent only when VPD B0h reports atomic limits"), Yes ),
    row!("WRITE ATOMIC (32)", 0x7f, Some(0x000f), span(4, None), None, "SBC 5.49", "", None, Yes ),
    row!("WRITE ATTRIBUTE", 0x8d, None, span(3, None), span(3, None), "SPC-6 6.48", "", None, Never(STATE) ),
    row!("WRITE BUFFER", 0x3b, None, span(3, None), span(3, None), "SPC-6 6.49", "", None, Never("can download microcode") ),
    row!("WRITE LONG (10)", 0x3f, None, span(3, None), None, "SBC 5.50", "", None, Never("makes blocks unreadable") ),
    row!("WRITE LONG (16)", 0x9f, Some(0x11), span(3, None), None, "SBC 5.51", "", None, Never("makes blocks unreadable") ),
    row!("WRITE SAME (10)", 0x41, None, span(3, None), None, "SBC 5.52", "Linux", Some("sent only when advertised (VPD B0h, B2h)"), Yes ),
    row!("WRITE SAME (16)", 0x93, None, span(3, None), None, "SBC 5.53", "Linux, VMware ESXi", Some("sent only when advertised (VPD B0h, B2h)"), Yes ),
    row!("WRITE SAME (32)", 0x7f, Some(0x000d), span(3, None), None, "SBC 5.54", "", None, Yes ),
    row!("WRITE SCATTERED (16)", 0x9f, Some(0x12), span(4, None), None, "SBC 5.55", "", None, Yes ),
    row!("WRITE SCATTERED (32)", 0x7f, Some(0x0011), span(4, None), None, "SBC 5.56", "", None, Yes ),
    row!("WRITE STREAM (16)", 0x9a, None, span(4, None), None, "SBC 5.57", "", None, Never("needs a stream opened by STREAM CONTROL") ),
    row!("WRITE STREAM (32)", 0x7f, Some(0x0010), span(4, None), None, "SBC 5.58", "", None, Never("needs a stream opened by STREAM CONTROL") ),
    row!("WRITE USING TOKEN", 0x83, Some(0x11), span(3, None), None, "SBC 5.59", "Windows", Some("sent only when VPD 8Fh advertises ODX"), Yes ),
    row!("ZONE ACTIVATE", 0x95, Some(0x08), span(4, None), None, "ZBC", "", None, Never(ZONED) ),
    row!("ZONE QUERY", 0x95, Some(0x09), span(4, None), None, "ZBC", "", None, Never(ZONED) ),
    // Listed by SBC-5 table 34, not by SBC-5 (SBC-5 clause 1).
    row!("MODE SELECT (6)", 0x15, None, span(3, Some(4)), span(3, Some(5)), "SPC-5 6.12", "Windows, Linux", Some("sent only when the write cache setting is changed"), Yes ),
    row!("MODE SENSE (6)", 0x1a, None, span(3, Some(4)), span(3, Some(5)), "SPC-5 6.14", ALL, None, Yes ),
    row!("SEQUENTIALIZE ZONE", 0x94, Some(0x10), span(4, Some(4)), None, "ZBC", "", None, Never(ZONED) ),
    // Made obsolete by SBC-4 (SBC-4 clause 1): in SBC-5 table 34 only.
    row!("ACCESS CONTROL IN", 0x86, None, span(3, Some(3)), span(3, Some(4)), "SBC-3", "", None, Never("obsolete since SBC-4") ),
    row!("ACCESS CONTROL OUT", 0x87, None, span(3, Some(3)), span(3, Some(4)), "SBC-3", "", None, Never("obsolete since SBC-4") ),
    row!("EXTENDED COPY (LID1)", 0x83, Some(0x00), span(3, Some(3)), span(3, Some(4)), "SBC-3", "VMware ESXi", Some("ESXi copies through the host without XCOPY"), Never("copies data between devices") ),
    row!("RECEIVE COPY RESULTS", 0x84, None, span(3, Some(3)), span(3, Some(3)), "SBC-3", "", None, Never("needs a copy operation") ),
    row!("READ LONG (10)", 0x3e, None, span(3, Some(3)), None, "SBC-3", "", None, Never("obsolete since SBC-4") ),
    row!("READ LONG (16)", 0x9e, Some(0x11), span(3, Some(3)), None, "SBC-3", "", None, Never("obsolete since SBC-4") ),
    row!("XDWRITEREAD (10)", 0x53, None, span(3, Some(3)), None, "SBC-3", "", None, Never("obsolete since SBC-4") ),
    row!("XPWRITE (10)", 0x51, None, span(3, Some(3)), None, "SBC-3", "", None, Never("obsolete since SBC-4") ),
    row!("XDWRITEREAD (32)", 0x7f, Some(0x0007), span(3, Some(3)), None, "SBC-3", "", None, Never("obsolete since SBC-4") ),
    row!("XPWRITE (32)", 0x7f, Some(0x0006), span(3, Some(3)), None, "SBC-3", "", None, Never("obsolete since SBC-4") ),
    // SPC-2 reservations, obsolete in SPC-4 to SPC-6 (SPC-6 table 88 note b): only Windows Server 2003
    // clusters send them.
    row!("RESERVE (6)", 0x16, None, None, None, "SPC-2", "Windows Server 2003 clusters", Some("only Windows Server 2003 clusters send it"), Yes ),
    row!("RELEASE (6)", 0x17, None, None, None, "SPC-2", "Windows Server 2003 clusters", Some("only Windows Server 2003 clusters send it"), Yes ),
];

/// The row named `name`.
pub(super) fn row(name: &str) -> Option<&'static Row> {
    ROWS.iter().find(|r| r.name == name)
}

/// The rows SBC edition `edition` lists.
#[cfg(test)]
fn edition(edition: u8) -> impl Iterator<Item = &'static Row> {
    ROWS.iter().filter(move |r| r.in_sbc(edition))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(name: &str) -> &'static Row {
        row(name).unwrap_or_else(|| panic!("no row {name}"))
    }

    #[test]
    fn rows_are_unique() {
        for (i, a) in ROWS.iter().enumerate() {
            assert!(
                ROWS[i + 1..].iter().all(|b| b.name != a.name),
                "{} twice",
                a.name
            );
            assert!(
                ROWS[i + 1..]
                    .iter()
                    .all(|b| (b.opcode, b.service_action) != (a.opcode, a.service_action)),
                "{} shares its code",
                a.name
            );
        }
    }

    #[test]
    fn membership_by_edition() {
        let mode_sense_6 = named("MODE SENSE (6)");
        assert!(mode_sense_6.in_sbc(4) && !mode_sense_6.in_sbc(5));
        let unmap = named("UNMAP");
        assert!([3, 4, 5].iter().all(|&n| unmap.in_sbc(n)));
        let atomic = named("WRITE ATOMIC (16)");
        assert!(atomic.in_sbc(4) && atomic.in_sbc(5) && !atomic.in_sbc(3));
        let read_long = named("READ LONG (16)");
        assert!(read_long.in_sbc(3) && !read_long.in_sbc(4) && !read_long.in_sbc(5));
        assert!(named("REMOVE ELEMENT AND MODIFY ZONES").in_sbc(5));
        assert!(!named("REMOVE ELEMENT AND MODIFY ZONES").in_sbc(4));
        assert!(named("SEQUENTIALIZE ZONE").in_sbc(4) && !named("SEQUENTIALIZE ZONE").in_sbc(5));
    }

    #[test]
    fn the_mandatory_set_is_the_same_in_every_edition() {
        let set = |n: u8| -> Vec<&str> {
            edition(n)
                .filter(|r| r.sbc.and_then(|s| s.mandatory_from).is_some_and(|m| m <= n))
                .map(|r| r.name)
                .collect()
        };
        let five = set(5);
        assert_eq!(five, set(4));
        assert_eq!(five, set(3));
        for name in [
            "FORMAT UNIT",
            "INQUIRY",
            "READ (10)",
            "READ (16)",
            "READ CAPACITY (10)",
            "READ CAPACITY (16)",
            "REPORT LUNS",
            "REQUEST SENSE",
            "TEST UNIT READY",
        ] {
            assert!(five.contains(&name), "{name}");
        }
        assert_eq!(five.len(), 9);
    }

    #[test]
    fn rules_follow_edition_and_hosts() {
        assert_eq!(named("READ (16)").rule(5), Rule::Mandatory);
        assert_eq!(named("FORMAT UNIT").rule(5), Rule::NotSent(SANITIZING));
        // A host sends MODE SENSE (6), so it stays strict where SBC-5 dropped it.
        assert_eq!(named("MODE SENSE (6)").rule(5), Rule::Host);
        assert!(named("MODE SENSE (6)").edition_note(5).is_some());
        assert!(
            named("MODE SENSE (6)").edition_note(4).is_some(),
            "SPC-6 obsoletes it"
        );
        assert_eq!(named("READ (12)").rule(5), Rule::Optional);
        assert!(matches!(
            named("BACKGROUND CONTROL").rule(3),
            Rule::Informational(_)
        ));
        assert_eq!(named("BACKGROUND CONTROL").rule(4), Rule::Optional);
        assert!(matches!(named("UNMAP").rule(5), Rule::HostFallback(_)));
        assert!(matches!(
            named("RESERVE (6)").rule(5),
            Rule::HostFallback(_)
        ));
    }
}
