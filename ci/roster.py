#!/usr/bin/env python3
"""Print the corpus roster as a GitHub Actions matrix.

    ci/roster.py            # every member
    ci/roster.py --subset   # only the members the standout PR lane builds

The subset is a per-member `subset = true`, not a list kept somewhere else: a
member joins the fast lane where it is defined, and the two workflows read the
same roster.
"""

import argparse
import json
import pathlib
import tomllib

MEMBERS = pathlib.Path(__file__).resolve().parent.parent / "members"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--subset", action="store_true")
    args = parser.parse_args()

    names = []
    for member_dir in sorted(MEMBERS.iterdir()):
        manifest = member_dir / "member.toml"
        if not manifest.is_file():
            continue
        member = tomllib.loads(manifest.read_text())
        if args.subset and not member.get("subset"):
            continue
        names.append(member_dir.name)
    print(json.dumps(names))


if __name__ == "__main__":
    main()
