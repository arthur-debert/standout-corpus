//! Post-output shaping of the stdout document.
//!
//! The spec pins the bytes of the machine document, and three things the
//! render pass alone does not give:
//!
//! * an empty workspace's `list` prints *nothing at all*, but `App::run` writes
//!   a handled command's text with `writeln!`, so an empty render would still
//!   leave a bare newline on stdout;
//! * the document is one line, while `standout-render` serializes JSON with
//!   `serde_json::to_string_pretty` (`docs/crates/render/topics/templating.md`,
//!   "Structured Output");
//! * the document's keys are in the order the spec spells them, while the
//!   framework's serialization sorts them alphabetically.
//!
//! All three are presentation concerns, so they sit in a post-output hook
//! rather than in a handler: the handler still returns the same view data
//! whatever `--output` says.

use clap::ArgMatches;
use standout_dispatch::{CommandContext, HookError, RenderedOutput};

/// A post-output hook that renders the command's structured document with the
/// given top-level key order, on one line.
pub fn shape_document(
    key_order: &'static [&'static str],
) -> impl Fn(&ArgMatches, &CommandContext, RenderedOutput) -> Result<RenderedOutput, HookError> {
    move |_matches, _ctx, output| {
        let RenderedOutput::Text(mut text) = output else {
            return Ok(output);
        };

        if text.formatted.is_empty() {
            return Ok(RenderedOutput::Silent);
        }

        text.formatted = compact_json(text.formatted, key_order);
        text.raw = compact_json(text.raw, key_order);
        Ok(RenderedOutput::Text(text))
    }
}

/// Collapse a pretty-printed JSON object onto one line, leading with
/// `key_order`. Text, YAML, XML and CSV renderings are left alone: none of them
/// parse as a JSON object.
fn compact_json(rendered: String, key_order: &[&str]) -> String {
    let Ok(serde_json::Value::Object(map)) = serde_json::from_str(&rendered) else {
        return rendered;
    };

    let mut members: Vec<String> = Vec::with_capacity(map.len());
    for key in key_order {
        if let Some(value) = map.get(*key) {
            members.push(member(key, value));
        }
    }
    for (key, value) in &map {
        if !key_order.contains(&key.as_str()) {
            members.push(member(key, value));
        }
    }

    format!("{{{}}}", members.join(","))
}

fn member(key: &str, value: &serde_json::Value) -> String {
    format!(
        "{}:{}",
        serde_json::Value::String(key.to_string()),
        value
    )
}

#[cfg(test)]
mod tests {
    use super::compact_json;

    const INSTALL_KEYS: &[&str] = &["installed", "count"];

    #[test]
    fn a_json_object_is_collapsed_onto_one_line() {
        let pretty = "{\n  \"packages\": [\n    {\n      \"name\": \"alpha\",\n      \"version\": \"1.0.0\"\n    }\n  ]\n}";
        assert_eq!(
            compact_json(pretty.to_string(), &["packages"]),
            r#"{"packages":[{"name":"alpha","version":"1.0.0"}]}"#
        );
    }

    #[test]
    fn the_declared_key_order_wins_over_the_alphabet() {
        let sorted = "{\n  \"count\": 1,\n  \"installed\": [\n    {\n      \"name\": \"a b\",\n      \"version\": \"1.0\"\n    }\n  ]\n}";
        assert_eq!(
            compact_json(sorted.to_string(), INSTALL_KEYS),
            r#"{"installed":[{"name":"a b","version":"1.0"}],"count":1}"#
        );
    }

    #[test]
    fn an_empty_document_keeps_its_braces() {
        assert_eq!(compact_json("{}".into(), INSTALL_KEYS), "{}");
        assert_eq!(
            compact_json("{\n  \"packages\": []\n}".into(), &["packages"]),
            r#"{"packages":[]}"#
        );
    }

    #[test]
    fn a_key_the_order_does_not_name_is_still_emitted() {
        assert_eq!(
            compact_json("{\"extra\": 1, \"count\": 2}".into(), INSTALL_KEYS),
            r#"{"count":2,"extra":1}"#
        );
    }

    #[test]
    fn other_renderings_pass_through() {
        assert_eq!(compact_json("alpha 1.0.0".into(), INSTALL_KEYS), "alpha 1.0.0");
        assert_eq!(
            compact_json("2 packages installed.".into(), INSTALL_KEYS),
            "2 packages installed."
        );
        assert_eq!(
            compact_json("installed:\n- name: alpha\ncount: 1\n".into(), INSTALL_KEYS),
            "installed:\n- name: alpha\ncount: 1\n"
        );
    }
}
