mod units;
mod view;

use std::io::Write;
use std::process::{Command as Process, Stdio};

use clap::{ArgAction, CommandFactory, Parser, Subcommand};
use standout::cli::{App, Dispatch, DispatchResult, Output, RunErrorKind, SuccessKind};
use standout::{embed_styles, embed_templates, handler, InputSources, TargetProperties};

use units::ActiveState;
use view::UnitListView;

#[derive(Parser)]
#[command(name = "systemdlike", about = "Inspect the service manager's units")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Dispatch)]
#[dispatch(handlers = handlers)]
enum Commands {
    /// List units. This is also what a bare `systemdlike` runs.
    #[dispatch(pure, default)]
    ListUnits {
        /// Only list units in this active state
        #[arg(long, value_parser = ["active", "inactive"])]
        state: Option<String>,

        /// Drop the header row and the trailing legend
        #[arg(id = "no-legend", long = "no-legend", action = ArgAction::SetTrue)]
        no_legend: bool,

        /// Never style the output, whatever the terminal or environment says
        #[arg(long, action = ArgAction::SetTrue)]
        plain: bool,

        /// Do not pipe the output through a pager
        #[arg(id = "no-pager", long = "no-pager", action = ArgAction::SetTrue)]
        no_pager: bool,
    },
}

mod handlers {
    use super::*;

    #[handler]
    pub fn list_units(
        #[arg] state: Option<String>,
        #[flag] no_legend: bool,
    ) -> Result<Output<UnitListView>, anyhow::Error> {
        // Clap's value_parser has already refused anything else.
        let state = state.as_deref().and_then(ActiveState::parse);
        let units = units::list_units(state);
        Ok(Output::Render(UnitListView::new(&units, !no_legend)))
    }
}

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let argv: Vec<String> = std::env::args().collect();
    // `--plain` and `--no-pager` steer the process edge — colour capability and
    // paging — rather than the handler, so they are read off the raw line here
    // as well as declared on the command for clap to validate.
    let plain = argv.iter().any(|arg| arg == "--plain");
    let no_pager = argv.iter().any(|arg| arg == "--no-pager");

    let app = match App::builder()
        .version(env!("CARGO_PKG_VERSION"))
        .templates(embed_templates!("src/templates"))
        .styles(embed_styles!("src/styles"))
        .default_theme("default")
        .commands(Commands::dispatch_config())
        .and_then(|builder| builder.build())
    {
        Ok(app) => app,
        Err(error) => {
            eprintln!("systemdlike: {}", error);
            return 1;
        }
    };

    // A handler reading an argument the command does not declare is a bug in
    // this file, not in the user's line; say so rather than panicking later.
    if let Err(error) = app.verify_command(&Cli::command()) {
        eprintln!("systemdlike: {}", error);
        return 1;
    }

    let colors = env_color_request();
    let mut target = TargetProperties::detect();
    if let Some(capable) = color_capability(plain, colors.as_deref(), no_color_is_set()) {
        target.stdout_color_capability = capable;
    }
    let stdout_is_terminal = target.stdout_is_terminal;

    let result = app.run_with(
        Cli::command(),
        argv.iter().cloned(),
        target,
        InputSources::from_process(),
    );
    let success = result.success_kind();

    match result.into_outcome() {
        DispatchResult::Handled(output) => {
            let is_command_output = success == Some(SuccessKind::Command);
            let pager = pager_for(
                stdout_is_terminal,
                no_pager,
                is_command_output,
                configured_pager(),
            );
            emit(output.as_str(), is_command_output, pager.as_deref())
        }
        DispatchResult::Silent => 0,
        DispatchResult::Error(error) => {
            eprintln!("{}", error);
            match error.kind() {
                RunErrorKind::ClapUsage => 2,
                _ => 1,
            }
        }
        // No handler claimed the command: for this app that can only be an
        // unknown command line.
        DispatchResult::NoMatch(_) => 2,
        _ => 1,
    }
}

/// Whether the destination should be reported as colour-capable, which is the
/// one fact `--output auto` reads. Rule 1 (`--plain`) beats rule 2
/// (`SYSTEMDLIKE_COLORS`) beats rule 3 (`NO_COLOR`); `None` leaves rule 4,
/// autodetection, in place. An explicit `--output term`/`text` overrides all of
/// this inside the framework, which is the other half of rule 1.
fn color_capability(plain: bool, colors: Option<&str>, no_color: bool) -> Option<bool> {
    if plain {
        return Some(false);
    }
    match colors {
        Some("1") => Some(true),
        Some("0") => Some(false),
        _ => no_color.then_some(false),
    }
}

fn env_color_request() -> Option<String> {
    std::env::var("SYSTEMDLIKE_COLORS").ok()
}

fn no_color_is_set() -> bool {
    std::env::var_os("NO_COLOR").is_some()
}

/// The pager to run, if any: only an attended stdout pages, only a command's
/// own listing pages (help and version displays go straight out), and
/// `--no-pager` opts the invocation out.
fn pager_for(
    stdout_is_terminal: bool,
    no_pager: bool,
    is_command_output: bool,
    configured: Option<String>,
) -> Option<String> {
    if !stdout_is_terminal || no_pager || !is_command_output {
        return None;
    }
    configured
}

/// `SYSTEMDLIKE_PAGER`, else `PAGER`. A variable that is set but empty names no
/// pager and does not fall through to the next one.
fn configured_pager() -> Option<String> {
    for name in ["SYSTEMDLIKE_PAGER", "PAGER"] {
        if let Some(value) = std::env::var_os(name) {
            let value = value.to_string_lossy().into_owned();
            return (!value.trim().is_empty()).then_some(value);
        }
    }
    None
}

/// The final write standout's own `run()` would have done, plus the pager.
///
/// A command's rendered page never carries its own trailing newline (the
/// template engine eats the last one), so it is written with `writeln!` the way
/// the framework does. Clap's own help and version displays already end in a
/// newline, so they are written as-is.
fn emit(text: &str, is_command_output: bool, pager: Option<&str>) -> i32 {
    if !is_command_output && text.ends_with('\n') {
        print!("{}", text);
        return 0;
    }
    if let Some(pager) = pager {
        if page(text, pager).is_ok() {
            return 0;
        }
    }
    let mut stdout = std::io::stdout();
    match writeln!(stdout, "{}", text).and_then(|()| stdout.flush()) {
        Ok(()) => 0,
        // A consumer that stopped reading early is not a failure.
        Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => 0,
        Err(error) => {
            eprintln!("systemdlike: error writing output: {}", error);
            1
        }
    }
}

fn page(text: &str, pager: &str) -> std::io::Result<()> {
    let mut child = Process::new("sh")
        .arg("-c")
        .arg(pager)
        .stdin(Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = writeln!(stdin, "{}", text);
    }
    child.wait()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_beats_every_colour_variable() {
        assert_eq!(color_capability(true, Some("1"), false), Some(false));
        assert_eq!(color_capability(true, None, false), Some(false));
    }

    #[test]
    fn colors_variable_beats_no_color() {
        assert_eq!(color_capability(false, Some("1"), true), Some(true));
        assert_eq!(color_capability(false, Some("0"), false), Some(false));
    }

    #[test]
    fn no_color_forces_plain_and_anything_else_autodetects() {
        assert_eq!(color_capability(false, None, true), Some(false));
        assert_eq!(color_capability(false, None, false), None);
        assert_eq!(color_capability(false, Some("yes"), false), None);
    }

    #[test]
    fn only_an_attended_listing_pages() {
        let less = || Some("less".to_string());
        assert_eq!(pager_for(true, false, true, less()), less());
        assert_eq!(pager_for(false, false, true, less()), None); // piped
        assert_eq!(pager_for(true, true, true, less()), None); // --no-pager
        assert_eq!(pager_for(true, false, false, less()), None); // help/version
        assert_eq!(pager_for(true, false, true, None), None); // no pager set
    }

    #[test]
    fn paging_writes_the_page_through_the_pager_command() {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("systemdlike-pager-test.txt");
        let _ = std::fs::remove_file(&out);
        page("two\nlines", &format!("cat > {}", out.display())).unwrap();
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "two\nlines\n");
        let _ = std::fs::remove_file(&out);
    }
}
