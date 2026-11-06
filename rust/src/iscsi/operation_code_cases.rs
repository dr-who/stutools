//! The command support cases of the `disk` section: REPORT SUPPORTED OPERATION CODES (SPC-6 6.32) in
//! all its forms, its list agreeing with the one-command answers, and REPORT SUPPORTED TASK
//! MANAGEMENT FUNCTIONS (6.35) with and without REPD.

use super::cases::{expect, record, Want, REFUSED_SA};
use super::disk::{label, Disk, SECTION};
use super::operation_codes::{parse_rsoc, rsoc_all, rsoc_one};
use super::port::Port;
use super::scsi_fields::be16;
use super::seen::Seen;
use super::session::Outcome;
use crate::compliance::Case;
use crate::Result;

/// SPC-6 6.32 REPORT SUPPORTED OPERATION CODES and 6.35 REPORT SUPPORTED TASK MANAGEMENT FUNCTIONS.
pub(super) fn operation_codes(
    s: &mut Port,
    name: &str,
    cases: &mut Vec<Case>,
    seen: &mut Seen,
    disk: &mut Disk,
) {
    let mut wants = vec![Want::Good];
    wants.extend(REFUSED_SA);
    let answer = expect(
        cases,
        SECTION,
        label(name, "REPORT SUPPORTED OPERATION CODES, all, RCTD"),
        &wants,
        rsoc_all(s, true),
    );
    let rctd = matches!(&answer.1, Some(o) if o.status == 0);
    seen.fact("RSOC RCTD", rctd);
    let list = if rctd {
        seen.answer("REPORT SUPPORTED OPERATION CODES", SECTION, &answer);
        answer.1.map(|o| o.data)
    } else {
        let again = expect(
            cases,
            SECTION,
            label(name, "REPORT SUPPORTED OPERATION CODES, all"),
            &wants,
            rsoc_all(s, false),
        );
        seen.answer("REPORT SUPPORTED OPERATION CODES", SECTION, &again);
        again.1.filter(|o| o.status == 0).map(|o| o.data)
    };
    seen.fact("RSOC 000b", list.is_some());
    disk.rsoc = list.as_deref().map(parse_rsoc);
    if let Some(rsoc) = &disk.rsoc {
        // The commands this section has already seen answer GOOD must be listed.
        let required: [(u8, Option<u16>, &str); 6] = [
            (0x12, None, "INQUIRY"),
            (0x25, None, "READ CAPACITY (10)"),
            (0x9e, Some(0x10), "READ CAPACITY (16)"),
            (0x5a, None, "MODE SENSE (10)"),
            (0xa3, Some(0x0c), "REPORT SUPPORTED OPERATION CODES"),
            (0x00, None, "TEST UNIT READY"),
        ];
        let missing: Vec<&str> = required
            .iter()
            .filter(|(op, sa, _)| {
                !rsoc
                    .iter()
                    .any(|c| c.opcode == *op && (sa.is_none() || c.service_action == *sa))
            })
            .map(|(_, _, n)| *n)
            .collect();
        record(
            cases,
            SECTION,
            label(
                name,
                "REPORT SUPPORTED OPERATION CODES lists the discovery commands",
            ),
            "all listed",
            if missing.is_empty() {
                format!("{} commands listed", rsoc.len())
            } else {
                format!("missing {}", missing.join(", "))
            },
            missing.is_empty(),
        );
        // SPC-6 6.32.3: one_command for READ (10) reports SUPPORT 011b and a 10-byte CDB usage map.
        let one = rsoc_one(s, 0x28, None);
        let ok = matches!(&one, Ok(o) if o.status == 0 && o.data[1] & 0x07 == 0x03 && be16(&o.data, 2) == 10 && o.data[4] == 0x28);
        seen.fact("RSOC 001b", ok);
        record(
            cases,
            SECTION,
            label(name, "one_command (001b) for READ (10)"),
            "SUPPORT 011b, CDB SIZE 10",
            show_one(&one),
            ok,
        );
        // SPC-6 6.32.3: 010b names an operation code with service actions: READ CAPACITY (16).
        let both = rsoc_one(s, 0x9e, Some(0x10));
        let ok = matches!(&both, Ok(o) if o.status == 0 && o.data[1] & 0x07 == 0x03 && be16(&o.data, 2) == 16);
        seen.fact("RSOC 010b", ok);
        record(
            cases,
            SECTION,
            label(name, "one_command (010b) for READ CAPACITY (16)"),
            "SUPPORT 011b, CDB SIZE 16",
            show_one(&both),
            ok,
        );
        // FORMAT UNIT is mandatory (SBC-5 table 34) and destroys data, so only RSOC is asked about it.
        disk.format_unit_support = rsoc_one(s, 0x04, None)
            .ok()
            .filter(|o| o.status == 0)
            .map(|o| o.data[1] & 0x07);
    }
    let a = 16u32.to_be_bytes();
    // SPC-6 6.35: the basic form first, so a device that has the command but not REPD (INVALID FIELD
    // IN CDB for that bit) is known to support it.
    let basic = s.command(
        &[0xa3, 0x0d, 0, 0, 0, 0, a[0], a[1], a[2], a[3], 0, 0],
        16,
        None,
    );
    let mut wants = vec![Want::Good];
    wants.extend(REFUSED_SA);
    let answer = expect(
        cases,
        SECTION,
        label(name, "REPORT SUPPORTED TASK MANAGEMENT FUNCTIONS"),
        &wants,
        basic,
    );
    seen.answer(
        "REPORT SUPPORTED TASK MANAGEMENT FUNCTIONS",
        SECTION,
        &answer,
    );
    if let Some(o) = answer.1.filter(|o| o.status == 0) {
        disk.tmf_bits = Some([o.data[0], o.data[1]]);
    }
    let repd = s.command(
        &[0xa3, 0x0d, 0x80, 0, 0, 0, a[0], a[1], a[2], a[3], 0, 0],
        16,
        None,
    );
    let mut wants = vec![Want::Good];
    wants.extend(REFUSED_SA);
    let answer = expect(
        cases,
        SECTION,
        label(name, "REPORT SUPPORTED TASK MANAGEMENT FUNCTIONS, REPD"),
        &wants,
        repd,
    );
    seen.answer(
        "REPORT SUPPORTED TASK MANAGEMENT FUNCTIONS",
        SECTION,
        &answer,
    );
    if let Some(o) = answer.1.filter(|o| o.status == 0) {
        // SPC-6 table 267: with REPD the additional data length is 0Ch.
        seen.fact("RSTMF REPD", o.data.get(3) == Some(&0x0c));
        disk.tmf_bits = Some([o.data[0], o.data[1]]);
        record(
            cases,
            SECTION,
            label(name, "REPD answer is the extended format"),
            "ADDITIONAL DATA LENGTH 0Ch",
            format!("{:02x}h", o.data[3]),
            o.data.get(3) == Some(&0x0c),
        );
    } else {
        seen.fact("RSTMF REPD", false);
    }
}

pub(super) fn show_one(r: &Result<Outcome>) -> String {
    match r {
        Ok(o) if o.status == 0 => format!(
            "SUPPORT {:03b}b, CDB SIZE {}",
            o.data[1] & 0x07,
            be16(&o.data, 2)
        ),
        Ok(o) => super::cases::describe(o),
        Err(e) => e.clone(),
    }
}
