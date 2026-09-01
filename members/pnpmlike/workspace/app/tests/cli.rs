//! Black-box assertions against the produced binary: argv, environment and
//! sandbox files in; stdout, stderr and exit status out — the terms SPEC.md
//! is written in.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const MANIFEST: &str = "\
dep alpha 1.0.0
dep beta 2.3.1
script build echo building
script fail exit 7
script args echo \"$1-$2-$*\"
script both sh -c \"echo out; echo err 1>&2\"
";

const UNPINNED: &str = "dep alpha *\ndep beta 2.3.1\n";

/// A workspace directory with its own manifest, so that tests do not share one.
struct Workspace {
    dir: PathBuf,
}

impl Workspace {
    fn new(name: &str) -> Self {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("workspace directory");
        Self { dir }
    }

    fn with_manifest(name: &str, contents: &str) -> Self {
        let workspace = Self::new(name);
        workspace.write("pnpmlike.pkg", contents);
        workspace
    }

    fn write(&self, file: &str, contents: &str) {
        std::fs::write(self.dir.join(file), contents).expect("write sandbox file");
    }

    fn run(&self, args: &[&str]) -> Run {
        let output = Command::new(env!("CARGO_BIN_EXE_pnpmlike"))
            .args(args)
            .current_dir(&self.dir)
            .output()
            .expect("the binary runs");
        Run::from(output)
    }
}

struct Run {
    status: i32,
    stdout: String,
    stderr: String,
}

impl From<Output> for Run {
    fn from(output: Output) -> Self {
        Self {
            status: output.status.code().expect("the child was not signalled"),
            stdout: String::from_utf8(output.stdout).expect("stdout is utf-8"),
            stderr: String::from_utf8(output.stderr).expect("stderr is utf-8"),
        }
    }
}

impl Run {
    #[track_caller]
    fn assert(&self, status: i32, stdout: &str, stderr: &str) {
        assert_eq!(self.status, status, "exit status");
        assert_eq!(self.stdout, stdout, "stdout");
        assert_eq!(self.stderr, stderr, "stderr");
    }
}

// --- list -------------------------------------------------------------------

#[test]
fn list_prints_one_line_per_package_and_nothing_else() {
    let workspace = Workspace::with_manifest("list", MANIFEST);
    workspace
        .run(&["list"])
        .assert(0, "alpha 1.0.0\nbeta 2.3.1\n", "");
}

#[test]
fn list_reports_no_steps_under_any_reporter() {
    let workspace = Workspace::with_manifest("list-no-steps", MANIFEST);
    for reporter in ["default", "append-only", "ndjson", "silent"] {
        workspace
            .run(&["list", "--reporter", reporter])
            .assert(0, "alpha 1.0.0\nbeta 2.3.1\n", "");
    }
}

#[test]
fn list_json_is_one_document() {
    let workspace = Workspace::with_manifest("list-json", MANIFEST);
    workspace.run(&["list", "--output", "json"]).assert(
        0,
        "{\"packages\":[{\"name\":\"alpha\",\"version\":\"1.0.0\"},\
         {\"name\":\"beta\",\"version\":\"2.3.1\"}]}\n",
        "",
    );
}

// --- install ----------------------------------------------------------------

#[test]
fn install_summarises_on_stdout_and_steps_on_stderr() {
    let workspace = Workspace::with_manifest("install", MANIFEST);
    workspace.run(&["install", "--reporter", "append-only"]).assert(
        0,
        "2 packages installed.\n",
        "1/2 alpha 1.0.0\n2/2 beta 2.3.1\n",
    );
}

#[test]
fn the_dynamic_reporter_rewrites_one_line() {
    let workspace = Workspace::with_manifest("install-default", MANIFEST);
    workspace.run(&["install", "--reporter", "default"]).assert(
        0,
        "2 packages installed.\n",
        "\r1/2 alpha 1.0.0\x1b[K\r2/2 beta 2.3.1\x1b[K\n",
    );
}

#[test]
fn the_machine_reporter_spells_its_objects_exactly() {
    let workspace = Workspace::with_manifest("install-ndjson", MANIFEST);
    workspace.run(&["install", "--reporter", "ndjson"]).assert(
        0,
        "2 packages installed.\n",
        "{\"event\":\"progress\",\"step\":1,\"total\":2,\"name\":\"alpha\",\"version\":\"1.0.0\"}\n\
         {\"event\":\"progress\",\"step\":2,\"total\":2,\"name\":\"beta\",\"version\":\"2.3.1\"}\n\
         {\"event\":\"done\",\"installed\":2}\n",
    );
}

#[test]
fn a_machine_output_mode_keeps_stdout_to_one_document() {
    let workspace = Workspace::with_manifest("install-json", MANIFEST);
    workspace.run(&["install", "--output", "json"]).assert(
        0,
        "{\"installed\":[{\"name\":\"alpha\",\"version\":\"1.0.0\"},\
         {\"name\":\"beta\",\"version\":\"2.3.1\"}],\"count\":2}\n",
        "{\"event\":\"progress\",\"step\":1,\"total\":2,\"name\":\"alpha\",\"version\":\"1.0.0\"}\n\
         {\"event\":\"progress\",\"step\":2,\"total\":2,\"name\":\"beta\",\"version\":\"2.3.1\"}\n\
         {\"event\":\"done\",\"installed\":2}\n",
    );
}

#[test]
fn an_explicit_reporter_outranks_the_machine_output_mode() {
    let workspace = Workspace::with_manifest("install-json-explicit", MANIFEST);
    let run = workspace.run(&["install", "--output", "json", "--reporter", "append-only"]);
    run.assert(
        0,
        "{\"installed\":[{\"name\":\"alpha\",\"version\":\"1.0.0\"},\
         {\"name\":\"beta\",\"version\":\"2.3.1\"}],\"count\":2}\n",
        "1/2 alpha 1.0.0\n2/2 beta 2.3.1\n",
    );
}

#[test]
fn a_detached_stderr_resolves_auto_to_the_append_only_form() {
    // The test harness gives the child pipes, not a terminal.
    let workspace = Workspace::with_manifest("install-auto", MANIFEST);
    workspace.run(&["install"]).assert(
        0,
        "2 packages installed.\n",
        "1/2 alpha 1.0.0\n2/2 beta 2.3.1\n",
    );
}

// --- run --------------------------------------------------------------------

#[test]
fn run_reports_one_step_before_the_child_speaks() {
    let workspace = Workspace::with_manifest("run", MANIFEST);
    workspace
        .run(&["run", "build", "--reporter", "append-only"])
        .assert(0, "building\n", "1/1 build\n");
}

#[test]
fn a_machine_output_mode_neither_wraps_nor_captures_the_child() {
    let workspace = Workspace::with_manifest("run-json", MANIFEST);
    workspace
        .run(&["run", "build", "--output", "json"])
        .assert(0, "building\n", "{\"event\":\"script\",\"name\":\"build\"}\n");
}

#[test]
fn both_of_the_childs_streams_arrive_unchanged() {
    let workspace = Workspace::with_manifest("run-both", MANIFEST);
    workspace
        .run(&["run", "both", "--reporter", "silent"])
        .assert(0, "out\n", "err\n");
    workspace
        .run(&["run", "both", "--output", "json", "--reporter", "silent"])
        .assert(0, "out\n", "err\n");
}

#[test]
fn a_failing_scripts_status_is_propagated_verbatim() {
    let workspace = Workspace::with_manifest("run-fail", MANIFEST);
    let run = workspace.run(&["run", "fail", "--reporter", "silent"]);
    assert_eq!(run.status, 7);
    assert_eq!(run.stdout, "");
}

#[test]
fn arguments_after_the_separator_are_the_scripts_positional_parameters() {
    let workspace = Workspace::with_manifest("run-args", MANIFEST);
    workspace
        .run(&["run", "args", "--reporter", "silent", "--", "a", "b"])
        .assert(0, "a-b-a b\n", "");
}

#[test]
fn a_flag_before_the_separator_belongs_to_pnpmlike() {
    let workspace = Workspace::with_manifest("run-flag", MANIFEST);
    workspace.write("other.pkg", "script build echo other\n");
    workspace
        .run(&["run", "build", "--manifest", "other.pkg", "--reporter", "silent"])
        .assert(0, "other\n", "");
}

#[test]
fn an_undeclared_script_is_exit_three() {
    let workspace = Workspace::with_manifest("run-unknown", MANIFEST);
    let run = workspace.run(&["run", "nope", "--reporter", "silent"]);
    assert_eq!(run.status, 3);
    assert_eq!(run.stdout, "");
    assert!(run.stderr.contains("nope"), "stderr names the script: {:?}", run.stderr);
}

// --- the log channel --------------------------------------------------------

#[test]
fn an_unpinned_package_warns_once() {
    let workspace = Workspace::new("log-warn");
    workspace.write("pnpmlike.pkg", UNPINNED);
    workspace.run(&["install", "--reporter", "append-only"]).assert(
        0,
        "2 packages installed.\n",
        "warning: alpha is unpinned (*)\n1/2 alpha *\n2/2 beta 2.3.1\n",
    );
}

#[test]
fn the_two_silencers_reach_two_different_channels() {
    let workspace = Workspace::new("log-silencers");
    workspace.write("pnpmlike.pkg", UNPINNED);

    // -q silences warnings and keeps progress.
    workspace.run(&["install", "--reporter", "append-only", "-q"]).assert(
        0,
        "2 packages installed.\n",
        "1/2 alpha *\n2/2 beta 2.3.1\n",
    );
    // --reporter silent silences progress and keeps warnings.
    workspace.run(&["install", "--reporter", "silent"]).assert(
        0,
        "2 packages installed.\n",
        "warning: alpha is unpinned (*)\n",
    );
    // --silent silences both.
    workspace
        .run(&["install", "--silent"])
        .assert(0, "2 packages installed.\n", "");
}

#[test]
fn every_log_line_precedes_the_first_reporter_step() {
    let workspace = Workspace::new("log-order");
    workspace.write("pnpmlike.pkg", UNPINNED);
    workspace.run(&["install", "--reporter", "append-only", "-v"]).assert(
        0,
        "2 packages installed.\n",
        "info: using manifest pnpmlike.pkg\n\
         warning: alpha is unpinned (*)\n\
         1/2 alpha *\n2/2 beta 2.3.1\n",
    );
}

#[test]
fn the_info_note_names_the_path_as_given() {
    let workspace = Workspace::new("log-path");
    workspace.write("named.pkg", UNPINNED);
    workspace
        .run(&["list", "--manifest", "named.pkg", "-v", "--loglevel", "info"])
        .assert(
            0,
            "alpha *\nbeta 2.3.1\n",
            "info: using manifest named.pkg\nwarning: alpha is unpinned (*)\n",
        );
}

#[test]
fn an_explicit_loglevel_outranks_the_shorthands() {
    let workspace = Workspace::new("log-outrank");
    workspace.write("pnpmlike.pkg", UNPINNED);
    workspace
        .run(&["install", "--reporter", "silent", "-q", "--loglevel", "info"])
        .assert(
            0,
            "2 packages installed.\n",
            "info: using manifest pnpmlike.pkg\nwarning: alpha is unpinned (*)\n",
        );
}

// --- four ways of having no answer -----------------------------------------

#[test]
fn an_absent_manifest_is_an_empty_workspace() {
    let workspace = Workspace::new("empty");
    workspace.run(&["list"]).assert(0, "", "");
    workspace
        .run(&["list", "--output", "json"])
        .assert(0, "{\"packages\":[]}\n", "");
    workspace
        .run(&["install", "--reporter", "append-only"])
        .assert(0, "0 packages installed.\n", "");
    assert_eq!(
        workspace.run(&["run", "build", "--reporter", "silent"]).status,
        3
    );
    // A named manifest that is not there is equally empty.
    workspace
        .run(&["list", "--manifest", "nowhere.pkg"])
        .assert(0, "", "");
}

#[test]
fn a_malformed_manifest_is_a_located_domain_error() {
    let workspace = Workspace::new("malformed");
    workspace.write("bad.pkg", "dep alpha 1.0.0\nnonsense here\n");
    for command in [
        vec!["list", "--manifest", "bad.pkg"],
        vec!["install", "--manifest", "bad.pkg"],
        vec!["run", "build", "--manifest", "bad.pkg"],
    ] {
        let run = workspace.run(&command);
        assert_eq!(run.status, 1, "{:?}", command);
        assert_eq!(run.stdout, "", "{:?}", command);
        assert!(
            run.stderr.contains("bad.pkg line 2"),
            "stderr locates the line: {:?}",
            run.stderr
        );
    }
}

#[test]
fn a_bad_flag_value_is_a_usage_error() {
    let workspace = Workspace::with_manifest("usage", MANIFEST);
    for command in [
        vec!["list", "--reporter", "nope"],
        vec!["list", "--loglevel", "nope"],
        vec!["list", "--bogus"],
        vec!["bogus"],
        vec![],
    ] {
        let run = workspace.run(&command);
        assert_eq!(run.status, 2, "{:?}", command);
        assert_eq!(run.stdout, "", "{:?}", command);
    }
}

// --- manifest shapes --------------------------------------------------------

#[test]
fn blank_lines_and_comments_are_ignored_and_commands_are_verbatim() {
    let workspace = Workspace::new("shapes");
    workspace.write(
        "pnpmlike.pkg",
        "# a comment\n\n   \n  dep gamma 3.0\nscript hash echo \"a # b\"\n",
    );
    workspace.run(&["list"]).assert(0, "gamma 3.0\n", "");
    workspace
        .run(&["run", "hash", "--reporter", "silent"])
        .assert(0, "a # b\n", "");
}
