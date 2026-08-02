#!/usr/bin/env bash
set -euo pipefail

REPO="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$REPO"
ROOT="$(mktemp -d "${TMPDIR:-/tmp}/worklouderctl-codex-e2e.XXXXXX")"
if [[ -n "${WORKLOUDERCTL_BIN:-}" ]]; then
  BIN="$WORKLOUDERCTL_BIN"
else
  cargo build --locked
  BIN="$REPO/target/debug/worklouderctl"
fi
SOCKET="$ROOT/bridge.sock"
TOKEN="$ROOT/bridge.token"
SERVER_PID=""

cleanup() {
  if [[ -n "$SERVER_PID" ]]; then
    kill "$SERVER_PID" 2>/dev/null || true
    wait "$SERVER_PID" 2>/dev/null || true
  fi
  if [[ "${KEEP_CODEX_BRIDGE_E2E:-0}" != 1 ]]; then
    rm -rf "$ROOT"
  fi
}
trap cleanup EXIT

node companion/codex-fixture-server.mjs "$SOCKET" "$TOKEN" \
  >"$ROOT/server.stdout" 2>"$ROOT/server.stderr" &
SERVER_PID=$!
for _ in $(seq 1 100); do
  [[ -S "$SOCKET" ]] && break
  sleep 0.05
done
[[ -S "$SOCKET" ]]

"$BIN" --json codex bridge --socket "$SOCKET" --token "$TOKEN" inspect \
  >"$ROOT/bridge.json"
"$BIN" --json codex config --socket "$SOCKET" --token "$TOKEN" snapshot \
  --output "$ROOT/baseline.json" >"$ROOT/baseline-receipt.json"
"$BIN" --json codex agent-key assignments --socket "$SOCKET" --token "$TOKEN" \
  >"$ROOT/agent-keys.json"
"$BIN" --json codex agent-source set --input "$ROOT/baseline.json" custom \
  --output "$ROOT/candidate.json" >"$ROOT/candidate-receipt.json"
"$BIN" --json codex config --socket "$SOCKET" --token "$TOKEN" apply \
  --input "$ROOT/candidate.json" --backup "$ROOT/pre-apply.json" \
  --idempotency-key fixture-apply-v1 >"$ROOT/apply.json"
"$BIN" --json codex config --socket "$SOCKET" --token "$TOKEN" snapshot \
  --output "$ROOT/modified.json" >"$ROOT/modified-receipt.json"
"$BIN" --json codex config --socket "$SOCKET" --token "$TOKEN" restore \
  --input "$ROOT/baseline.json" --backup "$ROOT/pre-restore.json" \
  --idempotency-key fixture-restore-v1 >"$ROOT/restore.json"
"$BIN" --json codex config --socket "$SOCKET" --token "$TOKEN" snapshot \
  --output "$ROOT/restored.json" >"$ROOT/restored-receipt.json"

python3 - "$ROOT" <<'PY'
import json
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
baseline = json.loads((root / "baseline.json").read_text())
modified = json.loads((root / "modified.json").read_text())
restored = json.loads((root / "restored.json").read_text())
keys = json.loads((root / "agent-keys.json").read_text())
apply = json.loads((root / "apply.json").read_text())
restore = json.loads((root / "restore.json").read_text())

assert baseline["settings"]["codex-micro-agent-source"] == "recent"
assert modified["settings"]["codex-micro-agent-source"] == "custom"
assert apply["changed"] is True and apply["rollbackPerformed"] is False
assert restore["changed"] is True and restore["rollbackPerformed"] is False
assert restored["settings"] == baseline["settings"]
assert restored["effectiveSettings"] == baseline["effectiveSettings"]
assert restored["sourceSha256"] == baseline["sourceSha256"]
assert keys["assignments"]["AG00"]["commandId"] == "fixture.command"
assert len(keys["slots"]) == 6

print(json.dumps({
    "status": "pass",
    "baselineAgentSource": baseline["settings"]["codex-micro-agent-source"],
    "modifiedAgentSource": modified["settings"]["codex-micro-agent-source"],
    "restoredAgentSource": restored["settings"]["codex-micro-agent-source"],
    "baselineSourceSha256": baseline["sourceSha256"],
    "modifiedSourceSha256": modified["sourceSha256"],
    "restoredSourceSha256": restored["sourceSha256"],
    "agentKeySlots": len(keys["slots"]),
}, separators=(",", ":")))
PY
