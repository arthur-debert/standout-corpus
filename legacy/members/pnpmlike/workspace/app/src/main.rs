//! `pnpmlike` — a workspace script runner that speaks on three channels:
//! data on stdout, progress on stderr through a reporter, and logs on stderr
//! through a level.

mod channels;
mod cli;
mod handlers;
mod hooks;
mod manifest;
mod session;
mod views;

use clap::CommandFactory;
use standout::cli::{App, Dispatch};
use standout::EmbeddedTemplates;
use standout_dispatch::Hooks;

use crate::cli::Cli;
use crate::session::Invocation;

/// The commands, bound to the handlers of the same name.
///
/// `run` hands the terminal to a child process and has nothing for a template
/// to render, so it is marked `silent`.
#[derive(Dispatch)]
#[dispatch(handlers = handlers)]
pub enum Commands {
    #[dispatch(pure)]
    List,
    #[dispatch(pure)]
    Install,
    #[dispatch(pure, silent)]
    Run,
}

/// Templates are inline rather than embedded from files: the bytes on stdout
/// are part of the contract, and a template file's trailing newline is not
/// something to leave to an editor. MiniJinja consumes one final newline and
/// `App::run` appends one, so neither template ends with one.
const TEMPLATES: &[(&str, &str)] = &[
    (
        "list",
        "{% for package in packages %}{{ package.name }} {{ package.version }}\
         {% if not loop.last %}\n{% endif %}{% endfor %}",
    ),
    ("install", "{{ count }} packages installed."),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let argv: Vec<String> = std::env::args().collect();

    let app = App::builder()
        .app_state(Invocation::from_argv(&argv))
        .templates(EmbeddedTemplates::new(TEMPLATES, ""))
        .commands(Commands::dispatch_config())?
        .hooks(
            "list",
            Hooks::new().post_output(hooks::shape_document(&["packages"])),
        )
        .hooks(
            "install",
            Hooks::new().post_output(hooks::shape_document(&["installed", "count"])),
        )
        .build()?;

    app.run(Cli::command(), argv);
    Ok(())
}
