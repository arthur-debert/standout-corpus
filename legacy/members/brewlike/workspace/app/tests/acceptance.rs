//! The spec, read back from the real binary.
//!
//! Exit codes, which stream a byte went to, and the exact bytes are all part
//! of `brewlike`'s contract, and those are settled at the process boundary —
//! so these run the compiled binary through `TestHarness::run_process` rather
//! than the in-process runner.

use standout_test::{serial, TestHarness};

const BIN: &str = env!("CARGO_BIN_EXE_brewlike");

fn run(args: &[&str]) -> standout_test::ProcessResult {
    TestHarness::new().run_process(BIN, args)
}

fn json(args: &[&str]) -> serde_json::Value {
    let result = run(args);
    assert_eq!(result.code(), Some(0), "stderr: {}", result.stderr());
    assert_eq!(result.stderr(), "", "machine mode writes nothing to stderr");
    serde_json::from_str(result.stdout()).expect("one JSON document on stdout")
}

// ---------------------------------------------------------------- human mode

#[test]
#[serial]
fn list_names_every_installed_formula_and_its_version() {
    let result = run(&["list"]);
    assert_eq!(result.code(), Some(0));
    assert_eq!(
        result.stdout(),
        "FORMULA VERSION\n\
         basalt  2.1.0\n\
         granite 1.4.2\n\
         pebble  0.9.0\n\
         quartz  3.0.1\n"
    );
}

#[test]
#[serial]
fn info_reports_both_versions_and_the_dependencies() {
    assert_eq!(
        run(&["info", "basalt"]).stdout(),
        "basalt: stable 2.1.0\n\
         installed: 2.1.0\n\
         depends on: pebble, quartz\n"
    );
}

#[test]
#[serial]
fn info_marks_an_installed_version_that_is_behind() {
    assert_eq!(
        run(&["info", "granite"]).stdout(),
        "granite: stable 1.5.0\n\
         installed: 1.4.2 (outdated)\n\
         depends on: pebble\n"
    );
}

#[test]
#[serial]
fn info_says_none_rather_than_trailing_an_empty_list() {
    assert_eq!(
        run(&["info", "pebble"]).stdout(),
        "pebble: stable 0.9.0\n\
         installed: 0.9.0\n\
         depends on: none\n"
    );
}

#[test]
#[serial]
fn deps_is_the_deduplicated_closure_one_name_per_line() {
    assert_eq!(run(&["deps", "basalt"]).stdout(), "pebble\nquartz\n");
}

#[test]
#[serial]
fn deps_of_a_leaf_prints_nothing_and_succeeds() {
    let result = run(&["deps", "pebble"]);
    assert_eq!(result.code(), Some(0));
    assert_eq!(result.stdout(), "");
}

#[test]
#[serial]
fn deps_tree_repeats_a_formula_reached_through_two_paths() {
    assert_eq!(
        run(&["deps", "--tree", "basalt"]).stdout(),
        "basalt\n\
         \x20 pebble\n\
         \x20 quartz\n\
         \x20   pebble\n"
    );
}

#[test]
#[serial]
fn outdated_shows_both_versions_of_what_is_behind() {
    assert_eq!(
        run(&["outdated"]).stdout(),
        "FORMULA INSTALLED LATEST\n\
         granite 1.4.2     1.5.0\n\
         quartz  3.0.1     3.2.0\n"
    );
}

#[test]
#[serial]
fn outdated_restricts_to_the_named_formulae() {
    assert_eq!(
        run(&["outdated", "granite"]).stdout(),
        "FORMULA INSTALLED LATEST\n\
         granite 1.4.2     1.5.0\n"
    );
}

#[test]
#[serial]
fn an_empty_outdated_answer_is_a_sentence_and_still_succeeds() {
    let result = run(&["outdated", "basalt", "pebble"]);
    assert_eq!(result.code(), Some(0));
    assert_eq!(result.stdout(), "No outdated formulae.\n");
}

// -------------------------------------------------------------- machine mode

#[test]
#[serial]
fn list_is_an_array_of_records_binding_their_own_values() {
    assert_eq!(
        json(&["list", "--output", "json"]),
        serde_json::json!([
            {"name": "basalt", "installed": "2.1.0", "latest": "2.1.0", "outdated": false},
            {"name": "granite", "installed": "1.4.2", "latest": "1.5.0", "outdated": true},
            {"name": "pebble", "installed": "0.9.0", "latest": "0.9.0", "outdated": false},
            {"name": "quartz", "installed": "3.0.1", "latest": "3.2.0", "outdated": true},
        ])
    );
}

#[test]
#[serial]
fn info_is_one_record_with_its_direct_dependencies() {
    assert_eq!(
        json(&["info", "granite", "--output", "json"]),
        serde_json::json!({
            "name": "granite",
            "installed": "1.4.2",
            "latest": "1.5.0",
            "outdated": true,
            "dependencies": ["pebble"],
        })
    );
}

#[test]
#[serial]
fn deps_is_an_array_of_names() {
    assert_eq!(
        json(&["deps", "basalt", "--output", "json"]),
        serde_json::json!(["pebble", "quartz"])
    );
}

#[test]
#[serial]
fn the_nesting_survives_serialization() {
    assert_eq!(
        json(&["deps", "--tree", "basalt", "--output", "json"]),
        serde_json::json!({
            "name": "basalt",
            "dependencies": [
                {"name": "pebble", "dependencies": []},
                {"name": "quartz", "dependencies": [{"name": "pebble", "dependencies": []}]},
            ],
        })
    );
}

#[test]
#[serial]
fn an_empty_result_is_an_empty_list_never_the_human_sentence() {
    assert_eq!(
        json(&["outdated", "basalt", "--output", "json"]),
        serde_json::json!([])
    );
    assert_eq!(
        json(&["deps", "pebble", "--output", "json"]),
        serde_json::json!([])
    );
}

// -------------------------------------------------------------- exit codes

#[test]
#[serial]
fn an_unknown_formula_is_a_domain_error_on_stderr() {
    for args in [
        vec!["info", "obsidian"],
        vec!["deps", "obsidian"],
        vec!["deps", "--tree", "obsidian"],
        vec!["outdated", "granite", "obsidian"],
    ] {
        let result = run(&args);
        assert_eq!(result.code(), Some(1), "{args:?}");
        assert_eq!(result.stdout(), "", "{args:?}");
        assert_eq!(
            result.stderr(),
            "brewlike: no formula found: obsidian\n",
            "{args:?}"
        );
    }
}

#[test]
#[serial]
fn a_usage_error_is_status_two_with_nothing_on_stdout() {
    for args in [
        vec![],
        vec!["obsidian"],
        vec!["list", "--nonesuch"],
        vec!["info"],
    ] {
        let result = run(&args);
        assert_eq!(result.code(), Some(2), "{args:?}");
        assert_eq!(result.stdout(), "", "{args:?}");
        assert!(!result.stderr().is_empty(), "{args:?}");
    }
}

#[test]
#[serial]
fn help_works_at_every_level_and_succeeds_on_stdout() {
    for args in [
        vec!["--help"],
        vec!["list", "--help"],
        vec!["info", "--help"],
        vec!["deps", "--help"],
        vec!["outdated", "--help"],
    ] {
        let result = run(&args);
        assert_eq!(result.code(), Some(0), "{args:?}");
        assert!(!result.stdout().is_empty(), "{args:?}");
    }
}
