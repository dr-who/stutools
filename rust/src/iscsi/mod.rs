//! A user-space iSCSI initiator for spit. It speaks the protocol directly, so spit can log in to a
//! target without the operating system's initiator. Login comes first; SCSI commands come later.

pub mod chap;
pub mod compliance;
pub mod conformance;
pub mod connection;
pub mod crc32c;
pub mod initiator;
pub mod ops;
pub mod pdu;
pub mod report;
pub mod scsi;
pub mod session;
