#!/usr/bin/env bash
set -euo pipefail

REPO="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$REPO"
ROOT="$(mktemp -d /tmp/wltxn.XXXXXX)"
BIN="${WORKLOUDERCTL_BIN:-$REPO/target/debug/worklouderctl}"
PIDS=()

cleanup() {
  for pid in "${PIDS[@]}"; do
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
  done
  if [[ "${KEEP_TRANSACTION_E2E:-0}" != 1 ]]; then
    rm -rf "$ROOT"
  else
    printf 'transaction_e2e_root=%s\n' "$ROOT"
  fi
}
trap cleanup EXIT

if [[ -z "${WORKLOUDERCTL_BIN:-}" ]]; then
  cargo build --locked
fi

wait_for_bridge() {
  local socket="$1"
  local token="$2"
  local log="$3"
  for _ in $(seq 1 200); do
    [[ -S "$socket" && -f "$token" ]] && return 0
    sleep 0.05
  done
  cat "$log" >&2
  return 1
}

stop_case_servers() {
  local input_pid="$1"
  local codex_pid="$2"
  kill "$input_pid" "$codex_pid" 2>/dev/null || true
  wait "$input_pid" 2>/dev/null || true
  wait "$codex_pid" 2>/dev/null || true
}

run_case() {
  local label="$1"
  local fail_settings_writes="$2"
  local root="$ROOT/$label"
  local input_socket="$root/input.sock"
  local input_token="$root/input.token"
  local codex_socket="$root/codex.sock"
  local codex_token="$root/codex.token"
  mkdir -p "$root"

  node companion/fixture-server.mjs "$input_socket" "$input_token" \
    >"$root/input-server.log" 2>&1 &
  local input_pid=$!
  PIDS+=("$input_pid")
  WORKLOUDERCTL_FIXTURE_FAIL_CODEX_SETTINGS_WRITES="$fail_settings_writes" \
    node companion/codex-fixture-server.mjs "$codex_socket" "$codex_token" \
    >"$root/codex-server.log" 2>&1 &
  local codex_pid=$!
  PIDS+=("$codex_pid")
  wait_for_bridge "$input_socket" "$input_token" "$root/input-server.log"
  wait_for_bridge "$codex_socket" "$codex_token" "$root/codex-server.log"

  "$BIN" --json device --transport bridge \
    --bridge-socket "$input_socket" --bridge-token "$input_token" status \
    >"$root/device-baseline.json"
  "$BIN" --json device --transport bridge \
    --bridge-socket "$input_socket" --bridge-token "$input_token" config snapshot \
    --output "$root/input-base.json" >"$root/input-base-receipt.json"
  "$BIN" --json layer rename --input "$root/input-base.json" \
    --profile 0 --id 1 --name "Transaction $label" \
    --output "$root/input-candidate.json" >"$root/input-candidate-receipt.json"
  "$BIN" --json input --bridge-socket "$input_socket" --bridge-token "$input_token" \
    permission command snapshot --output "$root/host-base.json" \
    >"$root/host-base-receipt.json"
  "$BIN" --json input permission command set --input "$root/host-base.json" enabled \
    --output "$root/host-candidate.json" >"$root/host-candidate-receipt.json"

  "$BIN" --json codex config --socket "$codex_socket" --token "$codex_token" snapshot \
    --output "$root/codex-base.json" >"$root/codex-base-receipt.json"
  "$BIN" --json codex lighting brightness set --input "$root/codex-base.json" 41 \
    --output "$root/codex-candidate.json" >"$root/codex-candidate-receipt.json"
  "$BIN" --json codex agent-key snapshot --socket "$codex_socket" --token "$codex_token" \
    --output "$root/agent-base.json" >"$root/agent-base-receipt.json"
  "$BIN" --json codex agent-key set --input "$root/agent-base.json" AG01 \
    --command fixture.transaction --output "$root/agent-candidate.json" \
    >"$root/agent-candidate-receipt.json"

  "$BIN" --json transaction plan \
    --codex-settings-base "$root/codex-base.json" \
    --codex-settings-candidate "$root/codex-candidate.json" \
    --codex-agent-keys-base "$root/agent-base.json" \
    --codex-agent-keys-candidate "$root/agent-candidate.json" \
    --input-config-base "$root/input-base.json" \
    --input-config-candidate "$root/input-candidate.json" \
    --input-host-settings-base "$root/host-base.json" \
    --input-host-settings-candidate "$root/host-candidate.json" \
    --output "$root/plan.json" >"$root/plan-receipt.json"
  "$BIN" --json transaction show --input "$root/plan.json" >"$root/plan-show.json"

  set +e
  "$BIN" --json transaction apply --plan "$root/plan.json" \
    --backup-dir "$root/apply-backup" --receipt "$root/apply.json" \
    --idempotency-key "four-authority-$label-apply" \
    --input-socket "$input_socket" --input-token "$input_token" \
    --codex-socket "$codex_socket" --codex-token "$codex_token" \
    >"$root/apply-stdout.json" 2>"$root/apply.stderr"
  local apply_status=$?
  set -e

  "$BIN" --json device --transport bridge \
    --bridge-socket "$input_socket" --bridge-token "$input_token" config snapshot \
    --output "$root/input-post-apply.json" >"$root/input-post-apply-receipt.json"
  "$BIN" --json input --bridge-socket "$input_socket" --bridge-token "$input_token" \
    permission command snapshot --output "$root/host-post-apply.json" \
    >"$root/host-post-apply-receipt.json"
  "$BIN" --json codex config --socket "$codex_socket" --token "$codex_token" snapshot \
    --output "$root/codex-post-apply.json" >"$root/codex-post-apply-receipt.json"
  "$BIN" --json codex agent-key snapshot --socket "$codex_socket" --token "$codex_token" \
    --output "$root/agent-post-apply.json" >"$root/agent-post-apply-receipt.json"
  "$BIN" --json backup inspect --input "$root/apply.json" \
    >"$root/apply-inspection.json"
  "$BIN" --json backup inspect --input "$root/apply-backup" \
    >"$root/catalog-inspection.json"
  "$BIN" --json backup migration-plan --input "$root/apply.json" \
    >"$root/apply-migration.json"

  if [[ "$fail_settings_writes" == 0 ]]; then
    [[ "$apply_status" == 0 ]]
    "$BIN" --json transaction apply --plan "$root/plan.json" \
      --backup-dir "$root/apply-backup" --receipt "$root/apply.json" \
      --idempotency-key "four-authority-$label-apply" \
      --input-socket "$input_socket" --input-token "$input_token" \
      --codex-socket "$codex_socket" --codex-token "$codex_token" \
      >"$root/apply-retry.json"
    local host_candidate_revision
    host_candidate_revision=$(python3 -c \
      'import json,sys; print(json.load(open(sys.argv[1]))["revision"])' \
      "$root/host-candidate.json")
    local host_base_revision
    host_base_revision=$(python3 -c \
      'import json,sys; print(json.load(open(sys.argv[1]))["revision"])' \
      "$root/host-base.json")
    "$BIN" --json input --bridge-socket "$input_socket" --bridge-token "$input_token" \
      permission command restore --input "$root/host-base.json" \
      --backup "$root/retry-drift-host-target.json" \
      --expected-revision "$host_candidate_revision" \
      --idempotency-key "four-authority-$label-retry-drift" \
      >"$root/retry-drift.json"
    if "$BIN" --json transaction apply --plan "$root/plan.json" \
      --backup-dir "$root/apply-backup" --receipt "$root/apply.json" \
      --idempotency-key "four-authority-$label-apply" \
      --input-socket "$input_socket" --input-token "$input_token" \
      --codex-socket "$codex_socket" --codex-token "$codex_token" \
      >"$root/drifted-retry.stdout" 2>"$root/drifted-retry.stderr"; then
      echo "transaction retry accepted drifted live state" >&2
      return 1
    fi
    "$BIN" --json input --bridge-socket "$input_socket" --bridge-token "$input_token" \
      permission command apply --input "$root/host-candidate.json" \
      --backup "$root/retry-drift-host-base.json" \
      --expected-revision "$host_base_revision" \
      --idempotency-key "four-authority-$label-retry-reset" \
      >"$root/retry-reset.json"
    "$BIN" --json transaction restore --apply-receipt "$root/apply.json" \
      --backup-dir "$root/restore-backup" --receipt "$root/restore.json" \
      --idempotency-key "four-authority-$label-restore" \
      --input-socket "$input_socket" --input-token "$input_token" \
      --codex-socket "$codex_socket" --codex-token "$codex_token" \
      >"$root/restore-stdout.json"
    "$BIN" --json transaction restore --apply-receipt "$root/apply.json" \
      --backup-dir "$root/restore-backup" --receipt "$root/restore.json" \
      --idempotency-key "four-authority-$label-restore" \
      --input-socket "$input_socket" --input-token "$input_token" \
      --codex-socket "$codex_socket" --codex-token "$codex_token" \
      >"$root/restore-retry.json"
    "$BIN" --json device --transport bridge \
      --bridge-socket "$input_socket" --bridge-token "$input_token" config snapshot \
      --output "$root/input-restored.json" >"$root/input-restored-receipt.json"
    "$BIN" --json input --bridge-socket "$input_socket" --bridge-token "$input_token" \
      permission command snapshot --output "$root/host-restored.json" \
      >"$root/host-restored-receipt.json"
    "$BIN" --json codex config --socket "$codex_socket" --token "$codex_token" snapshot \
      --output "$root/codex-restored.json" >"$root/codex-restored-receipt.json"
    "$BIN" --json codex agent-key snapshot --socket "$codex_socket" --token "$codex_token" \
      --output "$root/agent-restored.json" >"$root/agent-restored-receipt.json"
    "$BIN" --json backup inspect --input "$root/restore.json" \
      >"$root/restore-inspection.json"
  else
    [[ "$apply_status" == 6 ]]
  fi

  python3 - "$root" "$fail_settings_writes" <<'PY'
import json
import pathlib
import stat
import sys

root = pathlib.Path(sys.argv[1])
failed = int(sys.argv[2]) > 0
load = lambda name: json.loads((root / name).read_text())
plan = load("plan.json")
apply = load("apply.json")
input_base = load("input-base.json")
input_candidate = load("input-candidate.json")
input_post = load("input-post-apply.json")
host_base = load("host-base.json")
host_candidate = load("host-candidate.json")
host_post = load("host-post-apply.json")
codex_base = load("codex-base.json")
codex_candidate = load("codex-candidate.json")
codex_post = load("codex-post-apply.json")
agent_base = load("agent-base.json")
agent_candidate = load("agent-candidate.json")
agent_post = load("agent-post-apply.json")
catalog = load("apply-backup/catalog.json")
apply_inspection = load("apply-inspection.json")
catalog_inspection = load("catalog-inspection.json")
apply_migration = load("apply-migration.json")

assert load("plan-show.json") == plan
assert len(plan["authorities"]) == 4
assert all(item["changed"] for item in plan["authorities"])
assert [item["id"] for item in plan["authorities"]] == [
    "codex-settings", "codex-agent-keys", "input-config", "input-host-settings",
]
assert catalog["operation"] == "apply"
assert catalog["planRevision"] == plan["revision"]
assert apply_inspection["artifactKind"] == "worklouderctl-cross-authority-transaction"
assert apply_inspection["valid"] is True
assert catalog_inspection["artifactKind"] == "worklouderctl-private-backup-catalog"
assert catalog_inspection["itemCount"] == 4
assert apply_migration["migration"]["migrationRequired"] is False
assert stat.S_IMODE((root / "apply-backup").stat().st_mode) == 0o700
for path in (root / "apply-backup").rglob("*.json"):
    assert stat.S_IMODE(path.stat().st_mode) == 0o600

if failed:
    assert apply["operation"] == "apply" and apply["status"] == "rolled-back"
    assert "injected Codex settings write failure" in apply["failure"]
    assert [item["id"] for item in apply["mutations"]] == [
        "input-host-settings", "input-config", "codex-agent-keys",
    ]
    assert [item["id"] for item in apply["rollbackMutations"]] == [
        "codex-agent-keys", "input-config", "input-host-settings",
    ]
    assert input_post["revision"] == input_base["revision"]
    assert host_post["revision"] == host_base["revision"]
    assert codex_post["settings"] == codex_base["settings"]
    assert agent_post["assignments"] == agent_base["assignments"]
else:
    assert apply["operation"] == "apply" and apply["status"] == "applied"
    assert load("apply-stdout.json") == apply == load("apply-retry.json")
    assert "differed during transaction postflight" in (root / "drifted-retry.stderr").read_text()
    assert [item["id"] for item in apply["mutations"]] == [
        "input-host-settings", "input-config", "codex-agent-keys", "codex-settings",
    ]
    assert input_post["revision"] == input_candidate["revision"]
    assert host_post["revision"] == host_candidate["revision"]
    assert codex_post["settings"] == codex_candidate["settings"]
    assert agent_post["assignments"] == agent_candidate["assignments"]
    restore = load("restore.json")
    restore_inspection = load("restore-inspection.json")
    assert restore["operation"] == "restore" and restore["status"] == "restored"
    assert restore_inspection["restoreAvailable"] is False
    assert load("restore-stdout.json") == restore == load("restore-retry.json")
    assert [item["id"] for item in restore["mutations"]] == [
        "codex-settings", "codex-agent-keys", "input-config", "input-host-settings",
    ]
    assert load("input-restored.json")["revision"] == input_base["revision"]
    assert load("host-restored.json")["revision"] == host_base["revision"]
    assert load("codex-restored.json")["settings"] == codex_base["settings"]
    assert load("agent-restored.json")["assignments"] == agent_base["assignments"]
PY

  stop_case_servers "$input_pid" "$codex_pid"
}

run_case success 0
run_case injected-failure 1

printf '%s\n' \
  'cross_authority_four_provider_plan=verified' \
  'cross_authority_apply_readback_restore=verified' \
  'cross_authority_idempotent_retry=verified' \
  'cross_authority_retry_drift_rejected=verified' \
  'cross_authority_private_catalog_permissions=verified' \
  'cross_authority_backup_inspection=verified' \
  'cross_authority_migration_plan=verified' \
  'cross_authority_failure_auto_rollback=verified'
