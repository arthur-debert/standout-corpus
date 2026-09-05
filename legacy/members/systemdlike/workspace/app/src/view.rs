//! The serializable view data `list-units` hands to the renderer.

use serde::Serialize;

use crate::units::Unit;

/// One rendered row.
#[derive(Serialize)]
pub struct UnitRow {
    pub unit: String,
    pub load: String,
    pub active: String,
    pub sub: String,
}

/// Column widths, measured over the rows that are actually rendered (header
/// row included when it is shown). The `sub` column is last and never padded,
/// so it has no width here.
#[derive(Serialize)]
pub struct Widths {
    pub unit: usize,
    pub load: usize,
    pub active: usize,
}

#[derive(Serialize)]
pub struct UnitListView {
    pub units: Vec<UnitRow>,
    pub widths: Widths,
    pub show_legend: bool,
    pub count: usize,
}

const HEADERS: [&str; 3] = ["UNIT", "LOAD", "ACTIVE"];

impl UnitListView {
    pub fn new(units: &[Unit], show_legend: bool) -> Self {
        let rows: Vec<UnitRow> = units
            .iter()
            .map(|u| UnitRow {
                unit: u.unit.to_string(),
                load: u.load.to_string(),
                active: u.active.as_str().to_string(),
                sub: u.sub.to_string(),
            })
            .collect();

        let width = |cell: fn(&UnitRow) -> &str, header: &str| {
            let seed = if show_legend { header.chars().count() } else { 0 };
            rows.iter()
                .map(|row| cell(row).chars().count())
                .fold(seed, usize::max)
        };

        let widths = Widths {
            unit: width(|r| &r.unit, HEADERS[0]),
            load: width(|r| &r.load, HEADERS[1]),
            active: width(|r| &r.active, HEADERS[2]),
        };

        Self {
            count: rows.len(),
            units: rows,
            widths,
            show_legend,
        }
    }
}
