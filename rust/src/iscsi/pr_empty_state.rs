//! The `pr` cases of a LUN with no persistent reservation and no registrations, as Windows Failover
//! Clustering reads it before it registers (SPC-6 6.14.3): READ RESERVATION returns the 8-byte header
//! with ADDITIONAL LENGTH exactly zero. READ KEYS (SPC-6 6.14.2) finds whether any key is registered; a
//! LUN that still has registrations only informs.

use super::cases::{describe, record, record_info};
use super::ops;
use super::port::Port;
use super::pr_commands::reserve_in;
use super::pr_ports::SECTION;
use super::scsi_fields::be32;
use crate::compliance::Case;

/// The PRGENERATION and ADDITIONAL LENGTH of a PERSISTENT RESERVE IN answer, and the bytes returned.
pub(super) fn header(d: &[u8], received: usize) -> (u32, u32, usize) {
    (be32(d, 0), be32(d, 4), received)
}

pub(super) fn run(s: &mut Port, name: &str, cases: &mut Vec<Case>) {
    let at = |clause: &str, what: &str| format!("{name} {clause} {what}");
    let keys = reserve_in(s, ops::PR_IN_READ_KEYS, 1024);
    let (_, key_length, _) = match &keys {
        Ok(o) if o.status == 0 => header(&o.data, o.received),
        Ok(o) => {
            record(
                cases,
                SECTION,
                at("SPC-6 6.14.2", "READ KEYS with no registrations"),
                "GOOD",
                describe(o),
                false,
            );
            return;
        }
        Err(e) => {
            record(
                cases,
                SECTION,
                at("SPC-6 6.14.2", "READ KEYS with no registrations"),
                "GOOD",
                e.clone(),
                false,
            );
            return;
        }
    };
    if key_length != 0 {
        record_info(
            cases,
            SECTION,
            at("SPC-6 6.14.3", "READ RESERVATION with none held"),
            format!("not run: {} keys still registered", key_length / 8),
        );
        return;
    }
    let o = reserve_in(s, ops::PR_IN_READ_RESERVATIONS, 512);
    let (actual, passed) = match &o {
        Ok(o) if o.status == 0 => {
            let (_, length, got) = header(&o.data, o.received);
            (
                format!("ADDITIONAL LENGTH {length}, {got} bytes"),
                length == 0 && got == 8,
            )
        }
        Ok(o) => (describe(o), false),
        Err(e) => (e.clone(), false),
    };
    record(
        cases,
        SECTION,
        at(
            "SPC-6 6.14.3",
            "READ RESERVATION with none held: ADDITIONAL LENGTH 0",
        ),
        "GOOD, ADDITIONAL LENGTH 0, 8 bytes",
        actual,
        passed,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_header_is_generation_then_length() {
        let d = [0, 0, 0, 7, 0, 0, 0, 16, 1, 2];
        assert_eq!(header(&d, 10), (7, 16, 10));
        assert_eq!(header(&[], 0), (0, 0, 0));
    }
}
