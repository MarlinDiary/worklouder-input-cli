#!/usr/bin/env python3
"""Reject invalid JSON and duplicate object keys in checked-in contracts."""

import json
from pathlib import Path


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate object key {key!r}")
        result[key] = value
    return result


def main():
    repo = Path(__file__).resolve().parent.parent
    paths = sorted((repo / "spec").rglob("*.json"))
    if not paths:
        raise SystemExit("no JSON contracts found")
    for path in paths:
        try:
            json.loads(path.read_text(), object_pairs_hook=unique_object)
        except (json.JSONDecodeError, ValueError) as error:
            relative = path.relative_to(repo)
            raise SystemExit(f"{relative}: {error}") from error
    print(f"json_contracts={len(paths)}")
    print("json_duplicate_keys=absent")


if __name__ == "__main__":
    main()
