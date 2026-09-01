//! The shell adapter: the clap surface, the handlers, and the configured app.

use clap::{CommandFactory, Parser, Subcommand};
use standout::cli::{App, Dispatch, ExternalFailure, Output, SetupError};
use standout::{embed_styles, embed_templates, handler};

use crate::cellar::{self, Formula, UnknownFormula};

#[derive(Parser)]
#[command(
    name = "brewlike",
    about = "Query the built-in cellar: what is installed, what one formula \
             is, what it depends on, and what is out of date."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Dispatch)]
#[dispatch(handlers = handlers)]
pub enum Commands {
    /// Every installed formula and the version installed.
    #[dispatch(pure)]
    List,

    /// What one formula is.
    #[dispatch(pure)]
    Info {
        /// The formula to describe.
        formula: String,
    },

    /// The transitive dependency closure of a formula.
    #[dispatch(pure)]
    Deps {
        /// Show the closure as a tree instead of a set.
        #[arg(long)]
        tree: bool,
        /// The formula to resolve.
        formula: String,
    },

    /// The installed formulae a newer version exists for.
    #[dispatch(pure)]
    Outdated {
        /// Restrict the query to these formulae.
        formula: Vec<String>,
    },
}

/// The clap surface, ready to hand to `App::run`.
pub fn command() -> clap::Command {
    Cli::command()
}

/// The configured application.
pub fn app() -> Result<App, SetupError> {
    App::builder()
        .version(env!("CARGO_PKG_VERSION"))
        // Every command's document is a bare array or a bare record, because
        // that is the root `--output json` is contracted to have. A template
        // can only bind names out of a map, so the same data is offered back
        // to the templates under one name. Context is template-only: the
        // structured modes still serialize the handler's own data, untouched.
        .context_fn("view", |ctx: &standout::context::RenderContext| {
            minijinja::Value::from_serialize(ctx.data)
        })
        .templates(embed_templates!("src/templates"))
        .styles(embed_styles!("src/styles"))
        .default_theme("default")
        .commands(Commands::dispatch_config())?
        .build()
}

/// A domain error carrying `brewlike`'s own diagnostic.
///
/// Standout frames an ordinary handler error as `Error: {error}`, and this
/// spec asks for `brewlike: no formula found: <name>` verbatim on stderr with
/// status 1. `ExternalFailure` is the one seam that hands both the status and
/// the diagnostic bytes to the application untouched, so the domain error
/// travels through it.
fn domain_error(error: UnknownFormula) -> anyhow::Error {
    ExternalFailure::new(1, format!("brewlike: {error}\n"))
        .expect("status 1 is nonzero")
        .into()
}

/// Look a formula up, or fail with that diagnostic.
fn require(name: &str) -> Result<&'static Formula, anyhow::Error> {
    cellar::find(name).ok_or_else(|| domain_error(UnknownFormula(name.to_string())))
}

pub mod handlers {
    use super::*;
    use crate::view::{DependencyNode, DependencyView, FormulaDetail, FormulaRecord};

    #[handler]
    pub fn list() -> Result<Output<Vec<FormulaRecord>>, anyhow::Error> {
        let records = cellar::all().iter().map(FormulaRecord::from).collect();
        Ok(Output::Render(records))
    }

    #[handler]
    pub fn info(#[arg] formula: String) -> Result<Output<FormulaDetail>, anyhow::Error> {
        Ok(Output::Render(FormulaDetail::from(require(&formula)?)))
    }

    #[handler]
    pub fn deps(
        #[flag] tree: bool,
        #[arg] formula: String,
    ) -> Result<Output<DependencyView>, anyhow::Error> {
        let formula = require(&formula)?;
        let view = if tree {
            DependencyView::Tree(DependencyNode::of(formula))
        } else {
            DependencyView::Names(
                cellar::closure(formula)
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
            )
        };
        Ok(Output::Render(view))
    }

    #[handler]
    pub fn outdated(
        #[arg] formula: Vec<String>,
    ) -> Result<Output<Vec<FormulaRecord>>, anyhow::Error> {
        let formulae = cellar::outdated(&formula).map_err(domain_error)?;
        let records = formulae.into_iter().map(FormulaRecord::from).collect();
        Ok(Output::Render(records))
    }
}

#[cfg(test)]
mod tests {
    use super::handlers;
    use crate::view::DependencyView;
    use standout::cli::Output;

    #[test]
    fn list_binds_each_formulas_versions_to_its_own_record() {
        let Ok(Output::Render(records)) = handlers::list() else {
            panic!("expected rendered data");
        };
        let granite = records.iter().find(|r| r.name == "granite").unwrap();
        assert_eq!(granite.installed, "1.4.2");
        assert_eq!(granite.latest, "1.5.0");
        assert!(granite.outdated);
    }

    #[test]
    fn deps_answers_a_set_by_default_and_a_tree_with_the_flag() {
        let Ok(Output::Render(DependencyView::Names(names))) =
            handlers::deps(false, "basalt".into())
        else {
            panic!("expected the flat closure");
        };
        assert_eq!(names, ["pebble", "quartz"]);

        let Ok(Output::Render(DependencyView::Tree(root))) = handlers::deps(true, "basalt".into())
        else {
            panic!("expected the tree");
        };
        assert_eq!(root.name, "basalt");
        assert_eq!(root.dependencies[1].dependencies[0].name, "pebble");
    }

    #[test]
    fn an_unknown_formula_is_a_domain_error() {
        assert!(handlers::info("obsidian".into()).is_err());
        assert!(handlers::deps(false, "obsidian".into()).is_err());
        assert!(handlers::outdated(vec!["obsidian".into()]).is_err());
    }
}
