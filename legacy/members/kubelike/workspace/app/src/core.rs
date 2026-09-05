//! The application behavior: everything `kubelike` does, expressed as calls
//! that take plain arguments and return plain data. No clap, no standout, no
//! printing.

use serde_json::{json, Value};

use crate::cluster::{self, KindMeta, Resource};
use crate::config;
use crate::render;

/// What an invocation produces on the success side.
pub enum Emission {
    /// Exact bytes for stdout, trailing newline included.
    Text(String),
    /// Nothing on stdout; one prose line on stderr; still a success.
    Empty(String),
}

/// A failure that carries its own exit status.
pub enum Failure {
    /// Exit 1: unknown kind, absent resource, unknown field, no context.
    Domain(String),
    /// Exit 2: a namespace selector the verb accepts, on a cluster-scoped kind.
    Usage(String),
}

impl Failure {
    pub fn status(&self) -> u8 {
        match self {
            Failure::Domain(_) => 1,
            Failure::Usage(_) => 2,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Failure::Domain(message) | Failure::Usage(message) => message,
        }
    }
}

pub type Outcome = Result<Emission, Failure>;

/// The `-o` format vocabulary.
enum Format {
    Default,
    Wide,
    Name,
    Json,
    CustomColumns(Vec<(String, String)>),
}

fn parse_format(spec: Option<&str>) -> Result<Format, Failure> {
    let Some(spec) = spec else {
        return Ok(Format::Default);
    };
    match spec {
        "" => Ok(Format::Default),
        "wide" => Ok(Format::Wide),
        "name" => Ok(Format::Name),
        "json" => Ok(Format::Json),
        _ => {
            let Some(columns) = spec.strip_prefix("custom-columns=") else {
                return Err(Failure::Domain(format!(
                    "kubelike: unknown output format \"{}\"",
                    spec
                )));
            };
            let mut parsed = Vec::new();
            for column in columns.split(',') {
                let Some((header, field)) = column.split_once(':') else {
                    return Err(Failure::Domain(format!(
                        "kubelike: invalid custom-columns spec \"{}\"",
                        column
                    )));
                };
                parsed.push((header.to_string(), field.to_string()));
            }
            Ok(Format::CustomColumns(parsed))
        }
    }
}

/// `kubelike get <kind>[,<kind>...] [name] [-n] [-A] [-l] [-o]`
///
/// `namespace` is the already-resolved namespace in effect; `namespace_flag`
/// says whether `-n` is what put it there, which is the half that decides
/// whether a cluster-scoped kind refuses the invocation.
pub fn get(
    kinds_arg: &str,
    name: Option<&str>,
    namespace: &str,
    namespace_flag: bool,
    all_namespaces: bool,
    selector: Option<&str>,
    format_spec: Option<&str>,
) -> Outcome {
    let kinds = resolve_kinds(kinds_arg)?;
    let format = parse_format(format_spec)?;

    // The list is judged as one invocation: a cluster-scoped kind anywhere in
    // it refuses the namespace selectors for the whole command.
    if namespace_flag || all_namespaces {
        if let Some(kind) = kinds.iter().find(|kind| !kind.namespaced) {
            return Err(Failure::Usage(format!(
                "kubelike: {} is not a namespaced resource",
                kind.plural
            )));
        }
    }

    let mut selected: Vec<(&'static KindMeta, Vec<Resource>)> = Vec::new();
    for kind in &kinds {
        let mut rows = cluster::all_of(kind);
        if kind.namespaced && !all_namespaces {
            rows.retain(|row| row.namespace() == Some(namespace));
        }
        if let Some(name) = name {
            rows.retain(|row| row.name() == name);
        }
        if let Some(selector) = selector {
            rows.retain(|row| matches_selector(*row, selector));
        }
        selected.push((kind, rows));
    }

    // A get naming one resource that matched nothing is not an empty listing.
    if let Some(name) = name {
        if selected.iter().all(|(_, rows)| rows.is_empty()) {
            return Err(Failure::Domain(not_found(
                kinds[0],
                name,
                namespace,
                all_namespaces,
            )));
        }
    }

    // The field vocabulary is checked against every kind asked for, so an
    // unknown field is refused before any block renders.
    if let Format::CustomColumns(columns) = &format {
        for kind in &kinds {
            for (_, field) in columns {
                if !kind.fields.contains(&field.as_str()) {
                    return Err(Failure::Domain(format!(
                        "kubelike: unknown field \"{}\" for {}; valid fields: {}",
                        field,
                        kind.plural,
                        kind.fields.join(", ")
                    )));
                }
            }
        }
    }

    match &format {
        Format::Json => {
            if name.is_some() {
                // A get naming one resource emits that resource's bare object.
                let resource = selected
                    .iter()
                    .flat_map(|(_, rows)| rows.iter())
                    .next()
                    .expect("the not-found check above leaves at least one row");
                Ok(Emission::Text(format!("{}\n", resource.to_json())))
            } else {
                let items: Vec<Value> = selected
                    .iter()
                    .flat_map(|(_, rows)| rows.iter())
                    .map(|row| row.to_json())
                    .collect();
                Ok(Emission::Text(format!("{}\n", json!({ "items": items }))))
            }
        }
        Format::Name => {
            let lines: Vec<String> = selected
                .iter()
                .flat_map(|(kind, rows)| {
                    rows.iter()
                        .map(move |row| format!("{}/{}", kind.singular, row.name()))
                })
                .collect();
            if lines.is_empty() {
                return Ok(Emission::Empty(no_resources(namespace, all_namespaces)));
            }
            Ok(Emission::Text(format!("{}\n", lines.join("\n"))))
        }
        Format::Default | Format::Wide | Format::CustomColumns(_) => {
            let wide = matches!(format, Format::Wide);
            let mut blocks: Vec<String> = Vec::new();
            for (kind, rows) in &selected {
                if rows.is_empty() {
                    continue;
                }
                let (headers, cells) = match &format {
                    Format::CustomColumns(columns) => custom_block(columns, rows),
                    _ => standard_block(kind, rows, wide, all_namespaces),
                };
                blocks.push(render::table(&headers, &cells));
            }
            if blocks.is_empty() {
                return Ok(Emission::Empty(no_resources(namespace, all_namespaces)));
            }
            Ok(Emission::Text(format!("{}\n", blocks.join("\n\n"))))
        }
    }
}

/// `kubelike describe <kind> <name> [-n <ns>]`
///
/// `namespace` and `namespace_flag` carry the same meaning as in [`get`]:
/// `describe` resolves its namespace exactly as `get` does.
pub fn describe(kind_arg: &str, name: &str, namespace: &str, namespace_flag: bool) -> Outcome {
    let kind = resolve_kind(kind_arg)?;

    if namespace_flag && !kind.namespaced {
        return Err(Failure::Usage(format!(
            "kubelike: {} is not a namespaced resource",
            kind.plural
        )));
    }

    let resource = cluster::all_of(kind)
        .into_iter()
        .find(|row| row.name() == name && (!kind.namespaced || row.namespace() == Some(namespace)));

    let Some(resource) = resource else {
        return Err(Failure::Domain(not_found(kind, name, namespace, false)));
    };

    let entries: Vec<(&str, String)> = match resource {
        Resource::Pod(pod) => vec![
            ("Name", pod.name.to_string()),
            ("Namespace", pod.namespace.to_string()),
            ("Node", pod.node.to_string()),
            ("Status", pod.status.to_string()),
            ("Restarts", pod.restarts.to_string()),
            ("Labels", cluster::pairs_to_text(pod.labels)),
        ],
        Resource::Service(service) => vec![
            ("Name", service.name.to_string()),
            ("Namespace", service.namespace.to_string()),
            ("Type", service.type_.to_string()),
            ("ClusterIP", service.cluster_ip.to_string()),
            ("Port", service.port.to_string()),
            ("Selector", cluster::pairs_to_text(service.selector)),
        ],
        Resource::Node(node) => vec![
            ("Name", node.name.to_string()),
            ("Status", node.status.to_string()),
            ("Role", node.role.to_string()),
            ("Version", node.version.to_string()),
        ],
    };

    Ok(Emission::Text(format!("{}\n", render::detail(&entries))))
}

/// `kubelike api-resources` — the registry itself, as data.
pub fn api_resources() -> Outcome {
    let headers = vec![
        "NAME".to_string(),
        "SHORTNAMES".to_string(),
        "NAMESPACED".to_string(),
        "KIND".to_string(),
    ];
    let rows: Vec<Vec<String>> = cluster::KINDS
        .iter()
        .map(|kind| {
            vec![
                kind.plural.to_string(),
                kind.short.to_string(),
                kind.namespaced.to_string(),
                kind.kind.to_string(),
            ]
        })
        .collect();
    Ok(Emission::Text(format!("{}\n", render::table(&headers, &rows))))
}

/// `kubelike config current-context`
pub fn current_context() -> Outcome {
    match config::kubeconfig_value("current-context") {
        Some(context) => Ok(Emission::Text(format!("{}\n", context))),
        None => Err(Failure::Domain(
            "kubelike: no current context set".to_string(),
        )),
    }
}

fn resolve_kind(arg: &str) -> Result<&'static KindMeta, Failure> {
    cluster::resolve_kind(arg).ok_or_else(|| {
        Failure::Domain(format!(
            "kubelike: the server doesn't have a resource type \"{}\"",
            arg
        ))
    })
}

fn resolve_kinds(arg: &str) -> Result<Vec<&'static KindMeta>, Failure> {
    arg.split(',').map(resolve_kind).collect()
}

fn matches_selector(resource: Resource, selector: &str) -> bool {
    let Some((key, value)) = selector.split_once('=') else {
        return false;
    };
    resource
        .labels()
        .iter()
        .any(|(label, label_value)| *label == key && *label_value == value)
}

/// The kind by its plural registry name, whatever form the argument took.
fn not_found(kind: &KindMeta, name: &str, namespace: &str, all_namespaces: bool) -> String {
    if kind.namespaced && !all_namespaces {
        format!(
            "kubelike: {} \"{}\" not found in namespace \"{}\"",
            kind.plural, name, namespace
        )
    } else {
        format!("kubelike: {} \"{}\" not found", kind.plural, name)
    }
}

fn no_resources(namespace: &str, all_namespaces: bool) -> String {
    if all_namespaces {
        "No resources found.".to_string()
    } else {
        format!("No resources found in {} namespace.", namespace)
    }
}

fn standard_block(
    kind: &KindMeta,
    rows: &[Resource],
    wide: bool,
    all_namespaces: bool,
) -> (Vec<String>, Vec<Vec<String>>) {
    let mut headers: Vec<&str> = Vec::new();
    let mut fields: Vec<&str> = Vec::new();
    if kind.namespaced && all_namespaces {
        headers.push("NAMESPACE");
        fields.push("namespace");
    }
    match kind.plural {
        "pods" => {
            headers.extend(["NAME", "READY", "STATUS", "RESTARTS"]);
            fields.extend(["name", "ready", "status", "restarts"]);
            if wide {
                headers.push("NODE");
                fields.push("node");
            }
        }
        "services" => {
            headers.extend(["NAME", "TYPE", "CLUSTER-IP", "PORT"]);
            fields.extend(["name", "type", "clusterIP", "port"]);
        }
        _ => {
            headers.extend(["NAME", "STATUS", "ROLE", "VERSION"]);
            fields.extend(["name", "status", "role", "version"]);
        }
    }

    let cells = rows
        .iter()
        .map(|row| {
            fields
                .iter()
                .map(|field| row.field(field).unwrap_or_default())
                .collect()
        })
        .collect();
    (headers.into_iter().map(String::from).collect(), cells)
}

fn custom_block(columns: &[(String, String)], rows: &[Resource]) -> (Vec<String>, Vec<Vec<String>>) {
    let headers = columns.iter().map(|(header, _)| header.clone()).collect();
    let cells = rows
        .iter()
        .map(|row| {
            columns
                .iter()
                .map(|(_, field)| row.field(field).unwrap_or_default())
                .collect()
        })
        .collect();
    (headers, cells)
}
