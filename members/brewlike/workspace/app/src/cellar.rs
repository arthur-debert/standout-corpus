//! The frozen cellar and the questions that can be asked of it.
//!
//! This module is CLI-free: no clap, no standout, no printing. It answers
//! questions with ordinary Rust data so the handlers stay thin adapters.

use std::collections::BTreeSet;

/// One installed formula, as the cellar records it.
#[derive(Debug)]
pub struct Formula {
    pub name: &'static str,
    pub installed: &'static str,
    pub latest: &'static str,
    pub dependencies: &'static [&'static str],
}

impl Formula {
    /// A formula is out of date when a newer version than the installed one exists.
    pub fn is_outdated(&self) -> bool {
        self.installed != self.latest
    }
}

/// The one built-in cellar, in the name order every listing follows.
const CELLAR: &[Formula] = &[
    Formula {
        name: "basalt",
        installed: "2.1.0",
        latest: "2.1.0",
        dependencies: &["pebble", "quartz"],
    },
    Formula {
        name: "granite",
        installed: "1.4.2",
        latest: "1.5.0",
        dependencies: &["pebble"],
    },
    Formula {
        name: "pebble",
        installed: "0.9.0",
        latest: "0.9.0",
        dependencies: &[],
    },
    Formula {
        name: "quartz",
        installed: "3.0.1",
        latest: "3.2.0",
        dependencies: &["pebble"],
    },
];

/// Every installed formula, in name order.
pub fn all() -> &'static [Formula] {
    CELLAR
}

/// The formula of that name, or `None` when the cellar has never heard of it.
pub fn find(name: &str) -> Option<&'static Formula> {
    CELLAR.iter().find(|formula| formula.name == name)
}

/// The formulae a newer version exists for, in name order.
///
/// With no names, every installed formula is considered; with names, only
/// those. An unknown name is returned as `Err` rather than narrowing the query.
pub fn outdated(names: &[String]) -> Result<Vec<&'static Formula>, UnknownFormula> {
    let considered: Vec<&'static Formula> = if names.is_empty() {
        all().iter().collect()
    } else {
        let mut wanted = BTreeSet::new();
        for name in names {
            let formula = find(name).ok_or_else(|| UnknownFormula(name.clone()))?;
            wanted.insert(formula.name);
        }
        all()
            .iter()
            .filter(|formula| wanted.contains(formula.name))
            .collect()
    };

    Ok(considered
        .into_iter()
        .filter(|formula| formula.is_outdated())
        .collect())
}

/// The transitive dependency closure of `formula`, deduplicated, in name order.
///
/// The queried formula is not part of its own closure.
pub fn closure(formula: &'static Formula) -> Vec<&'static str> {
    let mut seen = BTreeSet::new();
    let mut pending: Vec<&'static str> = formula.dependencies.to_vec();

    while let Some(name) = pending.pop() {
        if !seen.insert(name) {
            continue;
        }
        if let Some(dependency) = find(name) {
            pending.extend_from_slice(dependency.dependencies);
        }
    }

    seen.into_iter().collect()
}

/// The direct dependencies of `formula`, in name order.
pub fn direct_dependencies(formula: &'static Formula) -> Vec<&'static str> {
    let mut names = formula.dependencies.to_vec();
    names.sort_unstable();
    names
}

/// A formula the cellar does not contain.
#[derive(Debug)]
pub struct UnknownFormula(pub String);

impl std::fmt::Display for UnknownFormula {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "no formula found: {}", self.0)
    }
}

impl std::error::Error for UnknownFormula {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closure_is_deduplicated_and_ordered() {
        let basalt = find("basalt").unwrap();
        assert_eq!(closure(basalt), ["pebble", "quartz"]);
    }

    #[test]
    fn a_leaf_has_an_empty_closure() {
        let pebble = find("pebble").unwrap();
        assert!(closure(pebble).is_empty());
    }

    #[test]
    fn outdated_without_names_covers_the_whole_cellar() {
        let names: Vec<&str> = outdated(&[])
            .unwrap()
            .into_iter()
            .map(|formula| formula.name)
            .collect();
        assert_eq!(names, ["granite", "quartz"]);
    }

    #[test]
    fn outdated_restricts_to_the_named_formulae() {
        let names: Vec<&str> = outdated(&["quartz".to_string()])
            .unwrap()
            .into_iter()
            .map(|formula| formula.name)
            .collect();
        assert_eq!(names, ["quartz"]);
    }

    #[test]
    fn a_current_formula_yields_an_empty_result() {
        assert!(outdated(&["basalt".to_string()]).unwrap().is_empty());
    }

    #[test]
    fn an_unknown_name_is_an_error() {
        let error = outdated(&["obsidian".to_string()]).unwrap_err();
        assert_eq!(error.0, "obsidian");
    }
}
