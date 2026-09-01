//! Text layout. One rule everywhere: columns separated by a single space,
//! each column but the last padded to its widest rendered cell, the header
//! included in that width.

/// A table block: the header row followed by one line per row.
pub fn table(headers: &[String], rows: &[Vec<String>]) -> String {
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            let width = cell.chars().count();
            if width > widths[index] {
                widths[index] = width;
            }
        }
    }

    let mut lines = Vec::with_capacity(rows.len() + 1);
    lines.push(line(headers, &widths));
    for row in rows {
        lines.push(line(row, &widths));
    }
    lines.join("\n")
}

fn line(cells: &[String], widths: &[usize]) -> String {
    let last = cells.len().saturating_sub(1);
    let mut out = String::new();
    for (index, cell) in cells.iter().enumerate() {
        if index > 0 {
            out.push(' ');
        }
        if index == last {
            out.push_str(cell);
        } else {
            out.push_str(cell);
            for _ in cell.chars().count()..widths[index] {
                out.push(' ');
            }
        }
    }
    out
}

/// A `describe` detail block: `<Key>:` padded to the widest key in the block,
/// then a single space, then the value.
pub fn detail(entries: &[(&str, String)]) -> String {
    let width = entries
        .iter()
        .map(|(key, _)| key.chars().count() + 1)
        .max()
        .unwrap_or(0);
    entries
        .iter()
        .map(|(key, value)| {
            let label = format!("{}:", key);
            let pad = width - label.chars().count();
            format!("{}{} {}", label, " ".repeat(pad), value)
        })
        .collect::<Vec<_>>()
        .join("\n")
}
