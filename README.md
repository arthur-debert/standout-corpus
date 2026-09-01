# standout-corpus

Accepted [standout](https://github.com/arthur-debert/standout) implementations,
frozen, with a build. **A red build here is a standout finding by default**: the
members do not change, so what changed is the framework.

This repository is the standing regression net standout's ADR-0036 describes. It
is deliberately not a product.

## What is in here, and what "accepted" means

A member is an implementation that **passed its acceptance suite**. Two kinds:

| Member | Kind | Accepted against | What its suite is |
| --- | --- | --- | --- |
| `systemdlike` | archetype | standout 9.0.0 | the archetype's `acceptance.toml` in the standout repo, replayed against the binary built here |
| `lookma` | downstream | standout 9.0.0 | its own `cargo test --workspace` — **declared, not yet built**: see below |

`lookma` is ported, pinned, and passing against a standout checkout on a laptop,
but no job here can clone it: the repository is private and this CI carries no
credential. Vendoring its sources would work and is refused — that turns a
repository whose port is its own work into a fork nobody maintains. The member
records the blocker and is left out of both workflows until `arthur-debert/lookma`
is public, at which point deleting one line in its `member.toml` enables it.

An **archetype** member is an application an agent wrote blind, from a written
spec, against the published documentation, under standout's corpus protocol —
committed here with the report it was accepted from. `systemdlike` passed 18 of
18 acceptance cases and 56 of 56 invariant checks. Nothing less is accepted: the
other four apps from the same batch each failed cases, so they are evidence in
the standout repo and are not members here.

A **downstream** member is a real application in its own repository, pointed at
by commit. The corpus holds the pointer, not a copy — a downstream's port is its
own repo's work.

## Frozen means frozen

No feature work, no refactoring, no cleanups, no dependency bumps, no
maintenance beyond porting passes some later standout epic explicitly budgets.
A member's manifest keeps the exact `=x.y.z` pins it was accepted against, and
those are never rewritten in place: they are the historical record of what the
implementation was accepted against.

## The build

`ci/build_member.py` copies a member, redirects its standout dependencies onto a
checked-out framework tree, builds it, and runs its suite.

The redirection cannot be a cargo `[patch]`. Patching changes where a package
comes from, but the member's `=` requirement must still be satisfied, and the
framework tree outgrows that pin the moment `main` bumps a version. So the copy's
manifests get path dependencies instead, which carry no version requirement at
all. `ci/prove_rewrite.py` proves this against `ci/pin-drift`, a fixture pinned
to a release the framework tree can never be again.

Until standout's ROB07 epic branch merges, the scheduled build against `main` is
red for a reason that is not a finding: `systemdlike` was accepted from a
schema-4 run report, and `corpus-runner` on `main` still reads schema 2–3. The
first scheduled run after that merge is the real baseline. Against the epic
branch the same member is green — the run this repository was verified with.

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

1. Confirm it passed its suite — for an archetype, every case on its expected
   outcome and every invariant passing, in a committed standout run report.
2. Add `members/<name>/member.toml`. For an archetype, commit the produced
   `workspace/app` sources and the report it was accepted from; for a
   downstream, record the repo and the exact commit.
3. Mark `subset = true` only if it is cheap enough to build on every standout PR.
