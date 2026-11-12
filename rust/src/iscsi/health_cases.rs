//! Health information, reported as information (never a failure): what the device says about its own
//! condition, read with standard commands. Temperature (LOG SENSE 0Dh, parameter 0000h), informational
//! exceptions (2Fh), self-test results (10h), the start-stop cycle counter (0Eh), and the Power Condition mode
//! page (1Ah), which says what power states the device claims (SPC-4 and SBC-3 4.21.4).

use super::cases::{describe, record_info};
use super::mode_page_cases::{mode_page_of, mode_sense_10};
use super::port::Port;
use crate::compliance::Case;

const SECTION: &str = "health";

/// The health readings of one target. Each one is recorded whether or not the device answers.
pub(super) fn run(s: &mut Port, name: &str, cases: &mut Vec<Case>) {
    power_condition(s, name, cases);
    log_page(s, name, 0x0d, "temperature (log 0Dh)", cases);
    log_page(s, name, 0x2f, "informational exceptions (log 2Fh)", cases);
    log_page(s, name, 0x10, "self-test results (log 10h)", cases);
    log_page(s, name, 0x0e, "start-stop cycle counter (log 0Eh)", cases);
}

fn power_condition(s: &mut Port, name: &str, cases: &mut Vec<Case>) {
    let what = format!("{name} health: power condition mode page (1Ah)");
    match mode_sense_10(s, 0, 0x1a, 0, true) {
        Ok(o) if o.status == 0 => match mode_page_of(&o.data, true) {
            Some(page) => record_info(cases, SECTION, what, format!("page {}", hex(page))),
            None => record_info(cases, SECTION, what, "not listed".to_string()),
        },
        Ok(o) => record_info(cases, SECTION, what, format!("refused: {}", describe(&o))),
        Err(e) => record_info(cases, SECTION, what, format!("no answer: {e}")),
    }
}

fn log_page(s: &mut Port, name: &str, page: u8, what: &str, cases: &mut Vec<Case>) {
    let label = format!("{name} health: {what}");
    // LOG SENSE: PC 01b (cumulative values), the page, an allocation length of 512.
    let cdb = [0x4d, 0, 0x40 | page, 0, 0, 0, 0, 0x02, 0x00, 0];
    match s.command(&cdb, 512, None) {
        Ok(o) if o.status == 0 && o.data.len() >= 4 => {
            let length = u16::from_be_bytes([o.data[2], o.data[3]]) as usize;
            let body = &o.data[4..o.data.len().min(4 + length)];
            let text = if page == 0x0d {
                temperature(body)
            } else {
                format!("{} bytes: {}", body.len(), hex(&body[..body.len().min(16)]))
            };
            record_info(cases, SECTION, label, text);
        }
        Ok(o) => record_info(
            cases,
            SECTION,
            label,
            format!("not supported: {}", describe(&o)),
        ),
        Err(e) => record_info(cases, SECTION, label, format!("no answer: {e}")),
    }
}

/// The temperature parameter 0000h (SPC-4 temperature log page): its value is the temperature in degrees C,
/// 0FFh when it is not known.
fn temperature(body: &[u8]) -> String {
    // A parameter: code (2 bytes), control, length, then the value.
    match body {
        [0, 0, _, len, value, ..] if *len >= 2 => match value {
            0xff => "temperature not known".into(),
            t => format!("{t} C"),
        },
        _ => format!(
            "no temperature parameter: {}",
            hex(&body[..body.len().min(16)])
        ),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}
