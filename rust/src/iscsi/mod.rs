//! A user-space iSCSI initiator for spit, and the iSCSI side of `--compliance`. It speaks the protocol
//! directly, so spit can log in to a target without the operating system's initiator.
//!
//! - The initiator: `path` and `credentials` say where and as whom, `login` reaches full feature
//!   phase, `session` runs commands, `send_targets` discovers, `pdu` and `crc32c` are the wire
//!   format, `chap` the authentication; `scsi` and `ops` build the SCSI commands; `connection` puts
//!   sessions in the shared path pool.
//! - What spit prints without a workload: `discovery_listing` for HOST, `report` and `identity` for
//!   HOST/IQN.
//! - `--compliance`: `compliance` runs the login, identity, scsi, chap and session sections
//!   (`*_cases`), then `conformance` runs the SCSI sections on each LUN, each in its own file, judged
//!   with `cases`, reaching the LUN through `lun_access`, sharing what they find through `seen`, and
//!   checking each block command against the SBC tables in `command_set`. `STANDARDS.md` beside this
//!   crate records the standards each case cites.

mod alua;
mod alua_transition;
mod ats_race;
mod block_commands;
mod block_io;
mod capacity_cases;
mod cases;
pub mod chap;
mod chap_cases;
mod chap_mutual_cases;
mod coherence;
mod command_probes;
mod command_set;
mod commands;
mod compare_and_write;
pub mod compliance;
mod conformance;
pub mod connection;
pub mod crc32c;
pub mod credentials;
mod data_path;
mod data_transfer_cases;
pub mod discovery_listing;
mod disk;
mod feature_sets;
mod header_digest;
mod host_traces;
mod identity;
mod identity_cases;
pub mod initiator;
mod inquiry_cases;
pub mod local_disk;
mod local_lun;
mod local_nexus;
mod login;
mod login_cases;
mod login_phase_cases;
mod login_step;
mod lun_access;
mod mode_page_cases;
mod negotiation;
mod odx;
mod operation_code_cases;
mod operation_codes;
pub mod ops;
pub mod path;
pub mod pdu;
mod pdu_connection;
mod port;
mod port_groups;
mod pr_command_set;
mod pr_commands;
mod pr_handover;
mod pr_ports;
mod provisioning;
pub mod report;
mod reservations;
pub mod scsi;
mod scsi_cases;
mod scsi_fields;
mod seen;
mod send_targets;
pub mod session;
mod session_cases;
mod spc_commands;
mod standby_cases;
mod task_management;
mod transfer_checks;
mod unit_state_cases;
mod vpd_pages;
mod windows;
