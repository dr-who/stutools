//! `--iscsi`: logs in to each target path, reads what identifies the LUN and its ALUA groups, and
//! prints one column per target with every difference between them and a same-LUN verdict.

use super::credentials::read_secret;
use super::identity::{collect, Collected};
use super::path::{parse_path, Path};
use super::scsi;
use crate::{cli::Config, Result};

/// Names of the identity rows (LUN identifiers, serial, vendor, product, revision, capacity) that
/// differ between the successful paths. Empty means the paths are one LUN.
pub fn identity_differences(results: &[Result<Collected>]) -> Vec<String> {
    rows(results)
        .into_iter()
        .filter(|r| r.identity)
        .filter(|r| {
            let values: Vec<&String> = r
                .cells
                .iter()
                .zip(results)
                .filter(|(_, res)| res.is_ok())
                .map(|(c, _)| c)
                .collect();
            values.len() > 1 && values.iter().any(|v| *v != values[0])
        })
        .map(|r| r.label.to_string())
        .collect()
}

fn designator_kind(kind: u8) -> String {
    match kind {
        1 => "T10".into(),
        2 => "EUI64".into(),
        3 => "NAA".into(),
        8 => "SCSI-name".into(),
        n => format!("type{n}"),
    }
}

/// One row of the table: a label, and one value per target column.
struct Row {
    label: &'static str,
    cells: Vec<String>,
    /// Rows that describe identity or size. A difference in these means the paths are not one LUN.
    identity: bool,
}

fn row(label: &'static str, identity: bool, cells: Vec<String>) -> Row {
    Row {
        label,
        cells,
        identity,
    }
}

/// The VPD 0x83 identifiers for one association: 0 names the LUN, 1 names the port that answered.
fn identifiers(c: &Collected, association: u8) -> String {
    let parts: Vec<String> = c
        .designators
        .iter()
        .filter(|d| d.association == association || (association == 1 && d.association == 2))
        .map(|d| format!("{}:{}", designator_kind(d.kind), d.value))
        .collect();
    if parts.is_empty() {
        "none".into()
    } else {
        parts.join(" ")
    }
}

fn rows(results: &[Result<Collected>]) -> Vec<Row> {
    let ok = |f: &dyn Fn(&Collected) -> String| -> Vec<String> {
        results
            .iter()
            .map(|r| r.as_ref().map(f).unwrap_or_else(|_| "-".into()))
            .collect()
    };
    let digest = |on: bool| if on { "CRC32C" } else { "none" }.to_string();
    vec![
        row(
            "auth",
            false,
            ok(&|c| match c.login.chap_algorithm {
                Some(5) => "CHAP (MD5)".into(),
                Some(6) => "CHAP (SHA-1)".into(),
                Some(n) => format!("CHAP ({n})"),
                None => "none".into(),
            }),
        ),
        row(
            "header digest",
            false,
            ok(&|c| digest(c.login.header_digest)),
        ),
        row("data digest", false, ok(&|c| digest(c.login.data_digest))),
        row(
            "max recv bytes",
            false,
            ok(&|c| c.login.max_recv.to_string()),
        ),
        row(
            "initial R2T",
            false,
            ok(&|c| if c.login.initial_r2t { "yes" } else { "no" }.into()),
        ),
        row("vendor", true, ok(&|c| c.inquiry.vendor.clone())),
        row("product", true, ok(&|c| c.inquiry.product.clone())),
        row("revision", true, ok(&|c| c.inquiry.revision.clone())),
        row(
            "peripheral",
            true,
            ok(&|c| format!("{:#04x}", c.inquiry.peripheral)),
        ),
        row("serial (0x80)", true, ok(&|c| c.serial.clone())),
        row("LUN identifiers", true, ok(&|c| identifiers(c, 0))),
        row("port identifiers", false, ok(&|c| identifiers(c, 1))),
        row("last LBA", true, ok(&|c| c.last_lba.to_string())),
        row("block bytes", true, ok(&|c| c.block_len.to_string())),
        row(
            "size",
            true,
            ok(&|c| {
                format!(
                    "{:.2} GiB",
                    (c.last_lba + 1) as f64 * c.block_len as f64 / 1_073_741_824.0
                )
            }),
        ),
        row(
            "this path ALUA",
            false,
            ok(&|c| match c.own_port {
                Some(port) => c
                    .groups
                    .iter()
                    .find(|g| g.ports.contains(&port))
                    .map(|g| format!("{} (group {})", scsi::state_name(g.state), g.id))
                    .unwrap_or_else(|| "port in no group".into()),
                None => "port unknown".into(),
            }),
        ),
        row(
            "ALUA groups",
            false,
            ok(&|c| {
                c.groups
                    .iter()
                    .map(|g| format!("{}:{}", g.id, scsi::state_name(g.state)))
                    .collect::<Vec<_>>()
                    .join(" ")
            }),
        ),
        row(
            "ALUA ports",
            false,
            ok(&|c| {
                c.groups
                    .iter()
                    .map(|g| format!("{}:{:?}", g.id, g.ports))
                    .collect::<Vec<_>>()
                    .join(" ")
            }),
        ),
    ]
}

const WIDTH: usize = 34;

/// Cuts a value to 48 characters for the difference lines, where the full text is hard to read.
fn shorten(text: &str) -> String {
    if text.chars().count() <= 48 {
        text.to_string()
    } else {
        let head: String = text.chars().take(46).collect();
        format!("{head}..")
    }
}

fn fit(text: &str) -> String {
    if text.chars().count() < WIDTH {
        text.to_string()
    } else {
        let head: String = text.chars().take(WIDTH - 2).collect();
        format!("{head}~")
    }
}

pub fn run(config: &Config) -> Result<()> {
    let initiator = config
        .iscsi_initiator
        .as_deref()
        .ok_or("--iscsi needs --iscsi-initiator IQN")?;
    let paths: Vec<Path> = config
        .iscsi
        .iter()
        .map(|p| parse_path(p))
        .collect::<Result<_>>()?;
    let secret = match (&config.chap_user, &config.chap_secret_file) {
        (Some(_), Some(file)) => Some(read_secret(file)?),
        (None, None) => None,
        _ => return Err("CHAP needs both --chap-user and --chap-secret-file".into()),
    };
    let chap = config.chap_user.as_deref().zip(secret.as_deref());
    let results: Vec<Result<Collected>> =
        paths.iter().map(|p| collect(p, initiator, chap)).collect();

    let short = |p: &Path| p.target.rsplit(':').next().unwrap_or(&p.target).to_string();
    let mut header = format!("{:<18}", "");
    for p in &paths {
        header += &format!("{:<WIDTH$}", fit(&short(p)));
    }
    println!("{header}");
    let table_rows: Vec<(&str, Vec<String>)> =
        std::iter::once(("portal", paths.iter().map(|p| p.portal.clone()).collect()))
            .chain(std::iter::once((
                "status",
                results
                    .iter()
                    .map(|r| match r {
                        Ok(_) => "ok".to_string(),
                        Err(_) => "FAILED".to_string(),
                    })
                    .collect(),
            )))
            .chain(rows(&results).into_iter().map(|r| (r.label, r.cells)))
            .collect();
    for (label, cells) in &table_rows {
        let mut line = format!("{label:<18}");
        for cell in cells {
            line += &format!("{:<WIDTH$}", fit(cell));
        }
        println!("{}", line.trim_end());
    }

    let mut failed = Vec::new();
    for (p, r) in paths.iter().zip(&results) {
        if let Err(e) = r {
            failed.push(format!("{} {}: {e}", p.portal, p.target));
        }
    }
    println!();
    let mut differences = Vec::new();
    let mut identity_differences = Vec::new();
    for r in rows(&results) {
        let values: Vec<&String> = r
            .cells
            .iter()
            .zip(&results)
            .filter(|(_, res)| res.is_ok())
            .map(|(c, _)| c)
            .collect();
        if values.len() > 1 && values.iter().any(|v| *v != values[0]) {
            let line = format!(
                "{}: {}",
                r.label,
                r.cells
                    .iter()
                    .map(|c| shorten(c))
                    .collect::<Vec<_>>()
                    .join(" | ")
            );
            if r.identity {
                identity_differences.push(line.clone());
            }
            differences.push(line);
        }
    }
    if differences.is_empty() {
        println!("differences: none");
    } else {
        println!("differences:");
        for d in &differences {
            println!("  {d}");
        }
    }
    if !failed.is_empty() {
        println!("paths not tested:");
        for f in &failed {
            println!("  {f}");
        }
        return Err(format!("{} of {} paths failed", failed.len(), paths.len()));
    }
    if !identity_differences.is_empty() {
        return Err("paths are not the same LUN".into());
    }
    println!("verdict: {} paths are the same LUN", paths.len());
    Ok(())
}
