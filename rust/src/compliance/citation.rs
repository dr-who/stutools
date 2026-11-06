//! A case's text at one SCSI level. Cases are written against the latest editions held, so they cite
//! "SPC-6 5.14.7", "SPC-6 table 88", "SBC-5 5.18" or "SBC-5 annex A". At an older level each such
//! citation becomes the same clause in that level's edition ("SPC-3 5.6.6"), from `clause_map`; one
//! that edition does not have keeps its number and says so ("SPC-6 6.37 (not in SPC-3)"), and is
//! counted, so a case whose every citation is missing at a level is no requirement there. An edition
//! named without a clause ("SPC-6, SBC-5: ...") becomes the level's. The commands SPC-6 made
//! obsolete cite SPC-5 ("SPC-5 6.14"), which levels 3 and 4 move the same way. A clause the map does
//! not hold is left as written. Nothing else in the text changes.

use super::clause_map::{SBC, SPC, SPC5};
use super::scsi_level::ScsiLevel;

/// The text at a level, and how many clauses it cites and how many of them that level lacks.
#[derive(Debug, PartialEq)]
pub struct Cited {
    pub text: String,
    pub cited: usize,
    pub absent: usize,
}

/// Where a clause of the latest edition lives at `level`: None when the map does not hold it,
/// Some(None) when that edition has no such clause.
fn moved(latest: &str, clause: &str, level: ScsiLevel) -> Option<Option<String>> {
    let older = |to: &[Option<&str>], edition: u8| match to.get(usize::from(edition) - 3) {
        Some(c) => c.map(str::to_string),
        None => Some(clause.to_string()),
    };
    if latest == "SPC-6" {
        let (_, to) = SPC.iter().find(|(c, _)| *c == clause)?;
        Some(older(to, level.spc()))
    } else if latest == "SPC-5" {
        let (_, to) = SPC5.iter().find(|(c, _)| *c == clause)?;
        Some(older(to, level.spc().min(5)))
    } else {
        let (_, to) = SBC.iter().find(|(c, _)| *c == clause)?;
        Some(older(to, level.sbc()))
    }
}

/// The length of the clause reference at the start of `rest`: "5.14.7", "A.2", "table 88" or
/// "annex A" (either case for the word), or 0 when there is none.
fn reference(rest: &str) -> usize {
    let b = rest.as_bytes();
    let word = |w: &str| {
        rest.get(..w.len())
            .is_some_and(|p| p.eq_ignore_ascii_case(w))
    };
    if word("table ") {
        let digits = b[6..].iter().take_while(|c| c.is_ascii_digit()).count();
        return if digits > 0 { 6 + digits } else { 0 };
    }
    if word("annex ") {
        return if b.get(6).is_some_and(u8::is_ascii_uppercase)
            && !b.get(7).is_some_and(u8::is_ascii_alphanumeric)
        {
            7
        } else {
            0
        };
    }
    let first = match b.first() {
        Some(c) if c.is_ascii_digit() => b.iter().take_while(|c| c.is_ascii_digit()).count(),
        Some(c)
            if c.is_ascii_uppercase()
                && b.get(1) == Some(&b'.')
                && b.get(2).is_some_and(u8::is_ascii_digit) =>
        {
            1
        }
        _ => return 0,
    };
    let mut n = first;
    while b.get(n) == Some(&b'.') && b.get(n + 1).is_some_and(u8::is_ascii_digit) {
        n += 1 + b[n + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
    }
    n
}

/// `text` as cited at `level`.
pub fn cite(text: &str, level: ScsiLevel) -> Cited {
    let mut out = Cited {
        text: String::with_capacity(text.len()),
        cited: 0,
        absent: 0,
    };
    let mut rest = text;
    while let Some(at) = ["SPC-6", "SBC-5", "SPC-5"]
        .iter()
        .filter_map(|e| rest.find(e).map(|i| (i, *e)))
        .min()
    {
        let (i, latest) = at;
        out.text.push_str(&rest[..i]);
        rest = &rest[i + latest.len()..];
        let edition = match latest {
            "SPC-6" => format!("SPC-{}", level.spc()),
            "SPC-5" => format!("SPC-{}", level.spc().min(5)),
            _ => format!("SBC-{}", level.sbc()),
        };
        let len = rest.strip_prefix(' ').map_or(0, reference);
        if len == 0 {
            // A bare SPC-5 names that edition, not the latest one.
            out.text
                .push_str(if latest == "SPC-5" { latest } else { &edition });
            continue;
        }
        let written = &rest[1..1 + len];
        let key = written.to_ascii_lowercase();
        let key = if key.starts_with("table ") || key.starts_with("annex ") {
            format!("{}{}", &key[..6], &written[6..])
        } else {
            written.to_string()
        };
        match moved(latest, &key, level) {
            Some(Some(to)) => {
                out.cited += 1;
                let capital = written.starts_with(['T', 'A']) && to.starts_with(['t', 'a']);
                let to = if capital {
                    format!("{}{}", to[..1].to_ascii_uppercase(), &to[1..])
                } else {
                    to
                };
                out.text.push_str(&format!("{edition} {to}"));
            }
            Some(None) => {
                out.cited += 1;
                out.absent += 1;
                out.text
                    .push_str(&format!("{latest} {written} (not in {edition})"));
            }
            None => out.text.push_str(&format!("{latest} {written}")),
        }
        rest = &rest[1 + len..];
    }
    out.text.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(n: u8, text: &str) -> Cited {
        cite(text, ScsiLevel::new(n).unwrap())
    }

    #[test]
    fn a_clause_moves_to_the_levels_edition() {
        let c = at(3, "SPC-6 5.14.7 registering, SBC-5 5.18 READ (16)");
        assert_eq!(c.text, "SPC-3 5.6.6 registering, SBC-3 5.13 READ (16)");
        assert_eq!((c.cited, c.absent), (2, 0));
        assert_eq!(at(4, "SPC-6 table 88").text, "SPC-4 table 120");
        assert_eq!(at(4, "SPC-6 Table 88").text, "SPC-4 Table 120");
        assert_eq!(at(4, "SBC-5 annex A").text, "SBC-4 annex H");
        assert_eq!(
            at(5, "SBC-5 6.6.4 and SPC-6 6.30.").text,
            "SBC-5 6.6.4 and SPC-5 6.32."
        );
        assert_eq!(
            at(3, "SPC-5 6.14 MODE SENSE (6)").text,
            "SPC-3 6.9 MODE SENSE (6)"
        );
        assert_eq!(
            at(6, "SPC-5 6.14 MODE SENSE (6)").text,
            "SPC-5 6.14 MODE SENSE (6)"
        );
        assert_eq!(
            at(3, "Discovery 2016 comes from SPC-5").text,
            "Discovery 2016 comes from SPC-5"
        );
    }

    #[test]
    fn level_six_is_the_text_as_written() {
        let text = "SPC-6 5.18.2.6 and SBC-5 4.21.2.4 (SPC-6, SBC-5)";
        let c = at(6, text);
        assert_eq!(c.text, text);
        assert_eq!((c.cited, c.absent), (2, 0));
    }

    #[test]
    fn a_clause_the_edition_lacks_is_marked_and_counted() {
        let c = at(3, "SPC-6 A.2 Discovery 2016");
        assert_eq!(c.text, "SPC-6 A.2 (not in SPC-3) Discovery 2016");
        assert_eq!((c.cited, c.absent), (1, 1));
        assert_eq!(at(4, "SPC-6 A.2").absent, 1);
        assert_eq!(at(5, "SPC-6 A.2").text, "SPC-5 A.2");
    }

    #[test]
    fn a_bare_edition_becomes_the_levels_and_unknown_clauses_stay() {
        assert_eq!(
            at(3, "SPC-6, SBC-5: a new disk").text,
            "SPC-3, SBC-3: a new disk"
        );
        assert_eq!(at(4, "SPC-6 INQUIRY").text, "SPC-4 INQUIRY");
        let c = at(3, "SPC-6 9.99 is not held");
        assert_eq!(c.text, "SPC-6 9.99 is not held");
        assert_eq!(c.cited, 0);
        assert_eq!(
            at(3, "SBC-4 annex H, SPC-3 or later").text,
            "SBC-4 annex H, SPC-3 or later"
        );
    }
}
