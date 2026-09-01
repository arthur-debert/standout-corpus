//! The serializable view data handlers hand back. These shapes *are* the
//! machine documents: Standout serializes them directly under `--output json`
//! and renders them through a template otherwise.

use serde::Serialize;

use crate::manifest::Dep;

#[derive(Debug, Serialize)]
pub struct PackageView {
    pub name: String,
    pub version: String,
}

impl From<&Dep> for PackageView {
    fn from(dep: &Dep) -> Self {
        Self {
            name: dep.name.clone(),
            version: dep.version.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ListView {
    pub packages: Vec<PackageView>,
}

#[derive(Debug, Serialize)]
pub struct InstallView {
    pub installed: Vec<PackageView>,
    pub count: usize,
}
