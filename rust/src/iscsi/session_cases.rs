//! The `session` cases of `--compliance` (RFC 7143 6.3, 11.14, 11.15): five login and logout cycles
//! on one target, three sessions open at once with distinct session handles (TSIH), and a login after
//! every session has logged out. The NOP-Out ping and session reinstatement cases live here too; the
//! header digest over an Extended CDB AHS is in `header_digest`.

use super::login::{self, reason};
use super::lun_access::{short_name, Creds};
use super::path::Path;
use crate::compliance::{case, Case};

/// Five login and logout cycles, three sessions at once with distinct handles, and a login after.
pub(super) fn run(p: &Path, initiator: &str, creds: Creds, cases: &mut Vec<Case>) {
    let name = short_name(p).to_string();
    let login_one = || login::login(&p.portal, &p.target, initiator, creds, "CRC32C,None");
    let cycle = |label: String, cases: &mut Vec<Case>| {
        let (actual, passed) = match login_one() {
            Ok(s) => {
                let tsih = s.negotiated.tsih;
                let out = s.logout();
                (
                    format!(
                        "tsih {tsih}, logout {}",
                        if out.is_ok() { "ok" } else { "failed" }
                    ),
                    out.is_ok(),
                )
            }
            Err(err) => (format!("refused ({})", reason(&err)), false),
        };
        cases.push(case(
            "session",
            label,
            "login ok, logout ok".into(),
            actual,
            passed,
        ));
    };
    for k in 1..=5 {
        cycle(format!("{name} login/logout cycle {k}"), cases);
    }
    let mut open = Vec::new();
    let mut failure = None;
    for _ in 0..3 {
        match login_one() {
            Ok(s) => open.push(s),
            Err(err) => {
                failure = Some(reason(&err));
                break;
            }
        }
    }
    let mut handles: Vec<u16> = open.iter().map(|s| s.negotiated.tsih).collect();
    handles.sort_unstable();
    handles.dedup();
    let distinct = handles.len() == 3;
    let mut logouts_ok = true;
    for s in open {
        logouts_ok &= s.logout().is_ok();
    }
    let actual = match &failure {
        Some(reason) => format!("refused ({reason})"),
        None => format!(
            "3 sessions, {} distinct handles, logouts {}",
            handles.len(),
            if logouts_ok { "ok" } else { "failed" }
        ),
    };
    cases.push(case(
        "session",
        format!("{name} 3 sessions at once"),
        "3 distinct handles, logouts ok".into(),
        actual,
        distinct && logouts_ok && failure.is_none(),
    ));
    cycle(format!("{name} login after all logouts"), cases);

    // RFC 7143 11.18, 11.19: a NOP-Out ping with an initiator task tag is answered by a NOP-In with the
    // same tag and the same data. Windows pings idle sessions this way.
    if let Ok(mut s) = login_one() {
        let data = b"spit compliance ping, 32 bytes.\0";
        let echo = s.ping(data);
        cases.push(case(
            "session",
            format!("{name} NOP-Out ping echoed"),
            "NOP-In with the same data".into(),
            match &echo {
                Ok(d) if d == data => "same data".into(),
                Ok(d) => format!("{} other bytes", d.len()),
                Err(e) => e.clone(),
            },
            matches!(&echo, Ok(d) if d == data),
        ));
        let _ = s.logout();
    }

    // RFC 7143 6.3.5: a login with the ISID of a session that is still open (TSIH 0) reinstates it: the
    // old session is closed. Windows logs in again this way when a path comes back.
    let isid = login::random_isid();
    let first = login::login_as(
        &p.portal,
        &p.target,
        initiator,
        isid,
        creds,
        "CRC32C,None",
        "5,6,7,8",
    );
    if let Ok(mut old) = first {
        let again = login::login_as(
            &p.portal,
            &p.target,
            initiator,
            isid,
            creds,
            "CRC32C,None",
            "5,6,7,8",
        );
        cases.push(case(
            "session",
            format!("{name} login with the ISID of an open session"),
            "login ok".into(),
            match &again {
                Ok(_) => "login ok".into(),
                Err(e) => format!("refused ({})", reason(e)),
            },
            again.is_ok(),
        ));
        let old_answer = old.command(&[0, 0, 0, 0, 0, 0], 0, None);
        cases.push(case(
            "session",
            format!("{name} the reinstated session is closed"),
            "the old session no longer answers".into(),
            match &old_answer {
                Ok(_) => "the old session still answers".into(),
                Err(_) => "closed".into(),
            },
            old_answer.is_err(),
        ));
        if let Ok(new) = again {
            let _ = new.logout();
        }
    }
}
