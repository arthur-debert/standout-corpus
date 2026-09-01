//! The unit table. CLI-free: no clap, no printing, no output concerns.

use serde::Serialize;

/// The `active` state of a unit, and the only thing `--state` filters on.
#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ActiveState {
    Active,
    Inactive,
}

impl ActiveState {
    pub fn as_str(self) -> &'static str {
        match self {
            ActiveState::Active => "active",
            ActiveState::Inactive => "inactive",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "active" => Some(ActiveState::Active),
            "inactive" => Some(ActiveState::Inactive),
            _ => None,
        }
    }
}

/// One row of the frozen unit table.
#[derive(Clone, Copy)]
pub struct Unit {
    pub unit: &'static str,
    pub load: &'static str,
    pub active: ActiveState,
    pub sub: &'static str,
}

/// The frozen table, in listing order.
const UNITS: &[Unit] = &[
    Unit {
        unit: "core.service",
        load: "loaded",
        active: ActiveState::Active,
        sub: "running",
    },
    Unit {
        unit: "web.service",
        load: "loaded",
        active: ActiveState::Active,
        sub: "running",
    },
    Unit {
        unit: "cache.service",
        load: "loaded",
        active: ActiveState::Inactive,
        sub: "dead",
    },
    Unit {
        unit: "backup.timer",
        load: "loaded",
        active: ActiveState::Active,
        sub: "waiting",
    },
];

/// The units to list, in table order, optionally narrowed to one active state.
pub fn list_units(state: Option<ActiveState>) -> Vec<Unit> {
    UNITS
        .iter()
        .copied()
        .filter(|unit| state.is_none_or(|wanted| unit.active == wanted))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unfiltered_listing_keeps_table_order() {
        let names: Vec<_> = list_units(None).iter().map(|u| u.unit).collect();
        assert_eq!(
            names,
            [
                "core.service",
                "web.service",
                "cache.service",
                "backup.timer"
            ]
        );
    }

    #[test]
    fn state_filter_preserves_order_and_drops_the_rest() {
        let names: Vec<_> = list_units(Some(ActiveState::Active))
            .iter()
            .map(|u| u.unit)
            .collect();
        assert_eq!(names, ["core.service", "web.service", "backup.timer"]);

        let names: Vec<_> = list_units(Some(ActiveState::Inactive))
            .iter()
            .map(|u| u.unit)
            .collect();
        assert_eq!(names, ["cache.service"]);
    }
}
