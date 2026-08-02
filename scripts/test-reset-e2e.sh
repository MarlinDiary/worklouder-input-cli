#!/bin/sh
set -eu

repo=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
root=$(mktemp -d /tmp/wlb-reset-e2e.XXXXXX)
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
node "$repo/companion/fixture-server.mjs" "$socket" "$token" \
  >"$server_log" 2>&1 &
server_pid=$!
attempt=0
while [ ! -S "$socket" ] || [ ! -f "$token" ]; do
  attempt=$((attempt + 1))
  if [ "$attempt" -ge 100 ]; then
    cat "$server_log" >&2
    echo "reset fixture did not start" >&2
    exit 1
  fi
  sleep 0.05
done

bin=$repo/target/debug/worklouderctl
node "$repo/companion/conformance.mjs" \
  --socket "$socket" --token "$token" \
  --require input.reset.plan.v1 \
  --require device.config.apply.v1 \
  --require device.config.restore.v1 >"$root/conformance.json"

"$bin" --json input --bridge-socket "$socket" --bridge-token "$token" \
  reset plan --device fixture-device --plan "$root/plan.json" \
  --candidate "$root/candidate.json" >"$root/plan-stdout.json"
"$bin" --json backup inspect --input "$root/plan.json" \
  >"$root/plan-inspection.json"
source_revision=$(python3 -c \
  'import json,sys; print(json.load(open(sys.argv[1]))["sourceRevision"])' \
  "$root/plan.json")
candidate_revision=$(python3 -c \
  'import json,sys; print(json.load(open(sys.argv[1]))["candidateRevision"])' \
  "$root/plan.json")

"$bin" --json input --bridge-socket "$socket" --bridge-token "$token" \
  reset apply --plan "$root/plan.json" --candidate "$root/candidate.json" \
  --backup "$root/source.json" --receipt "$root/apply.json" \
  --expected-revision "$source_revision" --idempotency-key reset-e2e-1 \
  >"$root/apply-stdout.json"
"$bin" --json input --bridge-socket "$socket" --bridge-token "$token" \
  reset apply --plan "$root/plan.json" --candidate "$root/candidate.json" \
  --backup "$root/source.json" --receipt "$root/replay.json" \
  --expected-revision "$source_revision" --idempotency-key reset-e2e-1 \
  >"$root/replay-stdout.json"
"$bin" --json backup inspect --input "$root/apply.json" \
  >"$root/apply-inspection.json"
"$bin" --json device --transport bridge --bridge-socket "$socket" \
  --bridge-token "$token" config snapshot --output "$root/post-reset.json" \
  >"$root/post-reset-stdout.json"

"$bin" --json device --transport bridge --bridge-socket "$socket" \
  --bridge-token "$token" config restore --input "$root/source.json" \
  --backup "$root/pre-restore.json" --expected-revision "$candidate_revision" \
  --idempotency-key reset-rollback-1 >"$root/restore.json"
"$bin" --json device --transport bridge --bridge-socket "$socket" \
  --bridge-token "$token" config snapshot --output "$root/restored.json" \
  >"$root/restored-stdout.json"

python3 - "$root" <<'PY'
import json
from pathlib import Path
import sys

root = Path(sys.argv[1])
load = lambda name: json.loads((root / name).read_text())
plan = load("plan.json")
plan_stdout = load("plan-stdout.json")
candidate = load("candidate.json")
source = load("source.json")
apply = load("apply.json")
apply_stdout = load("apply-stdout.json")
replay = load("replay.json")
post_reset = load("post-reset.json")
restored = load("restored.json")
plan_inspection = load("plan-inspection.json")
apply_inspection = load("apply-inspection.json")

assert plan_stdout["revision"] == plan["revision"]
assert plan["strategy"] == "input-default-layout"
assert plan["device"]["deviceType"] == "codex_micro"
assert plan["device"]["layoutType"] == "universal"
assert plan["candidateRevision"] == candidate["revision"]
assert plan["sourceRevision"] != plan["candidateRevision"]
assert apply == apply_stdout
assert apply["kind"] == "worklouderctl-input-reset-receipt"
assert apply["sourceRevision"] == source["revision"]
assert apply["candidateRevision"] == candidate["revision"]
assert apply["changed"] is True and apply["rollbackPerformed"] is False
assert replay["idempotentReplay"] is True
assert post_reset["revision"] == candidate["revision"]
assert restored["revision"] == source["revision"]
assert plan_inspection["artifactKind"] == plan["kind"]
assert apply_inspection["artifactKind"] == apply["kind"]
print("reset_input_default_authority=verified")
print("reset_plan_candidate_cas=verified")
print("reset_existing_transaction=verified")
print("reset_idempotent_replay=verified")
print("reset_exact_rollback=verified")
print("reset_receipt_readback=verified")
PY
