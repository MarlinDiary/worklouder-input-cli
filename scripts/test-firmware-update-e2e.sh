#!/bin/sh
set -eu

repo=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
root=$(mktemp -d /tmp/wlb-firmware-e2e.XXXXXX)
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
WORKLOUDERCTL_FIXTURE_USB=1 node "$repo/companion/fixture-server.mjs" \
  "$socket" "$token" >"$server_log" 2>&1 &
server_pid=$!

attempt=0
while [ ! -S "$socket" ] || [ ! -f "$token" ]; do
  attempt=$((attempt + 1))
  if [ "$attempt" -ge 100 ]; then
    cat "$server_log" >&2
    echo "firmware fixture did not start" >&2
    exit 1
  fi
  sleep 0.05
done

bin=$repo/target/debug/worklouderctl
node "$repo/companion/conformance.mjs" \
  --socket "$socket" --token "$token" \
  --require input.firmware.status.v1 \
  --require input.firmware.plan.v1 \
  --require input.firmware.update.v1 \
  >"$root/conformance.json"

"$bin" --json input --bridge-socket "$socket" --bridge-token "$token" \
  firmware plan --device fixture-device --output "$root/plan.json" \
  >"$root/plan-receipt.json"
config_revision=$(python3 -c \
  'import json,sys; print(json.load(open(sys.argv[1]))["configRevision"])' \
  "$root/plan.json")

"$bin" --json input --bridge-socket "$socket" --bridge-token "$token" \
  firmware update --plan "$root/plan.json" --backup "$root/backup.json" \
  --receipt "$root/update.json" --expected-revision "$config_revision" \
  --idempotency-key firmware-e2e-1 >"$root/update-stdout.json"
"$bin" --json backup inspect --input "$root/update.json" \
  >"$root/update-inspection.json"

"$bin" --json input --bridge-socket "$socket" --bridge-token "$token" \
  firmware update --plan "$root/plan.json" --backup "$root/backup.json" \
  --receipt "$root/replay.json" --expected-revision "$config_revision" \
  --idempotency-key firmware-e2e-1 >"$root/replay-stdout.json"
"$bin" --json input --bridge-socket "$socket" --bridge-token "$token" \
  firmware check --device fixture-device >"$root/postflight.json"

python3 - "$root" <<'PY'
import json
from pathlib import Path
import sys

root = Path(sys.argv[1])
load = lambda name: json.loads((root / name).read_text())
plan = load("plan.json")
receipt = load("update.json")
stdout = load("update-stdout.json")
replay = load("replay.json")
inspection = load("update-inspection.json")
postflight = load("postflight.json")

assert plan["ready"] is True and plan["blockers"] == []
assert plan["device"]["isUsbConnection"] is True
assert receipt == stdout
assert receipt["kind"] == "worklouderctl-input-firmware-update-receipt"
assert receipt["planRevision"] == plan["revision"]
assert receipt["beforeFirmwareVersion"] == "v0.6.0-fixture"
assert receipt["afterFirmwareVersion"] == "v0.7.0-fixture"
assert receipt["targetFirmwareVersion"] == "v0.7.0-fixture"
assert receipt["beforeConfigRevision"] == plan["configRevision"]
assert receipt["afterConfigRevision"] == plan["configRevision"]
assert receipt["configurationRestored"] is True
assert receipt["recoveryRequired"] is False
assert [p["name"] for p in receipt["phases"]] == plan["phases"]
assert all(p["status"] == "completed" for p in receipt["phases"])
assert replay["idempotentReplay"] is True
assert replay["planRevision"] == receipt["planRevision"]
assert inspection["artifactKind"] == receipt["kind"]
assert inspection["migration"]["migrationRequired"] is False
assert postflight["status"]["firmwareVersion"] == "v0.7.0-fixture"
assert postflight["update"]["updateAvailable"] is False
assert postflight["update"]["release"] is None
print("firmware_plan_cas=verified")
print("firmware_complete_backup=verified")
print("firmware_input_delegation=verified")
print("firmware_config_postflight=verified")
print("firmware_idempotent_replay=verified")
print("firmware_receipt_readback=verified")
PY
