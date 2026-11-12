//! `--hold` and `--hold-probe`: a persistent reservation held across a fault, driven by hand or by a script.
//!
//! `--hold TYPE` logs in to the path (`HOST/IQN`, no CHAP), registers the reservation key A and takes a
//! reservation of TYPE with it, then serves stdin commands, one per line: `status` (the holder and the
//! registered keys, as the target reports them), `release` (RELEASE by the holder), `quit` (CLEAR, then
//! logout). A process that is killed instead leaves its registration and reservation on the target: that is
//! what a session-kill scenario tests. `--hold-probe` logs in and reports `status` only, so a second process
//! can see what survived a fault (SPC-3 5.6.4: registrations and reservations persist until released).

use std::io::{BufRead, Write};

use super::lun_access::open;
use super::ops;
use super::path::parse_path;
use super::pr_commands::{out, read_holder, read_keys, show_holder, show_keys};
use super::pr_ports::{ISID_A, KEY_A};
use crate::{cli::Config, Result};

/// Runs the hold mode the configuration asks for. The first `--iscsi` entry is the path.
pub fn run(config: &Config) -> Result<()> {
    let text = config
        .iscsi
        .first()
        .ok_or("--hold needs one --iscsi HOST/IQN path")?;
    let path = parse_path(text)?;
    let initiator = config
        .iscsi_initiator
        .as_deref()
        .ok_or("--hold needs --iscsi-initiator IQN")?;
    let mut s = open(&path, initiator, None, ISID_A)?;
    match config.hold {
        None => {
            println!(
                "status: {}; keys: {}",
                show_holder(&read_holder(&mut s)),
                show_keys(&read_keys(&mut s))
            );
            let _ = s.logout();
            Ok(())
        }
        Some(kind) => hold(s, kind),
    }
}

/// Registers KEY_A, takes a reservation of `kind` with it, then serves stdin commands until `quit` or end of
/// input. Each reply is one line on stdout, so a script can read it.
fn hold(mut s: super::port::Port, kind: u8) -> Result<()> {
    let s = &mut s;
    // SPC-3 6.12.3: a nexus that is already registered (a session killed earlier left its key) must send the
    // registered key as the RESERVATION KEY; only a new nexus sends zero.
    let already = matches!(read_keys(s), Ok((_, keys)) if keys.contains(&KEY_A));
    let registered = if already {
        out(s, ops::PR_REGISTER, 0, KEY_A, KEY_A)
    } else {
        out(s, ops::PR_REGISTER, 0, 0, KEY_A)
    };
    if !matches!(&registered, Ok(o) if o.status == 0) {
        return Err(format!("REGISTER refused: {registered:?}"));
    }
    let reserved = out(s, ops::PR_RESERVE, kind, KEY_A, 0);
    if !matches!(&reserved, Ok(o) if o.status == 0) {
        return Err(format!("RESERVE refused: {reserved:?}"));
    }
    println!("held: type {kind}, key {KEY_A:#x}");
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    for line in std::io::stdin().lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        match line.trim() {
            "status" => println!(
                "status: {}; keys: {}",
                show_holder(&read_holder(s)),
                show_keys(&read_keys(s))
            ),
            "release" => {
                let r = out(s, ops::PR_RELEASE, kind, KEY_A, 0);
                println!("release: {}", describe_out(&r));
            }
            "quit" => {
                let r = out(s, ops::PR_CLEAR, 0, KEY_A, 0);
                println!("clear: {}", describe_out(&r));
                break;
            }
            "" => {}
            other => println!("unknown command: {other}"),
        }
        std::io::stdout().flush().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn describe_out(r: &Result<super::session::Outcome>) -> String {
    match r {
        Ok(o) if o.status == 0 => "GOOD".into(),
        Ok(o) => super::cases::describe(o),
        Err(e) => e.clone(),
    }
}
