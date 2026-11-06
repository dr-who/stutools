//! The header digest case of the `session` section: a header digest covers the whole header
//! segment group, the Basic Header Segment and any Additional Header Segment after it (RFC 7143 11.2,
//! 11.2.3). A command with a 32-byte CDB carries its last 16 bytes in an Extended CDB AHS, so on a
//! session with HeaderDigest CRC32C its digest covers the AHS too. The case sends READ (32) on such a
//! session and passes on any SCSI answer, GOOD or CHECK CONDITION alike; a target that drops the
//! connection instead fails it, because it checked the digest over the BHS alone or cannot take the
//! AHS. When the target does not negotiate CRC32C header digests, the case only reports that.

use super::cases::{describe, record, record_info};
use super::command_probes::cdb32;
use super::login;
use super::lun_access::{open, unit_attentions, Creds};
use super::path::Path;
use crate::compliance::Case;

const SECTION: &str = "session";

/// The READ (32) service action (SBC-5 5.19).
const READ_32: u16 = 0x0009;

pub(super) fn run(
    path: &Path,
    initiator: &str,
    creds: Creds,
    name: &str,
    block_len: u32,
    cases: &mut Vec<Case>,
) {
    let label = format!("{name} header digest over the Extended CDB AHS (RFC 7143 11.2.3)");
    let mut s = match open(path, initiator, creds, login::random_isid()) {
        Ok(s) => s,
        Err(err) => {
            record(cases, SECTION, label, "login ok", err, false);
            return;
        }
    };
    if !s.negotiated().is_some_and(|n| n.header_digest) {
        record_info(
            cases,
            SECTION,
            label,
            "not tested: the target chose HeaderDigest None",
        );
        let _ = s.logout();
        return;
    }
    let _ = unit_attentions(&mut s);
    let answer = s.command(&cdb32(READ_32, 0, 1), block_len, None);
    let passed = answer.is_ok();
    let actual = match &answer {
        Ok(o) => describe(o),
        Err(e) => format!("connection dropped ({e})"),
    };
    record(
        cases,
        SECTION,
        label,
        "a SCSI answer (GOOD or CHECK CONDITION)",
        actual,
        passed,
    );
    if passed {
        let _ = s.logout();
    }
}
