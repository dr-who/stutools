//! The READ and WRITE CDB lengths of SBC-3: READ (10), (12), (16) and (32), and WRITE (10), (12), (16) and
//! (32) (SBC-3 5.11 to 5.14 and 5.33 to 5.36). A form the target serves must read back correctly: a WRITE
//! in any served form must read back, in READ (10), as what it wrote, and every served READ form must return
//! what WRITE (10) put there. A form the target does not implement must be refused as an invalid command
//! operation code (or, for a service action it lacks, an invalid field in the CDB), and REPORT SUPPORTED
//! OPERATION CODES must say the same (SPC-3 6.23.1, table 159). The 32-byte forms are variable length CDBs
//! with a service action (SPC-3 4.3.3). The test blocks are the last two blocks of the logical unit, read
//! first and written back at the end, so the logical unit's data is unchanged.

use super::block_io::cdb10;
use super::cases::{describe, record, refused};
use super::disk::{label, Disk, SECTION};
use super::operation_codes::rsoc_one;
use super::port::Port;
use super::session::Outcome;
use crate::compliance::Case;
use crate::Result;

const READ_10: u8 = 0x28;
const WRITE_10: u8 = 0x2a;
const READ_12: u8 = 0xa8;
const WRITE_12: u8 = 0xaa;
const READ_16: u8 = 0x88;
const WRITE_16: u8 = 0x8a;
const VARIABLE_LENGTH: u8 = 0x7f;
const READ_32_SERVICE: u16 = 0x0009;
const WRITE_32_SERVICE: u16 = 0x000b;
/// ADDITIONAL CDB LENGTH of a 32-byte CDB: the bytes after byte 7 (SPC-3 4.3.3).
const ADDITIONAL_LENGTH: u8 = 0x18;
/// SUPPORT 011b and 101b: supported in conformance with a standard, or vendor specific (SPC-3 table 159).
const SUPPORT_SUPPORTED: [u8; 2] = [0x03, 0x05];
const GOOD: u8 = 0;

/// Blocks each form moves.
const BLOCKS: u32 = 2;

/// The forms the cases name: their CDB length and the clauses that define them.
#[derive(Clone, Copy)]
enum Form {
    Ten,
    Twelve,
    Sixteen,
    ThirtyTwo,
}

impl Form {
    fn name(self) -> &'static str {
        match self {
            Form::Ten => "10",
            Form::Twelve => "12",
            Form::Sixteen => "16",
            Form::ThirtyTwo => "32",
        }
    }

    fn read_clause(self) -> &'static str {
        match self {
            Form::Ten => "SBC-3 5.11",
            Form::Twelve => "SBC-3 5.12",
            Form::Sixteen => "SBC-3 5.13",
            Form::ThirtyTwo => "SBC-3 5.14",
        }
    }

    fn write_clause(self) -> &'static str {
        match self {
            Form::Ten => "SBC-3 5.33",
            Form::Twelve => "SBC-3 5.34",
            Form::Sixteen => "SBC-3 5.35",
            Form::ThirtyTwo => "SBC-3 5.36",
        }
    }

    /// The operation code and, for the variable length form, the service action.
    fn operation(self, write: bool) -> (u8, Option<u16>) {
        match (self, write) {
            (Form::Ten, false) => (READ_10, None),
            (Form::Ten, true) => (WRITE_10, None),
            (Form::Twelve, false) => (READ_12, None),
            (Form::Twelve, true) => (WRITE_12, None),
            (Form::Sixteen, false) => (READ_16, None),
            (Form::Sixteen, true) => (WRITE_16, None),
            (Form::ThirtyTwo, false) => (VARIABLE_LENGTH, Some(READ_32_SERVICE)),
            (Form::ThirtyTwo, true) => (VARIABLE_LENGTH, Some(WRITE_32_SERVICE)),
        }
    }

    fn cdb(self, write: bool, lba: u64, count: u32) -> Vec<u8> {
        let (op, service) = self.operation(write);
        match self {
            Form::Ten => cdb10(op, 0, lba as u32, count as u16).to_vec(),
            Form::Twelve => {
                let mut c = vec![op, 0];
                c.extend_from_slice(&(lba as u32).to_be_bytes());
                c.extend_from_slice(&count.to_be_bytes());
                c.extend_from_slice(&[0, 0]);
                c
            }
            Form::Sixteen => {
                let mut c = vec![op, 0];
                c.extend_from_slice(&lba.to_be_bytes());
                c.extend_from_slice(&count.to_be_bytes());
                c.extend_from_slice(&[0, 0]);
                c
            }
            Form::ThirtyTwo => {
                let mut c = vec![op, 0, 0, 0, 0, 0, 0, ADDITIONAL_LENGTH];
                c.extend_from_slice(&service.unwrap_or(0).to_be_bytes());
                c.extend_from_slice(&[0, 0]);
                c.extend_from_slice(&lba.to_be_bytes());
                c.extend_from_slice(&[0; 8]);
                c.extend_from_slice(&count.to_be_bytes());
                c
            }
        }
    }
}

/// What a READ that returned other data gave, next to what READ (10) of the same blocks gives now: whether the
/// reply itself was wrong, or the blocks changed.
fn mismatch_detail(
    s: &mut Port,
    outcome: &Result<Outcome>,
    data: &[u8],
    lba: u64,
    total: usize,
) -> String {
    let hex = |b: &[u8]| {
        b.iter()
            .take(8)
            .map(|x| format!("{x:02x}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let got = match outcome {
        Ok(o) => format!("first bytes {}", hex(&o.data)),
        Err(e) => format!("no data ({e})"),
    };
    let again = match read(s, Form::Ten, lba, total) {
        Ok(o) if o.data == data => {
            "READ (10) of the same blocks now returns the written data".to_string()
        }
        Ok(o) => format!(
            "READ (10) of the same blocks now returns other data, first bytes {}",
            hex(&o.data)
        ),
        Err(e) => format!("READ (10) gave no data ({e})"),
    };
    format!(
        "other data: {got}, expected first bytes {}; {again}",
        hex(data)
    )
}

/// A pattern for the test blocks: each byte says where it is, so a misplaced block cannot read back as
/// right.
fn pattern(len: usize, seed: u8) -> Vec<u8> {
    (0..len * BLOCKS as usize)
        .map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed))
        .collect()
}

fn write(s: &mut Port, form: Form, lba: u64, data: &[u8]) -> Result<Outcome> {
    s.command(&form.cdb(true, lba, BLOCKS), 0, Some(data))
}

fn read(s: &mut Port, form: Form, lba: u64, len: usize) -> Result<Outcome> {
    s.command(&form.cdb(false, lba, BLOCKS), len as u32, None)
}

/// Whether the transport could not send the command at all (recorded as not run, not as an answer).
fn not_run(outcome: &Result<Outcome>) -> bool {
    matches!(outcome, Err(e) if e.to_string().starts_with("not run:"))
}

/// Whether the answer is GOOD.
fn served(outcome: &Result<Outcome>) -> bool {
    matches!(outcome, Ok(o) if o.status == GOOD)
}

/// Whether the answer is the refusal a target gives for a form it does not implement.
fn not_implemented(outcome: &Result<Outcome>, form: Form, write: bool) -> bool {
    let service = form.operation(write).1.is_some();
    matches!(outcome, Ok(o) if refused(o, service))
}

/// Records what the target answered for one form: served, refused as not implemented, or something else
/// (a failure).
fn record_answer(
    cases: &mut Vec<Case>,
    at: &dyn Fn(&str) -> String,
    clause: &str,
    form: Form,
    write: bool,
    outcome: &Result<Outcome>,
) -> bool {
    let verb = if write { "WRITE" } else { "READ" };
    let expected =
        "GOOD, or CHECK CONDITION 5/20/00 (5/24/00 for a service action) if not implemented";
    let actual = match outcome {
        Ok(o) => describe(o),
        Err(e) => e.to_string(),
    };
    let ok = served(outcome) || not_implemented(outcome, form, write);
    record(
        cases,
        SECTION,
        at(&format!(
            "{clause} {verb} ({}) answered correctly",
            form.name()
        )),
        expected,
        actual,
        ok,
    );
    served(outcome)
}

/// Checks REPORT SUPPORTED OPERATION CODES against what the target did for one operation (SPC-3 6.23.1,
/// table 159): a served operation must be reported supported, a refused one not supported.
fn agrees(
    cases: &mut Vec<Case>,
    s: &mut Port,
    at: &dyn Fn(&str) -> String,
    clause: &str,
    form: Form,
    write: bool,
    served_here: bool,
) {
    let (op, service) = form.operation(write);
    let reported = rsoc_one(s, op, service)
        .ok()
        .and_then(|o| o.data.get(1).map(|b| b & 0x07));
    let supported = reported.map(|support| SUPPORT_SUPPORTED.contains(&support));
    let verb = if write { "WRITE" } else { "READ" };
    record(
        cases,
        SECTION,
        at(&format!(
            "SPC-3 6.23.1 REPORT SUPPORTED OPERATION CODES agrees with {clause} {verb} ({})",
            form.name()
        )),
        if served_here {
            "supported (SUPPORT 011b or 101b)"
        } else {
            "not supported (SUPPORT 001b)"
        },
        match (supported, reported) {
            (Some(true), _) => "supported".to_string(),
            (Some(false), Some(support)) => format!("not supported (SUPPORT {support:03b}b)"),
            _ => "no answer".to_string(),
        },
        supported == Some(served_here),
    );
}

/// The fixed length forms: 6, 10, 12 and 16 bytes. Run with every other case of the target.
pub(super) fn run(s: &mut Port, name: &str, disk: &Disk, cases: &mut Vec<Case>) {
    run_forms(
        s,
        name,
        disk,
        cases,
        &[Form::Ten, Form::Twelve, Form::Sixteen],
    );
}

/// The variable length form, READ (32) and WRITE (32). A target that closes the connection on it loses the
/// session for everything after, so it runs last, on its own.
pub(super) fn run_variable_length(s: &mut Port, name: &str, disk: &Disk, cases: &mut Vec<Case>) {
    run_forms(s, name, disk, cases, &[Form::ThirtyTwo]);
}

fn run_forms(s: &mut Port, name: &str, disk: &Disk, cases: &mut Vec<Case>, forms: &[Form]) {
    let len = disk.block_len as usize;
    let Some(lba) = disk.last_lba.checked_sub(BLOCKS as u64) else {
        return;
    };
    let total = len * BLOCKS as usize;
    let Ok(original) = read(s, Form::Ten, lba, total).map(|o| o.data) else {
        return;
    };
    let at = |what: &str| label(name, what);

    // Each WRITE form in turn. A served form reads back in READ (10); a refused one must be reported so.
    for &form in forms {
        let data = pattern(len, form.name().len() as u8);
        let outcome = write(s, form, lba, &data);
        let did_serve = record_answer(cases, &at, form.write_clause(), form, true, &outcome);
        if did_serve {
            let back = read(s, Form::Ten, lba, total).map(|o| o.data);
            let same = back.is_ok_and(|b| b == data);
            record(
                cases,
                SECTION,
                at(&format!(
                    "{} WRITE ({}) read back by READ (10) returns what it wrote",
                    form.write_clause(),
                    form.name()
                )),
                "the written blocks",
                if same {
                    "the written blocks"
                } else {
                    "other data"
                },
                same,
            );
        }
        if not_run(&outcome) {
            continue;
        }
        agrees(cases, s, &at, form.write_clause(), form, true, did_serve);
    }

    // Each READ form of blocks WRITE (10) wrote.
    let data = pattern(len, 0x5a);
    if served(&write(s, Form::Ten, lba, &data)) {
        for &form in forms {
            let outcome = read(s, form, lba, total);
            let did_serve = record_answer(cases, &at, form.read_clause(), form, false, &outcome);
            if did_serve {
                let same = outcome.as_ref().is_ok_and(|o| o.data == data);
                let actual = if same {
                    "the written blocks".to_string()
                } else {
                    mismatch_detail(s, &outcome, &data, lba, total)
                };
                record(
                    cases,
                    SECTION,
                    at(&format!(
                        "{} READ ({}) returns what WRITE (10) wrote",
                        form.read_clause(),
                        form.name()
                    )),
                    "the written blocks",
                    actual,
                    same,
                );
            }
            if not_run(&outcome) {
                continue;
            }
            agrees(cases, s, &at, form.read_clause(), form, false, did_serve);
        }
    }
    let _ = write(s, Form::Ten, lba, &original);
}
