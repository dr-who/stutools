//! COMPARE AND WRITE between two hosts, the way VMFS takes its locks (SBC-5 5.3, 4.25 table 28: the
//! read, compare and write are one uninterrupted sequence). First what one host writes, the other
//! sees and compares against: A swaps the block from V to its ID, B reads A's ID, B's compare
//! against V miscompares, B's compare against A's ID swaps in B's, and A reads B's. Then the race:
//! A and B each send, at the same moment, COMPARE AND WRITE from V to their own ID, round after
//! round; every round exactly one wins (GOOD), the other miscompares, and the block holds the
//! winner's ID. Runs only where COMPARE AND WRITE works; the block gets its own data back.

use super::cases::{describe, record, Want};
use super::disk::Disk;
use super::lun_access::{open, read_block, unit_attentions, write_block, Creds};
use super::ops;
use super::path::Path;
use super::port::Port;
use super::pr_ports::{ISID_A, ISID_B};
use crate::compliance::Case;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Barrier;

const SECTION: &str = "ats";

/// Rounds of the race.
const ROUNDS: usize = 100;

fn block(len: usize, byte: u8) -> Vec<u8> {
    vec![byte; len]
}

/// COMPARE AND WRITE of one block from `from` to `to`: GOOD, miscompare, or what else came.
fn swap(s: &mut Port, lba: u64, from: &[u8], to: &[u8]) -> Result<bool, String> {
    let mut data = from.to_vec();
    data.extend_from_slice(to);
    match ops::compare_and_write(s, lba, 1, &data) {
        Ok(o) if o.status == 0 => Ok(true),
        Ok(o) if Want::Asc(0xe, 0x1d).matches(&o) => Ok(false),
        Ok(o) => Err(describe(&o)),
        Err(e) => Err(e),
    }
}

pub(super) fn run(
    path: &Path,
    initiator: &str,
    second: Option<&str>,
    creds: Creds,
    name: &str,
    disk: &Disk,
    cases: &mut Vec<Case>,
) {
    let (last_lba, block_len) = (disk.last_lba, disk.block_len);
    let label = |what: &str| format!("{name} {what}");
    let (Ok(mut a), Ok(mut b)) = (
        open(path, initiator, creds, ISID_A),
        open(path, second.unwrap_or(initiator), creds, ISID_B),
    ) else {
        return;
    };
    let _ = (unit_attentions(&mut a), unit_attentions(&mut b));
    let Ok(target) = u32::try_from(last_lba.saturating_sub(1)) else {
        return;
    };
    let lba = u64::from(target);
    let len = block_len as usize;
    let Ok(original) = read_block(&mut a, target, len) else {
        return;
    };
    let (v, id_a, id_b) = (block(len, 0x11), block(len, 0xaa), block(len, 0xbb));
    if write_block(&mut a, target, &v).is_err() || swap(&mut a, lba, &v, &v) != Ok(true) {
        let _ = write_block(&mut a, target, &original);
        return;
    }

    // What one host writes, the other sees and compares against.
    let steps: [(&str, bool); 4] = [
        (
            "A swaps V for its ID",
            swap(&mut a, lba, &v, &id_a) == Ok(true),
        ),
        (
            "B reads A's ID",
            read_block(&mut b, target, len).is_ok_and(|d| d == id_a),
        ),
        (
            "B's compare against V miscompares",
            swap(&mut b, lba, &v, &id_b) == Ok(false),
        ),
        (
            "B's compare against A's ID swaps in B's",
            swap(&mut b, lba, &id_a, &id_b) == Ok(true),
        ),
    ];
    for (what, ok) in steps {
        record(
            cases,
            SECTION,
            label(&format!("two hosts: {what}")),
            "as SBC-5 5.3 says",
            if ok { "yes" } else { "no" },
            ok,
        );
    }
    let seen = read_block(&mut a, target, len).is_ok_and(|d| d == id_b);
    record(
        cases,
        SECTION,
        label("two hosts: A reads B's ID"),
        "B's ID",
        if seen { "B's ID" } else { "other" },
        seen,
    );

    // The race: each host opens its own session in its own thread; a barrier lines up each round
    // (A resets the block, both swap at once, A reads what won).
    let _ = write_block(&mut a, target, &original);
    let _ = (a.logout(), b.logout());
    let gate = Barrier::new(2);
    let abort = AtomicBool::new(false);
    type Round = (Result<bool, String>, Option<Vec<u8>>);
    let racer = |who: &str, isid: [u8; 6], mine: &[u8], resets: bool| -> Vec<Round> {
        let port = open(path, who, creds, isid);
        if port.is_err() {
            abort.store(true, Ordering::SeqCst);
        }
        gate.wait();
        let mut out = Vec::with_capacity(ROUNDS);
        let Ok(mut p) = port else {
            return out;
        };
        if abort.load(Ordering::SeqCst) {
            let _ = p.logout();
            return out;
        }
        let _ = unit_attentions(&mut p);
        for _ in 0..ROUNDS {
            if resets {
                let _ = write_block(&mut p, target, &v);
            }
            gate.wait();
            let r = swap(&mut p, lba, &v, mine);
            gate.wait();
            let now = if resets {
                read_block(&mut p, target, len).ok()
            } else {
                None
            };
            gate.wait();
            out.push((r, now));
        }
        let _ = p.logout();
        out
    };
    let (ra, rb) = std::thread::scope(|scope| {
        let ta = scope.spawn(|| racer(initiator, ISID_A, &id_a, true));
        let tb = scope.spawn(|| racer(second.unwrap_or(initiator), ISID_B, &id_b, false));
        (ta.join().unwrap_or_default(), tb.join().unwrap_or_default())
    });
    let mut one_winner = 0;
    let mut problems: Vec<String> = Vec::new();
    if ra.len() < ROUNDS || rb.len() < ROUNDS {
        problems.push("a host could not open its session".into());
    }
    for (round, ((a_said, now), (b_said, _))) in ra.iter().zip(&rb).enumerate() {
        match (a_said, b_said) {
            (Ok(true), Ok(false)) if now.as_deref() == Some(&id_a[..]) => one_winner += 1,
            (Ok(false), Ok(true)) if now.as_deref() == Some(&id_b[..]) => one_winner += 1,
            _ => problems.push(format!(
                "round {round}: A {}, B {}, block {}",
                show(a_said),
                show(b_said),
                match now {
                    Some(d) if *d == id_a => "A's ID",
                    Some(d) if *d == id_b => "B's ID",
                    Some(d) if *d == v => "V",
                    _ => "mixed or unread",
                }
            )),
        }
    }
    record(
        cases,
        SECTION,
        label(&format!(
            "two hosts race {ROUNDS} rounds: exactly one wins each"
        )),
        format!("{ROUNDS} rounds with one GOOD and one miscompare, the block the winner's"),
        if problems.is_empty() {
            format!("{one_winner} of {ROUNDS}")
        } else {
            format!("{one_winner} of {ROUNDS}; first: {}", problems[0])
        },
        problems.is_empty(),
    );
    if let Ok(mut a) = open(path, initiator, creds, ISID_A) {
        let _ = unit_attentions(&mut a);
        let _ = write_block(&mut a, target, &original);
        let _ = a.logout();
    }
}

fn show(r: &Result<bool, String>) -> String {
    match r {
        Ok(true) => "GOOD".into(),
        Ok(false) => "miscompare".into(),
        Err(e) => e.clone(),
    }
}
