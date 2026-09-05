//! The CLI view data. These types *are* the machine contract: in `--output
//! json` the framework serializes exactly what a handler returns, so the root
//! of each document is the type the handler chose.

use serde::Serialize;

use crate::cellar::{self, Formula};

/// A formula record: name, both versions, and whether a newer one exists.
///
/// The values are bound to the record rather than spread over parallel arrays.
#[derive(Serialize)]
pub struct FormulaRecord {
    pub name: String,
    pub installed: String,
    pub latest: String,
    pub outdated: bool,
}

impl From<&'static Formula> for FormulaRecord {
    fn from(formula: &'static Formula) -> Self {
        FormulaRecord {
            name: formula.name.to_string(),
            installed: formula.installed.to_string(),
            latest: formula.latest.to_string(),
            outdated: formula.is_outdated(),
        }
    }
}

/// A formula record with its direct dependency names added.
#[derive(Serialize)]
pub struct FormulaDetail {
    pub name: String,
    pub installed: String,
    pub latest: String,
    pub outdated: bool,
    pub dependencies: Vec<String>,
}

impl From<&'static Formula> for FormulaDetail {
    fn from(formula: &'static Formula) -> Self {
        FormulaDetail {
            name: formula.name.to_string(),
            installed: formula.installed.to_string(),
            latest: formula.latest.to_string(),
            outdated: formula.is_outdated(),
            dependencies: cellar::direct_dependencies(formula)
                .into_iter()
                .map(str::to_string)
                .collect(),
        }
    }
}

/// One node of a dependency tree: a name, and the nodes hanging under it.
#[derive(Serialize)]
pub struct DependencyNode {
    pub name: String,
    pub dependencies: Vec<DependencyNode>,
}

impl DependencyNode {
    /// The tree rooted at `formula`, direct dependencies in name order. A
    /// formula reached through more than one path appears in full each time.
    pub fn of(formula: &'static Formula) -> Self {
        DependencyNode {
            name: formula.name.to_string(),
            dependencies: cellar::direct_dependencies(formula)
                .into_iter()
                .filter_map(cellar::find)
                .map(DependencyNode::of)
                .collect(),
        }
    }
}

/// What `deps` answers with: a set of names, or one tree node.
///
/// `untagged` keeps each shape's own root in the serialized document — an
/// array of names for the set, an object for the tree — rather than wrapping
/// either in a variant envelope.
#[derive(Serialize)]
#[serde(untagged)]
pub enum DependencyView {
    Names(Vec<String>),
    Tree(DependencyNode),
}
