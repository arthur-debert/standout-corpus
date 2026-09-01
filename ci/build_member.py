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
    verdict(json.loads(report.read_text()))


def verdict(report: dict) -> None:
    """Fail on anything the accepted run got right and this one did not."""
    acceptance = report["acceptance"]
    if not acceptance["built"]:
        raise Failed(f"the app did not build: {acceptance.get('build_detail')}")

    regressions = [
        f"case {case['name']}: expected {case['expected']}, got {case['outcome']}"
        + (f" — {case['detail']}" if case.get("detail") else "")
        for case in acceptance["cases"]
        if case["outcome"] != case["expected"]
    ]
    regressions += [
        f"invariant {check['command'] or '<naked>'} [{check['mode']}/{check['color']}]"
        f" {check['check']}: {check.get('detail') or 'failed'}"
        for check in report["invariants"]
        if check["status"] == "fail"
    ]
    if regressions:
        raise Failed(
            f"{len(regressions)} check(s) the accepted run passed now fail:\n  "
            + "\n  ".join(regressions)
        )
    print(
        f"[corpus] {len(acceptance['cases'])} acceptance cases and "
        f"{len(report['invariants'])} invariant checks as accepted",
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
