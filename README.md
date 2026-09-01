# standout-corpus

Accepted [standout](https://github.com/arthur-debert/standout) implementations,
frozen, with a build. **A red build here is a standout finding by default**: the
members do not change, so what changed is the framework.

This repository is the standing regression net standout's ADR-0036 describes. It
is deliberately not a product.

## What is in here, and what "accepted" means

A member is an implementation that **passed its acceptance suite**. Two kinds:

| Member | Kind | Accepted against | In the PR subset | What it did when accepted |
| --- | --- | --- | --- | --- |
| `systemdlike` | archetype | standout 9.0.0 | yes | 18/18 cases, 56 invariant cells passing, none failing |
| `kubelike` | archetype | standout 9.0.0 | no | 42/42 cases; 8 invariant cells failing, all standout#467 |
| `pnpmlike` | archetype | standout 9.0.0 | no | 32/32 cases, 28 invariant cells passing, none failing |
| `brewlike` | archetype | standout 9.0.0 | no | 26 passing + 2 authored expected-fail (PAR02), 84 invariant cells passing |
| `lookma` | downstream | standout 9.0.0 | — | its own `cargo test --workspace` — **declared, not yet built**: see below |

An archetype's suite is the `acceptance.toml` in the standout repo, replayed
against the binary built here. A downstream's suite is its own test command.

**The PR subset** is the members cheap enough for the standout PR lane, which
ADR-0036 scopes to the pilot archetypes plus lookma. The completion archetypes
above run on the schedule instead, so a framework PR pays for one member rather
than four.

`lookma` is ported, pinned, and passing against a standout checkout on a laptop,
but no job here can clone it: the repository is private and this CI carries no
credential. Vendoring its sources would work and is refused — that turns a
repository whose port is its own work into a fork nobody maintains. The member
records the blocker and is left out of both workflows until `arthur-debert/lookma`
is public, at which point deleting one line in its `member.toml` enables it.

An **archetype** member is an application an agent wrote blind, from a written
spec, against the published documentation, under standout's corpus protocol —
committed here with the report it was accepted from. A **downstream** member is a
real application in its own repository, pointed at by commit; the corpus holds
the pointer, not a copy, because a downstream's port is its own repo's work.

### What "passes its acceptance suite" admits, and what it does not

ADR-0036 admits an app that passes its acceptance suite. Three readings came up
while promoting the runs above, and each member's `member.toml` carries the one
that applies to it:

- **An authored expected-fail case that failed as authored counts as passing.**
  A suite saying "this fails today, and here is the gap" is passing when the case
  fails for that reason — the runner's own `CaseOutcome::is_expected` says so.
  The other reading would make every archetype carrying a gap tripwire
  permanently unfreezable, and tripwires are exactly what a regression net wants
  to watch. (`brewlike`, two PAR02 cases.)
- **An invariant-matrix failure is not an acceptance-suite failure.** The
  invariant matrix is a separate instrument, and when the instrument is the thing
  at fault the app should not be refused for it. `kubelike` passes 42 of 42
  acceptance cases and fails 8 invariant cells to standout#467, a defect in the
  matrix's own vocabulary: the archetype declares each command `rendered` or
  `opaque-bytes` before any application exists, but which one a command is is the
  application's choice, and standout documents two conforming ways to answer.
  #467 says it outright — "the framework did what it documents; nothing here is a
  framework defect."
- **An unexpected-pass is not passing.** It means a gap tripwire's premise no
  longer holds — a real signal, owned by the parity epic and by standout's
  `gaps.toml` ledger, and an open question rather than a stable baseline. An app
  whose run is full of them is not frozen here. (`cargolike` with 21 and
  `gcloudlike` with 24 were declined on this.)

A real acceptance failure is still a refusal, with no reading available:
`dockerlike` failed two cases it was supposed to pass, and `ghlike`, `gitlike`,
`formlike` and `validity` each failed or unexpectedly passed cases in the 9.0
re-run.

## Frozen means frozen

No feature work, no refactoring, no cleanups, no dependency bumps, no
maintenance beyond porting passes some later standout epic explicitly budgets.
A member's manifest keeps the exact `=x.y.z` pins it was accepted against, and
those are never rewritten in place: they are the historical record of what the
implementation was accepted against.

## The build

`ci/build_member.py` copies a member, redirects its standout dependencies onto a
checked-out framework tree, builds it, and runs its suite.

**What makes a build red.** The result is compared against the run the member was
accepted from, not against an all-green ideal — a member is frozen with whatever
it actually did, authored expected-fail cases and standout#467's eight cells
included, and holding those against every later build would make the member
permanently red while saying nothing about the framework. So: a case or an
invariant cell that **now fails** is red, because a produced application stopped
working. Movement the other way — a known-failing cell that started passing, a
gap tripwire whose premise no longer holds — is printed as `IMPROVED`, leaves the
build green, and means the member should be re-accepted from a fresh run.
standout's own `gaps.toml` ledger test is the alarm for a closed gap; a second
alarm here would only teach people that corpus red does not mean an application
broke. `ci/test_verdict.py` pins all of that and runs before any member is
judged by it.

The redirection cannot be a cargo `[patch]`. Patching changes where a package
comes from, but the member's `=` requirement must still be satisfied, and the
framework tree outgrows that pin the moment `main` bumps a version. So the copy's
manifests get path dependencies instead, which carry no version requirement at
all. `ci/prove_rewrite.py` proves this against `ci/pin-drift`, a fixture pinned
to a release the framework tree can never be again.

Until standout's ROB07 epic branch merges, the scheduled build against `main` is
red for a reason that is not a finding: every member was accepted from a schema-4
run report, and `corpus-runner` on `main` still reads schema 2–3 — and the four
completion archetypes do not exist on `main` at all. The first scheduled run
after that merge is the real baseline. Against the epic branch every member is
green, which is what this repository was verified with.

Two workflows run it:

- **This repository's `Corpus` workflow** builds every member against standout
  `main`, daily and on demand.
- **standout's own `Corpus` workflow** checks out this repository on framework
  PRs and builds the fast subset — the members marked `subset = true`.

## No secrets

Members are untrusted code (standout ADR-0023), and this repository's CI builds
and runs them. It carries no secrets, and no workflow here may be given one.
Should some job ever need a credential, the mechanism is Doppler through its
GitHub Action, in a job that does not check out, build, or run a member: a
secret in a job that executes untrusted code is exposed to that code however it
is stored.

## Adding a member

1. Confirm it passed its suite, using the readings above. Where one of them is
   what admits the member — an authored expected-fail, an invariant failure that
   belongs to the instrument — say so in `member.toml`, with the issue number.
   An unexplained failure is a refusal.
2. Add `members/<name>/member.toml`. For an archetype, commit the produced
   `workspace/app` sources (no build output) and the sanitized report it was
   accepted from; for a downstream, record the repo and the exact commit.
3. Mark `subset = true` only if it is cheap enough to build on every standout PR.
4. Run `ci/build_member.py <name>` against a standout checkout before pushing.
