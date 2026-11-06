//! Where a clause spit cites lives in each edition it can judge against. Cases cite the latest
//! editions held, SPC-6 and SBC-5; a report at an older SCSI level cites the same clause in that
//! level's editions instead (`citation`). Each entry maps an SPC-6 clause, table or annex to SPC-3,
//! SPC-4 and SPC-5, or an SBC-5 one to SBC-3 and SBC-4, with None where that edition has no such
//! clause (the command, page or rule came later). The entries were matched by clause title and parent
//! title in each edition's contents, then checked by hand where titles changed (SPC-3 calls REPORT
//! IDENTIFYING INFORMATION "REPORT DEVICE IDENTIFIER"; SPC-4 has LID1 and LID4 forms of the copy
//! commands, mapped to the LID4 form SPC-5 kept). Only clause numbers are held, never their text.

/// SPC-6 clause → [SPC-3, SPC-4, SPC-5].
#[rustfmt::skip]
pub(super) const SPC: [(&str, [Option<&str>; 3]); 98] = [
    ("4.2.5", [Some("4.3.4"), Some("4.2.5"), Some("4.2.5")]), // Common CDB fields
    ("4.2.5.1", [Some("4.3.4.1"), Some("4.2.5.1"), Some("4.2.5.1")]), // Operation code
    ("4.2.5.2", [Some("4.3.4.2"), Some("4.2.5.2"), Some("4.2.5.2")]), // Service action
    ("4.2.5.6", [Some("4.3.4.6"), Some("4.2.5.6"), Some("4.2.5.6")]), // Allocation length
    ("4.3.1", [Some("4.4.1"), Some("4.3.1"), Some("4.3.1")]), // ASCII data field requirements
    ("5.14", [Some("5.6"), Some("5.12"), Some("5.14")]), // Reservations
    ("5.14.1", [Some("5.6.1"), Some("5.12.1"), Some("5.14.1")]), // Persistent Reservations overview
    ("5.14.3", [Some("5.6.3"), Some("5.12.3"), Some("5.14.3")]), // Exceptions to SPC-2 RESERVE and RELEASE behavior
    ("5.14.6", [Some("5.6.5"), Some("5.12.6"), Some("5.14.6")]), // Finding persistent reservations and reservation keys
    ("5.14.7", [Some("5.6.6"), Some("5.12.7"), Some("5.14.7")]), // Registering
    ("5.14.9", [Some("5.6.8"), Some("5.12.9"), Some("5.14.9")]), // Reserving
    ("5.14.11", [Some("5.6.10"), Some("5.12.11"), Some("5.14.11")]), // Releasing persistent reservations and removing registrations
    ("5.14.11.2.2", [Some("5.6.10.2"), Some("5.12.11.2.2"), Some("5.14.11.2.2")]), // Releasing
    ("5.14.11.2.3", [Some("5.6.10.3"), Some("5.12.11.2.3"), Some("5.14.11.2.3")]), // Unregistering
    ("5.14.11.2.4", [Some("5.6.10.4"), Some("5.12.11.2.4"), Some("5.14.11.2.4")]), // Preempting
    ("5.14.11.2.7", [Some("5.6.10.6"), Some("5.12.11.2.7"), Some("5.14.11.2.7")]), // Clearing
    ("5.18", [Some("5.8"), Some("5.15"), Some("5.17")]), // Target port group asymmetric access states
    ("5.18.2.5.4", [Some("5.8.2.4.4"), Some("5.15.2.4.4"), Some("5.17.2.5.4")]), // Standby state
    ("5.18.2.6", [Some("5.8.2.5"), Some("5.15.2.5"), Some("5.17.2.6")]), // Transitions between target port asymmetric access states
    ("6.3", [Some("6.2"), Some("6.2"), Some("6.3")]), // CHANGE ALIASES command
    ("6.4", [None, Some("6.3"), Some("6.4")]), // COPY OPERATION ABORT command
    ("6.6", [None, Some("6.4"), Some("6.6")]), // EXTENDED COPY command
    ("6.7", [Some("6.4"), Some("6.6"), Some("6.7")]), // INQUIRY command
    ("6.7.1", [Some("6.4.1"), Some("6.6.1"), Some("6.7.1")]), // INQUIRY command introduction
    ("6.7.2", [Some("6.4.2"), Some("6.6.2"), Some("6.7.2")]), // Standard INQUIRY data
    ("6.8", [Some("6.5"), Some("6.7"), Some("6.8")]), // LOG SELECT command
    ("6.9", [Some("6.6"), Some("6.8"), Some("6.9")]), // LOG SENSE command
    ("6.11", [None, Some("6.10"), Some("6.11")]), // MANAGEMENT PROTOCOL OUT command
    ("6.12", [Some("6.8"), Some("6.12"), Some("6.13")]), // MODE SELECT(10) command
    ("6.13", [Some("6.10"), Some("6.14"), Some("6.15")]), // MODE SENSE(10) command
    ("6.14", [Some("6.11"), Some("6.15"), Some("6.16")]), // PERSISTENT RESERVE IN command
    ("6.14.2", [Some("6.11.2"), Some("6.15.2"), Some("6.16.2")]), // READ KEYS service action
    ("6.14.3", [Some("6.11.3"), Some("6.15.3"), Some("6.16.3")]), // READ RESERVATION service action
    ("6.14.3.3", [Some("6.11.3.4"), Some("6.15.3.3"), Some("6.16.3.3")]), // Persistent reservations type
    ("6.14.4", [Some("6.11.4"), Some("6.15.4"), Some("6.16.4")]), // REPORT CAPABILITIES service action
    ("6.14.5", [Some("6.11.5"), Some("6.15.5"), Some("6.16.5")]), // READ FULL STATUS service action
    ("6.15", [Some("6.12"), Some("6.16"), Some("6.17")]), // PERSISTENT RESERVE OUT command
    ("6.15.2", [Some("6.12.2"), Some("6.16.2"), Some("6.17.2")]), // PERSISTENT RESERVE OUT service actions and parameter list fo
    ("6.15.3", [Some("6.12.3"), Some("6.16.3"), Some("6.17.3")]), // Basic PERSISTENT RESERVE OUT parameter list
    ("6.15.4", [Some("6.12.4"), Some("6.16.4"), Some("6.17.4")]), // ?
    ("6.17", [Some("6.14"), Some("6.17"), Some("6.19")]), // READ ATTRIBUTE command
    ("6.18", [Some("6.15"), Some("6.18"), Some("6.20")]), // READ BUFFER(10) command
    ("6.18.4", [Some("6.15.5"), Some("6.18.4"), Some("6.20.4")]), // Descriptor mode (03h)
    ("6.19", [None, None, Some("6.21")]), // READ BUFFER(16) command
    ("6.20", [Some("6.16"), Some("6.19"), Some("6.22")]), // READ MEDIA SERIAL NUMBER command
    ("6.22", [None, Some("6.20"), Some("6.24")]), // RECEIVE COPY DATA command
    ("6.23", [None, Some("6.24"), Some("6.25")]), // RECEIVE COPY STATUS command
    ("6.24", [Some("6.18"), Some("6.27"), Some("6.26")]), // RECEIVE DIAGNOSTIC RESULTS command
    ("6.25", [None, Some("6.28"), Some("6.27")]), // RECEIVE ROD TOKEN INFORMATION command
    ("6.26", [None, None, Some("6.28")]), // REMOVE I_T NEXUS command
    ("6.27", [Some("6.19"), Some("6.30"), Some("6.29")]), // REPORT ALIASES command
    ("6.28", [None, Some("6.31"), Some("6.30")]), // REPORT ALL ROD TOKENS command
    ("6.29", [Some("6.20"), Some("6.32"), Some("6.31")]), // REPORT IDENTIFYING INFORMATION command
    ("6.30", [Some("6.21"), Some("6.33"), Some("6.32")]), // REPORT LUNS command
    ("6.31", [Some("6.22"), Some("6.34"), Some("6.33")]), // REPORT PRIORITY command
    ("6.32", [Some("6.23"), Some("6.35"), Some("6.34")]), // REPORT SUPPORTED OPERATION CODES command
    ("6.32.2", [Some("6.23.2"), Some("6.35.2"), Some("6.34.2")]), // All_commands parameter data format
    ("6.32.3", [Some("6.23.3"), Some("6.35.3"), Some("6.34.3")]), // One_command parameter data format
    ("6.33", [Some("6.24"), Some("6.36"), Some("6.35")]), // REPORT SUPPORTED TASK MANAGEMENT FUNCTIONS command
    ("6.34", [Some("6.25"), Some("6.37"), Some("6.36")]), // REPORT TARGET PORT GROUPS command
    ("6.35", [Some("6.26"), Some("6.38"), Some("6.37")]), // REPORT TIMESTAMP command
    ("6.36", [Some("6.27"), Some("6.39"), Some("6.38")]), // REQUEST SENSE command
    ("6.37", [None, Some("6.40"), Some("6.39")]), // SECURITY PROTOCOL IN command
    ("6.38", [None, Some("6.41"), Some("6.40")]), // SECURITY PROTOCOL OUT command
    ("6.39", [Some("6.28"), Some("6.42"), Some("6.41")]), // SEND DIAGNOSTIC command
    ("6.41", [Some("6.29"), Some("6.43"), Some("6.43")]), // SET IDENTIFYING INFORMATION command
    ("6.42", [Some("6.30"), Some("6.44"), Some("6.44")]), // SET PRIORITY command
    ("6.43", [Some("6.31"), Some("6.45"), Some("6.45")]), // SET TARGET PORT GROUPS command
    ("6.44", [Some("6.32"), Some("6.46"), Some("6.46")]), // SET TIMESTAMP command
    ("6.46", [Some("6.33"), Some("6.47"), Some("6.48")]), // TEST UNIT READY command
    ("6.48", [Some("6.34"), Some("6.48"), Some("6.50")]), // WRITE ATTRIBUTE command
    ("6.49", [Some("6.35"), Some("6.49"), Some("6.51")]), // WRITE BUFFER command
    ("7.5.3", [Some("7.6.6"), Some("7.5.2"), Some("7.5.3")]), // Mode page policies
    ("7.5.6", [Some("7.4.3"), Some("7.5.5"), Some("7.5.6")]), // Mode parameter header format
    ("7.5.8", [Some("7.4.5"), Some("7.5.7"), Some("7.5.8")]), // Mode page and subpage formats and page codes
    ("7.6.4.5", [Some("7.5.4.6"), Some("7.6.4.6"), Some("7.6.4.5")]), // TransportID for initiator ports using SCSI over iSCSI
    ("7.7", [Some("7.6"), Some("7.8"), Some("7.7")]), // Vital product data parameters
    ("7.7.6", [Some("7.6.3"), Some("7.8.6"), Some("7.7.6")]), // Device Identification VPD page
    ("7.7.6.6", [Some("7.6.3.6"), Some("7.8.6.6"), Some("7.7.6.6")]), // NAA designator format
    ("7.7.7", [Some("7.6.4"), Some("7.8.7"), Some("7.7.7")]), // Extended INQUIRY Data VPD page
    ("7.7.14", [None, None, Some("7.7.14")]), // SCSI Feature Sets VPD page
    ("7.7.17", [Some("7.6.9"), Some("7.8.16"), Some("7.7.17")]), // Supported VPD Pages VPD page
    ("7.7.18.4", [None, Some("7.8.17.4"), Some("7.7.18.4")]), // Supported Commands third-party copy descriptor
    ("7.7.19", [Some("7.6.10"), Some("7.8.18"), Some("7.7.19")]), // Unit Serial Number VPD page
    ("A.2", [None, None, Some("A.2")]), // Discovery 2016 feature set
    ("annex A", [None, None, Some("annex A")]), // ?
    ("table 88", [Some("table 44"), Some("table 120"), Some("table 88")]), // Commands for all device types
    ("table 148", [Some("table 81"), Some("table 176"), Some("table 148")]), // Standard INQUIRY data
    ("table 152", [Some("6.4.2"), Some("table 179"), Some("table 151")]), // VERSION field
    ("table 257", [Some("6.23"), Some("table 295"), Some("table 258")]), // REPORT SUPPORTED OPERATION CODES REPORTING OPTIONS field
    ("table 267", [None, Some("table 303"), Some("table 268")]), // REPORT SUPPORTED TASK MANAGEMENT FUNCTIONS extended paramete
    ("table 268", [Some("table 162"), Some("table 304"), Some("table 269")]), // REPORT TARGET PORT GROUPS command
    ("table 272", [Some("table 164"), Some("table 308"), Some("table 273")]), // Target port group descriptor
    ("table 298", [Some("table 181"), Some("table 332"), Some("table 299")]), // ASYMMETRIC ACCESS STATE field
    ("table 461", [Some("table 245"), Some("table 459"), Some("table 452")]), // Control mode page
    ("table 525", [Some("7.6.3.1"), Some("table 590"), Some("table 515")]), // Designation descriptor
    ("table 553", [Some("table 318"), Some("table 615"), Some("table 543")]), // Extended INQUIRY Data VPD page
    ("table 572", [None, None, Some("table 562")]), // SCSI Feature Sets VPD page
];

/// SBC-5 clause → [SBC-3, SBC-4].
#[rustfmt::skip]
pub(super) const SBC: [(&str, [Option<&str>; 2]); 92] = [
    ("4.5", [Some("4.5"), Some("4.5")]), // Logical blocks
    ("4.7", [Some("4.7"), Some("4.7")]), // Logical block provisioning
    ("4.7.3.1", [Some("4.7.3.1"), Some("4.7.3.1")]), // Logical block provisioning management overview
    ("4.7.3.4", [Some("4.7.3.4"), Some("4.7.3.4")]), // Unmapping LBAs
    ("4.7.4.5", [Some("4.7.4.6"), Some("4.7.4.5")]), // LBP1:Mapped state
    ("4.21.2", [Some("4.22.2"), Some("4.21.2")]), // Protection types
    ("4.21.2.4", [Some("4.22.2.4"), Some("4.21.2.4")]), // Type 2 protection
    ("4.28", [Some("4.30"), Some("4.28")]), // Block device ROD token operations
    ("5.2", [None, Some("5.2")]), // BACKGROUND CONTROL command
    ("5.3", [Some("5.2"), Some("5.3")]), // COMPARE AND WRITE command
    ("5.4", [Some("5.3"), Some("5.4")]), // FORMAT UNIT command
    ("5.5", [None, Some("5.5")]), // FORMAT WITH PRESET command
    ("5.6", [Some("5.4"), Some("5.6")]), // GET LBA STATUS (16) command
    ("5.7", [None, Some("5.7")]), // GET LBA STATUS (32) command
    ("5.8", [None, Some("5.8")]), // GET PHYSICAL ELEMENT STATUS command
    ("5.9", [None, Some("5.9")]), // GET STREAM STATUS command
    ("5.10", [Some("5.5"), Some("5.10")]), // ORWRITE (16) command
    ("5.11", [Some("5.6"), Some("5.11")]), // ORWRITE (32) command
    ("5.12", [Some("5.7"), Some("5.12")]), // POPULATE TOKEN command
    ("5.13", [Some("5.8"), Some("5.13")]), // PRE-FETCH (10) command
    ("5.14", [Some("5.9"), Some("5.14")]), // PRE-FETCH (16) command
    ("5.15", [Some("5.10"), Some("5.15")]), // PREVENT ALLOW MEDIUM REMOVAL command
    ("5.16", [Some("5.11"), Some("5.16")]), // READ (10) command
    ("5.17", [Some("5.12"), Some("5.17")]), // READ (12) command
    ("5.18", [Some("5.13"), Some("5.18")]), // READ (16) command
    ("5.19", [Some("5.14"), Some("5.19")]), // READ (32) command
    ("5.20", [Some("5.15"), Some("5.20")]), // READ CAPACITY (10) command
    ("5.20.2", [Some("5.15.2"), Some("5.20.2")]), // READ CAPACITY (10) parameter data
    ("5.21", [Some("5.16"), Some("5.21")]), // READ CAPACITY (16) command
    ("5.21.2", [Some("5.16.2"), Some("5.21.2")]), // READ CAPACITY (16) parameter data
    ("5.22", [Some("5.17"), Some("5.22")]), // READ DEFECT DATA (10) command
    ("5.23", [Some("5.18"), Some("5.23")]), // READ DEFECT DATA (12) command
    ("5.24", [Some("5.21"), Some("5.24")]), // REASSIGN BLOCKS command
    ("5.26", [None, Some("5.26")]), // REMOVE ELEMENT AND TRUNCATE command
    ("5.27", [None, Some("5.27")]), // REPORT PROVISIONING INITIALIZATION PATTERN command
    ("5.28", [Some("5.23"), Some("5.28")]), // REPORT REFERRALS command
    ("5.29", [None, Some("5.29")]), // RESTORE ELEMENTS AND REBUILD command
    ("5.30", [Some("5.24"), Some("5.30")]), // SANITIZE command
    ("5.31", [Some("5.25"), Some("5.31")]), // START STOP UNIT command
    ("5.32", [None, Some("5.32")]), // STREAM CONTROL command
    ("5.33", [Some("5.26"), Some("5.33")]), // SYNCHRONIZE CACHE (10) command
    ("5.34", [Some("5.27"), Some("5.34")]), // SYNCHRONIZE CACHE (16) command
    ("5.35", [Some("5.28"), Some("5.35")]), // UNMAP command
    ("5.35.2", [Some("5.28.2"), Some("5.35.2")]), // UNMAP parameter list
    ("5.36", [Some("5.29"), Some("5.36")]), // VERIFY (10) command
    ("5.37", [Some("5.30"), Some("5.37")]), // VERIFY (12) command
    ("5.38", [Some("5.31"), Some("5.38")]), // VERIFY (16) command
    ("5.39", [Some("5.32"), Some("5.39")]), // VERIFY (32) command
    ("5.40", [Some("5.33"), Some("5.40")]), // WRITE (10) command
    ("5.41", [Some("5.34"), Some("5.41")]), // WRITE (12) command
    ("5.42", [Some("5.35"), Some("5.42")]), // WRITE (16) command
    ("5.43", [Some("5.36"), Some("5.43")]), // WRITE (32) command
    ("5.44", [Some("5.37"), Some("5.44")]), // WRITE AND VERIFY (10) command
    ("5.45", [Some("5.38"), Some("5.45")]), // WRITE AND VERIFY (12) command
    ("5.46", [Some("5.39"), Some("5.46")]), // WRITE AND VERIFY (16) command
    ("5.47", [Some("5.40"), Some("5.47")]), // WRITE AND VERIFY (32) command
    ("5.48", [None, Some("5.48")]), // WRITE ATOMIC (16) command
    ("5.49", [None, Some("5.49")]), // WRITE ATOMIC (32) command
    ("5.50", [Some("5.41"), Some("5.50")]), // WRITE LONG (10) command
    ("5.51", [Some("5.42"), Some("5.51")]), // WRITE LONG (16) command
    ("5.52", [Some("5.43"), Some("5.52")]), // WRITE SAME (10) command
    ("5.53", [Some("5.44"), Some("5.53")]), // WRITE SAME (16) command
    ("5.54", [Some("5.45"), Some("5.54")]), // WRITE SAME (32) command
    ("5.55", [None, Some("5.55")]), // WRITE SCATTERED (16) command
    ("5.56", [None, Some("5.56")]), // WRITE SCATTERED (32) command
    ("5.57", [None, Some("5.57")]), // WRITE STREAM (16) command
    ("5.58", [None, Some("5.58")]), // WRITE STREAM (32) command
    ("5.59", [Some("5.46"), Some("5.59")]), // WRITE USING TOKEN command
    ("6.5", [Some("6.5"), Some("6.5")]), // Mode parameters
    ("6.6", [Some("6.6"), Some("6.6")]), // Vital product data (VPD) parameters
    ("6.6.2", [Some("6.6.2"), Some("6.6.2")]), // Block Device Characteristics VPD page
    ("6.6.4", [Some("6.6.3"), Some("6.6.4")]), // Block Limits VPD page
    ("6.6.9", [Some("6.6.4"), Some("6.6.7")]), // Logical Block Provisioning VPD page
    ("A.2", [None, Some("H.2")]), // SBC Base 2010 feature set
    ("A.3", [None, Some("H.3")]), // SBC Base 2016 feature set
    ("A.4", [None, Some("H.4")]), // Basic Provisioning 2016 feature set
    ("A.5", [None, Some("H.5")]), // Drive Maintenance 2016 feature set
    ("annex A", [None, Some("annex H")]), // ?
    ("table 13", [Some("table 12"), Some("table 13")]), // SBC-5 commands that are allowed in the presence of various r
    ("table 33", [Some("table 209"), Some("table 33")]), // Transfer limits for commands
    ("table 34", [Some("table 32"), Some("table 34")]), // Commands for direct access block devices
    ("table 50", [Some("table 43"), Some("table 50")]), // GET LBA STATUS parameter data
    ("table 74", [Some("table 52"), Some("table 72")]), // POPULATE TOKEN parameter list
    ("table 80", [Some("table 58"), Some("table 78")]), // READ (10) command
    ("table 85", [Some("table 62"), Some("table 83")]), // READ (32) command
    ("table 89", [Some("table 66"), Some("table 87")]), // READ CAPACITY (16) parameter data
    ("table 150", [Some("table 121"), Some("table 148")]), // WRITE SAME (16) command
    ("table 161", [Some("table 125"), Some("table 159")]), // WRITE USING TOKEN parameter list
    ("table 231", [Some("table 180"), Some("table 230")]), // DEVICE-SPECIFIC PARAMETER field for direct access block devi
    ("table 239", [Some("table 186"), Some("table 238")]), // Caching mode page
    ("table 269", [Some("table 208"), Some("table 269")]), // Block Limits VPD page
    ("table 281", [Some("table 210"), Some("table 277")]), // Logical Block Provisioning VPD page
];

/// SPC-5 clauses of commands SPC-6 made obsolete → [SPC-3, SPC-4]; at levels 5 and 6 they stay
/// SPC-5's.
pub(super) const SPC5: [(&str, [Option<&str>; 2]); 2] = [
    ("6.12", [Some("6.7"), Some("6.11")]), // MODE SELECT(6) command
    ("6.14", [Some("6.9"), Some("6.13")]), // MODE SENSE(6) command
];
