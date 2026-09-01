//! The clap surface. Standout augments this command with its own `--output`
//! flag and parses it; every flag here is global so that `run --manifest …`
//! reads the same way `list --manifest …` does.

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ReporterArg {
    Auto,
    Default,
    AppendOnly,
    Ndjson,
    Silent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LogLevelArg {
    Error,
    Warn,
    Info,
}

#[derive(Parser)]
#[command(
    name = "pnpmlike",
    about = "A workspace script runner in the pnpm mold",
    subcommand_required = true,
    arg_required_else_help = false
)]
pub struct Cli {
    /// Read this manifest instead of ./pnpmlike.pkg.
    #[arg(long, global = true, value_name = "PATH")]
    pub manifest: Option<String>,

    /// How progress is reported on stderr.
    #[arg(
        long,
        global = true,
        value_name = "REPORTER",
        value_enum,
        default_value = "auto"
    )]
    pub reporter: ReporterArg,

    /// How much the log channel says on stderr.
    #[arg(
        long,
        global = true,
        value_name = "LEVEL",
        value_enum,
        default_value = "warn"
    )]
    pub loglevel: LogLevelArg,

    /// Silence warnings, keep progress.
    #[arg(short = 'q', long, global = true)]
    pub quiet: bool,

    /// Report info notes as well as warnings.
    #[arg(short = 'v', long, global = true)]
    pub verbose: bool,

    /// Silence both stderr channels.
    #[arg(long, global = true)]
    pub silent: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// List the declared packages.
    List,

    /// Install every declared package.
    Install,

    /// Run a declared script.
    Run {
        /// The declared script to run.
        script: String,

        /// Positional parameters for the script, after `--`.
        #[arg(last = true, allow_hyphen_values = true, num_args = 0..)]
        args: Vec<String>,
    },
}
