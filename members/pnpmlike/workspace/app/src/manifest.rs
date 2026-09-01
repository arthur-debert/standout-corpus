//! The package manifest: reading and validating `pnpmlike.pkg`.
//!
//! CLI-free. Everything here takes bytes or a path in and hands facts back;
//! nothing prints, and nothing knows about reporters, output modes or exit
//! codes.

use std::path::Path;

/// One `dep <name> <version>` directive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dep {
    pub name: String,
    pub version: String,
}

impl Dep {
    /// A version of exactly `*` pins nothing.
    pub fn is_unpinned(&self) -> bool {
        self.version == "*"
    }
}

/// One `script <name> <command...>` directive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Script {
    pub name: String,
    pub command: String,
}

/// A parsed manifest. A workspace with no manifest file is an empty one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifest {
    pub deps: Vec<Dep>,
    pub scripts: Vec<Script>,
}

impl Manifest {
    pub fn script(&self, name: &str) -> Option<&Script> {
        self.scripts.iter().find(|script| script.name == name)
    }
}

/// A manifest that exists but does not parse, located by 1-based line number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestError {
    pub line: Option<usize>,
    pub message: String,
}

impl ManifestError {
    /// The diagnostic as the spec spells it: the file as given, then
    /// `line <n>` when a line is to blame.
    pub fn render(&self, file_as_given: &str) -> String {
        match self.line {
            Some(line) => format!("error: {} line {}: {}\n", file_as_given, line, self.message),
            None => format!("error: {}: {}\n", file_as_given, self.message),
        }
    }
}

/// Read a manifest from disk. A missing file is an empty workspace, not an
/// error; any other read failure is a domain error.
pub fn load(path: &Path) -> Result<Manifest, ManifestError> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse(&text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Manifest::default()),
        Err(error) => Err(ManifestError {
            line: None,
            message: error.to_string(),
        }),
    }
}

/// Parse manifest text. Blank lines and comments are skipped; every other line
/// is one directive, and a line matching neither is an error naming its number.
pub fn parse(text: &str) -> Result<Manifest, ManifestError> {
    let mut manifest = Manifest::default();

    for (index, raw) in text.lines().enumerate() {
        let number = index + 1;
        let line = raw.trim_start();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(rest) = directive_body(line, "dep") {
            let tokens: Vec<&str> = rest.split_whitespace().collect();
            if tokens.len() != 2 {
                return Err(unrecognized(number));
            }
            manifest.deps.push(Dep {
                name: tokens[0].to_string(),
                version: tokens[1].to_string(),
            });
        } else if let Some(rest) = directive_body(line, "script") {
            let rest = rest.trim_start();
            let Some((name, command)) = rest.split_once(char::is_whitespace) else {
                return Err(unrecognized(number));
            };
            if name.is_empty() || command.trim().is_empty() {
                return Err(unrecognized(number));
            }
            manifest.scripts.push(Script {
                name: name.to_string(),
                // The rest of the line, verbatim.
                command: command.to_string(),
            });
        } else {
            return Err(unrecognized(number));
        }
    }

    Ok(manifest)
}

/// The text after a directive keyword, or `None` when the line does not open
/// with that keyword followed by whitespace (`deps` is not `dep`).
fn directive_body<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(keyword)?;
    if rest.starts_with(|c: char| c.is_whitespace()) {
        Some(rest)
    } else {
        None
    }
}

fn unrecognized(line: usize) -> ManifestError {
    ManifestError {
        line: Some(line),
        message: "unrecognized directive".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_and_comment_lines_are_ignored() {
        let manifest = parse("\n  \n# a comment\n   # indented comment\n").unwrap();
        assert_eq!(manifest, Manifest::default());
    }

    #[test]
    fn deps_keep_manifest_order_and_verbatim_versions() {
        let manifest = parse("dep alpha 1.0.0\ndep beta 2.3.1\n").unwrap();
        assert_eq!(
            manifest.deps,
            vec![
                Dep { name: "alpha".into(), version: "1.0.0".into() },
                Dep { name: "beta".into(), version: "2.3.1".into() },
            ]
        );
    }

    #[test]
    fn a_star_version_is_unpinned() {
        let manifest = parse("dep alpha *\ndep beta 1.0\n").unwrap();
        assert!(manifest.deps[0].is_unpinned());
        assert!(!manifest.deps[1].is_unpinned());
    }

    #[test]
    fn a_script_command_is_the_rest_of_the_line() {
        let manifest = parse("script build echo hi && echo there\n").unwrap();
        assert_eq!(manifest.scripts[0].name, "build");
        assert_eq!(manifest.scripts[0].command, "echo hi && echo there");
    }

    #[test]
    fn an_unrecognized_line_names_its_number() {
        let error = parse("dep alpha 1.0.0\nnonsense\n").unwrap_err();
        assert_eq!(error.line, Some(2));
        assert_eq!(
            error.render("pnpmlike.pkg"),
            "error: pnpmlike.pkg line 2: unrecognized directive\n"
        );
    }

    #[test]
    fn a_keyword_prefix_is_not_a_directive() {
        assert_eq!(parse("deps alpha 1.0.0\n").unwrap_err().line, Some(1));
        assert_eq!(parse("dep\n").unwrap_err().line, Some(1));
        assert_eq!(parse("dep alpha\n").unwrap_err().line, Some(1));
        assert_eq!(parse("script build\n").unwrap_err().line, Some(1));
    }
}
