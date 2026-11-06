//! The report as an AsciiDoc document (`--compliance-report OUT.adoc`), ready for asciidoctor or
//! asciidoctor-pdf, and kept short. In order:
//! - a title page with what was tested, when, on which machine and by which spit, and a table of
//!   contents;
//! - the scores in total, a column per SCSI level;
//! - the compatibility matrix: each use (Windows MPIO, clustering, ESXi, cluster file systems...)
//!   against each kind of device tested, and what blocks it (`compatibility`);
//! - the scores section by section;
//! - what was tested (devices, initiators, the command) and how cases are judged;
//! - a chapter per part (SPC, SBC, iSCSI, host needs, NVMe): what the standard is, then each section
//!   once, what it does, its score at each level, and its failures and warnings with a verdict
//!   column per level. A finding the same on several devices is one row naming them all.
//!
//! Each case appears once, cited at the lowest level judged. Text from cases is escaped, so nothing
//! in it is read as markup.

use super::case::{Case, Verdict};
use super::citation::cite;
use super::compatibility::{Fit, USES};
use super::device::Device;
use super::judged::Row;
use super::parts::Part;
use super::score::Tally;
use super::scsi_level::ScsiLevel;
use super::sections::{Section, SECTIONS};

/// The NVMe editions the NVMe/TCP cases are judged against.
const NVME_STANDARDS: &str = "NVM Express Base 2.1, NVM Command Set 1.1, TCP Transport 1.1";

/// What a run tested, for the title page and the "what was tested" chapter.
pub struct Heading<'a> {
    pub version: &'a str,
    pub utc: String,
    pub machine: String,
    pub command: String,
    pub iscsi: &'a [String],
    pub nvme: &'a [String],
    pub initiators: Vec<String>,
    pub devices: &'a [Device],
    pub untested: &'a [(String, String)],
}

/// Text from a case, with every character AsciiDoc could read as markup written as a character
/// reference.
fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '|' | '*' | '_' | '`' | '#' | '+' | '^' | '~' | '[' | ']' | '{' | '}' | '\\' | '\''
            | '"' => out.push_str(&format!("&#{};", c as u32)),
            '\n' => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

fn percent(t: &Tally) -> String {
    format!("{:.1} %", t.percent())
}

fn word(v: Verdict) -> &'static str {
    match v {
        Verdict::Pass => "pass",
        Verdict::Warn => "warn",
        Verdict::Fail => "fail",
        Verdict::Info => "info",
    }
}

/// The tally at level `i` of the rows `keep` selects.
fn tally(rows: &[Row], i: usize, keep: impl Fn(&Case) -> bool) -> Tally {
    Tally::of(rows.iter().map(|r| &r.at[i]).filter(|c| keep(c)))
}

fn levels_named(levels: &[ScsiLevel]) -> String {
    match levels {
        [one] => format!("SCSI level {}", one.spc()),
        [first, .., last] => format!("SCSI levels {} to {}", first.spc(), last.spc()),
        [] => String::new(),
    }
}

/// The column heading for a level in a part's tables: the edition that part is judged against.
fn column(part: Part, level: ScsiLevel) -> String {
    match part {
        Part::Spc => format!("SPC-{}", level.spc()),
        Part::Sbc if level.spc() > level.sbc() => format!("SBC-{} (L{})", level.sbc(), level.spc()),
        Part::Sbc => format!("SBC-{}", level.sbc()),
        _ => format!("Level {}", level.spc()),
    }
}

/// The devices tested, as the title names them: each transport and maker, with how many.
fn devices_named(devices: &[Device]) -> String {
    let mut kinds: Vec<(String, usize)> = Vec::new();
    for d in devices {
        let maker = if d.vendor.is_empty() {
            &d.product
        } else {
            &d.vendor
        };
        let kind = format!("{} {}", d.transport, maker.trim());
        match kinds.iter_mut().find(|(k, _)| *k == kind) {
            Some((_, n)) => *n += 1,
            None => kinds.push((kind, 1)),
        }
    }
    if kinds.is_empty() {
        return "no device".into();
    }
    kinds
        .iter()
        .map(|(k, n)| {
            if *n == 1 {
                k.clone()
            } else {
                format!("{n} {k}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn title_page(out: &mut String, h: &Heading, levels: &[ScsiLevel], rows: &[Row]) {
    let lowest = tally(rows, 0, |_| true);
    out.push_str(&format!(
        "= spit compliance report: {} at {}\n",
        esc(&devices_named(h.devices)),
        levels_named(levels)
    ));
    out.push_str(&format!("spit {} on {}\n", esc(h.version), esc(&h.machine)));
    out.push_str(&format!(":revdate: {}\n", esc(&h.utc)));
    let mut tested = Vec::new();
    if !h.iscsi.is_empty() {
        tested.push(format!("iSCSI {}", h.iscsi.join(", ")));
    }
    if !h.nvme.is_empty() {
        tested.push(format!("NVMe/TCP {}", h.nvme.join(", ")));
    }
    let mut remark = format!("Tested {}", tested.join("; "));
    if !h.initiators.is_empty() {
        remark.push_str(&format!(" as {}", h.initiators.join(" and ")));
    }
    remark.push_str(&format!(
        ". At {}: {} of {} cases passed ({}), {} failed, {} warned.",
        levels[0],
        lowest.passed,
        lowest.scored(),
        percent(&lowest),
        lowest.failed,
        lowest.warned
    ));
    out.push_str(&format!(":revremark: {}\n", esc(&remark)));
    for attr in [
        ":doctype: article",
        ":title-page:",
        ":toc:",
        ":toclevels: 2",
        ":sectnums:",
        ":sectnumlevels: 2",
        ":pdf-page-size: A4",
    ] {
        out.push_str(attr);
        out.push('\n');
    }
    out.push('\n');
}

fn summary(out: &mut String, levels: &[ScsiLevel], rows: &[Row], devices: &[Device]) {
    out.push_str("== Summary\n\n");
    out.push_str("[cols=\"3,1,1,1,1,1,1\",options=\"header\"]\n|===\n");
    out.push_str("|SCSI level |Cases |Pass |Warn |Fail |Info |Score\n");
    for (i, level) in levels.iter().enumerate() {
        let t = tally(rows, i, |_| true);
        out.push_str(&format!(
            "|{} (SPC-{}, SBC-{}) |{} |{} |{} |{} |{} |*{}*\n",
            level.spc(),
            level.spc(),
            level.sbc(),
            t.scored(),
            t.passed,
            t.warned,
            t.failed,
            t.info,
            percent(&t)
        ));
    }
    out.push_str("|===\n\n");
    out.push_str(&matrix(rows, devices, levels));
    let n = levels.len();
    out.push_str(&format!(
        ".Score by section, failures in brackets\n[cols=\"2,1{}\",options=\"header\"]\n|===\n|Section |Part",
        ",2".repeat(n)
    ));
    for level in levels {
        out.push_str(&format!(" |Level {}", level.spc()));
    }
    out.push('\n');
    for s in SECTIONS
        .iter()
        .filter(|s| rows.iter().any(|r| r.first().section == s.name))
    {
        out.push_str(&format!("|{} |{}", s.name, part_word(s.part)));
        let cell = |i: usize| {
            let t = tally(rows, i, |c| c.section == s.name);
            if t.failed > 0 {
                format!("{} ({})", percent(&t), t.failed)
            } else {
                percent(&t)
            }
        };
        if s.part.by_level() {
            for i in 0..n {
                out.push_str(&format!(" |{}", cell(i)));
            }
        } else {
            out.push_str(&format!(" {n}+|{}", cell(0)));
        }
        out.push('\n');
    }
    out.push_str("|===\n\n");
}

fn part_word(part: Part) -> &'static str {
    match part {
        Part::Iscsi => "iSCSI",
        Part::Spc => "SPC",
        Part::Sbc => "SBC",
        Part::Hosts => "hosts",
        Part::Nvme => "NVMe",
    }
}

/// The kinds of device tested (transport and maker), each with the names its cases begin with.
fn kinds(devices: &[Device]) -> Vec<(String, Vec<&str>)> {
    let mut out: Vec<(String, Vec<&str>)> = Vec::new();
    for d in devices {
        let maker = if d.vendor.is_empty() {
            &d.product
        } else {
            &d.vendor
        };
        let kind = format!("{} {}", d.transport, maker.trim());
        match out.iter_mut().find(|(k, _)| *k == kind) {
            Some((_, names)) => names.push(&d.name),
            None => out.push((kind, vec![&d.name])),
        }
    }
    out
}

fn matrix(rows: &[Row], devices: &[Device], levels: &[ScsiLevel]) -> String {
    let kinds = kinds(devices);
    if kinds.is_empty() {
        return String::new();
    }
    let mut out = format!(
        ".Compatibility at {}: what each use needs, and whether the devices give it\n[cols=\"3,5{}\",options=\"header\"]\n|===\n|Use |Needs",
        levels[0],
        ",2".repeat(kinds.len())
    );
    for (k, _) in &kinds {
        out.push_str(&format!(" |{}", esc(k)));
    }
    out.push('\n');
    let of_kind: Vec<Vec<&Case>> = kinds
        .iter()
        .map(|(_, names)| {
            rows.iter()
                .map(|r| r.first())
                .filter(|c| names.contains(&split(&c.name, devices).0))
                .collect()
        })
        .collect();
    let mut notes = Vec::new();
    for u in &USES {
        out.push_str(&format!("|{} |{}", esc(u.name), esc(u.commands)));
        for ((kind, _), cases) in kinds.iter().zip(&of_kind) {
            let (fit, mine) = u.judge(cases);
            let cell = match fit {
                Fit::NotApplicable => "not applicable".to_string(),
                Fit::Ready => "*ready*".into(),
                Fit::Warnings(n) => format!("ready, {n} warning{}", if n == 1 { "" } else { "s" }),
                Fit::AtRisk(n) => {
                    format!("*at risk* ({n} failure{})", if n == 1 { "" } else { "s" })
                }
                Fit::Partly => "partly tested".into(),
            };
            out.push_str(&format!(" |{cell}"));
            let mut why: Vec<String> = Vec::new();
            for c in mine.iter().filter(|c| c.verdict() == Verdict::Fail) {
                let what = split(&c.name, devices).1.to_string();
                if !why.contains(&what) {
                    why.push(what);
                }
            }
            if fit != Fit::NotApplicable {
                let missing = u.untested(cases);
                if !why.is_empty() || !missing.is_empty() {
                    let mut line = format!("*{}* on {}:", esc(u.name), esc(kind));
                    if !why.is_empty() {
                        let shown: Vec<String> = why.iter().take(4).map(|w| esc(w)).collect();
                        line.push_str(&format!(" fails {}", shown.join("; ")));
                        if why.len() > 4 {
                            line.push_str(&format!(" and {} more", why.len() - 4));
                        }
                    }
                    if !missing.is_empty() {
                        line.push_str(&format!(
                            "{}not tested: {}",
                            if why.is_empty() { " " } else { "; " },
                            missing.join(", ")
                        ));
                    }
                    notes.push(line);
                }
            }
        }
        out.push('\n');
    }
    out.push_str("|===\n\n");
    for n in notes {
        out.push_str(&format!("* {n}.\n"));
    }
    out.push('\n');
    out
}

fn tested(out: &mut String, h: &Heading, levels: &[ScsiLevel]) {
    out.push_str("== What was tested\n\n");
    if !h.devices.is_empty() {
        out.push_str("[cols=\"2,4,3,2,3,2\",options=\"header\"]\n|===\n");
        out.push_str("|Device |Target |Reached through |Product |Serial number |Capacity\n");
        for d in h.devices {
            out.push_str(&format!(
                "|{} ({}) |{} |{} |{} {} {} |{} |{}, {}-byte blocks\n",
                esc(&d.name),
                d.transport,
                esc(&d.target),
                esc(&d.paths.join(", ")),
                esc(&d.vendor),
                esc(&d.product),
                esc(&d.revision),
                esc(&d.serial),
                crate::columns::human_bytes(d.bytes()),
                d.block_len
            ));
        }
        out.push_str("|===\n\n");
    }
    for (name, why) in h.untested {
        out.push_str(&format!(
            "* Not tested past login: {}: {}\n",
            esc(name),
            esc(why)
        ));
    }
    out.push_str(&format!(
        "* spit {} on {}, {}; judged at {}.\n",
        esc(h.version),
        esc(&h.machine),
        esc(&h.utc),
        levels_named(levels)
    ));
    for level in levels {
        out.push_str(&format!(
            "* Level {}: {} and {}.\n",
            level.spc(),
            level.spc_title(),
            level.sbc_title()
        ));
    }
    if !h.initiators.is_empty() {
        out.push_str(&format!(
            "* Initiator names: {}.\n",
            esc(&h.initiators.join(", "))
        ));
    }
    out.push_str("\n....\n");
    out.push_str(&h.command.replace("....", ". . ."));
    out.push_str("\n....\n\n");
    out.push_str("A *shall* not met fails, a *should* not met warns, and an informational case only records what the device offers; the score is the share of the other cases that passed. SCSI level _n_ judges against SPC-_n_ and SBC-_n_ (level 6: SPC-6 and SBC-5): each case cites that edition's clause, and a requirement an edition does not define only informs there. iSCSI (RFC 7143, RFC 7144), SAM-6 and NVMe (");
    out.push_str(NVME_STANDARDS);
    out.push_str(") hold at every level. Cases are cited below at the lowest level judged.\n\n");
}

/// One failure or warning, on every device it was found on.
struct Finding<'a> {
    case: &'a Case,
    what: &'a str,
    devices: Vec<&'a str>,
    verdicts: Vec<Verdict>,
}

/// The case name without the device it begins with, and that device.
fn split<'a>(name: &'a str, devices: &'a [Device]) -> (&'a str, &'a str) {
    devices
        .iter()
        .filter_map(|d| {
            name.strip_prefix(d.name.as_str())
                .and_then(|r| r.strip_prefix(' '))
                .map(|r| (d.name.as_str(), r))
        })
        .max_by_key(|(d, _)| d.len())
        .unwrap_or(("", name))
}

fn findings<'a>(rows: &'a [Row], section: &str, devices: &'a [Device]) -> Vec<Finding<'a>> {
    let mut out: Vec<Finding> = Vec::new();
    for r in rows.iter().filter(|r| r.first().section == section) {
        let verdicts: Vec<Verdict> = r.at.iter().map(|c| c.verdict()).collect();
        if !verdicts
            .iter()
            .any(|v| matches!(v, Verdict::Fail | Verdict::Warn))
        {
            continue;
        }
        let c = r.first();
        let (device, what) = split(&c.name, devices);
        match out
            .iter_mut()
            .find(|f| f.what == what && f.case.expected == c.expected && f.verdicts == verdicts)
        {
            Some(f) => f.devices.push(device),
            None => out.push(Finding {
                case: c,
                what,
                devices: vec![device],
                verdicts,
            }),
        }
    }
    out
}

fn section(out: &mut String, s: &Section, levels: &[ScsiLevel], rows: &[Row], devices: &[Device]) {
    if !rows.iter().any(|r| r.first().section == s.name) {
        return;
    }
    let by_level = s.part.by_level();
    out.push_str(&format!(
        "=== {}: {}\n\n{}\n\n",
        s.name,
        esc(&cite(s.standard, levels[0]).text),
        esc(s.tests)
    ));
    let shown = if by_level { levels.len() } else { 1 };
    let scores: Vec<String> = (0..shown)
        .map(|i| {
            let t = tally(rows, i, |c| c.section == s.name);
            let at = if by_level {
                format!("{}: ", column(s.part, levels[i]))
            } else {
                String::new()
            };
            format!("{at}{} of {} ({})", t.passed, t.scored(), percent(&t))
        })
        .collect();
    out.push_str(&format!("Passed: {}.\n\n", scores.join("; ")));
    let found = findings(rows, s.name, devices);
    if found.is_empty() {
        return;
    }
    out.push_str(&format!(
        "[cols=\"5,3,3{}\",options=\"header\"]\n|===\n|Finding |Expected |Got",
        ",1".repeat(shown)
    ));
    for &level in &levels[..shown] {
        if by_level {
            out.push_str(&format!(" |{}", column(s.part, level)));
        } else {
            out.push_str(" |Verdict");
        }
    }
    out.push('\n');
    for f in &found {
        let on = f.devices.join(", ");
        out.push_str(&format!(
            "|{}{} |{} |{}",
            esc(f.what),
            if on.is_empty() {
                String::new()
            } else {
                format!(" (on {})", esc(&on))
            },
            esc(&f.case.expected),
            esc(&f.case.actual)
        ));
        for v in &f.verdicts[..shown] {
            out.push_str(&format!(" |{}", word(*v)));
        }
        out.push('\n');
    }
    out.push_str("|===\n\n");
}

fn chapter(out: &mut String, p: Part, levels: &[ScsiLevel], rows: &[Row], devices: &[Device]) {
    let of: Vec<&Section> = SECTIONS
        .iter()
        .filter(|s| s.part == p && rows.iter().any(|r| r.first().section == s.name))
        .collect();
    if of.is_empty() {
        return;
    }
    let title = match (p, levels) {
        (Part::Spc | Part::Sbc, [one]) => p.title(*one),
        (Part::Spc, _) => "SPC: SCSI Primary Commands".into(),
        (Part::Sbc, _) => "SBC: SCSI Block Commands".into(),
        _ => p.title(levels[0]),
    };
    out.push_str(&format!("== {title}\n\n{}\n\n", p.about()));
    for s in of {
        section(out, s, levels, rows, devices);
    }
}

/// The whole document. `rows` holds every case judged at each of `levels` (lowest first, never
/// empty).
pub fn document(h: &Heading, levels: &[ScsiLevel], rows: &[Row]) -> String {
    let mut out = String::new();
    title_page(&mut out, h, levels, rows);
    summary(&mut out, levels, rows, h.devices);
    tested(&mut out, h, levels);
    for p in [Part::Spc, Part::Sbc, Part::Iscsi, Part::Hosts, Part::Nvme] {
        chapter(&mut out, p, levels, rows, h.devices);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::case::case;
    use super::super::judged;
    use super::*;

    #[test]
    fn case_text_cannot_become_markup() {
        assert_eq!(esc("a|b *c* `d`"), "a&#124;b &#42;c&#42; &#96;d&#96;");
        assert_eq!(esc("\\x00"), "&#92;x00");
    }

    #[test]
    fn each_case_is_one_row_with_a_column_per_level_and_devices_merged() {
        let release = |lun: &str| {
            case(
                "pr",
                format!("{lun} SPC-6 5.14.11.2.2 RELEASE of the wrong type"),
                "5/26/04".into(),
                "RESERVATION CONFLICT | x".into(),
                false,
            )
        };
        let cases = vec![
            release("lun-1"),
            release("lun-4"),
            case(
                "login",
                "lun-1 login".into(),
                "ok".into(),
                "ok".into(),
                true,
            ),
        ];
        let levels: Vec<ScsiLevel> = [3, 6].iter().map(|&n| ScsiLevel::new(n).unwrap()).collect();
        let rows = judged::rows(&cases, &levels);
        let device = |name: &str| Device {
            name: name.into(),
            transport: "iSCSI",
            target: format!("iqn.t:{name}"),
            paths: vec!["10.0.0.5:3260".into()],
            vendor: "LIO-ORG".into(),
            product: "disk".into(),
            revision: "4.0".into(),
            serial: "s".into(),
            blocks: 2048,
            block_len: 512,
        };
        let devices = [device("lun-1"), device("lun-4")];
        let h = Heading {
            version: "2.3.0",
            utc: "2026-11-06T10:00:00Z".into(),
            machine: "lab".into(),
            command: "spit --iscsi 10.0.0.5 --compliance".into(),
            iscsi: &["10.0.0.5".into()],
            nvme: &[],
            initiators: vec!["iqn.a".into()],
            devices: &devices,
            untested: &[],
        };
        let doc = document(&h, &levels, &rows);
        assert!(
            doc.starts_with("= spit compliance report: 2 iSCSI LIO-ORG at SCSI levels 3 to 6\n")
        );
        assert!(doc.contains(":toc:"));
        assert!(doc.find("== Summary").unwrap() < doc.find("== What was tested").unwrap());
        assert!(doc.contains("|pr |SPC |0.0 % (2) |0.0 % (2)\n"));
        assert!(doc.contains("|login |iSCSI 2+|100.0 %\n"));
        assert!(doc.contains("== SPC: SCSI Primary Commands"));
        assert!(doc.contains("|Finding |Expected |Got |SPC-3 |SPC-6\n"));
        assert!(doc.contains(
            "|SPC-3 5.6.10.2 RELEASE of the wrong type (on lun-1, lun-4) |5/26/04 |RESERVATION CONFLICT &#124; x |fail |fail\n"
        ));
        assert!(!doc.contains("== SCSI level"));
    }
}
