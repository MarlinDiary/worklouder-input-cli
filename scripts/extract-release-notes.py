#!/usr/bin/env python3
"""Extract one version's curated release notes from CHANGELOG.md."""

from __future__ import annotations

import argparse
import re
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("changelog", type=Path)
    parser.add_argument("version")
    args = parser.parse_args()

    version = re.escape(args.version)
    heading = re.compile(rf"^## \[{version}\](?: - \d{{4}}-\d{{2}}-\d{{2}})?$")
    lines = args.changelog.read_text(encoding="utf-8").splitlines()
    start = next((index + 1 for index, line in enumerate(lines) if heading.fullmatch(line)), None)
    if start is None:
        raise SystemExit(f"release heading not found for {args.version}")

    end = next(
        (
            index
            for index in range(start, len(lines))
            if lines[index].startswith("## ")
            or re.match(r"^\[[^]]+\]:\s", lines[index])
        ),
        len(lines),
    )
    notes = "\n".join(lines[start:end]).strip()
    if not notes:
        raise SystemExit(f"release notes are empty for {args.version}")
    print(notes)


if __name__ == "__main__":
    main()
