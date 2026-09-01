"""Point a member's standout dependencies at a checked-out framework tree.

A member keeps the exact `=x.y.z` crates.io pins it was accepted against, and
those manifests are never rewritten in place. A cargo `[patch]` cannot do the
redirection either: patching changes where a package comes from but the `=` pin
must still be satisfied, and the framework tree outgrows it the moment `main`
bumps a version. So the build copies the member and rewrites the copy, replacing
every `standout`/`standout-*` requirement with a path dependency on the
framework tree's crate of the same name.

The rewrite is line-oriented on purpose: it edits the requirement and leaves the
rest of the manifest — comments included — exactly as the member wrote it.
"""

import pathlib
import re

DEP_TABLE = re.compile(r"^\s*\[(?:[\w.-]+\.)?(?:dependencies|dev-dependencies|build-dependencies)\]\s*$")
OTHER_TABLE = re.compile(r"^\s*\[")
# `name = ...` or `name.workspace = true`; captures the crate name.
DEP_LINE = re.compile(r"^(?P<name>[A-Za-z0-9_-]+)\s*(?:\.\w+)?\s*=")


def framework_crate(framework: pathlib.Path, name: str) -> pathlib.Path | None:
    """The framework tree's directory for a standout crate, if it has one."""
    if name != "standout" and not name.startswith("standout-"):
        return None
    crate = framework / "crates" / name
    return crate if (crate / "Cargo.toml").is_file() else None


def rewrite_manifest(manifest: pathlib.Path, framework: pathlib.Path) -> list[str]:
    """Redirect one manifest's standout dependencies. Returns the crates hit."""
    lines = manifest.read_text().splitlines(keepends=True)
    out: list[str] = []
    redirected: list[str] = []
    in_deps = False
    for line in lines:
        if DEP_TABLE.match(line):
            in_deps = True
            out.append(line)
            continue
        if OTHER_TABLE.match(line):
            in_deps = False
            out.append(line)
            continue
        match = DEP_LINE.match(line) if in_deps else None
        crate = framework_crate(framework, match.group("name")) if match else None
        if crate is None:
            out.append(line)
            continue
        name = match.group("name")
        out.append(f'{name} = {{ path = "{crate}" }}\n')
        redirected.append(name)
    manifest.write_text("".join(out))
    return redirected


def rewrite_tree(root: pathlib.Path, framework: pathlib.Path) -> dict[str, list[str]]:
    """Redirect every manifest under `root`, skipping build output."""
    hits: dict[str, list[str]] = {}
    for manifest in sorted(root.rglob("Cargo.toml")):
        if "target" in manifest.relative_to(root).parts:
            continue
        redirected = rewrite_manifest(manifest, framework)
        if redirected:
            hits[str(manifest.relative_to(root))] = redirected
    return hits
