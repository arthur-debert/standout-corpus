//! The two stderr channels: the reporter (progress) and the log (warnings and
//! info notes).
//!
//! Standout owns stdout — handlers return view data and the framework renders
//! it. Neither channel here touches stdout: both write to stderr, and they are
//! silenced by two different switches.

use std::io::Write;

use serde::Serialize;

use crate::manifest::Manifest;

/// The reporter forms, after `auto` has been resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReporterKind {
    Default,
    AppendOnly,
    Ndjson,
    Silent,
}

/// The log channel level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
}

/// ANSI erase-to-end-of-line: the three bytes `1B 5B 4B`.
const ERASE_TO_EOL: &str = "\x1b[K";

#[derive(Serialize)]
struct ProgressEvent<'a> {
    event: &'a str,
    step: usize,
    total: usize,
    name: &'a str,
    version: &'a str,
}

#[derive(Serialize)]
struct DoneEvent<'a> {
    event: &'a str,
    installed: usize,
}

#[derive(Serialize)]
struct ScriptEvent<'a> {
    event: &'a str,
    name: &'a str,
}

/// Progress on stderr. Steps are pushed as the work happens; `end_steps` closes
/// the dynamic form's single line.
pub struct Reporter {
    kind: ReporterKind,
    wrote_step: bool,
}

impl Reporter {
    pub fn new(kind: ReporterKind) -> Self {
        Self { kind, wrote_step: false }
    }

    #[cfg(test)]
    pub fn kind(&self) -> ReporterKind {
        self.kind
    }


    /// One `install` step: `<i>/<n> <name> <version>`.
    pub fn install_step(&mut self, step: usize, total: usize, name: &str, version: &str) {
        match self.kind {
            ReporterKind::Silent => {}
            ReporterKind::Ndjson => self.write_line(&json_line(&ProgressEvent {
                event: "progress",
                step,
                total,
                name,
                version,
            })),
            _ => {
                let text = format!("{}/{} {} {}", step, total, name, version);
                self.write_text_step(&text);
            }
        }
        self.wrote_step = true;
    }

    /// The single `run` step: `1/1 <script>`.
    pub fn script_step(&mut self, name: &str) {
        match self.kind {
            ReporterKind::Silent => {}
            ReporterKind::Ndjson => {
                self.write_line(&json_line(&ScriptEvent { event: "script", name }))
            }
            _ => {
                let text = format!("1/1 {}", name);
                self.write_text_step(&text);
            }
        }
        self.wrote_step = true;
    }

    /// Close the run of steps. The dynamic reporter keeps every step on one
    /// line, so the single line feed belongs here — and only if a step was
    /// actually reported.
    pub fn end_steps(&mut self) {
        if self.kind == ReporterKind::Default && self.wrote_step {
            write_stderr("\n");
        }
        flush_stderr();
    }

    /// The terminal entry the machine form emits for a successful `install`.
    pub fn install_done(&mut self, installed: usize) {
        if self.kind == ReporterKind::Ndjson {
            self.write_line(&json_line(&DoneEvent { event: "done", installed }));
            flush_stderr();
        }
    }

    fn write_text_step(&self, text: &str) {
        match self.kind {
            ReporterKind::Default => {
                write_stderr(&format!("\r{}{}", text, ERASE_TO_EOL));
            }
            _ => {
                write_stderr(&format!("{}\n", text));
            }
        }
    }

    fn write_line(&self, line: &str) {
        write_stderr(&format!("{}\n", line));
    }
}

fn json_line<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("reporter events serialize")
}

fn write_stderr(text: &str) {
    let stderr = std::io::stderr();
    let mut handle = stderr.lock();
    let _ = handle.write_all(text.as_bytes());
}

pub fn flush_stderr() {
    let _ = std::io::stderr().flush();
}

/// Warnings and info notes on stderr, governed by a level.
#[derive(Debug, Clone, Copy)]
pub struct Log {
    level: LogLevel,
}

impl Log {
    pub fn new(level: LogLevel) -> Self {
        Self { level }
    }

    #[cfg(test)]
    pub fn level(&self) -> LogLevel {
        self.level
    }

    /// The one note that precedes everything else, at `info`.
    pub fn using_manifest(&self, path_as_given: &str) {
        if self.level >= LogLevel::Info {
            write_stderr(&format!("info: using manifest {}\n", path_as_given));
        }
    }

    /// One warning per unpinned package, at `warn` and above.
    pub fn unpinned(&self, manifest: &Manifest) {
        if self.level < LogLevel::Warn {
            return;
        }
        for dep in &manifest.deps {
            if dep.is_unpinned() {
                write_stderr(&format!("warning: {} is unpinned (*)\n", dep.name));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_order_from_quiet_to_loud() {
        assert!(LogLevel::Error < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Info);
    }

    #[test]
    fn machine_events_are_spelled_exactly() {
        assert_eq!(
            json_line(&ProgressEvent {
                event: "progress",
                step: 1,
                total: 2,
                name: "alpha",
                version: "1.0.0"
            }),
            r#"{"event":"progress","step":1,"total":2,"name":"alpha","version":"1.0.0"}"#
        );
        assert_eq!(
            json_line(&DoneEvent { event: "done", installed: 2 }),
            r#"{"event":"done","installed":2}"#
        );
        assert_eq!(
            json_line(&ScriptEvent { event: "script", name: "build" }),
            r#"{"event":"script","name":"build"}"#
        );
    }
}
