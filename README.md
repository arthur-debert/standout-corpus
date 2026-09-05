# standout-corpus

Where we learn, from the adopter's seat, how well
[standout](https://github.com/arthur-debert/standout) works as a whole: whether
what an application author needs, what the framework offers, what the
documentation says and what an agent can actually find line up, and where they
do not. Agents build small things with standout under recorded sessions; a
judge classifies what happened; the traces and verdicts are the material for
improving the framework's information layer first and its design second.

## Why this exists

standout grew feature by feature, mostly through agent sessions, without an
overall view of the applications it serves. Features landed that conflict with
each other, common configuration points went unconsidered, and whole concerns
(error handling, the process edge) were left for every application to
reimplement. The first attempt at seeing this from the outside built a heavy
apparatus (sandboxed blind runs, a credential broker, frozen accepted
implementations, committed run evidence, hand-authored scorecards; all under
`legacy/`) and spent most of its effort on the apparatus. Its real yield was a
dozen framework findings and two lists of documentation errata, all from its
first four runs.

This repository keeps the idea and drops the apparatus. The idea: put an agent
in an adopter's position, with only the framework and its documentation, give
it a representative task, and read closely what it did.

## The loop we want

1. **Touchpoints.** A written inventory of what applications need from
   standout: the needs an adopter meets in order, from the first `Cargo.toml`
   line to the process edge, with how standout means to serve each one. The
   sources are the real downstreams (dodot, padz, rustloc, lookma,
   edward-legacy, and the in-repo `tdoo`), the CLI-shape survey behind
   `legacy/corpus/archetypes/*/spec.md`, and standout's own feature list. This
   is design work: finding needs, combining common themes, choosing the
   beachheads and the ergonomics. Nothing of it exists yet, here or in
   standout.
2. **Tasks.** Small development tasks derived from the touchpoints: build this
   little tool, add this capability to that one, make this existing app do X.
   Each is a prompt plus black-box checks. A task is representative when a
   real adopter would recognise it, and small when a run takes minutes.
3. **Runs.** An agent, standout at a chosen ref with its documentation, the
   prompt. What the agent sees is a variable of the experiment: the docs
   alone, the docs plus the `standout` skill, a different prompt framing.
4. **Tapes and the judge.** Every run is one trace: what the agent read,
   tried, built, worked around, and whether the checks pass. A judge reads
   the tape and classifies each need the task raised as worked,
   hard-to-discover, docs-wrong, workaround, gap or agent-error, with evidence
   and the layer a fix belongs to (docs, prompt, CLI help, API).
5. **Insights.** Across runs, the classifications aggregate into the thing we
   are actually after: which documentation pages send agents on detours, which
   workarounds recur, which needs the framework cannot express, how the same
   task fares at two refs. That is a ranked list of information-layer fixes
   and a list of design needs, per framework version.
6. **Feeding back.** Information-layer findings become documentation, prompt,
   skill and help changes here and in standout, then the task re-runs.
   Design-layer findings (workarounds, gaps) are needs to design against in
   standout; once shipped, the task re-runs against the new ref.

The runs and evals improve documentation, comprehension and discoverability
directly. They do not fix bad designs; they surface the needs and workarounds
that a design pass then takes as input.

## What exists today

The runner is complete and validated once, end to end, against v12.0.0.

- `bin/run <task> --ref <ref>` starts a `claude -p` session on
  `tasks/<task>/PROMPT.md` in an empty `/tmp/standout-corpus/<run>/app`. A
  `SessionStart` hook (`hooks/checkout`) clones standout at the ref beside it
  and tells the session where the checkout and its documentation are; the
  agent depends on the framework by path, so any commit is testable. The
  transcript is kept, `tasks/<task>/check` runs, the judge runs, the trace is
  exported. Every step reruns on its own against an existing run directory.
- `bin/judge <run>` is a tool-less `claude -p` session with a JSON schema; the
  rubric at the top of the file is the whole judge.
- `bin/export <run>` builds one OTLP/JSON trace (a root span with the run's
  hard facts, a generation span per assistant message, a tool span per call,
  an evaluator span for the judge, judge scores on the trace) and posts it to
  `OTEL_EXPORTER_OTLP_*`, or to a Langfuse project derived from
  `LANGFUSE_*` keys. Ours live in Doppler (`github/prd`, the
  `LANGFUSE_STANDOUT_*` secrets); `doppler.yaml` points `doppler run` there.
  Nothing in this repository names a destination or a key.
- `tasks/smoke` is the loop's own proof, not a measurement: a two-flag
  `greet` command with five checks.

```bash
doppler run -- bin/run smoke --ref v12.0.0
```

Flags: `--ref`, `--repo` (a URL or a local path), `--model`, `--max-turns`,
`--runs-dir`, `--judge-model` (default `claude-opus-5`), `--no-judge`. The
hook works interactively too: `CORPUS_FRAMEWORK_REF=v12.0.0 claude --settings
<run>/settings.json` in any directory gives a human the same checkout and
context an agent gets.

Claude Code's own OpenTelemetry export is not used: it emits logs and metrics
but no traces, and Langfuse ingests traces only.

## What is not there yet

In the order it is needed:

- **The touchpoint inventory** (step 1). Everything downstream is only as
  representative as this. Start from the real downstreams, which are the
  valid candidates: porting them onto the current release (standout #480) is
  the cheapest way to discover what they need and work around.
- **A task set** derived from it (step 2). `smoke` is a proof. The legacy
  archetype specs are material, but they are shaped as synthetic CLIs, not as
  touchpoints, and their acceptance suites pin bytes rather than needs.
- **The information layer as a variable** (step 3). Today the agent sees the
  checkout's `docs/`. The `standout` skill, the CLI's own `--help` output and
  alternative prompt framings are not yet things a run can be given or
  withheld.
- **Repeats and comparison** (steps 4 and 5). One run per task says little;
  the same task should run several times and at two refs, and something has
  to read the traces side by side. The trace carries `corpus.task`,
  `corpus.framework.ref` and `corpus.framework.commit` so a query can select
  them; no aggregation over runs exists.
- **Judge calibration** (step 4). The rubric is a first draft. It has not
  been checked against a human reading of the same tape, findings are not
  deduplicated across runs, and the scores are not yet known to mean
  anything.
- **The insight report** (step 5). The deliverable is a short, readable
  ranking per framework version: doc pages to fix, workarounds that recur,
  needs the API cannot express. Nothing produces it yet.
- **Closing the loop** (step 6). Documentation and prompt changes should be
  proposed from findings and re-tested; design needs should be written up
  for standout in the adopter's words.

Not planned, on purpose: sandboxing or credential brokering (the runs are
local and attended), freezing produced applications, committing run evidence
to git, and any scheduled or per-PR automation. Whether and how to automate
is decided after the loop above produces value by hand.

## Layout

- `bin/run`, `bin/judge`, `bin/export`, `hooks/checkout`, `tasks/` — the
  runner.
- `legacy/` — the earlier program as it stood at standout 12.0.0, verbatim
  and unbuilt: the archetype roster and suites (`legacy/corpus`), the
  sandboxed runner (`legacy/corpus-runner`), committed run reports and
  scorecards, the frozen members and their CI (`legacy/members`,
  `legacy/ci`). Material, not machinery.
