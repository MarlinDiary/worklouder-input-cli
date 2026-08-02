#!/bin/sh
set -eu

repo=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
root=$(mktemp -d /tmp/wlb-recovery-e2e.XXXXXX)
socket=$root/bridge.sock
token=$root/bridge.token
server_log=$root/server.log
server_pid=

cleanup() {
  if [ -n "$server_pid" ] && kill -0 "$server_pid" 2>/dev/null; then
    kill -TERM "$server_pid"
    wait "$server_pid"
  fi
  rm -rf "$root"
}
trap cleanup EXIT INT TERM

cargo build --locked --manifest-path "$repo/Cargo.toml"
WORKLOUDERCTL_FIXTURE_RECOVERY=1 \
  node "$repo/companion/fixture-server.mjs" "$socket" "$token" \
  >"$server_log" 2>&1 &
server_pid=$!
attempt=0
while [ ! -S "$socket" ] || [ ! -f "$token" ]; do
  attempt=$((attempt + 1))
  if [ "$attempt" -ge 100 ]; then
    cat "$server_log" >&2
    echo "recovery fixture did not start" >&2
    exit 1
  fi
  sleep 0.05
done

bin=$repo/target/debug/worklouderctl
node "$repo/companion/conformance.mjs" \
  --socket "$socket" --token "$token" \
  --require input.recovery.plan.v1 \
  --require input.recovery.apply.v1 \
  --require device.config.apply.v1 >"$root/conformance.json"

"$bin" --json device --transport bridge --bridge-socket "$socket" \
  --bridge-token "$token" config snapshot --output "$root/before.json" \
  >"$root/before-stdout.json"
"$bin" --json input --bridge-socket "$socket" --bridge-token "$token" \
  recovery plan --backup "$root/before.json" --plan "$root/plan.json" \
  >"$root/plan-stdout.json"
"$bin" --json backup inspect --input "$root/plan.json" \
  >"$root/plan-inspection.json"

"$bin" --json input --bridge-socket "$socket" --bridge-token "$token" \
  recovery apply --plan "$root/plan.json" --backup "$root/before.json" \
  --receipt "$root/apply.json" --idempotency-key recovery-e2e-1 \
  >"$root/apply-stdout.json"
"$bin" --json input --bridge-socket "$socket" --bridge-token "$token" \
  recovery apply --plan "$root/plan.json" --backup "$root/before.json" \
  --receipt "$root/replay.json" --idempotency-key recovery-e2e-1 \
  >"$root/replay-stdout.json"
"$bin" --json backup inspect --input "$root/apply.json" \
  >"$root/apply-inspection.json"
"$bin" --json device --transport bridge --bridge-socket "$socket" \
  --bridge-token "$token" config snapshot --output "$root/after.json" \
  >"$root/after-stdout.json"

python3 - "$root" <<'PY'
import json
from pathlib import Path
import sys

root = Path(sys.argv[1])
load = lambda name: json.loads((root / name).read_text())
before = load("before.json")
plan = load("plan.json")
plan_stdout = load("plan-stdout.json")
apply = load("apply.json")
apply_stdout = load("apply-stdout.json")
replay = load("replay.json")
after = load("after.json")
plan_inspection = load("plan-inspection.json")
apply_inspection = load("apply-inspection.json")

assert plan_stdout["revision"] == plan["revision"]
assert plan["ready"] is True and plan["blockers"] == []
assert plan["bootloader"]["transport"] == "usb-bootloader"
assert plan["configurationRevision"] == before["revision"]
assert apply == apply_stdout
assert apply["kind"] == "worklouderctl-input-recovery-receipt"
assert apply["targetFirmwareVersion"] == "v0.8.0-recovery-fixture"
assert apply["afterFirmwareVersion"] == apply["targetFirmwareVersion"]
assert apply["beforeConfigRevision"] == before["revision"]
assert apply["recoveredConfigRevision"] != before["revision"]
assert apply["afterConfigRevision"] == before["revision"]
assert apply["configurationRestored"] is True
assert apply["recoveryRequired"] is False
assert replay["idempotentReplay"] is True
assert after["revision"] == before["revision"]
assert after["status"]["firmwareVersion"] == apply["targetFirmwareVersion"]
assert plan_inspection["artifactKind"] == plan["kind"]
assert apply_inspection["artifactKind"] == apply["kind"]
assert apply_inspection["restoreAvailable"] is True
print("recovery_input_bootloader_authority=verified")
print("recovery_plan_backup_binding=verified")
print("recovery_input_programmer_delegation=verified")
print("recovery_exact_configuration_restore=verified")
print("recovery_idempotent_replay=verified")
print("recovery_receipt_readback=verified")
PY
