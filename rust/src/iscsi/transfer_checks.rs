//! The rules the PDUs of one task keep (RFC 7143 11.4, 11.7, 11.8), judged over what a task brought
//! back: Data-In numbering (DataSN from 0 by one, 11.7.5), offsets in order without gaps or overlaps
//! (11.7.6 with DataPDUInOrder=Yes), every data segment within the MaxRecvDataSegmentLength spit
//! declared (11.7.1), every Data-In sequence within MaxBurstLength and ended by the F bit (11.7.1),
//! status only with F (11.7.3), no A bit at ErrorRecoveryLevel 0 (11.7.2); R2T numbering (R2TSN from
//! 0, 11.8.2), a target transfer tag that is not FFFFFFFFh (11.8.5), DDTL from 1 to MaxBurstLength
//! (11.8.4) and ranges that follow on (11.8 with DataSequenceInOrder=Yes); the residual flags and
//! count for what was sent against EDTL (11.4.5.2); StatSN by one on each status (4.2.2.2). Each
//! check returns the first PDU that breaks its rule, in words.

use super::pdu::{self, NO_TTT};
use super::pdu_connection::Pdu;

/// The Data-In PDUs among `pdus` (plus the final one when it carries data).
fn data_ins(pdus: &[Pdu]) -> Vec<&Pdu> {
    pdus.iter().filter(|p| p.opcode() == pdu::DATA_IN).collect()
}

/// DataSN counts 0, 1, 2 … over the task's Data-In PDUs.
pub(super) fn data_sn_in_order(pdus: &[Pdu]) -> Result<(), String> {
    for (i, p) in data_ins(pdus).iter().enumerate() {
        if p.sn36() != i as u32 {
            return Err(format!("Data-In {i} has DataSN {}", p.sn36()));
        }
    }
    Ok(())
}

/// Each Data-In starts where the one before ended, from 0, and none runs past `edtl`.
pub(super) fn offsets_in_order(pdus: &[Pdu], edtl: u32) -> Result<(), String> {
    let mut next = 0u64;
    for (i, p) in data_ins(pdus).iter().enumerate() {
        if u64::from(p.offset()) != next {
            return Err(format!(
                "Data-In {i} at offset {} after {next} bytes",
                p.offset()
            ));
        }
        next += p.data.len() as u64;
        if next > u64::from(edtl) {
            return Err(format!("Data-In {i} ends at {next}, past EDTL {edtl}"));
        }
    }
    Ok(())
}

/// No data segment is longer than `max_recv`.
pub(super) fn within_segment_limit(pdus: &[Pdu], max_recv: usize) -> Result<(), String> {
    for (i, p) in pdus.iter().enumerate() {
        if p.data.len() > max_recv {
            return Err(format!(
                "PDU {i} (opcode {:#04x}) carries {} bytes",
                p.opcode(),
                p.data.len()
            ));
        }
    }
    Ok(())
}

/// Every Data-In sequence (PDUs up to one with F=1) totals at most `max_burst` bytes, and the last
/// Data-In has F=1.
pub(super) fn bursts_end_with_f(pdus: &[Pdu], max_burst: usize) -> Result<(), String> {
    let ins = data_ins(pdus);
    let mut run = 0usize;
    for (i, p) in ins.iter().enumerate() {
        run += p.data.len();
        if run > max_burst {
            return Err(format!(
                "Data-In {i} brings its sequence to {run} bytes, past MaxBurstLength {max_burst}"
            ));
        }
        if p.flags() & 0x80 != 0 {
            run = 0;
        }
    }
    match ins.last() {
        Some(p) if p.flags() & 0x80 == 0 => {
            Err(format!("the last Data-In ({}) has F=0", ins.len() - 1))
        }
        _ => Ok(()),
    }
}

/// A Data-In with S=1 has F=1 and a status of GOOD or CONDITION MET (RFC 7143 11.7.3), and none sets
/// the A bit at ErrorRecoveryLevel 0 (11.7.2).
pub(super) fn status_flags(pdus: &[Pdu]) -> Result<(), String> {
    for (i, p) in data_ins(pdus).iter().enumerate() {
        if p.flags() & 0x40 != 0 {
            return Err(format!(
                "Data-In {i} sets the A bit at ErrorRecoveryLevel 0"
            ));
        }
        if p.flags() & 0x01 != 0 {
            if p.flags() & 0x80 == 0 {
                return Err(format!("Data-In {i} has S=1 and F=0"));
            }
            if !matches!(p.status(), 0x00 | 0x04 | 0x10 | 0x14) {
                return Err(format!("Data-In {i} carries status {:#04x}", p.status()));
            }
        }
    }
    Ok(())
}

/// The R2Ts of one write: R2TSN 0, 1 …; a TTT that is not FFFFFFFFh; DDTL from 1 to `max_burst`;
/// offsets that follow on from `first_offset` (the unsolicited part), each R2T asking for the bytes
/// after the one before, ending at or before `edtl`; no header data.
pub(super) fn r2t_fields(
    r2ts: &[&Pdu],
    first_offset: u32,
    edtl: u32,
    max_burst: usize,
) -> Result<(), String> {
    let mut next = u64::from(first_offset);
    for (i, r) in r2ts.iter().enumerate() {
        if r.sn36() != i as u32 {
            return Err(format!("R2T {i} has R2TSN {}", r.sn36()));
        }
        if r.ttt() == NO_TTT {
            return Err(format!("R2T {i} has target transfer tag FFFFFFFFh"));
        }
        let ddtl = r.word44() as usize;
        if ddtl == 0 || ddtl > max_burst {
            return Err(format!(
                "R2T {i} asks for {ddtl} bytes (MaxBurstLength {max_burst})"
            ));
        }
        if u64::from(r.offset()) != next {
            return Err(format!(
                "R2T {i} asks from offset {} after {next}",
                r.offset()
            ));
        }
        next += ddtl as u64;
        if next > u64::from(edtl) {
            return Err(format!("R2T {i} asks up to {next}, past EDTL {edtl}"));
        }
        if !r.data.is_empty() {
            return Err(format!("R2T {i} carries {} data bytes", r.data.len()));
        }
    }
    Ok(())
}

/// What the residual of a final PDU (SCSI Response, or Data-In with S=1) says: overflow, underflow,
/// and the count.
pub(super) fn residual(last: &Pdu) -> (bool, bool, u32) {
    (
        last.flags() & 0x04 != 0,
        last.flags() & 0x02 != 0,
        last.word44(),
    )
}

/// The residual RFC 7143 11.4.5.2 requires for a task whose SCSI layer had `spdtl` bytes for an
/// EDTL of `edtl`: overflow with SPDTL − EDTL, underflow with EDTL − SPDTL, or neither.
pub(super) fn expected_residual(spdtl: u32, edtl: u32) -> (bool, bool, u32) {
    if spdtl > edtl {
        (true, false, spdtl - edtl)
    } else if spdtl < edtl {
        (false, true, edtl - spdtl)
    } else {
        (false, false, 0)
    }
}

/// The residual in words, for the expected and actual columns.
pub(super) fn show_residual(r: (bool, bool, u32)) -> String {
    match r {
        (true, false, n) => format!("O=1 residual {n}"),
        (false, true, n) => format!("U=1 residual {n}"),
        (false, false, _) => "no residual".into(),
        (true, true, n) => format!("O=1 and U=1, residual {n}"),
    }
}

/// StatSN goes up by one on each PDU that carries status (SCSI Response, Data-In with S=1, NOP-In
/// answering a ping, Text, Logout, Reject, Task Management responses), and the PDUs that carry the
/// next StatSN without status (R2T, NOP-In pings, Data-In without S) show the one to come.
pub(super) fn stat_sn_steps(log: &[Pdu]) -> Result<(), String> {
    let mut next: Option<u32> = None;
    for (i, p) in log.iter().enumerate() {
        let carries = match p.opcode() {
            pdu::DATA_IN => p.flags() & 0x01 != 0,
            pdu::R2T => false,
            pdu::NOP_IN => p.itt() != NO_TTT,
            // An Asynchronous Message advances StatSN as a status does (RFC 7143 11.9).
            _ => true,
        };
        let shows = p.opcode() != pdu::DATA_IN || carries;
        match next {
            Some(n) if shows && p.stat_sn() != n => {
                return Err(format!(
                    "PDU {i} (opcode {:#04x}) has StatSN {} where {n} was next",
                    p.opcode(),
                    p.stat_sn()
                ))
            }
            _ => {}
        }
        if carries {
            next = Some(p.stat_sn().wrapping_add(1));
        } else if shows && next.is_none() {
            next = Some(p.stat_sn());
        }
    }
    Ok(())
}

/// MaxCmdSN is never below ExpCmdSN − 1 (RFC 7143 4.2.2.1) and never goes down (serial arithmetic).
pub(super) fn command_window(log: &[Pdu]) -> Result<(), String> {
    let mut last: Option<u32> = None;
    for (i, p) in log.iter().enumerate() {
        if p.opcode() == pdu::DATA_IN && p.flags() & 0x01 == 0 {
            continue;
        }
        if (p.max_cmd_sn().wrapping_sub(p.exp_cmd_sn()) as i32) < -1 {
            return Err(format!(
                "PDU {i}: MaxCmdSN {} below ExpCmdSN {} - 1",
                p.max_cmd_sn(),
                p.exp_cmd_sn()
            ));
        }
        if let Some(l) = last {
            if (p.max_cmd_sn().wrapping_sub(l) as i32) < 0 {
                return Err(format!(
                    "PDU {i}: MaxCmdSN went down from {l} to {}",
                    p.max_cmd_sn()
                ));
            }
        }
        last = Some(p.max_cmd_sn());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::pdu::BHS_LEN;
    use super::*;

    fn pdu(opcode: u8, flags: u8, sn: u32, offset: u32, len: usize) -> Pdu {
        let mut bhs = [0u8; BHS_LEN];
        bhs[0] = opcode;
        bhs[1] = flags;
        bhs[36..40].copy_from_slice(&sn.to_be_bytes());
        bhs[40..44].copy_from_slice(&offset.to_be_bytes());
        Pdu {
            bhs,
            data: vec![0; len],
        }
    }

    fn with(mut p: Pdu, at: usize, v: u32) -> Pdu {
        p.bhs[at..at + 4].copy_from_slice(&v.to_be_bytes());
        p
    }

    #[test]
    fn data_in_numbering_offsets_and_bursts() {
        let good = [
            pdu(pdu::DATA_IN, 0x00, 0, 0, 512),
            pdu(pdu::DATA_IN, 0x80, 1, 512, 512),
            pdu(pdu::DATA_IN, 0x81, 2, 1024, 512),
        ];
        assert!(data_sn_in_order(&good).is_ok());
        assert!(offsets_in_order(&good, 1536).is_ok());
        assert!(offsets_in_order(&good, 1024).is_err());
        assert!(within_segment_limit(&good, 512).is_ok());
        assert!(within_segment_limit(&good, 511).is_err());
        assert!(bursts_end_with_f(&good, 1024).is_ok());
        assert!(bursts_end_with_f(&good, 512).is_err());
        assert!(status_flags(&good).is_ok());
        let skipped = [good[0].clone(), pdu(pdu::DATA_IN, 0x81, 2, 512, 512)];
        assert!(data_sn_in_order(&skipped).is_err());
        let gap = [good[0].clone(), pdu(pdu::DATA_IN, 0x81, 1, 1024, 512)];
        assert!(offsets_in_order(&gap, 4096).is_err());
        let open = [pdu(pdu::DATA_IN, 0x00, 0, 0, 512)];
        assert!(bursts_end_with_f(&open, 4096).is_err());
        assert!(status_flags(&[pdu(pdu::DATA_IN, 0x01, 0, 0, 4)]).is_err());
        assert!(status_flags(&[pdu(pdu::DATA_IN, 0xc0, 0, 0, 4)]).is_err());
    }

    #[test]
    fn r2t_rules() {
        let r = |sn, off, ddtl| with(with(pdu(pdu::R2T, 0x80, sn, off, 0), 44, ddtl), 20, 7);
        let a = r(0, 512, 512);
        let b = r(1, 1024, 1024);
        assert!(r2t_fields(&[&a, &b], 512, 2048, 1024).is_ok());
        assert!(r2t_fields(&[&a, &b], 0, 2048, 1024).is_err(), "offset");
        assert!(r2t_fields(&[&a, &b], 512, 1536, 1024).is_err(), "past EDTL");
        assert!(r2t_fields(&[&a, &b], 512, 2048, 512).is_err(), "burst");
        let zero = r(0, 0, 0);
        assert!(r2t_fields(&[&zero], 0, 512, 512).is_err());
        let no_tag = with(r(0, 0, 512), 20, NO_TTT);
        assert!(r2t_fields(&[&no_tag], 0, 512, 512).is_err());
        let renumbered = r(1, 512, 512);
        assert!(r2t_fields(&[&renumbered], 512, 1024, 512).is_err());
    }

    #[test]
    fn residuals_follow_spdtl_and_edtl() {
        assert_eq!(expected_residual(36, 255), (false, true, 219));
        assert_eq!(expected_residual(36, 8), (true, false, 28));
        assert_eq!(expected_residual(16, 16), (false, false, 0));
        let p = with(pdu(pdu::SCSI_RESPONSE, 0x82, 0, 0, 0), 44, 219);
        assert_eq!(residual(&p), (false, true, 219));
        assert_eq!(show_residual(residual(&p)), "U=1 residual 219");
    }

    #[test]
    fn stat_sn_and_window() {
        let s =
            |op, flags, stat: u32, itt: u32| with(with(pdu(op, flags, 0, 0, 0), 24, stat), 16, itt);
        let log = [
            s(pdu::R2T, 0x80, 5, 1),
            s(pdu::SCSI_RESPONSE, 0x80, 5, 1),
            s(pdu::DATA_IN, 0x00, 0, 2),
            s(pdu::DATA_IN, 0x81, 6, 2),
            s(pdu::NOP_IN, 0x80, 7, NO_TTT),
            s(pdu::NOP_IN, 0x80, 7, 3),
        ];
        assert!(stat_sn_steps(&log).is_ok());
        let skipped = [
            s(pdu::SCSI_RESPONSE, 0x80, 5, 1),
            s(pdu::SCSI_RESPONSE, 0x80, 7, 2),
        ];
        assert!(stat_sn_steps(&skipped).is_err());
        let w = |exp: u32, max: u32| {
            with(
                with(pdu(pdu::SCSI_RESPONSE, 0x80, 0, 0, 0), 28, exp),
                32,
                max,
            )
        };
        assert!(command_window(&[w(5, 4), w(6, 10)]).is_ok());
        assert!(command_window(&[w(5, 3)]).is_err());
        assert!(command_window(&[w(5, 10), w(6, 9)]).is_err());
    }
}
