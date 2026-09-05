#!/usr/bin/env python3
"""Prove the dependency rewrite works when the framework outgrew the pin.

    ci/prove_rewrite.py --framework <standout checkout> --work <scratch dir>

`ci/pin-drift` pins `standout = "=8.1.1"`, a release the framework tree can
never be again. The rewrite replaces the requirement outright, so the fixture
builds against the checkout; anything that merely re-sourced the package while
keeping the `=` requirement — a cargo `[patch]`, for one — cannot.
"""

import argparse
import pathlib
import shutil
import subprocess
import sys
import tomllib

import rewrite_deps

FIXTURE = pathlib.Path(__file__).resolve().parent / "pin-drift"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--framework", required=True, type=pathlib.Path)
    parser.add_argument("--work", required=True, type=pathlib.Path)
    args = parser.parse_args()

    framework = args.framework.resolve()
    work = args.work.resolve()
    work.mkdir(parents=True, exist_ok=True)
    root = work / "pin-drift"
    if root.exists():
        shutil.rmtree(root)
    shutil.copytree(FIXTURE, root)

    pinned = (FIXTURE / "Cargo.toml").read_text()
    manifest = tomllib.loads((framework / "crates" / "standout" / "Cargo.toml").read_text())
    version = manifest["package"]["version"]
    if f'"={version}"' in pinned:
        print(
            f"[corpus] the pin-drift fixture pins ={version}, which the framework tree "
            "now carries — it no longer proves anything; move it to an older release",
            file=sys.stderr,
        )
        return 2

    rewrite_deps.rewrite_tree(root, framework)
    result = subprocess.run(["cargo", "build"], cwd=root)
    if result.returncode != 0:
        print(
            f"[corpus] the rewrite does not survive a pin the framework outgrew: the "
            f"fixture pins a release the tree ({version}) is not, and did not build",
            file=sys.stderr,
        )
        return 1
    print(f"[corpus] rewrite proven: a =8.1.1 pin built against the {version} tree", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
