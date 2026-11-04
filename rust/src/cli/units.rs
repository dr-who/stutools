//! Parsing of sizes, durations and numbers from the command line.
use crate::Result;

pub(super) fn finite(s: &str) -> Result<f64> {
    let v = s
        .parse::<f64>()
        .map_err(|_| format!("invalid number {s}"))?;
    if !v.is_finite() {
        return Err(format!("nonfinite number {s}"));
    }
    Ok(v)
}
pub(super) fn duration(s: &str) -> Result<Option<f64>> {
    if s == "-1" {
        return Ok(None);
    }
    let (number, multiplier) = if let Some(s) = s.strip_suffix("ms") {
        (s, 0.001)
    } else if let Some(s) = s.strip_suffix('s') {
        (s, 1.)
    } else if let Some(s) = s.strip_suffix('m') {
        (s, 60.)
    } else if let Some(s) = s.strip_suffix('h') {
        (s, 3600.)
    } else if let Some(s) = s.strip_suffix('d') {
        (s, 86400.)
    } else if let Some(s) = s.strip_suffix('w') {
        (s, 604800.)
    } else {
        (s, 1.)
    };
    let seconds = finite(number)? * multiplier;
    if seconds <= 0. || std::time::Duration::try_from_secs_f64(seconds).is_err() {
        return Err("duration must be positive (seconds, ms/s/m/h/d/w suffix) or -1".into());
    }
    Ok(Some(seconds))
}
pub(super) fn size(s: &str, default: f64) -> Result<u64> {
    let n = s
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(s.len());
    let unit = match &s[n..] {
        "" => default as u128,
        "B" => 1,
        "k" | "K" | "KB" => 1000,
        "M" | "MB" => 1_000_000,
        "G" | "GB" => 1_000_000_000,
        "T" | "TB" => 1_000_000_000_000,
        "KiB" => 1024,
        "MiB" => 1048576,
        "GiB" => 1073741824,
        "TiB" => 1099511627776,
        _ => return Err(format!("invalid size {s}")),
    };
    let (whole, fraction) = s[..n].split_once('.').unwrap_or((&s[..n], ""));
    if whole.is_empty() && fraction.is_empty() {
        return Err(format!("invalid size {s}"));
    }
    let whole = if whole.is_empty() {
        0
    } else {
        whole
            .parse::<u128>()
            .map_err(|_| format!("invalid size {s}"))?
    };
    let scale = 10u128
        .checked_pow(fraction.len() as u32)
        .ok_or("size precision overflow")?;
    let fraction = if fraction.is_empty() {
        0
    } else {
        fraction
            .parse::<u128>()
            .map_err(|_| format!("invalid size {s}"))?
    };
    let value = whole
        .checked_mul(scale)
        .and_then(|v| v.checked_add(fraction))
        .and_then(|v| v.checked_mul(unit))
        .ok_or("size overflow")?;
    if value % scale != 0 || value / scale > i64::MAX as u128 {
        return Err(format!("invalid size {s}"));
    }
    Ok((value / scale) as u64)
}
pub(super) fn integer(s: &str) -> Result<u64> {
    s.parse()
        .map_err(|_| format!("invalid unsigned integer {s}"))
}
