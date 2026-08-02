#!/bin/sh
set -eu

repo=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
fixtures="$repo/fixtures"
tmp=$(mktemp -d "${TMPDIR:-/tmp}/worklouderctl-fixtures.XXXXXX")
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

python3 "$repo/scripts/build-sanitized-fixtures.py" --output "$tmp/generated"
diff -ru "$fixtures" "$tmp/generated"

find "$fixtures" -type f -name '*.json' -exec jq -e . {} \; >/dev/null

python3 - "$fixtures" <<'PY'
import hashlib
import json
import pathlib
import re
import sys

root = pathlib.Path(sys.argv[1])
for manifest_path in root.rglob("manifest.json"):
    manifest = json.loads(manifest_path.read_text())
    for record in manifest["files"]:
        path = manifest_path.parent / record["relativePath"]
        content = path.read_bytes()
        assert len(content) == record["size"], path
        assert hashlib.sha256(content).hexdigest() == record["sha256"], path

for path in root.rglob("*"):
    if not path.is_file():
        continue
    text = path.read_text(errors="strict")
    assert not re.search(r"/Users/[^$<]", text), path
    assert not re.search(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}", text), path
    assert not re.search(r'(?i)(token|api[_-]?key|password|secret)\s*[:=]\s*[^<\s]', text), path
PY

cargo build --locked --manifest-path "$repo/Cargo.toml" >/dev/null
bin="$repo/target/debug/worklouderctl"
for version in 0.17.3 0.18.0; do
  snapshot="$fixtures/input/$version/codex-micro-v0.6.0/config-snapshot.json"
  "$bin" --json config validate "$snapshot" >"$tmp/validate-$version.json"
  jq -e '.valid == true and .kind == "json-file"' "$tmp/validate-$version.json" >/dev/null
  "$bin" --json profile list --input "$snapshot" >"$tmp/profiles-$version.json"
  jq -e '.kind == "worklouderctl-profile-list" and (.profiles | length == 1) and .profiles[0].name == "Sanitized Profile"' "$tmp/profiles-$version.json" >/dev/null
done

printf '%s\n' \
  'fixture_generation=deterministic' \
  'fixture_manifests=sha256-verified' \
  'fixture_sensitive_patterns=absent' \
  'input_0.17.3_snapshot=valid' \
  'input_0.18.0_snapshot=valid' \
  'firmware_v0.6.0_status=valid-json'
