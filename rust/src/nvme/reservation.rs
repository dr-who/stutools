//! The NVMe reservation commands (NVM Command Set Reservation Register, Report, Acquire and Release;
//! Base 2.1 7.5 to 7.8, Figures 569 to 584): their actions and types, the command builders, the
//! extended Reservation Report data, and which reads and writes each type allows a host that does
//! not hold it (Base 2.1 8.1.22, Figures 645 and 646). Reservations belong to a host, named by its
//! 128-bit host identifier, not to a controller.

use super::sqe::{self, Command};

/// ONCS bit 5: the controller supports reservations.
pub const ONCS_RESERVATIONS: u16 = 1 << 5;
/// The reservation notification log page.
pub const LOG_RESERVATION: u8 = 0x80;
/// The namespace features a controller with reservations supports (Base 2.1 8.1.22, Figures 32 and
/// 385).
pub const FEAT_RESV_MASK: u8 = 0x82;
pub const FEAT_RESV_PERSIST: u8 = 0x83;

/// Register actions (RREGA), acquire actions (RACQA) and release actions (RRELA).
pub const REGISTER: u8 = 0;
pub const UNREGISTER: u8 = 1;
pub const REPLACE: u8 = 2;
pub const ACQUIRE: u8 = 0;
pub const PREEMPT: u8 = 1;
pub const PREEMPT_AND_ABORT: u8 = 2;
pub const RELEASE: u8 = 0;
pub const CLEAR: u8 = 1;

/// Reservation types and their names. RESCAP bit N says type N is supported.
pub const TYPES: [(u8, &str); 6] = [
    (1, "Write Exclusive"),
    (2, "Exclusive Access"),
    (3, "Write Exclusive, Registrants Only"),
    (4, "Exclusive Access, Registrants Only"),
    (5, "Write Exclusive, All Registrants"),
    (6, "Exclusive Access, All Registrants"),
];

/// Reservation Register: `action` is RREGA; the data is `register_data`.
pub fn register(nsid: u32, action: u8, ignore_key: bool) -> Command {
    let mut cmd = Command::new(sqe::OP_RESV_REGISTER, 0);
    cmd.set_nsid(nsid);
    cmd.set_u32(40, action as u32 | (ignore_key as u32) << 3);
    cmd
}

/// Reservation Acquire: `action` is RACQA and `rtype` the reservation type; the data is two keys.
pub fn acquire(nsid: u32, action: u8, rtype: u8) -> Command {
    let mut cmd = Command::new(sqe::OP_RESV_ACQUIRE, 0);
    cmd.set_nsid(nsid);
    cmd.set_u32(40, action as u32 | (rtype as u32) << 8);
    cmd
}

/// Reservation Release: `action` is RRELA and `rtype` the type held; the data is the current key.
pub fn release(nsid: u32, action: u8, rtype: u8) -> Command {
    let mut cmd = Command::new(sqe::OP_RESV_RELEASE, 0);
    cmd.set_nsid(nsid);
    cmd.set_u32(40, action as u32 | (rtype as u32) << 8);
    cmd
}

/// Reservation Report of `len` bytes; `extended` asks for the extended data structure (EDS), which
/// carries 128-bit host identifiers.
pub fn report(nsid: u32, len: usize, extended: bool) -> Command {
    let mut cmd = Command::new(sqe::OP_RESV_REPORT, 0);
    cmd.set_nsid(nsid);
    cmd.set_u32(40, (len / 4 - 1) as u32);
    cmd.set_u32(44, extended as u32);
    cmd
}

/// Two keys as the 16 bytes Register (current, new) and Acquire (current, preempt) take.
pub fn two_keys(first: u64, second: u64) -> [u8; 16] {
    let mut d = [0u8; 16];
    d[0..8].copy_from_slice(&first.to_le_bytes());
    d[8..16].copy_from_slice(&second.to_le_bytes());
    d
}

/// One registrant in an extended report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registrant {
    pub cntlid: u16,
    /// RCSTS bit 0: this host holds the reservation.
    pub holds: bool,
    pub key: u64,
    pub hostid: [u8; 16],
}

/// A Reservation Report with the extended data structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub generation: u32,
    /// RTYPE: the reservation type held, 0 for none.
    pub rtype: u8,
    pub registrants: Vec<Registrant>,
}

/// Parses an extended Reservation Report: a 64-byte header (generation at 0, RTYPE at 4, the
/// registrant count REGCTL at 5-6), then a 64-byte entry per registrant (CNTLID at 0, RCSTS at 2,
/// the key at 8, the 128-bit host identifier at 16).
pub fn parse_report(d: &[u8]) -> Report {
    let count = u16::from_le_bytes([d[5], d[6]]) as usize;
    let registrants = (0..count)
        .filter_map(|i| d.get(64 + 64 * i..128 + 64 * i))
        .map(|e| Registrant {
            cntlid: u16::from_le_bytes([e[0], e[1]]),
            holds: e[2] & 1 != 0,
            key: u64::from_le_bytes(e[8..16].try_into().unwrap()),
            hostid: e[16..32].try_into().unwrap(),
        })
        .collect();
    Report {
        generation: u32::from_le_bytes(d[0..4].try_into().unwrap()),
        rtype: d[4],
        registrants,
    }
}

/// Whether a host that does not hold the reservation may read and write under `rtype`, when it is
/// registered and when it is not (the base specification's command behaviour table).
pub fn allowed(rtype: u8, registered: bool) -> (bool, bool) {
    match rtype {
        1 => (true, false),
        2 => (false, false),
        3 | 5 => (true, registered),
        4 | 6 => (registered, registered),
        _ => (true, true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write Exclusive, Registrants Only, and two keys.
    const WERO: u8 = 3;
    const KEY_A: u64 = 0xa1;
    const KEY_B: u64 = 0xb2;

    #[test]
    fn commands_put_the_action_type_and_flags_in_cdw10() {
        let c = register(1, REPLACE, true);
        assert_eq!(c.0[0], sqe::OP_RESV_REGISTER);
        assert_eq!(&c.0[40..44], &(2u32 | 8).to_le_bytes());
        let c = acquire(1, PREEMPT_AND_ABORT, WERO);
        assert_eq!(&c.0[40..44], &(2u32 | 3 << 8).to_le_bytes());
        let c = release(1, CLEAR, 0);
        assert_eq!(c.0[0], sqe::OP_RESV_RELEASE);
        assert_eq!(&c.0[40..44], &1u32.to_le_bytes());
        let c = report(1, 4096, true);
        assert_eq!(&c.0[40..44], &1023u32.to_le_bytes());
        assert_eq!(&c.0[44..48], &1u32.to_le_bytes());
        assert_eq!(&two_keys(1, 2)[8..16], &2u64.to_le_bytes());
    }

    #[test]
    fn an_extended_report_lists_each_registrant() {
        let mut d = vec![0u8; 256];
        d[0..4].copy_from_slice(&7u32.to_le_bytes());
        d[4] = WERO;
        d[5] = 2;
        d[64] = 1;
        d[66] = 1;
        d[72..80].copy_from_slice(&KEY_A.to_le_bytes());
        d[80..96].copy_from_slice(&[0xaa; 16]);
        d[136..144].copy_from_slice(&KEY_B.to_le_bytes());
        let r = parse_report(&d);
        assert_eq!(r.generation, 7);
        assert_eq!(r.rtype, WERO);
        assert_eq!(r.registrants.len(), 2);
        assert!(r.registrants[0].holds && !r.registrants[1].holds);
        assert_eq!(r.registrants[0].hostid, [0xaa; 16]);
        assert_eq!(r.registrants[1].key, KEY_B);
    }

    #[test]
    fn access_follows_the_reservation_type() {
        assert_eq!(allowed(1, true), (true, false));
        assert_eq!(allowed(2, true), (false, false));
        assert_eq!(allowed(3, true), (true, true));
        assert_eq!(allowed(3, false), (true, false));
        assert_eq!(allowed(6, false), (false, false));
    }
}
