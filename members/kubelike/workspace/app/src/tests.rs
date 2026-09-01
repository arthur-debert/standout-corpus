//! Behavior tests over the CLI-free core. The core takes an already-resolved
//! namespace, so none of these depend on the ambient chain (flag, env,
//! kubeconfig, `HOME`) — that half is the handler's, and `config` owns it.

use crate::core::{self, Emission, Failure, Outcome};
use crate::render;

fn text(outcome: Outcome) -> String {
    match outcome {
        Ok(Emission::Text(text)) => text,
        Ok(Emission::Empty(note)) => panic!("expected text, got the empty note {:?}", note),
        Err(failure) => panic!("expected text, got {:?}", failure.message()),
    }
}

fn empty(outcome: Outcome) -> String {
    match outcome {
        Ok(Emission::Empty(note)) => note,
        Ok(Emission::Text(text)) => panic!("expected an empty listing, got {:?}", text),
        Err(failure) => panic!("expected an empty listing, got {:?}", failure.message()),
    }
}

fn failure(outcome: Outcome) -> (u8, String) {
    match outcome {
        Err(failure) => (failure.status(), failure.message().to_string()),
        Ok(Emission::Text(text)) => panic!("expected a failure, got {:?}", text),
        Ok(Emission::Empty(note)) => panic!("expected a failure, got the note {:?}", note),
    }
}

/// A tiny stand-in for the parsed `get` line: the kind list first, then flags.
/// The namespace in effect is `default` unless `-n` names another, which is
/// what the resolution chain produces for an unconfigured machine.
fn get(args: &[&str]) -> Outcome {
    let mut kinds = "";
    let mut name = None;
    let mut namespace = "default";
    let mut namespace_flag = false;
    let mut all_namespaces = false;
    let mut selector = None;
    let mut format = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match *arg {
            "-n" => {
                namespace = rest.next().expect("-n takes a value");
                namespace_flag = true;
            }
            "-A" => all_namespaces = true,
            "-l" => selector = Some(*rest.next().expect("-l takes a value")),
            "-o" => format = Some(*rest.next().expect("-o takes a value")),
            other if kinds.is_empty() => kinds = other,
            other => name = Some(other),
        }
    }
    core::get(
        kinds,
        name,
        namespace,
        namespace_flag,
        all_namespaces,
        selector,
        format,
    )
}

#[test]
fn kinds_resolve_by_plural_singular_and_short_name() {
    let expected = text(get(&["pods"]));
    assert_eq!(text(get(&["pod"])), expected);
    assert_eq!(text(get(&["po"])), expected);
}

#[test]
fn the_default_table_pads_every_column_but_the_last() {
    assert_eq!(
        text(get(&["pods"])),
        "NAME  READY STATUS  RESTARTS\n\
         web-1 1/1   Running 0\n\
         web-2 1/1   Running 2\n\
         db-0  1/1   Running 0\n"
    );
}

#[test]
fn all_namespaces_adds_a_leading_namespace_column() {
    assert_eq!(
        text(get(&["pods", "-A"])),
        "NAMESPACE   NAME  READY STATUS  RESTARTS\n\
         default     web-1 1/1   Running 0\n\
         default     web-2 1/1   Running 2\n\
         default     db-0  1/1   Running 0\n\
         kube-system dns-1 1/1   Running 0\n"
    );
}

#[test]
fn wide_adds_node_for_pods_and_nothing_elsewhere() {
    assert!(text(get(&["pods", "-o", "wide"])).starts_with("NAME  READY STATUS  RESTARTS NODE\n"));
    assert_eq!(text(get(&["services", "-o", "wide"])), text(get(&["services"])));
    assert_eq!(text(get(&["nodes", "-o", "wide"])), text(get(&["nodes"])));
}

#[test]
fn name_format_is_one_identifier_per_line() {
    assert_eq!(
        text(get(&["pods", "-o", "name"])),
        "pod/web-1\npod/web-2\npod/db-0\n"
    );
}

#[test]
fn a_heterogeneous_listing_renders_one_block_per_kind() {
    let rendered = text(get(&["pods,services"]));
    let blocks: Vec<&str> = rendered.trim_end().split("\n\n").collect();
    assert_eq!(blocks.len(), 2);
    assert!(blocks[0].starts_with("NAME  READY"));
    assert!(blocks[1].starts_with("NAME TYPE"));
}

#[test]
fn a_heterogeneous_listing_emits_one_flat_json_list() {
    let rendered = text(get(&["pods,services", "-o", "json"]));
    let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
    let items = parsed["items"].as_array().unwrap();
    assert_eq!(items.len(), 4);
    assert_eq!(items[0]["kind"], "Pod");
    assert_eq!(items[3]["kind"], "Service");
    assert_eq!(items[0]["restarts"], 0);
    assert_eq!(items[0]["labels"]["app"], "web");
    assert!(items[3].get("node").is_none());
}

#[test]
fn a_get_naming_one_resource_emits_a_bare_json_object() {
    let rendered = text(get(&["pods", "web-1", "-o", "json"]));
    let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(parsed["name"], "web-1");
    assert!(parsed.get("items").is_none());
}

#[test]
fn nodes_carry_no_namespace_in_json() {
    let rendered = text(get(&["nodes", "-o", "json"]));
    let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
    assert!(parsed["items"][0].get("namespace").is_none());
}

#[test]
fn custom_columns_take_the_callers_headers_in_order() {
    assert_eq!(
        text(get(&["pods", "-o", "custom-columns=NAME:name,NODE:node"])),
        "NAME  NODE\nweb-1 node-a\nweb-2 node-b\ndb-0  node-a\n"
    );
}

#[test]
fn an_unknown_custom_columns_field_names_the_offender_and_the_vocabulary() {
    assert_eq!(
        failure(get(&["pods", "-o", "custom-columns=X:bogus"])),
        (
            1,
            "kubelike: unknown field \"bogus\" for pods; valid fields: \
             name, namespace, ready, status, restarts, node"
                .to_string()
        )
    );
}

#[test]
fn a_selector_keeps_matching_labels_and_widths_follow_the_survivors() {
    assert_eq!(
        text(get(&["pods", "-l", "app=db"])),
        "NAME READY STATUS  RESTARTS\ndb-0 1/1   Running 0\n"
    );
}

#[test]
fn an_empty_listing_is_a_success_that_explains_itself() {
    assert_eq!(
        empty(get(&["pods", "-n", "nowhere"])),
        "No resources found in nowhere namespace."
    );
    assert_eq!(
        empty(get(&["pods", "-A", "-l", "app=nope"])),
        "No resources found."
    );
}

#[test]
fn an_empty_json_listing_is_an_empty_list_on_stdout() {
    assert_eq!(text(get(&["pods", "-n", "nowhere", "-o", "json"])), "{\"items\":[]}\n");
}

#[test]
fn namespace_selectors_are_a_usage_error_on_a_cluster_scoped_kind() {
    let expected = (2, "kubelike: nodes is not a namespaced resource".to_string());
    assert_eq!(failure(get(&["nodes", "-n", "default"])), expected);
    assert_eq!(failure(get(&["nodes", "-A"])), expected);
    // The list is judged as one invocation, and no block renders.
    assert_eq!(failure(get(&["pods,nodes", "-n", "default"])), expected);
    assert_eq!(
        failure(core::describe("node", "node-a", "default", true)),
        expected
    );
}

#[test]
fn a_mixed_list_without_those_flags_lists_each_kind_in_its_own_scope() {
    let rendered = text(get(&["pods,nodes"]));
    assert!(rendered.contains("web-1"));
    assert!(rendered.contains("node-a"));
}

#[test]
fn an_unknown_kind_is_a_domain_error() {
    assert_eq!(
        failure(get(&["widgets"])),
        (
            1,
            "kubelike: the server doesn't have a resource type \"widgets\"".to_string()
        )
    );
}

#[test]
fn an_absent_name_reports_the_kinds_plural_registry_name() {
    assert_eq!(
        failure(core::describe("svc", "nope", "default", true)),
        (
            1,
            "kubelike: services \"nope\" not found in namespace \"default\"".to_string()
        )
    );
    assert_eq!(
        failure(get(&["po", "nope"])),
        (
            1,
            "kubelike: pods \"nope\" not found in namespace \"default\"".to_string()
        )
    );
}

#[test]
fn describe_pads_keys_to_the_widest_key_in_its_own_block() {
    assert_eq!(
        text(core::describe("pod", "web-1", "default", false)),
        "Name:      web-1\n\
         Namespace: default\n\
         Node:      node-a\n\
         Status:    Running\n\
         Restarts:  0\n\
         Labels:    app=web\n"
    );
    assert_eq!(
        text(core::describe("node", "node-a", "default", false)),
        "Name:    node-a\n\
         Status:  Ready\n\
         Role:    control-plane\n\
         Version: v1.0.0\n"
    );
}

#[test]
fn api_resources_prints_the_registry_as_data() {
    assert_eq!(
        text(core::api_resources()),
        "NAME     SHORTNAMES NAMESPACED KIND\n\
         pods     po         true       Pod\n\
         services svc        true       Service\n\
         nodes    no         false      Node\n"
    );
}

#[test]
fn the_layout_rule_leaves_the_last_column_unpadded() {
    let headers = vec!["A".to_string(), "B".to_string()];
    let rows = vec![vec!["long-value".to_string(), "x".to_string()]];
    assert_eq!(render::table(&headers, &rows), "A          B\nlong-value x");
}

#[test]
fn a_failures_status_separates_domain_from_usage() {
    let domain = Failure::Domain("nope".into());
    let usage = Failure::Usage("nope".into());
    assert_eq!((domain.status(), usage.status()), (1, 2));
}
