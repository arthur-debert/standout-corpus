//! `kubelike` — a cluster resource client in the kubectl mold.
//!
//! Two verbs over a registry of kinds: the kind is an argument, not a
//! subcommand, and its registry metadata decides which flags are legal.

mod cluster;
mod config;
mod core;
mod handlers;
mod render;
#[cfg(test)]
mod tests;

use clap::{CommandFactory, Parser, Subcommand};
use standout::cli::{App, Dispatch};
use standout::EmbeddedTemplates;

#[derive(Parser)]
#[command(name = "kubelike", about = "A cluster resource client")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Dispatch)]
#[dispatch(handlers = handlers)]
enum Commands {
    /// Display one or many resources
    #[dispatch(pure)]
    Get {
        /// Resource kind, or a comma-separated list of kinds
        kinds: String,
        /// Restrict the listing to one resource by name
        name: Option<String>,
        #[arg(short = 'n', long = "namespace")]
        namespace: Option<String>,
        #[arg(id = "all-namespaces", short = 'A', long = "all-namespaces")]
        all_namespaces: bool,
        #[arg(short = 'l', long = "selector")]
        selector: Option<String>,
        /// Output format: wide, name, json, or custom-columns=<HEADER>:<field>[,...]
        #[arg(short = 'o')]
        o: Option<String>,
    },

    /// Show details of a specific resource
    #[dispatch(pure)]
    Describe {
        /// Resource kind
        kind: String,
        /// Resource name
        name: String,
        #[arg(short = 'n', long = "namespace")]
        namespace: Option<String>,
    },

    /// Print the supported resource kinds
    #[dispatch(pure)]
    ApiResources,

    /// Modify or inspect kubeconfig values
    #[command(subcommand)]
    #[dispatch(nested)]
    Config(ConfigCommands),
}

#[derive(Subcommand, Dispatch)]
#[dispatch(handlers = handlers)]
enum ConfigCommands {
    /// Print the current context name
    #[dispatch(pure)]
    CurrentContext,
}

/// Every command writes its own bytes, so these entries are only reached when
/// a command has a report to render — today, the empty-listing note.
const TEMPLATES: &[(&str, &str)] = &[
    ("get", "{{ report.note }}"),
    ("describe", "{{ report.note }}"),
    ("api-resources", "{{ report.note }}"),
    ("config/current-context", "{{ report.note }}"),
];

fn main() -> anyhow::Result<()> {
    let app = App::builder()
        .templates(EmbeddedTemplates::new(TEMPLATES, ""))
        .commands(Commands::dispatch_config())?
        .build()?;

    app.run(Cli::command(), std::env::args());
    Ok(())
}
