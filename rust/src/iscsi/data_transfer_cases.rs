//! The data-transfer cases of the `io` and `session` sections, each on a `PduConnection` so every PDU
//! field is judged (RFC 7143 11.4, 11.7, 11.8): a READ whose Data-In PDUs are checked for DataSN,
//! offset, the MaxRecvDataSegmentLength spit declared, the burst F bit and status flags; INQUIRY and
//! REPORT LUNS residuals for under- and overflow; a WRITE with immediate and unsolicited data (the
//! path Linux and Windows use) read back; the R2T fields of a solicited WRITE; the three digest
//! combinations with I/O; a Data-Out with a bad data digest that must not reach the medium; NOP-Out
//! ping forms; and a Logout of an unknown connection. Data-changing cases work on the LUN's
//! second-to-last block and write the original data back. A target that does not negotiate a key a
//! case needs (ImmediateData, InitialR2T, a digest) has that case reported "not supported", not failed.

use super::cases::{record, record_info, record_should};
use super::commands::Commands;
use super::login::SessionKeys;
use super::login_cases::Reachable;
use super::lun_access::{short_name, Creds};
use super::path::Path;
use super::pdu::{self, NO_TTT};
use super::pdu_connection::{Pdu, PduConnection, Received, WriteForm, PDU_WAIT};
use super::scsi;
use super::transfer_checks as tc;
use crate::compliance::Case;
use crate::Result;

const IO: &str = "io";
const SESSION: &str = "session";

/// Runs the data-transfer cases on every reachable iSCSI target.
pub(super) fn run(
    reachable: &[Reachable],
    initiator: &str,
    chap_good: Creds,
    cases: &mut Vec<Case>,
) {
    for r in reachable.iter().filter(|r| !r.path.is_local()) {
        let creds = if r.chap { chap_good } else { None };
        let name = short_name(r.path);
        if let Err(e) = target(r.path, initiator, creds, &name, cases) {
            record(
                cases,
                SESSION,
                format!("{name} data transfer"),
                "a session to the LUN",
                e,
                false,
            );
        }
    }
}

/// The LUN's block length and the scratch LBA (its second-to-last block).
fn geometry(path: &Path, initiator: &str, creds: Creds) -> Result<(u32, u64)> {
    let mut c = PduConnection::open(path, initiator, creds, &SessionKeys::spit("None"))?;
    c.drain_attentions()?;
    let (last, block) = scsi::capacity(&mut c)?;
    let _ = c.logout();
    Ok((block, last.saturating_sub(1)))
}

fn target(
    path: &Path,
    initiator: &str,
    creds: Creds,
    name: &str,
    cases: &mut Vec<Case>,
) -> Result<()> {
    let (block, scratch) = geometry(path, initiator, creds)?;
    data_in_fields(path, initiator, creds, name, block, cases)?;
    residuals(path, initiator, creds, name, cases)?;
    r2t_and_write(path, initiator, creds, name, block, scratch, cases)?;
    immediate_write(path, initiator, creds, name, block, scratch, cases)?;
    digest_io(path, initiator, creds, name, block, scratch, cases)?;
    data_out_bad_digest(path, initiator, creds, name, block, scratch, cases)?;
    nop_and_logout(path, initiator, creds, name, cases)?;
    Ok(())
}

/// READ on a session that declared MaxRecvDataSegmentLength 512, so the target must split the data;
/// its Data-In fields are judged one rule per case (RFC 7143 11.7, UNH FFP 3.1, 8.2, 9.x, Login 27.1).
fn data_in_fields(
    path: &Path,
    initiator: &str,
    creds: Creds,
    name: &str,
    block: u32,
    cases: &mut Vec<Case>,
) -> Result<()> {
    let mut keys = SessionKeys::spit("None");
    keys.max_recv = 512;
    keys.max_burst = 4096;
    let mut c = PduConnection::open(path, initiator, creds, &keys)?;
    c.drain_attentions()?;
    let blocks = (8192 / block).max(4);
    let edtl = blocks * block;
    let (mut pdus, end) = c.read_command(&scsi::block_cdb(0x28, 0, blocks as u16), edtl)?;
    if let Received::Pdu(p) = &end {
        pdus.push(p.clone());
    }
    let max_recv = c.negotiated.max_recv;
    judge(
        cases,
        IO,
        format!("{name} Data-In DataSN counts from 0"),
        tc::data_sn_in_order(&pdus),
        &end,
    );
    judge(
        cases,
        IO,
        format!("{name} Data-In offsets in order and within EDTL"),
        tc::offsets_in_order(&pdus, edtl),
        &end,
    );
    judge(
        cases,
        IO,
        format!("{name} Data-In within MaxRecvDataSegmentLength 512"),
        tc::within_segment_limit(&pdus, max_recv.min(512)),
        &end,
    );
    judge(
        cases,
        IO,
        format!("{name} Data-In bursts end with the F bit"),
        tc::bursts_end_with_f(&pdus, c.negotiated.max_burst.max(512)),
        &end,
    );
    judge(
        cases,
        IO,
        format!("{name} Data-In status only with the F bit, no A bit at ERL 0"),
        tc::status_flags(&pdus),
        &end,
    );
    judge(
        cases,
        SESSION,
        format!("{name} StatSN advances by one per status PDU"),
        tc::stat_sn_steps(&c.log),
        &end,
    );
    judge(
        cases,
        SESSION,
        format!("{name} MaxCmdSN never below ExpCmdSN-1"),
        tc::command_window(&c.log),
        &end,
    );
    let _ = c.logout();
    Ok(())
}

/// Judges a per-PDU rule: pass when it holds, fail with the offending PDU; a closed connection
/// before any data fails too.
fn judge(
    cases: &mut Vec<Case>,
    section: &'static str,
    label: String,
    result: std::result::Result<(), String>,
    end: &Received,
) {
    let (actual, passed) = match (&result, end) {
        (Ok(()), Received::Pdu(_)) => ("as the RFC requires".to_string(), true),
        (Ok(()), other) => (format!("no status ({other:?})"), false),
        (Err(why), _) => (why.clone(), false),
    };
    record(
        cases,
        section,
        label,
        "the field follows RFC 7143 11.4/11.7/11.8",
        actual,
        passed,
    );
}

/// INQUIRY and REPORT LUNS with an EDTL that forces under- and overflow; the U/O bits and Residual
/// Count must match (RFC 7143 11.4.5.2, UNH FFP 16.1.x, 16.2.x).
fn residuals(
    path: &Path,
    initiator: &str,
    creds: Creds,
    name: &str,
    cases: &mut Vec<Case>,
) -> Result<()> {
    let mut c = PduConnection::open(path, initiator, creds, &SessionKeys::spit("None"))?;
    c.drain_attentions()?;
    // The INQUIRY length (SPDTL) from a read whose EDTL does not cap it: allocation 255, EDTL 255.
    let inq_len = c.command(&[0x12, 0, 0, 0, 255, 0], 255, None)?.received as u32;
    // Underflow: allocation and EDTL 255, the device presents fewer bytes.
    let (_p, end) = c.read_command(&[0x12, 0, 0, 0, 255, 0], 255)?;
    residual_case(
        cases,
        format!("{name} INQUIRY residual underflow"),
        &end,
        tc::expected_residual(inq_len, 255),
    );
    // Overflow: allocation 255, but an EDTL smaller than the data, so more is presented than asked for.
    let small = (inq_len / 2).max(4);
    let (_p, end) = c.read_command(&[0x12, 0, 0, 0, 255, 0], small)?;
    residual_case(
        cases,
        format!("{name} INQUIRY residual overflow"),
        &end,
        tc::expected_residual(inq_len, small),
    );
    // REPORT LUNS: EDTL sized to the list, then half of it for an overflow.
    let rl_len = c.command(&report_luns_cdb(0xffff), 0xffff, None)?.received as u32;
    let rl_small = (rl_len / 2).max(8);
    let (_p, end) = c.read_command(&report_luns_cdb(rl_len.max(16)), rl_small)?;
    residual_case(
        cases,
        format!("{name} REPORT LUNS residual overflow"),
        &end,
        tc::expected_residual(rl_len, rl_small),
    );
    let _ = c.logout();
    Ok(())
}

fn report_luns_cdb(alloc: u32) -> [u8; 12] {
    let a = alloc.to_be_bytes();
    [0xa0, 0, 0, 0, 0, 0, a[0], a[1], a[2], a[3], 0, 0]
}

fn residual_case(cases: &mut Vec<Case>, label: String, end: &Received, want: (bool, bool, u32)) {
    match end {
        Received::Pdu(p) => {
            let got = tc::residual(p);
            record(
                cases,
                SESSION,
                label,
                tc::show_residual(want),
                tc::show_residual(got),
                got == want,
            );
        }
        other => {
            record(
                cases,
                SESSION,
                label,
                tc::show_residual(want),
                format!("no status ({other:?})"),
                false,
            );
        }
    }
}

/// A WRITE of two blocks on a session with InitialR2T=Yes: the R2T fields are judged, the data is
/// read back, then the original is restored (RFC 7143 11.8, UNH FFP 4.1, 8.1, 10.1).
fn r2t_and_write(
    path: &Path,
    initiator: &str,
    creds: Creds,
    name: &str,
    block: u32,
    scratch: u64,
    cases: &mut Vec<Case>,
) -> Result<()> {
    let mut c = PduConnection::open(path, initiator, creds, &SessionKeys::spit("None"))?;
    c.drain_attentions()?;
    let lba = scratch as u32;
    let original = c
        .command(&scsi::block_cdb(0x28, lba, 2), block * 2, None)?
        .data;
    let payload = super::lun_access::pattern((block * 2) as usize, 0xa7);
    let (pre, end) = c.write(
        &scsi::block_cdb(0x2a, lba, 2),
        &payload,
        &WriteForm::default(),
    )?;
    let r2ts: Vec<&Pdu> = pre.iter().filter(|p| p.opcode() == pdu::R2T).collect();
    let ok = matches!(&end, Received::Pdu(p) if p.status() == 0);
    if r2ts.is_empty() {
        record_info(
            cases,
            IO,
            format!("{name} R2T fields of a solicited WRITE"),
            "the target sent no R2T (it took the write without one)",
        );
    } else {
        let res = tc::r2t_fields(&r2ts, 0, block * 2, c.negotiated.max_burst.max(512));
        record(
            cases,
            IO,
            format!("{name} R2T fields of a solicited WRITE"),
            "R2TSN, TTT, DDTL and offsets per RFC 7143 11.8",
            res.clone()
                .err()
                .unwrap_or_else(|| "as the RFC requires".into()),
            res.is_ok(),
        );
    }
    // Read back and restore.
    let back = c.command(&scsi::block_cdb(0x28, lba, 2), block * 2, None)?;
    record(
        cases,
        IO,
        format!("{name} solicited WRITE reads back"),
        "GOOD and the data matches",
        if ok && back.data == payload {
            "written and read back".into()
        } else {
            format!(
                "status {}, {} bytes match",
                end_status(&end),
                back.data
                    .iter()
                    .zip(&payload)
                    .filter(|(a, b)| a == b)
                    .count()
            )
        },
        ok && back.data == payload,
    );
    let _ = c.write(
        &scsi::block_cdb(0x2a, lba, 2),
        &original,
        &WriteForm::default(),
    );
    let _ = c.logout();
    Ok(())
}

/// The write path hosts use: ImmediateData=Yes, InitialR2T=No, so the first burst is immediate and
/// unsolicited data (RFC 7143 4.2.5.2, 13.10, 13.11; UNH FFP 16.3.1, WHCP.ISCSI.10). Reported "not
/// supported" when the target answers ImmediateData=No or InitialR2T=Yes.
fn immediate_write(
    path: &Path,
    initiator: &str,
    creds: Creds,
    name: &str,
    block: u32,
    scratch: u64,
    cases: &mut Vec<Case>,
) -> Result<()> {
    let mut keys = SessionKeys::host_writes("None");
    keys.first_burst = (block * 2).max(512);
    keys.max_burst = (block * 4).max(512);
    let mut c = PduConnection::open(path, initiator, creds, &keys)?;
    let label = format!("{name} WRITE with immediate and unsolicited data");
    if !c.negotiated.immediate_data && c.negotiated.initial_r2t {
        record_info(
            cases,
            IO,
            label,
            "not supported: the target answered ImmediateData=No and InitialR2T=Yes",
        );
        let _ = c.logout();
        return Ok(());
    }
    c.drain_attentions()?;
    let lba = scratch as u32;
    let original = c
        .command(&scsi::block_cdb(0x28, lba, 2), block * 2, None)?
        .data;
    let payload = super::lun_access::pattern((block * 2) as usize, 0x5c);
    // Immediate data up to the first segment; the rest unsolicited if InitialR2T=No, else on R2Ts.
    let imm = if c.negotiated.immediate_data {
        (block as usize)
            .min(c.negotiated.max_recv)
            .min(c.negotiated.first_burst)
    } else {
        0
    };
    let unsolicited = if !c.negotiated.initial_r2t {
        (payload.len() - imm).min(c.negotiated.first_burst.saturating_sub(imm))
    } else {
        0
    };
    let form = WriteForm {
        immediate: imm,
        unsolicited,
        ..WriteForm::default()
    };
    let (_pre, end) = c.write(&scsi::block_cdb(0x2a, lba, 2), &payload, &form)?;
    let ok = matches!(&end, Received::Pdu(p) if p.status() == 0);
    let back = c.command(&scsi::block_cdb(0x28, lba, 2), block * 2, None)?;
    record(
        cases,
        IO,
        label,
        "GOOD and the data reads back",
        if ok && back.data == payload {
            format!("{imm} B immediate, {unsolicited} B unsolicited, read back")
        } else {
            format!(
                "status {}, data {}",
                end_status(&end),
                if back.data == payload {
                    "matches"
                } else {
                    "differs"
                }
            )
        },
        ok && back.data == payload,
    );
    let _ = c.write(
        &scsi::block_cdb(0x2a, lba, 2),
        &original,
        &WriteForm::default(),
    );
    let _ = c.logout();
    Ok(())
}

/// The three digest combinations with I/O (RFC 7143 13.1, HLK ISCSI-DIG.1/2/3): header only, data
/// only, both. Each logs in offering that one value per key, writes and reads back the scratch block.
fn digest_io(
    path: &Path,
    initiator: &str,
    creds: Creds,
    name: &str,
    block: u32,
    scratch: u64,
    cases: &mut Vec<Case>,
) -> Result<()> {
    for (hd, dd, what) in [
        ("CRC32C", "None", "header digest only"),
        ("None", "CRC32C", "data digest only"),
        ("CRC32C", "CRC32C", "both digests"),
    ] {
        let mut keys = SessionKeys::spit("None");
        keys.header_digest = hd.to_string();
        keys.data_digest = dd.to_string();
        let label = format!("{name} {what}, with I/O");
        let mut c = match PduConnection::open(path, initiator, creds, &keys) {
            Ok(c) => c,
            Err(e) => {
                record(
                    cases,
                    SESSION,
                    label,
                    format!("{hd}/{dd} negotiated, data read back"),
                    e,
                    false,
                );
                continue;
            }
        };
        let got_hd = c.negotiated.header_digest;
        let got_dd = c.negotiated.data_digest;
        if got_hd != (hd == "CRC32C") || got_dd != (dd == "CRC32C") {
            record_info(
                cases,
                SESSION,
                label,
                format!(
                    "not supported: the target answered HeaderDigest={} DataDigest={}",
                    if got_hd { "CRC32C" } else { "None" },
                    if got_dd { "CRC32C" } else { "None" }
                ),
            );
            let _ = c.logout();
            continue;
        }
        if c.drain_attentions().is_err() {
            record(
                cases,
                SESSION,
                label,
                format!("{hd}/{dd} negotiated, data read back"),
                "the session dropped after login",
                false,
            );
            continue;
        }
        let lba = scratch as u32;
        let original = c.command(&scsi::block_cdb(0x28, lba, 1), block, None)?.data;
        let payload = super::lun_access::pattern(block as usize, 0x3e);
        let (_p, end) = c.write(
            &scsi::block_cdb(0x2a, lba, 1),
            &payload,
            &WriteForm::default(),
        )?;
        let ok = matches!(&end, Received::Pdu(p) if p.status() == 0);
        let back = c.command(&scsi::block_cdb(0x28, lba, 1), block, None)?;
        record(
            cases,
            SESSION,
            label,
            format!("HeaderDigest={hd} DataDigest={dd}, data read back"),
            if ok && back.data == payload {
                "negotiated and data read back".into()
            } else {
                format!("status {}", end_status(&end))
            },
            ok && back.data == payload,
        );
        let _ = c.write(
            &scsi::block_cdb(0x2a, lba, 1),
            &original,
            &WriteForm::default(),
        );
        let _ = c.logout();
    }
    Ok(())
}

/// A Data-Out whose data digest is wrong at ErrorRecoveryLevel 0 (RFC 7143 7.8, UNH FFP 18.2.2, ER
/// 4.1.1): the target must Reject with reason 02h and then recover (recovery R2T or CHECK CONDITION
/// 0B/47/05 after F=1) or close the connection, and the block must not hold the corrupted bytes.
/// Reported "not supported" when the target does not negotiate DataDigest=CRC32C.
fn data_out_bad_digest(
    path: &Path,
    initiator: &str,
    creds: Creds,
    name: &str,
    block: u32,
    scratch: u64,
    cases: &mut Vec<Case>,
) -> Result<()> {
    let mut keys = SessionKeys::spit("None");
    keys.data_digest = "CRC32C".to_string();
    let label = format!("{name} Data-Out with a bad data digest is rejected and not written");
    let mut c = PduConnection::open(path, initiator, creds, &keys)?;
    if !c.negotiated.data_digest {
        record_info(
            cases,
            SESSION,
            label,
            "not supported: the target answered DataDigest=None",
        );
        let _ = c.logout();
        return Ok(());
    }
    c.drain_attentions()?;
    let lba = scratch as u32;
    let original = c.command(&scsi::block_cdb(0x28, lba, 1), block, None)?.data;
    let payload = super::lun_access::pattern(block as usize, 0x9b);
    // One block in several Data-Out PDUs; the first has a wrong data digest.
    let form = WriteForm {
        segment: Some((block as usize / 4).max(1)),
        bad_data_out: Some(0),
        ..WriteForm::default()
    };
    let (pre, end) = c.write(&scsi::block_cdb(0x2a, lba, 1), &payload, &form)?;
    let rejected = pre
        .iter()
        .any(|p| p.opcode() == pdu::REJECT && p.bhs[2] == 0x02);
    let recovery_r2t = pre.iter().any(|p| p.opcode() == pdu::R2T);
    let check = matches!(&end, Received::Pdu(p) if p.status() == 0x02 && p.sense().get(..3).map_or(false, |s| s[2] & 0x0f == 0x0b));
    let good = matches!(&end, Received::Pdu(p) if p.status() == 0);
    let closed = matches!(&end, Received::Closed);
    // The block must be unchanged unless the write actually completed after a recovery.
    let handled = rejected || check || closed || recovery_r2t;
    let actual = format!(
        "reject02={rejected}, recoveryR2T={recovery_r2t}, end={}",
        match &end {
            Received::Pdu(p) => format!("status {:#04x}", p.status()),
            o => format!("{o:?}"),
        }
    );
    record(cases, SESSION, label, "Reject reason 02h then recovery, or CHECK CONDITION 0B/47/05, or a close; never silent GOOD", actual, handled && !(good && !rejected && !recovery_r2t));
    // Restore: a fresh session (this one may be dropped) and the original bytes.
    let _ = c.logout();
    if let Ok(mut c2) = PduConnection::open(path, initiator, creds, &SessionKeys::spit("None")) {
        let _ = c2.drain_attentions();
        let _ = c2.write(
            &scsi::block_cdb(0x2a, lba, 1),
            &original,
            &WriteForm::default(),
        );
        let _ = c2.logout();
    }
    Ok(())
}

/// NOP-Out ping forms (immediate and not, 0 and 32 bytes of data) and a Logout of an unknown
/// connection (RFC 7143 11.18, 11.19, 11.14, UNH FFP 15.1.x, 13.3).
fn nop_and_logout(
    path: &Path,
    initiator: &str,
    creds: Creds,
    name: &str,
    cases: &mut Vec<Case>,
) -> Result<()> {
    let mut c = PduConnection::open(path, initiator, creds, &SessionKeys::spit("None"))?;
    c.drain_attentions()?;
    for (immediate, len, what) in [
        (true, 0usize, "immediate, no data"),
        (false, 0, "non-immediate, no data"),
        (true, 32, "immediate, 32 bytes"),
        (false, 32, "non-immediate, 32 bytes"),
    ] {
        let data: Vec<u8> = (0..len).map(|i| (i as u8) ^ 0x33).collect();
        let (bhs, itt) = c.ping_header(len, immediate);
        let label = format!("{name} NOP-Out ping ({what})");
        if c.send(&bhs, &data).is_err() {
            record(
                cases,
                SESSION,
                label,
                "NOP-In with the same ITT and data",
                "send failed",
                false,
            );
            continue;
        }
        let answer = loop {
            match c.recv(PDU_WAIT) {
                Received::Pdu(p) if p.opcode() == pdu::NOP_IN && p.itt() == itt => break Some(p),
                Received::Pdu(p) if p.opcode() == pdu::NOP_IN => {
                    let _ = c.answer_ping(&p);
                }
                Received::Pdu(_) => continue,
                _ => break None,
            }
        };
        match answer {
            Some(p) => {
                let echoed = p.data.len() == len.min(c.negotiated.own_max_recv) && p.data == data;
                record(
                    cases,
                    SESSION,
                    label,
                    "NOP-In with the same ITT and TTT FFFFFFFFh, data echoed",
                    format!(
                        "ITT match, {} data bytes{}",
                        p.data.len(),
                        if echoed { " echoed" } else { "" }
                    ),
                    p.ttt() == NO_TTT && (len == 0 || echoed),
                );
            }
            None => {
                record(
                    cases,
                    SESSION,
                    label,
                    "NOP-In with the same ITT",
                    "no NOP-In",
                    false,
                );
            }
        }
    }
    // Logout of a CID that is not this connection's: response 1 (CID not found), RFC 7143 11.15.1.
    let label = format!("{name} logout of an unknown connection: CID not found");
    let itt = c.next_itt();
    let mut bhs = pdu::logout_request(itt, 0, c.cmd_sn, c.exp_stat_sn);
    bhs[1] = 0x80 | 0x01; // reason 1: close the connection
    bhs[20..22].copy_from_slice(&0x5555u16.to_be_bytes()); // a CID this session does not have
    if c.send(&bhs, &[]).is_ok() {
        let answer = loop {
            match c.recv(PDU_WAIT) {
                Received::Pdu(p) if p.opcode() == 0x26 => break Some(p.bhs[2]),
                Received::Pdu(p) if p.opcode() == pdu::NOP_IN => {
                    let _ = c.answer_ping(&p);
                }
                Received::Pdu(_) => continue,
                _ => break None,
            }
        };
        match answer {
            Some(code) => {
                record_should(
                    cases,
                    SESSION,
                    label,
                    "Logout response 1 (CID not found)",
                    format!("response {code}"),
                    code == 1,
                );
            }
            None => record_info(
                cases,
                SESSION,
                label,
                "no logout response (the target may have closed the connection)",
            ),
        }
    }
    Ok(())
}

fn end_status(end: &Received) -> String {
    match end {
        Received::Pdu(p) => format!("{:#04x}", p.status()),
        other => format!("{other:?}"),
    }
}
