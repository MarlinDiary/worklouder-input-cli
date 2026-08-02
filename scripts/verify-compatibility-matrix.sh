#!/bin/sh
set -eu

repo=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cargo build --locked --manifest-path "$repo/Cargo.toml" >/dev/null
bin=$repo/target/debug/worklouderctl
root=$(mktemp -d "${TMPDIR:-/tmp}/worklouderctl-compatibility.XXXXXX")

"$bin" --json compatibility verify >"$root/verify.json"
"$bin" --json compatibility list >"$root/list.json"
"$bin" --json compatibility show >"$root/current.json"
"$bin" --json schema show compatibility-matrix-v1 >"$root/schema.json"

python3 - "$repo" "$root" <<'PY'
import json
from pathlib import Path
import re
import sys

repo, root = map(Path, sys.argv[1:])
cargo = (repo / "Cargo.toml").read_text()
version = re.search(r'^version = "([^"]+)"$', cargo, re.MULTILINE).group(1)
matrix = json.loads((repo / "spec/compatibility-matrix-v1.json").read_text())
verify = json.loads((root / "verify.json").read_text())
listed = json.loads((root / "list.json").read_text())
current = json.loads((root / "current.json").read_text())
schema = json.loads((root / "schema.json").read_text())

entries = [release for release in matrix["releases"] if release["cliVersion"] == version]
assert len(entries) == 1
assert current == entries[0]
assert verify["valid"] is True
assert verify["currentCliVersion"] == version
assert [item["cliVersion"] for item in listed] == [
    item["cliVersion"] for item in matrix["releases"]
]
assert schema["$id"].endswith("compatibility-matrix-v1.schema.json")
print(f"compatibility_current_release={version}")
print(f"compatibility_release_entries={len(matrix['releases'])}")
print("compatibility_matrix=verified")
PY
