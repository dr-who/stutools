//! The report's view of the external conformance suites (`suite`, `suites`): a table of each suite's
//! results on the devices it is for, and a chapter per suite with every test by its own ID and title,
//! in the suite's own result words, with the value the device answered and what spit does not yet
//! run. Every test is listed, Not Run ones too, so the gap stays visible.

use super::case::Case;
use super::device::Device;
use super::suite::{judge, Coverage, Judged, Outcome, Suite};
use super::suites::SUITES;

/// The cases of the devices a suite is for, and the transports tested.
fn of_suite<'a>(suite: &Suite, devices: &[Device], cases: &'a [Case]) -> Vec<&'a Case> {
    let mine: Vec<&Device> = devices
        .iter()
        .filter(|d| suite.applies(&[d.transport]))
        .collect();
    cases
        .iter()
        .filter(|c| mine.iter().any(|d| c.name.starts_with(&d.name)))
        .collect()
}

fn applicable<'a>(devices: &[Device]) -> Vec<&'a Suite> {
    let transports: Vec<&str> = devices.iter().map(|d| d.transport).collect();
    SUITES
        .iter()
        .copied()
        .filter(|s| s.applies(&transports))
        .collect()
}

/// Every test of `suite`, judged.
fn judged(suite: &Suite, devices: &[Device], cases: &[Case]) -> Vec<Judged> {
    let mine: Vec<Case> = of_suite(suite, devices, cases)
        .into_iter()
        .cloned()
        .collect();
    suite.tests.iter().map(|t| judge(t, &mine)).collect()
}

/// How many tests had each outcome: passed (with informative), warned, failed, not run, N/A.
fn counts(results: &[Judged]) -> [usize; 5] {
    let mut n = [0; 5];
    for j in results {
        let i = match j.outcome {
            Outcome::Passed | Outcome::Informative => 0,
            Outcome::Warned => 1,
            Outcome::Failed => 2,
            Outcome::NotRun => 3,
            Outcome::NotApplicable => 4,
        };
        n[i] += 1;
    }
    n
}

/// The summary table: one row per suite that applies to the devices tested.
pub fn summary(devices: &[Device], cases: &[Case], esc: fn(&str) -> String) -> String {
    let suites = applicable(devices);
    if suites.is_empty() {
        return String::new();
    }
    let mut out = String::from(
        ".Conformance suites: each suite's tests as spit runs them (the conformance suites document lists every test)\n[cols=\"6,1,1,1,1,1,1\",options=\"header\"]\n|===\n|Suite |Tests |Passed |Warned |Failed |Not run |N/A\n",
    );
    for s in suites {
        let n = counts(&judged(s, devices, cases));
        out.push_str(&format!(
            "|{} |{} |{} |{} |{} |{} |{}\n",
            esc(s.title),
            s.tests.len(),
            n[0],
            n[1],
            n[2],
            n[3],
            n[4]
        ));
    }
    out.push_str("|===\n\n");
    out
}

/// A chapter per suite that applies, with every test.
pub fn chapters(devices: &[Device], cases: &[Case], esc: fn(&str) -> String) -> String {
    let suites = applicable(devices);
    if suites.is_empty() {
        return String::new();
    }
    let mut out = String::from(
        "Each suite's tests, by the suite's own number and title, as spit's cases decide them; nothing is sent twice. A test is the worst of its cases. _Not Run_ (_Not Tested_) marks a test spit does not run yet, or whose cases did not run on these devices; a test run only in part says so. The suites' own procedures are not reproduced.\n\n",
    );
    for s in suites {
        let results = judged(s, devices, cases);
        let n = counts(&results);
        out.push_str(&format!(
            "== {}\n\n{}: {} +\nSummary: Total={}, Passed={}, Warned={}, Failed={}, Not Run={}, Not Applicable={}.\n\n",
            esc(s.title),
            esc(s.publisher),
            esc(s.source),
            s.tests.len(),
            n[0],
            n[1],
            n[2],
            n[3],
            n[4]
        ));
        let c = s.style.columns();
        out.push_str(&format!(
            "[cols=\"2,6,2,5,5\",options=\"header\"]\n|===\n|{} |{} |{} |{} |{}\n",
            c[0], c[1], c[2], c[3], c[4]
        ));
        for (t, j) in s.tests.iter().zip(&results) {
            let partial = t.coverage == Coverage::Partial;
            let word = s.style.word(j.outcome, partial);
            let shown = if matches!(j.outcome, Outcome::Failed) {
                format!("*{}*", esc(&word))
            } else {
                esc(&word)
            };
            let mut comment = esc(t.note);
            if j.cases > 0 {
                let ran = format!(
                    "{} spit case{}",
                    j.cases,
                    if j.cases == 1 { "" } else { "s" }
                );
                comment = if comment.is_empty() {
                    ran
                } else {
                    format!("{ran}; not run: {comment}")
                };
            }
            out.push_str(&format!(
                "|{} |{} |{} |{} |{}\n",
                esc(t.id),
                esc(t.title),
                shown,
                esc(&j.got),
                comment
            ));
        }
        out.push_str("|===\n\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::case::case;
    use super::*;

    fn device(name: &str, transport: &'static str) -> Device {
        Device {
            name: name.into(),
            transport,
            target: String::new(),
            paths: vec![],
            vendor: "V".into(),
            product: "P".into(),
            revision: String::new(),
            serial: String::new(),
            blocks: 1,
            block_len: 512,
        }
    }

    #[test]
    fn a_suite_is_judged_on_the_devices_it_is_for_and_lists_every_test() {
        let cases = vec![case(
            "spc",
            "lun-1 TEST UNIT READY".into(),
            "GOOD".into(),
            "GOOD".into(),
            true,
        )];
        let devices = [device("lun-1", "iSCSI")];
        let doc = chapters(&devices, &cases, |s| s.to_string());
        assert!(doc.contains("|SC2-1 |TEST UNIT READY Basic Verification Test |Passed |GOOD"));
        assert!(doc.contains("UNH-IOL iSCSI Target Login Phase"));
        assert!(!doc.contains("NVM Command Set Conformance"));
        let nvme = [device("ns-1", "NVMe/TCP")];
        let first = judged(SUITES[0], &nvme, &cases);
        assert!(first.iter().all(|j| j.outcome != Outcome::Passed));
    }
}
