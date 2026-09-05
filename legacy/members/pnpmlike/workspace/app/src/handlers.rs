//! CLI adapters. Each one resolves the invocation's channels, reads the
//! manifest, speaks on stderr, and hands view data back to Standout for stdout.

use std::io::Write;
use std::process::Command;

use clap::ArgMatches;
use standout::cli::{CommandContext, ExternalFailure, Output};
use standout::handler;

use crate::manifest::{self, Manifest, ManifestError};
use crate::session::{Invocation, Session};
use crate::views::{InstallView, ListView, PackageView};

/// A manifest that exists but does not parse: a located domain error, exit 1.
const MANIFEST_ERROR_STATUS: u8 = 1;
/// A script name the manifest does not declare.
const UNKNOWN_SCRIPT_STATUS: u8 = 3;

#[handler]
pub fn list(
    #[matches] matches: &ArgMatches,
    #[ctx] ctx: &CommandContext,
) -> Result<Output<ListView>, anyhow::Error> {
    let mut session = open(matches, ctx)?;
    let manifest = read_manifest(&mut session)?;

    // `list` is instantaneous by construction, so it reports no steps.
    Ok(Output::Render(ListView {
        packages: manifest.deps.iter().map(PackageView::from).collect(),
    }))
}

#[handler]
pub fn install(
    #[matches] matches: &ArgMatches,
    #[ctx] ctx: &CommandContext,
) -> Result<Output<InstallView>, anyhow::Error> {
    let mut session = open(matches, ctx)?;
    let manifest = read_manifest(&mut session)?;

    let total = manifest.deps.len();
    for (index, dep) in manifest.deps.iter().enumerate() {
        session
            .reporter
            .install_step(index + 1, total, &dep.name, &dep.version);
        install_package(dep);
    }
    session.reporter.end_steps();
    session.reporter.install_done(total);

    Ok(Output::Render(InstallView {
        installed: manifest.deps.iter().map(PackageView::from).collect(),
        count: total,
    }))
}

#[handler]
pub fn run(
    #[matches] matches: &ArgMatches,
    #[ctx] ctx: &CommandContext,
) -> Result<Output<()>, anyhow::Error> {
    let mut session = open(matches, ctx)?;
    let manifest = read_manifest(&mut session)?;

    let name: &String = matches
        .get_one("script")
        .expect("clap requires the script name");

    let Some(script) = manifest.script(name) else {
        return Err(ExternalFailure::new(
            UNKNOWN_SCRIPT_STATUS,
            format!("error: no script named \"{}\"\n", name),
        )?
        .into());
    };
    let command = script.command.clone();

    let args: Vec<String> = matches
        .get_many::<String>("args")
        .map(|values| values.cloned().collect())
        .unwrap_or_default();

    // The one step, written and flushed before the child starts, so that it
    // can never interleave with the child's own output.
    session.reporter.script_step(name);
    session.reporter.end_steps();
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();

    // The child inherits both streams: its bytes reach our stdout and stderr
    // unchanged, whatever `--output` says.
    let status = Command::new("/bin/sh")
        .arg("-c")
        .arg(&command)
        .arg("pnpmlike")
        .args(&args)
        .status()?;

    match exit_status_code(&status) {
        0 => Ok(Output::Silent),
        // The child's status becomes ours, verbatim. What we say about the
        // failure is unspecified, so we say nothing.
        code => Err(ExternalFailure::new(code, String::new())?.into()),
    }
}

/// Resolve this invocation's channels and emit the info note that precedes
/// everything else.
fn open(matches: &ArgMatches, ctx: &CommandContext) -> Result<Session, anyhow::Error> {
    let invocation = *ctx.app_state.get_required::<Invocation>()?;
    let session = Session::resolve(matches, invocation);
    session.log.using_manifest(&session.manifest_as_given);
    Ok(session)
}

/// Read and validate the manifest before any work starts, so that every log
/// line lands before the first reporter step.
fn read_manifest(session: &mut Session) -> Result<Manifest, anyhow::Error> {
    let manifest = manifest::load(&session.manifest_path).map_err(|error| {
        located_manifest_error(&error, &session.manifest_as_given)
    })?;
    session.log.unpinned(&manifest);
    crate::channels::flush_stderr();
    Ok(manifest)
}

fn located_manifest_error(error: &ManifestError, file_as_given: &str) -> anyhow::Error {
    match ExternalFailure::new(MANIFEST_ERROR_STATUS, error.render(file_as_given)) {
        Ok(failure) => failure.into(),
        Err(other) => other.into(),
    }
}

/// Installing is simulated work: nothing is fetched.
fn install_package(_dep: &crate::manifest::Dep) {}

#[cfg(unix)]
fn exit_status_code(status: &std::process::ExitStatus) -> u8 {
    use std::os::unix::process::ExitStatusExt;

    if let Some(code) = status.code() {
        return code as u8;
    }
    // Killed by a signal: the shell convention is 128 + signal number.
    match status.signal() {
        Some(signal) => 128u8.saturating_add(signal as u8),
        None => 1,
    }
}

#[cfg(not(unix))]
fn exit_status_code(status: &std::process::ExitStatus) -> u8 {
    status.code().unwrap_or(1) as u8
}
