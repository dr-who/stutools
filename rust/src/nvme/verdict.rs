//! How the NVMe cases judge a completion: the status codes they expect, as (status code type, status
//! code) pairs, and the two ways a completion becomes a case. `status_case` passes on one of the
//! expected statuses; `rev_case` also lets a controller refuse a capability newer than the NVMe
//! revision it implements (its VS property).

use super::revisions::Rev;
use super::sqe::Completion;
use crate::compliance::{case, Case};
use crate::Result;

/// Status codes as (status code type, status code). SCT 0 is the generic command status (Base 2.1
/// Figure 102; NVM 1.1 Figure 17 for 80h).
pub const SUCCESS: (u8, u8) = (0, 0x00);
pub const INVALID_OPCODE: (u8, u8) = (0, 0x01);
pub const INVALID_FIELD: (u8, u8) = (0, 0x02);
pub const INVALID_NAMESPACE: (u8, u8) = (0, 0x0b);
pub const HOST_ID_INCONSISTENT: (u8, u8) = (0, 0x18);
pub const LBA_OUT_OF_RANGE: (u8, u8) = (0, 0x80);
pub const RESERVATION_CONFLICT: (u8, u8) = (0, 0x83);
/// SCT 1, command specific: Invalid Log Page (Base 2.1 Figure 103, Get Log Page).
pub const INVALID_LOG_PAGE: (u8, u8) = (1, 0x09);
/// SCT 2, media and data integrity errors: Compare Failure (NVM 1.1 Figure 19).
pub const COMPARE_FAILURE: (u8, u8) = (2, 0x85);

/// A case that passes when a command completed with one of `want`, each a (status code type, status
/// code) pair; (0, 0) is success. Returns whether it passed.
pub fn status_case(
    cases: &mut Vec<Case>,
    section: &'static str,
    name: String,
    want: &[(u8, u8)],
    expected: &str,
    done: &Result<Completion>,
) -> bool {
    let (actual, passed) = match done {
        Ok(c) if c.ok() => ("success".to_string(), want.contains(&SUCCESS)),
        Ok(c) => {
            let got = c.code();
            (
                format!("status type {:#x} code {:#04x}", got.0, got.1),
                want.contains(&got),
            )
        }
        Err(e) => (e.clone(), false),
    };
    cases.push(case(section, name, expected.into(), actual, passed));
    passed
}

/// A case for `rev`, judged against the revision the controller implements (`vs`). When that revision
/// defines it, the command must end with one of `want`. When it does not, a refusal (Invalid Command
/// Opcode, Invalid Field, Invalid Log Page) passes too: the controller predates the capability.
#[allow(clippy::too_many_arguments)]
pub fn rev_case(
    cases: &mut Vec<Case>,
    section: &'static str,
    prefix: &str,
    rev: &Rev,
    vs: u32,
    want: &[(u8, u8)],
    expected: &str,
    done: &Result<Completion>,
) -> bool {
    let name = format!("{prefix} {}", rev.name());
    if rev.defined_in(vs) {
        return status_case(cases, section, name, want, expected, done);
    }
    let refused = matches!(done, Ok(c) if !c.ok() && [INVALID_OPCODE, INVALID_FIELD, INVALID_LOG_PAGE].contains(&c.code()));
    if refused {
        cases.push(case(
            section,
            name,
            format!("{expected}, or refused"),
            rev.absent(vs),
            true,
        ));
        return true;
    }
    status_case(
        cases,
        section,
        name,
        want,
        &format!("{expected}, or refused"),
        done,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_cases_match_type_and_code() {
        let mut cases = Vec::new();
        let mut b = [0u8; 16];
        b[14..16].copy_from_slice(&((LBA_OUT_OF_RANGE.1 as u16) << 1).to_le_bytes());
        let done = Ok(Completion::parse(&b));
        assert!(status_case(
            &mut cases,
            "nvme-io",
            "past".into(),
            &[LBA_OUT_OF_RANGE],
            "LBA Out of Range",
            &done
        ));
        assert!(!status_case(
            &mut cases,
            "nvme-io",
            "ok".into(),
            &[SUCCESS],
            "success",
            &done
        ));
        // The same code in another status code type is another status.
        b[14..16].copy_from_slice(&((1u16 << 9 | 0x80) << 1).to_le_bytes());
        let done = Ok(Completion::parse(&b));
        assert!(!status_case(
            &mut cases,
            "nvme-io",
            "type".into(),
            &[LBA_OUT_OF_RANGE],
            "LBA Out of Range",
            &done
        ));
        assert!(!cases[1].passed);
    }

    #[test]
    fn a_capability_newer_than_the_controller_may_be_refused() {
        let mut cases = Vec::new();
        let mut b = [0u8; 16];
        b[14..16].copy_from_slice(
            &((INVALID_LOG_PAGE.0 as u16) << 9 | (INVALID_LOG_PAGE.1 as u16) << 1).to_le_bytes(),
        );
        let refused = Ok(Completion::parse(&b));
        let ana = super::super::revisions::LOG_ANA;
        assert!(rev_case(
            &mut cases,
            "nvme-ana",
            "x",
            &ana,
            0x0001_0300,
            &[SUCCESS],
            "success",
            &refused
        ));
        assert_eq!(
            cases[0].actual,
            "not in NVMe 1.3, which the controller implements"
        );
        assert!(!rev_case(
            &mut cases,
            "nvme-ana",
            "x",
            &ana,
            0x0001_0400,
            &[SUCCESS],
            "success",
            &refused
        ));
        assert_eq!(cases[1].name, "x NVMe 1.4 ANA log page");
    }
}
