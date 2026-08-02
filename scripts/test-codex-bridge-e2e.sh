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
  --output "$ROOT/source-candidate.json" >"$ROOT/source-candidate-receipt.json"
"$BIN" --json codex lighting brightness set --input "$ROOT/source-candidate.json" 37 \
  --output "$ROOT/brightness-candidate.json" >"$ROOT/brightness-candidate-receipt.json"
"$BIN" --json codex lighting auto-off set --input "$ROOT/brightness-candidate.json" \
  10-minutes --output "$ROOT/candidate.json" >"$ROOT/candidate-receipt.json"
"$BIN" --json codex lighting brightness get --input "$ROOT/candidate.json" \
  >"$ROOT/brightness-get.json"
"$BIN" --json codex lighting auto-off get --input "$ROOT/candidate.json" \
  >"$ROOT/auto-off-get.json"
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

"$BIN" --json codex agent-key snapshot --socket "$SOCKET" --token "$TOKEN" \
  --output "$ROOT/agent-baseline.json" >"$ROOT/agent-baseline-receipt.json"
"$BIN" --json codex agent-key get --input "$ROOT/agent-baseline.json" AG00 \
  >"$ROOT/agent-get-baseline.json"
"$BIN" --json codex agent-key set --input "$ROOT/agent-baseline.json" AG01 \
  --skill-name Review --skill-path /tmp/fixture-review/SKILL.md \
  --output "$ROOT/agent-skill.json" >"$ROOT/agent-skill-receipt.json"
"$BIN" --json codex agent-key set --input "$ROOT/agent-skill.json" AG02 \
  --thread-host local --thread-key fixture-thread --title "Fixture Task" \
  --output "$ROOT/agent-thread.json" >"$ROOT/agent-thread-receipt.json"
"$BIN" --json codex agent-key set --input "$ROOT/agent-thread.json" AG03 \
  --keycap GIT --output "$ROOT/agent-keycap.json" \
  >"$ROOT/agent-keycap-receipt.json"
"$BIN" --json codex agent-key set --input "$ROOT/agent-keycap.json" AG04 \
  --command fixture.command.two --output "$ROOT/agent-command.json" \
  >"$ROOT/agent-command-receipt.json"
"$BIN" --json codex agent-key clear --input "$ROOT/agent-command.json" AG00 \
  --output "$ROOT/agent-candidate.json" >"$ROOT/agent-clear-receipt.json"
"$BIN" --json codex agent-key apply --socket "$SOCKET" --token "$TOKEN" \
  --input "$ROOT/agent-candidate.json" --backup "$ROOT/agent-pre-apply.json" \
  --idempotency-key fixture-agent-apply-v1 >"$ROOT/agent-apply.json"
"$BIN" --json codex agent-key apply --socket "$SOCKET" --token "$TOKEN" \
  --input "$ROOT/agent-candidate.json" --backup "$ROOT/agent-pre-apply.json" \
  --idempotency-key fixture-agent-apply-v1 >"$ROOT/agent-apply-replay.json"
if "$BIN" --json codex agent-key apply --socket "$SOCKET" --token "$TOKEN" \
  --input "$ROOT/agent-candidate.json" --backup "$ROOT/agent-pre-apply.json" \
  --expected-global-state-revision "$(printf '0%.0s' {1..64})" \
  --idempotency-key fixture-agent-stale >"$ROOT/agent-stale.stdout" \
  2>"$ROOT/agent-stale.stderr"; then
  echo "stale Agent Key CAS unexpectedly succeeded" >&2
  exit 1
fi
"$BIN" --json codex agent-key snapshot --socket "$SOCKET" --token "$TOKEN" \
  --output "$ROOT/agent-modified.json" >"$ROOT/agent-modified-receipt.json"
"$BIN" --json codex agent-key restore --socket "$SOCKET" --token "$TOKEN" \
  --input "$ROOT/agent-baseline.json" --backup "$ROOT/agent-pre-restore.json" \
  --idempotency-key fixture-agent-restore-v1 >"$ROOT/agent-restore.json"
"$BIN" --json codex agent-key snapshot --socket "$SOCKET" --token "$TOKEN" \
  --output "$ROOT/agent-restored.json" >"$ROOT/agent-restored-receipt.json"

python3 - "$ROOT" <<'PY'
import json
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
bridge = json.loads((root / "bridge.json").read_text())
baseline = json.loads((root / "baseline.json").read_text())
modified = json.loads((root / "modified.json").read_text())
restored = json.loads((root / "restored.json").read_text())
keys = json.loads((root / "agent-keys.json").read_text())
apply = json.loads((root / "apply.json").read_text())
restore = json.loads((root / "restore.json").read_text())
brightness_get = json.loads((root / "brightness-get.json").read_text())
auto_off_get = json.loads((root / "auto-off-get.json").read_text())
agent_baseline = json.loads((root / "agent-baseline.json").read_text())
agent_modified = json.loads((root / "agent-modified.json").read_text())
agent_restored = json.loads((root / "agent-restored.json").read_text())
agent_apply = json.loads((root / "agent-apply.json").read_text())
agent_replay = json.loads((root / "agent-apply-replay.json").read_text())
agent_restore = json.loads((root / "agent-restore.json").read_text())

assert baseline["settings"]["codex-micro-agent-source"] == "recent"
assert "codex.agentKeys.apply.v1" in bridge["capabilities"]
assert "codex.agentKeys.restore.v1" in bridge["capabilities"]
assert modified["settings"]["codex-micro-agent-source"] == "custom"
assert baseline["effectiveSettings"]["codex-micro-lighting-brightness"] == 100
assert baseline["effectiveSettings"]["codex-micro-lighting-auto-off"] == "3-minutes"
assert brightness_get["value"] == 37 and brightness_get["explicit"] is True
assert auto_off_get["value"] == "10-minutes" and auto_off_get["explicit"] is True
assert modified["settings"]["codex-micro-lighting-brightness"] == 37
assert modified["settings"]["codex-micro-lighting-auto-off"] == "10-minutes"
assert apply["changed"] is True and apply["rollbackPerformed"] is False
assert restore["changed"] is True and restore["rollbackPerformed"] is False
assert restored["settings"] == baseline["settings"]
assert restored["effectiveSettings"] == baseline["effectiveSettings"]
assert restored["sourceSha256"] == baseline["sourceSha256"]
assert keys["assignments"]["AG00"]["commandId"] == "fixture.command"
assert len(keys["slots"]) == 6
assert agent_baseline["assignments"]["AG00"]["commandId"] == "fixture.command"
assert agent_modified["assignments"]["AG00"] is None
assert agent_modified["assignments"]["AG01"]["type"] == "skill"
assert agent_modified["assignments"]["AG02"]["threadKey"] == "fixture-thread"
assert agent_modified["assignments"]["AG03"]["keycapId"] == "GIT"
assert agent_modified["assignments"]["AG04"]["commandId"] == "fixture.command.two"
assert agent_apply["changed"] is True and agent_apply["rollbackPerformed"] is False
assert agent_replay["idempotentReplay"] is True
assert agent_restore["changed"] is True and agent_restore["rollbackPerformed"] is False
assert agent_restored["assignments"] == agent_baseline["assignments"]
assert agent_restored["globalStateRevision"] == agent_baseline["globalStateRevision"]
assert "revision conflict" in (root / "agent-stale.stderr").read_text()

print(json.dumps({
    "status": "pass",
    "baselineAgentSource": baseline["settings"]["codex-micro-agent-source"],
    "modifiedAgentSource": modified["settings"]["codex-micro-agent-source"],
    "restoredAgentSource": restored["settings"]["codex-micro-agent-source"],
    "baselineLightingBrightness": baseline["effectiveSettings"]["codex-micro-lighting-brightness"],
    "modifiedLightingBrightness": modified["settings"]["codex-micro-lighting-brightness"],
    "restoredLightingBrightness": restored["effectiveSettings"]["codex-micro-lighting-brightness"],
    "baselineLightingAutoOff": baseline["effectiveSettings"]["codex-micro-lighting-auto-off"],
    "modifiedLightingAutoOff": modified["settings"]["codex-micro-lighting-auto-off"],
    "restoredLightingAutoOff": restored["effectiveSettings"]["codex-micro-lighting-auto-off"],
    "baselineSourceSha256": baseline["sourceSha256"],
    "modifiedSourceSha256": modified["sourceSha256"],
    "restoredSourceSha256": restored["sourceSha256"],
    "agentKeySlots": len(keys["slots"]),
    "agentKeysBaselineRevision": agent_baseline["globalStateRevision"],
    "agentKeysModifiedRevision": agent_modified["globalStateRevision"],
    "agentKeysRestoredRevision": agent_restored["globalStateRevision"],
    "agentKeysIdempotentReplay": agent_replay["idempotentReplay"],
    "agentKeysStaleCasRejected": True,
}, separators=(",", ":")))
PY
