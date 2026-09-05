Build a small Rust command-line tool named `greet` in the current directory,
using the standout framework. Your session context says where standout is
checked out; depend on it with path dependencies. Do not add crates.io
dependencies on standout.

Behaviour:

- `greet hello <name>` prints `Hello, <name>!` on stdout and exits 0.
- `greet hello <name> --shout` prints the same greeting in upper case.
- `greet hello <name> --output json` prints a JSON object with a `greeting`
  field holding the greeting text, because the command's output goes through
  standout's rendering rather than a direct print.

Learn standout from its documentation. If you have to read its source code to
get unblocked, that is allowed, but record each time in NOTES.md.

When `cargo build` succeeds and the three behaviours work, write `NOTES.md` in
the current directory with four sections: what you built, which documentation
files you read, where the documentation was wrong or missing (quote it), and
workarounds you left in the code. Then stop.
