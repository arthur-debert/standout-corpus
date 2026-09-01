#!/usr/bin/env python3
"""Build one corpus member against a standout checkout and check it.

    ci/build_member.py --framework <standout checkout> --work <scratch dir> <member>

The member is frozen; the framework is whatever was checked out. So a failure
here is a finding about the framework by default — that is the whole point of
the corpus (standout ADR-0036).

Two kinds of member, two meanings of "passes its suite":

- `archetype`: a produced app from a blind corpus run, committed with the report
  it was accepted from. Its suite is the archetype's `acceptance.toml` in the
  standout checkout, replayed by `corpus-runner reevaluate` against the binary
  built here. Every case must land on its expected outcome and every invariant
  must pass — that is what "accepted" meant, so anything less is a regression.
- `downstream`: a real application in its own repository, cloned at a pinned
  commit. Its suite is its own test command.

The work directory holds the disposable copy. It must not live beneath the
framework checkout: the runner refuses to evaluate a workspace nested under the
source tree the blind protocol excludes.
"""

import argparse
import json
import pathlib
import shutil
import subprocess
import sys
import tomllib

import rewrite_deps

REPO = pathlib.Path(__file__).resolve().parent.parent


class Failed(Exception):
    """A member did not pass. The message is the finding."""


def run(command: list[str], cwd: pathlib.Path, what: str) -> None:
    print(f"[corpus] {what}: {' '.join(str(part) for part in command)}", flush=True)
    result = subprocess.run(command, cwd=cwd)
    if result.returncode != 0:
        raise Failed(f"{what} failed with exit {result.returncode}")


def resolved_standout_source(root: pathlib.Path) -> str:
    """Where cargo says `standout` came from, after the rewrite."""
    metadata = subprocess.run(
        ["cargo", "metadata", "--format-version", "1"],
        cwd=root,
        capture_output=True,
        text=True,
    )
    if metadata.returncode != 0:
        raise Failed(f"cargo metadata failed:\n{metadata.stderr}")
    packages = json.loads(metadata.stdout)["packages"]
    for package in packages:
        if package["name"] == "standout":
            return package["source"] or f"path+{package['manifest_path']}"
    raise Failed("the member's dependency graph has no `standout` package")


def check_redirection(root: pathlib.Path) -> None:
    source = resolved_standout_source(root)
    if source is not None and source.startswith("registry"):
        raise Failed(
            f"the dependency rewrite did not take: `standout` still resolves to {source}, "
            "so this build would measure the published release rather than the checkout"
        )
    print(f"[corpus] standout resolves to {source}", flush=True)


def prepare(member_dir: pathlib.Path, member: dict, work: pathlib.Path) -> pathlib.Path:
    """The disposable copy of the member, deps already redirected."""
    root = work / member_dir.name
    if root.exists():
        shutil.rmtree(root)
    if member["kind"] == "archetype":
        shutil.copytree(member_dir / "workspace", root)
    else:
        run(["git", "clone", "--quiet", member["repo"], str(root)], work, "clone")
        run(["git", "checkout", "--quiet", member["commit"]], root, "checkout")
        shutil.rmtree(root / ".git")
    return root


def check_archetype(
    root: pathlib.Path, member_dir: pathlib.Path, member: dict, framework: pathlib.Path
) -> None:
    app = root / "app"
    run(["cargo", "build", "--bin", member["binary"]], app, "build")
    binary = app / "target" / "debug" / member["binary"]
    if not binary.is_file():
        raise Failed(f"the build produced no {binary}")

    report = root / "report.json"
    run(
        [
            "cargo", "run", "--quiet", "-p", "corpus-runner", "--",
            "reevaluate", member["archetype"],
            "--corpus-dir", str(framework / "corpus"),
            "--docs-dir", str(framework / "docs"),
            "--workspace", str(root),
            "--source-report", str(member_dir / "baseline-report.json"),
            "--output-report", str(report),
            "--binary", str(binary),
        ],
        framework,
        "acceptance suite",
    )
    verdict(json.loads(report.read_text()), json.loads((member_dir / "baseline-report.json").read_text()))


# The runner's own vocabulary (`CaseOutcome::is_expected`): a case behaved as
# its author wrote it when it passed, or when it failed and the suite said it
# would.
#
# Only `fail` means the produced application stopped working, and only that is
# red here. `unexpected-pass` — a gap tripwire whose premise no longer holds —
# is the framework having gained something, and standout's own `gaps.toml`
# ledger test already fails when that happens; a second alarm here would only
# teach people that corpus red does not mean an application broke.
AS_AUTHORED = {"pass", "expected-fail"}
BROKEN = "fail"


def cell_key(check: dict) -> tuple:
    return (check["command"], check["mode"], check["color"], check["theme"], check["check"])


def verdict(report: dict, baseline: dict) -> None:
    """Compare this run against the run the member was accepted from.

    The comparison is against the baseline rather than against an absolute
    all-green, because a member is frozen with whatever it actually did when it
    was accepted: authored expected-fail cases, and — for one member — eight
    invariant cells a defect in the invariant matrix's own vocabulary makes
    fail (standout#467). Holding those against every later build would make the
    member permanently red and say nothing about the framework.

    Movement in either direction is reported. Only a produced application that
    stopped working is red: a case that now fails, or an invariant cell that now
    fails. Movement the other way — a gap tripwire whose premise no longer
    holds, a known-failing cell that started passing — is a framework change
    someone else owns (standout's `gaps.toml` ledger, standout#467) and a reason
    to re-accept the member from a fresh run, not a reason to fail this build.
    """
    acceptance = report["acceptance"]
    if not acceptance["built"]:
        raise Failed(f"the app did not build: {acceptance.get('build_detail')}")

    was = {case["name"]: case["outcome"] for case in baseline["acceptance"]["cases"]}
    regressions: list[str] = []
    improvements: list[str] = []
    for case in acceptance["cases"]:
        before, now = was.get(case["name"]), case["outcome"]
        if before is None:
            regressions.append(f"case {case['name']}: not in the accepted run — the suite changed")
            continue
        if before == now:
            continue
        moved = f"case {case['name']}: {before} when accepted, {now} now"
        if now == BROKEN:
            detail = case.get("detail")
            regressions.append(moved + (f" — {detail}" if detail else ""))
        else:
            improvements.append(moved)

    was_cells = {cell_key(check): check["status"] for check in baseline["invariants"]}
    for check in report["invariants"]:
        before, now = was_cells.get(cell_key(check)), check["status"]
        if before is None or before == now:
            continue
        moved = (
            f"invariant {check['command'] or '<naked>'} [{check['mode']}/{check['color']}]"
            f" {check['check']}: {before} when accepted, {now} now"
        )
        if now == "pass":
            improvements.append(moved)
        else:
            regressions.append(moved + (f" — {check['detail']}" if check.get("detail") else ""))

    for moved in improvements:
        print(f"[corpus] IMPROVED {moved}", flush=True)

    if regressions:
        raise Failed(
            f"{len(regressions)} check(s) no longer behave the way this member was "
            "accepted behaving:\n  " + "\n  ".join(regressions)
        )
    print(
        f"[corpus] {len(acceptance['cases'])} acceptance cases and "
        f"{len(report['invariants'])} invariant cells as accepted"
        + (f", {len(improvements)} improved" if improvements else ""),
        flush=True,
    )


def check_downstream(root: pathlib.Path, member: dict) -> None:
    run(member["check"], root, "suite")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("member")
    parser.add_argument("--framework", required=True, type=pathlib.Path)
    parser.add_argument("--work", required=True, type=pathlib.Path)
    args = parser.parse_args()

    framework = args.framework.resolve()
    work = args.work.resolve()
    if framework in work.parents or work == framework:
        print(
            "[corpus] the work directory must not live beneath the framework checkout",
            file=sys.stderr,
        )
        return 2
    work.mkdir(parents=True, exist_ok=True)

    member_dir = REPO / "members" / args.member
    member = tomllib.loads((member_dir / "member.toml").read_text())
    if member.get("blocked"):
        print(f"[corpus] {args.member} is not built: {member['blocked']}", file=sys.stderr)
        return 2

    try:
        root = prepare(member_dir, member, work)
        hits = rewrite_deps.rewrite_tree(root, framework)
        if not hits:
            raise Failed("no manifest under the member declares a standout dependency")
        for manifest, crates in hits.items():
            print(f"[corpus] {manifest}: redirected {', '.join(crates)}", flush=True)
        check_redirection(root if (root / "Cargo.toml").is_file() else root / "app")

        if member["kind"] == "archetype":
            check_archetype(root, member_dir, member, framework)
        else:
            check_downstream(root, member)
    except Failed as failure:
        print(f"[corpus] FINDING — {args.member}: {failure}", file=sys.stderr, flush=True)
        return 1

    print(f"[corpus] {args.member}: as accepted", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
