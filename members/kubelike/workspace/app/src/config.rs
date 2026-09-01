//! Kubeconfig reading and the namespace precedence chain.

use std::env;
use std::path::PathBuf;

pub const DEFAULT_NAMESPACE: &str = "default";

/// `$KUBECONFIG` when that variable is set, otherwise `$HOME/.kubelike/config`.
pub fn kubeconfig_path() -> Option<PathBuf> {
    match env::var("KUBECONFIG") {
        Ok(path) if !path.is_empty() => Some(PathBuf::from(path)),
        _ => env::var("HOME")
            .ok()
            .filter(|home| !home.is_empty())
            .map(|home| PathBuf::from(home).join(".kubelike").join("config")),
    }
}

/// The value of one `key: value` line of the kubeconfig. A missing file is not
/// an error — it reads as an absent value.
pub fn kubeconfig_value(key: &str) -> Option<String> {
    let text = std::fs::read_to_string(kubeconfig_path()?).ok()?;
    for line in text.lines() {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.trim() == key {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Flag, then `KUBELIKE_NAMESPACE`, then the kubeconfig, then `default`.
pub fn resolve_namespace(flag: Option<&str>) -> String {
    if let Some(namespace) = flag.filter(|value| !value.is_empty()) {
        return namespace.to_string();
    }
    if let Ok(namespace) = env::var("KUBELIKE_NAMESPACE") {
        if !namespace.is_empty() {
            return namespace;
        }
    }
    if let Some(namespace) = kubeconfig_value("namespace") {
        return namespace;
    }
    DEFAULT_NAMESPACE.to_string()
}
