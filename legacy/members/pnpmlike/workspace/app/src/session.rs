//! Resolving one invocation's channel settings: which manifest, which
//! reporter, which log level.

use std::io::IsTerminal;
use std::path::PathBuf;

use clap::parser::ValueSource;
use clap::ArgMatches;

use crate::channels::{Log, LogLevel, Reporter, ReporterKind};
use crate::cli::{LogLevelArg, ReporterArg};

/// The default manifest, read when `--manifest` is absent.
pub const DEFAULT_MANIFEST: &str = "pnpmlike.pkg";

/// Facts about the invocation that are settled before dispatch and that a
/// handler cannot read off `ArgMatches`.
///
/// `--output` is Standout's own flag, and the resolved output mode is
/// deliberately absent from `CommandContext` (see `docs/topics/output-modes.md`,
/// "Keep Output Mode Out of Handlers"). The reporter, though, is a *stderr*
/// concern that has to know whether stdout is carrying a machine document, so
/// this one bit is read off argv at startup and handed to handlers as app
/// state.
#[derive(Debug, Clone, Copy)]
pub struct Invocation {
    pub machine_output: bool,
}

/// Output modes that make stdout a single machine document.
const MACHINE_MODES: [&str; 4] = ["json", "yaml", "xml", "csv"];

/// Flags that take a separate value, whose value must not be mistaken for a
/// flag of its own while scanning.
const VALUE_FLAGS: [&str; 5] = [
    "--output",
    "--output-file-path",
    "--manifest",
    "--reporter",
    "--loglevel",
];

impl Invocation {
    pub fn from_argv<I, S>(args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut machine_output = false;
        let mut expect_value: Option<String> = None;

        for arg in args.into_iter().skip(1) {
            let arg = arg.as_ref();

            if let Some(flag) = expect_value.take() {
                if flag == "--output" {
                    machine_output = is_machine_mode(arg);
                }
                continue;
            }

            // Everything after `--` belongs to the script, not to pnpmlike.
            if arg == "--" {
                break;
            }

            if let Some(value) = arg.strip_prefix("--output=") {
                machine_output = is_machine_mode(value);
            } else if VALUE_FLAGS.contains(&arg) {
                expect_value = Some(arg.to_string());
            }
        }

        Self { machine_output }
    }
}

fn is_machine_mode(value: &str) -> bool {
    MACHINE_MODES.contains(&value)
}

/// Everything one command needs to speak on its three channels.
pub struct Session {
    pub manifest_path: PathBuf,
    /// The manifest path as the user gave it, which is what diagnostics name.
    pub manifest_as_given: String,
    pub log: Log,
    pub reporter: Reporter,
}

impl Session {
    pub fn resolve(matches: &ArgMatches, invocation: Invocation) -> Self {
        Self::resolve_with(matches, invocation, std::io::stderr().is_terminal())
    }

    pub fn resolve_with(
        matches: &ArgMatches,
        invocation: Invocation,
        stderr_attended: bool,
    ) -> Self {
        let silent = matches.get_flag("silent");

        let requested = if was_given(matches, "reporter") {
            *matches
                .get_one::<ReporterArg>("reporter")
                .expect("reporter has a default")
        } else if silent {
            ReporterArg::Silent
        } else {
            ReporterArg::Auto
        };

        let reporter = match requested {
            ReporterArg::Default => ReporterKind::Default,
            ReporterArg::AppendOnly => ReporterKind::AppendOnly,
            ReporterArg::Ndjson => ReporterKind::Ndjson,
            ReporterArg::Silent => ReporterKind::Silent,
            ReporterArg::Auto => {
                if invocation.machine_output {
                    ReporterKind::Ndjson
                } else if stderr_attended {
                    ReporterKind::Default
                } else {
                    ReporterKind::AppendOnly
                }
            }
        };

        let level = if was_given(matches, "loglevel") {
            match matches
                .get_one::<LogLevelArg>("loglevel")
                .expect("loglevel has a default")
            {
                LogLevelArg::Error => LogLevel::Error,
                LogLevelArg::Warn => LogLevel::Warn,
                LogLevelArg::Info => LogLevel::Info,
            }
        } else if silent || matches.get_flag("quiet") {
            LogLevel::Error
        } else if matches.get_flag("verbose") {
            LogLevel::Info
        } else {
            LogLevel::Warn
        };

        let manifest_as_given = matches
            .get_one::<String>("manifest")
            .cloned()
            .unwrap_or_else(|| DEFAULT_MANIFEST.to_string());

        Self {
            manifest_path: PathBuf::from(&manifest_as_given),
            manifest_as_given,
            log: Log::new(level),
            reporter: Reporter::new(reporter),
        }
    }
}

fn was_given(matches: &ArgMatches, id: &str) -> bool {
    matches.value_source(id) == Some(ValueSource::CommandLine)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(args: &[&str]) -> Invocation {
        Invocation::from_argv(args.iter().copied())
    }

    #[test]
    fn a_machine_output_mode_is_seen_in_either_spelling() {
        assert!(argv(&["pnpmlike", "list", "--output", "json"]).machine_output);
        assert!(argv(&["pnpmlike", "list", "--output=yaml"]).machine_output);
        assert!(!argv(&["pnpmlike", "list", "--output=text"]).machine_output);
        assert!(!argv(&["pnpmlike", "list"]).machine_output);
    }

    #[test]
    fn script_arguments_after_the_separator_are_not_pnpmlike_flags() {
        assert!(!argv(&["pnpmlike", "run", "build", "--", "--output", "json"]).machine_output);
    }

    #[test]
    fn a_flag_value_is_not_mistaken_for_a_flag() {
        assert!(!argv(&["pnpmlike", "list", "--manifest", "--output=json"]).machine_output);
    }

    /// Resolve a command line against a stated machine-output bit and stated
    /// stderr attendance. The attendance is a parameter precisely so the
    /// `auto` table can be pinned without a terminal.
    fn resolve(args: &[&str], machine_output: bool, stderr_attended: bool) -> Session {
        use clap::CommandFactory;

        let matches = crate::cli::Cli::command()
            .try_get_matches_from(args)
            .expect("the test command line parses");
        let (_, sub) = matches.subcommand().expect("a subcommand was named");
        Session::resolve_with(sub, Invocation { machine_output }, stderr_attended)
    }

    #[test]
    fn auto_follows_stderrs_attendance_not_stdouts() {
        assert_eq!(
            resolve(&["pnpmlike", "install"], false, true).reporter.kind(),
            ReporterKind::Default
        );
        assert_eq!(
            resolve(&["pnpmlike", "install"], false, false).reporter.kind(),
            ReporterKind::AppendOnly
        );
    }

    #[test]
    fn a_machine_output_mode_outranks_attendance() {
        assert_eq!(
            resolve(&["pnpmlike", "install"], true, true).reporter.kind(),
            ReporterKind::Ndjson
        );
        assert_eq!(
            resolve(&["pnpmlike", "install"], true, false).reporter.kind(),
            ReporterKind::Ndjson
        );
    }

    #[test]
    fn an_explicit_reporter_outranks_every_row() {
        for (value, want) in [
            ("default", ReporterKind::Default),
            ("append-only", ReporterKind::AppendOnly),
            ("ndjson", ReporterKind::Ndjson),
            ("silent", ReporterKind::Silent),
        ] {
            let session = resolve(&["pnpmlike", "install", "--reporter", value], true, true);
            assert_eq!(session.reporter.kind(), want, "--reporter {}", value);
        }
    }

    #[test]
    fn an_explicit_auto_still_resolves_through_the_table() {
        assert_eq!(
            resolve(&["pnpmlike", "install", "--reporter", "auto"], false, false)
                .reporter
                .kind(),
            ReporterKind::AppendOnly
        );
    }

    #[test]
    fn the_two_silencers_reach_two_channels() {
        let quiet = resolve(&["pnpmlike", "install", "-q"], false, true);
        assert_eq!(quiet.log.level(), LogLevel::Error);
        assert_eq!(quiet.reporter.kind(), ReporterKind::Default);

        let progress_off = resolve(&["pnpmlike", "install", "--reporter", "silent"], false, true);
        assert_eq!(progress_off.log.level(), LogLevel::Warn);
        assert_eq!(progress_off.reporter.kind(), ReporterKind::Silent);

        let both = resolve(&["pnpmlike", "install", "--silent"], false, true);
        assert_eq!(both.log.level(), LogLevel::Error);
        assert_eq!(both.reporter.kind(), ReporterKind::Silent);
    }

    #[test]
    fn an_explicit_loglevel_outranks_the_shorthands() {
        assert_eq!(
            resolve(&["pnpmlike", "install", "-q", "--loglevel", "info"], false, true)
                .log
                .level(),
            LogLevel::Info
        );
        assert_eq!(
            resolve(&["pnpmlike", "install", "-v", "--loglevel", "error"], false, true)
                .log
                .level(),
            LogLevel::Error
        );
        assert_eq!(
            resolve(&["pnpmlike", "install", "--silent", "--loglevel", "warn"], false, true)
                .log
                .level(),
            LogLevel::Warn
        );
    }

    #[test]
    fn the_manifest_is_named_as_given_or_discovered() {
        assert_eq!(
            resolve(&["pnpmlike", "list"], false, true).manifest_as_given,
            DEFAULT_MANIFEST
        );
        assert_eq!(
            resolve(&["pnpmlike", "list", "--manifest", "a/b.pkg"], false, true).manifest_as_given,
            "a/b.pkg"
        );
    }

    #[test]
    fn every_command_takes_the_manifest_flag() {
        for args in [
            vec!["pnpmlike", "list", "--manifest", "x.pkg"],
            vec!["pnpmlike", "install", "--manifest", "x.pkg"],
            vec!["pnpmlike", "run", "build", "--manifest", "x.pkg"],
        ] {
            assert_eq!(resolve(&args, false, true).manifest_as_given, "x.pkg");
        }
    }
}
