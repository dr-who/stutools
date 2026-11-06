//! The SCSI level a report judges against: level n is SPC-n with SBC-n, and level 6 is SPC-6 with
//! SBC-5, the latest block edition. `--scsilevel` takes one level, a range "3..6" or a list "3,5", and
//! may be repeated; without it a run judges at level 3. A level outside 3 to 6, the editions held, is
//! refused with what is accepted.

use std::fmt;

/// The levels spit holds the standards for.
const LOWEST: u8 = 3;
const HIGHEST: u8 = 6;

/// One SCSI level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScsiLevel(u8);

impl ScsiLevel {
    pub const DEFAULT: ScsiLevel = ScsiLevel(LOWEST);

    pub fn new(n: u8) -> Option<ScsiLevel> {
        (LOWEST..=HIGHEST).contains(&n).then_some(ScsiLevel(n))
    }

    /// The SPC edition: SPC-3 to SPC-6.
    pub fn spc(self) -> u8 {
        self.0
    }

    /// The SBC edition: SBC-3 to SBC-5.
    pub fn sbc(self) -> u8 {
        self.0.min(5)
    }

    /// The SPC edition with its INCITS number.
    pub fn spc_title(self) -> &'static str {
        match self.0 {
            3 => "SPC-3 (INCITS 408-2005)",
            4 => "SPC-4 (INCITS 513-2015)",
            5 => "SPC-5 (INCITS 502-2019)",
            _ => "SPC-6 (INCITS 566-2025)",
        }
    }

    /// The SBC edition with its INCITS number.
    pub fn sbc_title(self) -> &'static str {
        match self.sbc() {
            3 => "SBC-3 (INCITS 514-2014)",
            4 => "SBC-4 (INCITS 506-2021)",
            _ => "SBC-5 (INCITS 571-2025)",
        }
    }
}

impl fmt::Display for ScsiLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SCSI level {} (SPC-{}, SBC-{})",
            self.0,
            self.spc(),
            self.sbc()
        )
    }
}

/// The levels one `--scsilevel` value names, lowest first: "4", "3..6" or "3,5".
pub fn parse(v: &str) -> Result<Vec<ScsiLevel>, String> {
    let refused = || {
        format!("--scsilevel takes {LOWEST} to {HIGHEST}, a range such as {LOWEST}..{HIGHEST}, or a list such as 3,5 (got {v})")
    };
    let one = |s: &str| {
        s.trim()
            .parse::<u8>()
            .ok()
            .and_then(ScsiLevel::new)
            .ok_or_else(refused)
    };
    let mut out = Vec::new();
    for part in v.split(',') {
        match part.split_once("..") {
            Some((a, b)) => {
                let (a, b) = (one(a)?, one(b)?);
                if a > b {
                    return Err(refused());
                }
                out.extend((a.0..=b.0).map(ScsiLevel));
            }
            None => out.push(one(part)?),
        }
    }
    out.sort_unstable();
    out.dedup();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_level_pairs_spc_n_with_sbc_n_up_to_sbc_5() {
        let three = ScsiLevel::new(3).unwrap();
        assert_eq!((three.spc(), three.sbc()), (3, 3));
        let six = ScsiLevel::new(6).unwrap();
        assert_eq!((six.spc(), six.sbc()), (6, 5));
        assert_eq!(six.sbc_title(), "SBC-5 (INCITS 571-2025)");
        assert_eq!(ScsiLevel::DEFAULT, three);
        assert_eq!(three.to_string(), "SCSI level 3 (SPC-3, SBC-3)");
    }

    #[test]
    fn levels_come_as_one_a_range_or_a_list() {
        let n = |v: &str| {
            parse(v)
                .unwrap()
                .iter()
                .map(|l| l.spc())
                .collect::<Vec<_>>()
        };
        assert_eq!(n("4"), vec![4]);
        assert_eq!(n("3..6"), vec![3, 4, 5, 6]);
        assert_eq!(n("5,3,5"), vec![3, 5]);
        assert_eq!(n("3..4,6"), vec![3, 4, 6]);
    }

    #[test]
    fn levels_outside_the_editions_held_are_refused() {
        for v in ["2", "7", "3..7", "6..3", "x", ""] {
            let err = parse(v).unwrap_err();
            assert!(err.starts_with("--scsilevel takes 3 to 6"), "{v}: {err}");
        }
    }
}
