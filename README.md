# standout-corpus

Agent runs that build small things with [standout](https://github.com/arthur-debert/standout),
recorded as OpenTelemetry traces. A run is one Claude Code session on one
task prompt against one framework ref; what it read, what it built, what it
had to work around, and whether the task's checks pass all land in the
trace. The traces are the material for judging where standout, its
documentation and its discoverability work together and where they do not.

## Running a task

```bash
doppler run -- bin/run smoke --ref v12.0.0
```

`bin/run <task>` does, in order:

1. Makes `/tmp/standout-corpus/<task>-<timestamp>/` with an empty `app/`
   directory as the session's working directory.
2. Starts `claude -p` on `tasks/<task>/PROMPT.md`, with a settings file whose
   only content is a `SessionStart` hook, `hooks/checkout`. The hook clones
   the framework at `--ref` (default `main`, from `--repo`, default the
   GitHub repository) to `<run>/standout` and tells the session where the
   checkout and its documentation are. The agent depends on the framework
   by path, so any commit is testable, released or not.
3. Keeps the session's stream-json transcript as `<run>/transcript.jsonl`,
   each event stamped with the time it arrived.
4. Runs `tasks/<task>/check` in `app/`, if the task has one. It prints one
   `ok <name>` or `fail <name> <detail>` line per check; the lines land in
   `<run>/checks.txt`.
5. Judges the run with `bin/judge <run>`: a tool-less `claude -p` session
   reads the transcript, the agent's `NOTES.md` and the checks, and returns
   findings under a fixed rubric (worked, hard-to-discover, docs-wrong,
   workaround, gap, agent-error; each with evidence and the layer a fix
   belongs to) plus four scores in [0, 1]. The rubric is the top of
   `bin/judge`; iterating on the judge is editing it. `<run>/judge.json`
   holds the verdict.
6. Exports the run as one trace with `bin/export <run>`.

Flags: `--ref`, `--repo` (a URL or a local path), `--model`, `--max-turns`,
`--runs-dir`, `--judge-model` (default `claude-opus-5`), `--no-judge`.
Every step can be re-run on its own against an existing run directory.

The `SessionStart` hook works interactively too: in any directory,
`CORPUS_FRAMEWORK_REF=v12.0.0 claude --settings <run>/settings.json` gives a
human the same checkout and context an agent gets.

## Where the trace goes

`bin/export` speaks OTLP/JSON over HTTP and needs nothing but Python. It
sends to, in order of preference: `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT`;
`OTEL_EXPORTER_OTLP_ENDPOINT` plus `/v1/traces`; or a Langfuse project's
OTLP endpoint derived from `LANGFUSE_BASE_URL`, `LANGFUSE_PUBLIC_KEY` and
`LANGFUSE_SECRET_KEY` (the `LANGFUSE_STANDOUT_*` spellings also work).
Headers come from `OTEL_EXPORTER_OTLP_TRACES_HEADERS` or
`OTEL_EXPORTER_OTLP_HEADERS`. Nothing in this repository names a
destination or a key: the Langfuse project we use lives in Doppler
(`github` project, `prd` config, the `LANGFUSE_STANDOUT_*` secrets), which
is what `doppler.yaml` points `doppler run` at. The trace is always also
written to `<run>/trace.json`.

Claude Code's own OpenTelemetry export is not used: this build emits logs
and metrics but no traces, and Langfuse ingests traces only.

The trace shape, in [Langfuse's OTLP vocabulary](https://langfuse.com/integrations/native/opentelemetry)
and the `gen_ai.*` semantic conventions, so any OTLP backend reads it:

- One root span per run, named after the task, with the prompt as input and
  the agent's final message as output. Its attributes are the run's hard
  facts: `corpus.task`, `corpus.run_id`, `corpus.framework.{repo,ref,commit}`,
  `corpus.agent.{session_id,exit_code,turns,cost_usd,duration_ms,result}`,
  `corpus.framework.doc_reads` and `corpus.framework.source_reads` (how many
  times the agent read the framework's documentation versus its source),
  `gen_ai.usage.*` totals, and `corpus.checks.{total,passed}` plus the
  `corpus.checks` list. `langfuse.session.id` is the run id and the tags are
  the task and the ref.
- One generation span per assistant message: model, token usage, the tool
  results it saw as input, its text and tool calls as output.
- One tool span per tool call, under the generation that made it, with the
  call's input and result; an errored tool result marks the span.
- One evaluator span for the judge, carrying its findings as output; the
  scores are root attributes (`corpus.judge.*`) and, when the destination is
  Langfuse, scores on the trace.

## Tasks

A task is a directory under `tasks/` with `PROMPT.md` and, optionally, an
executable `check`. `smoke` is the loop's own proof: a two-flag `greet`
command whose checks build it and run it three ways. Tasks that measure
something about standout come from the touchpoints real adopters need; the
archetype specs under `legacy/corpus/archetypes/*/spec.md` are one source of
material for them.

## Layout

- `bin/run`, `bin/export`, `hooks/checkout`, `tasks/` — the runner above.
- `members/`, `ci/`, `.github/workflows/corpus.yml` — the earlier frozen-member
  regression build: accepted implementations rebuilt against a standout
  checkout on a schedule. Unchanged by the runner above; the standout PR
  lane that used to build its fast subset is gone from the standout
  repository.
- `legacy/` — the blind-run corpus program as it stood in standout at
  12.0.0, moved here verbatim and unbuilt: the archetype roster and its
  suites (`legacy/corpus`), the sandboxed runner (`legacy/corpus-runner`,
  which depended on standout workspace crates by path), the committed run
  reports and scorecards, and the page on running a set.
