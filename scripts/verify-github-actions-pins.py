#!/usr/bin/env python3
"""Require immutable commit pins for every external GitHub Action."""

from __future__ import annotations

import re
from pathlib import Path


USES = re.compile(r"^\s*(?:-\s*)?uses:\s*([^\s#]+)(?:\s+#\s*(\S+))?\s*$")
COMMIT = re.compile(r"^[0-9a-f]{40}$")
VERSION = re.compile(r"^v\d+(?:\.\d+){0,2}(?:[-+][0-9A-Za-z.-]+)?$")


def main() -> None:
    workflows = sorted(Path(".github/workflows").glob("*.y*ml"))
    if not workflows:
        raise SystemExit("no GitHub Actions workflows found")

    external = 0
    errors: list[str] = []
    for workflow in workflows:
        for number, line in enumerate(workflow.read_text(encoding="utf-8").splitlines(), 1):
            match = USES.match(line)
            if not match:
                continue
            value, comment = match.groups()
            if value.startswith("./"):
                continue
            external += 1
            if "@" not in value:
                errors.append(f"{workflow}:{number}: external action has no ref")
                continue
            action, ref = value.rsplit("@", 1)
            if not COMMIT.fullmatch(ref):
                errors.append(f"{workflow}:{number}: {action} is not pinned to a full commit")
            if comment is None or not VERSION.fullmatch(comment):
                errors.append(f"{workflow}:{number}: immutable pin needs a version comment")

    if errors:
        raise SystemExit("\n".join(errors))
    if external == 0:
        raise SystemExit("no external GitHub Actions references found")
    print(f"github_actions_external_pins={external}")
    print("github_actions_pins=verified")


if __name__ == "__main__":
    main()
