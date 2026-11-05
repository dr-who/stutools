//! Printing rows of text as left-aligned columns two spaces apart (the last column unpadded), and a
//! byte count in the largest binary unit that keeps it at 1 or more, to one decimal place. The iSCSI
//! and NVMe/TCP listings print their tables this way.

/// `bytes` in the largest binary unit that keeps it at 1 or more, to one decimal place.
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Prints rows as left-aligned columns two spaces apart; the last column is not padded.
pub fn print_columns(rows: &[Vec<String>]) {
    let columns = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let widths: Vec<usize> = (0..columns)
        .map(|i| {
            rows.iter()
                .filter_map(|r| r.get(i))
                .map(|c| c.chars().count())
                .max()
                .unwrap_or(0)
        })
        .collect();
    for row in rows {
        let mut line = String::new();
        for (i, cell) in row.iter().enumerate() {
            if i + 1 == row.len() {
                line.push_str(cell);
            } else {
                line.push_str(&format!("{cell:<width$}  ", width = widths[i]));
            }
        }
        println!("{}", line.trim_end());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_use_the_largest_unit() {
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(20 * 1024 * 1024 * 1024), "20.0 GiB");
        assert_eq!(human_bytes(1536), "1.5 KiB");
    }
}
