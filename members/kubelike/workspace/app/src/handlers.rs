//! CLI adapters. Each reads its arguments, calls the core, and maps the
//! result onto standout's output vocabulary.

use serde::Serialize;
use standout::cli::{Artifact, ExternalFailure, Output};
use standout::handler;

use crate::config;
use crate::core::{self, Emission, Failure, Outcome};

/// The prose a successful-but-empty listing puts on stderr. It reaches stderr
/// as an artifact report, which is the channel standout gives a command that
/// writes bytes to stdout and still has something to say about them.
#[derive(Serialize)]
pub struct Note {
    pub note: String,
}

/// Turn a core outcome into the framework's shape: exact bytes for stdout, a
/// stderr note on an empty listing, or a failure carrying its own status.
fn emit(outcome: Outcome) -> Result<Output<Note>, anyhow::Error> {
    match outcome {
        Ok(Emission::Text(text)) => Ok(Output::Artifact(
            Artifact::new(text.into_bytes()).allow_stdout(),
        )),
        Ok(Emission::Empty(note)) => Ok(Output::Artifact(
            Artifact::new(Vec::new())
                .allow_stdout()
                .with_report(Note { note }),
        )),
        Err(failure) => Err(to_failure(failure)),
    }
}

fn to_failure(failure: Failure) -> anyhow::Error {
    let status = failure.status();
    let diagnostic = format!("{}\n", failure.message());
    match ExternalFailure::new(status, diagnostic) {
        Ok(external) => external.into(),
        Err(error) => error.into(),
    }
}

#[handler]
pub fn get(
    #[arg] kinds: String,
    #[arg] name: Option<String>,
    #[arg] namespace: Option<String>,
    #[flag(name = "all-namespaces")] all_namespaces: bool,
    #[arg] selector: Option<String>,
    #[arg(name = "o")] o: Option<String>,
) -> Result<Output<Note>, anyhow::Error> {
    emit(core::get(
        &kinds,
        name.as_deref(),
        &config::resolve_namespace(namespace.as_deref()),
        namespace.is_some(),
        all_namespaces,
        selector.as_deref(),
        o.as_deref(),
    ))
}

#[handler]
pub fn describe(
    #[arg] kind: String,
    #[arg] name: String,
    #[arg] namespace: Option<String>,
) -> Result<Output<Note>, anyhow::Error> {
    emit(core::describe(
        &kind,
        &name,
        &config::resolve_namespace(namespace.as_deref()),
        namespace.is_some(),
    ))
}

#[handler]
pub fn api_resources() -> Result<Output<Note>, anyhow::Error> {
    emit(core::api_resources())
}

#[handler]
pub fn current_context() -> Result<Output<Note>, anyhow::Error> {
    emit(core::current_context())
}
