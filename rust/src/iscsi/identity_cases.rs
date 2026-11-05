//! The `identity` cases of `--compliance` (SPC-6 6.7, 7.7.6): whether the paths reach one LUN. A path's
//! LUN key is its association 0 designators (VPD 83h), which name the LUN the same way on every path.
//! Paths whose keys agree must report one LUN in every identity field (`report::identity_differences`),
//! and the verdict must not depend on the order the paths are compared in: every ordering of every
//! subset is tried, up to `ORDERING_LIMIT` paths, and every ordered pair beyond it.

use super::identity::Collected;
use super::login_cases::Reachable;
use super::lun_access::short_name;
use super::report::identity_differences;
use crate::compliance::{case, Case};
use crate::Result;

/// Orderings are tried over every subset of up to this many paths, and over pairs beyond it.
const ORDERING_LIMIT: usize = 5;

/// Runs the identity cases over what `collected` read from each reachable path.
pub(super) fn run(reachable: &[Reachable], collected: &[Result<Collected>], cases: &mut Vec<Case>) {
    let groups = if reachable.len() <= ORDERING_LIMIT {
        orderings(reachable.len())
    } else {
        pairs(reachable.len())
    };
    for order in groups.into_iter().filter(|o| o.len() > 1) {
        let names: Vec<String> = order
            .iter()
            .map(|&i| short_name(reachable[i].path))
            .collect();
        let results: Vec<Result<Collected>> = order.iter().map(|&i| collected[i].clone()).collect();
        let keys: Vec<Option<String>> = results
            .iter()
            .map(|r| r.as_ref().ok().map(lun_key))
            .collect();
        if keys.iter().any(|k| k.is_none()) {
            cases.push(case(
                "identity",
                names.join(" + "),
                "every path answers".into(),
                "a path failed".into(),
                false,
            ));
            continue;
        }
        let expect_same = keys.windows(2).all(|w| w[0] == w[1]);
        let differences = identity_differences(&results);
        let actual_same = differences.is_empty();
        cases.push(case(
            "identity",
            names.join(" + "),
            if expect_same {
                "same LUN"
            } else {
                "different LUN"
            }
            .into(),
            if actual_same {
                "same LUN".to_string()
            } else {
                format!("different: {}", differences.join(", "))
            },
            actual_same == expect_same,
        ));
    }
}

/// The LUN's own designators (association 0), which name it the same way on every path.
fn lun_key(c: &Collected) -> String {
    let mut ids: Vec<String> = c
        .designators
        .iter()
        .filter(|d| d.association == 0)
        .map(|d| format!("{}:{}", d.kind, d.value))
        .collect();
    ids.sort();
    ids.join(",")
}

/// All orderings of every non-empty subset of `0..n`. Their verdicts must not depend on the order.
fn orderings(n: usize) -> Vec<Vec<usize>> {
    fn extend(prefix: &mut Vec<usize>, n: usize, out: &mut Vec<Vec<usize>>) {
        if !prefix.is_empty() {
            out.push(prefix.clone());
        }
        for i in 0..n {
            if !prefix.contains(&i) {
                prefix.push(i);
                extend(prefix, n, out);
                prefix.pop();
            }
        }
    }
    let mut out = Vec::new();
    extend(&mut Vec::new(), n, &mut out);
    out
}

/// Every ordered pair of distinct indices below `n`.
fn pairs(n: usize) -> Vec<Vec<usize>> {
    (0..n)
        .flat_map(|a| (0..n).filter(move |&b| b != a).map(move |b| vec![a, b]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orderings_cover_every_subset_in_every_order() {
        // 3 + 6 + 6 orderings of the subsets of three paths.
        assert_eq!(orderings(3).len(), 15);
        assert!(orderings(3).contains(&vec![2, 0, 1]));
    }

    #[test]
    fn pairs_are_ordered_and_distinct() {
        assert_eq!(pairs(3).len(), 6);
        assert!(pairs(3).iter().all(|p| p[0] != p[1]));
    }
}
