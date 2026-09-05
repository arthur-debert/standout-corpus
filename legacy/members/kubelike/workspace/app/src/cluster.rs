//! The frozen cluster state and the kind registry.
//!
//! CLI-free: nothing here knows about clap, standout, or output formats.

use serde_json::{json, Value};

/// Registry metadata for one resource kind.
pub struct KindMeta {
    pub plural: &'static str,
    pub singular: &'static str,
    pub short: &'static str,
    pub namespaced: bool,
    pub kind: &'static str,
    /// The scalar keys of the kind's JSON object, minus `kind`, in the order
    /// the unknown-field diagnostic lists them.
    pub fields: &'static [&'static str],
}

pub const KINDS: [KindMeta; 3] = [
    KindMeta {
        plural: "pods",
        singular: "pod",
        short: "po",
        namespaced: true,
        kind: "Pod",
        fields: &["name", "namespace", "ready", "status", "restarts", "node"],
    },
    KindMeta {
        plural: "services",
        singular: "service",
        short: "svc",
        namespaced: true,
        kind: "Service",
        fields: &["name", "namespace", "type", "clusterIP", "port"],
    },
    KindMeta {
        plural: "nodes",
        singular: "node",
        short: "no",
        namespaced: false,
        kind: "Node",
        fields: &["name", "status", "role", "version"],
    },
];

/// A kind argument may be the plural name, the singular, or the short name.
pub fn resolve_kind(arg: &str) -> Option<&'static KindMeta> {
    KINDS
        .iter()
        .find(|k| arg == k.plural || arg == k.singular || arg == k.short)
}

pub struct Pod {
    pub namespace: &'static str,
    pub name: &'static str,
    pub ready: &'static str,
    pub status: &'static str,
    pub restarts: u32,
    pub node: &'static str,
    pub labels: &'static [(&'static str, &'static str)],
}

pub const PODS: [Pod; 4] = [
    Pod {
        namespace: "default",
        name: "web-1",
        ready: "1/1",
        status: "Running",
        restarts: 0,
        node: "node-a",
        labels: &[("app", "web")],
    },
    Pod {
        namespace: "default",
        name: "web-2",
        ready: "1/1",
        status: "Running",
        restarts: 2,
        node: "node-b",
        labels: &[("app", "web")],
    },
    Pod {
        namespace: "default",
        name: "db-0",
        ready: "1/1",
        status: "Running",
        restarts: 0,
        node: "node-a",
        labels: &[("app", "db")],
    },
    Pod {
        namespace: "kube-system",
        name: "dns-1",
        ready: "1/1",
        status: "Running",
        restarts: 0,
        node: "node-a",
        labels: &[("app", "dns")],
    },
];

pub struct Service {
    pub namespace: &'static str,
    pub name: &'static str,
    pub type_: &'static str,
    pub cluster_ip: &'static str,
    pub port: &'static str,
    pub selector: &'static [(&'static str, &'static str)],
}

pub const SERVICES: [Service; 2] = [
    Service {
        namespace: "default",
        name: "web",
        type_: "ClusterIP",
        cluster_ip: "10.0.0.10",
        port: "80/TCP",
        selector: &[("app", "web")],
    },
    Service {
        namespace: "kube-system",
        name: "dns",
        type_: "ClusterIP",
        cluster_ip: "10.0.0.53",
        port: "53/UDP",
        selector: &[("app", "dns")],
    },
];

pub struct Node {
    pub name: &'static str,
    pub status: &'static str,
    pub role: &'static str,
    pub version: &'static str,
}

pub const NODES: [Node; 2] = [
    Node {
        name: "node-a",
        status: "Ready",
        role: "control-plane",
        version: "v1.0.0",
    },
    Node {
        name: "node-b",
        status: "Ready",
        role: "worker",
        version: "v1.0.0",
    },
];

/// One resource of any kind, borrowed from the frozen state.
#[derive(Clone, Copy)]
pub enum Resource {
    Pod(&'static Pod),
    Service(&'static Service),
    Node(&'static Node),
}

impl Resource {
    pub fn name(&self) -> &'static str {
        match self {
            Resource::Pod(p) => p.name,
            Resource::Service(s) => s.name,
            Resource::Node(n) => n.name,
        }
    }

    pub fn namespace(&self) -> Option<&'static str> {
        match self {
            Resource::Pod(p) => Some(p.namespace),
            Resource::Service(s) => Some(s.namespace),
            Resource::Node(_) => None,
        }
    }

    /// The label set a `-l` selector is matched against. Nodes carry none.
    pub fn labels(&self) -> &'static [(&'static str, &'static str)] {
        match self {
            Resource::Pod(p) => p.labels,
            Resource::Service(s) => s.selector,
            Resource::Node(_) => &[],
        }
    }

    /// A `custom-columns` field value. `None` means the field is not this
    /// kind's; the caller turns that into the unknown-field diagnostic.
    pub fn field(&self, field: &str) -> Option<String> {
        match self {
            Resource::Pod(p) => match field {
                "name" => Some(p.name.to_string()),
                "namespace" => Some(p.namespace.to_string()),
                "ready" => Some(p.ready.to_string()),
                "status" => Some(p.status.to_string()),
                "restarts" => Some(p.restarts.to_string()),
                "node" => Some(p.node.to_string()),
                _ => None,
            },
            Resource::Service(s) => match field {
                "name" => Some(s.name.to_string()),
                "namespace" => Some(s.namespace.to_string()),
                "type" => Some(s.type_.to_string()),
                "clusterIP" => Some(s.cluster_ip.to_string()),
                "port" => Some(s.port.to_string()),
                _ => None,
            },
            Resource::Node(n) => match field {
                "name" => Some(n.name.to_string()),
                "status" => Some(n.status.to_string()),
                "role" => Some(n.role.to_string()),
                "version" => Some(n.version.to_string()),
                _ => None,
            },
        }
    }

    /// The `-o json` shape. Field order is the order of the object literal.
    pub fn to_json(&self) -> Value {
        match self {
            Resource::Pod(p) => json!({
                "kind": "Pod",
                "namespace": p.namespace,
                "name": p.name,
                "ready": p.ready,
                "status": p.status,
                "restarts": p.restarts,
                "node": p.node,
                "labels": pairs_to_json(p.labels),
            }),
            Resource::Service(s) => json!({
                "kind": "Service",
                "namespace": s.namespace,
                "name": s.name,
                "type": s.type_,
                "clusterIP": s.cluster_ip,
                "port": s.port,
                "selector": pairs_to_json(s.selector),
            }),
            Resource::Node(n) => json!({
                "kind": "Node",
                "name": n.name,
                "status": n.status,
                "role": n.role,
                "version": n.version,
            }),
        }
    }
}

fn pairs_to_json(pairs: &'static [(&'static str, &'static str)]) -> Value {
    let mut map = serde_json::Map::new();
    for (key, value) in pairs {
        map.insert((*key).to_string(), Value::String((*value).to_string()));
    }
    Value::Object(map)
}

/// Render a label/selector map the way `describe` prints it.
pub fn pairs_to_text(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join(",")
}

/// Every resource of `kind`, in the frozen listing order.
pub fn all_of(kind: &KindMeta) -> Vec<Resource> {
    match kind.plural {
        "pods" => PODS.iter().map(Resource::Pod).collect(),
        "services" => SERVICES.iter().map(Resource::Service).collect(),
        _ => NODES.iter().map(Resource::Node).collect(),
    }
}
