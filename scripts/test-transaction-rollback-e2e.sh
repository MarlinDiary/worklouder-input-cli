#!/bin/sh
set -eu

repo=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
root=$(mktemp -d /tmp/wlb-transaction-rollback.XXXXXX)
socket=$root/bridge.sock
token=$root/bridge.token
server_log=$root/server.log
server_pid=

cleanup() {
  if [ -n "$server_pid" ] && kill -0 "$server_pid" 2>/dev/null; then
    kill -TERM "$server_pid"
    wait "$server_pid"
  fi
}
trap cleanup EXIT INT TERM

cargo build --locked --manifest-path "$repo/Cargo.toml"
WORKLOUDERCTL_FIXTURE_CONFIG_WRITE_FAILURE=after-once \
  node "$repo/companion/fixture-server.mjs" "$socket" "$token" \
  >"$server_log" 2>&1 &
server_pid=$!
attempt=0
while [ ! -S "$socket" ] || [ ! -f "$token" ]; do
  attempt=$((attempt + 1))
  if [ "$attempt" -ge 100 ]; then
    cat "$server_log" >&2
    echo "bridge fixture did not start" >&2
    exit 1
  fi
  sleep 0.05
done

bin=$repo/target/debug/worklouderctl
"$bin" --json device --transport bridge \
  --bridge-socket "$socket" --bridge-token "$token" config snapshot \
  --output "$root/config-baseline.json" >"$root/config-baseline-receipt.json"
"$bin" --json input --bridge-socket "$socket" --bridge-token "$token" \
  permission command snapshot --output "$root/host-baseline.json" \
  >"$root/host-baseline-receipt.json"
"$bin" --json layer color --input "$root/config-baseline.json" \
  --profile 0 --id 1 --color '#A1B2C3' \
  --output "$root/config-candidate.json" >"$root/config-candidate-receipt.json"
"$bin" --json input permission command set --input "$root/host-baseline.json" \
  enabled --output "$root/host-candidate.json" >"$root/host-candidate-receipt.json"
"$bin" --json transaction plan \
  --input-config-base "$root/config-baseline.json" \
  --input-config-candidate "$root/config-candidate.json" \
  --input-host-settings-base "$root/host-baseline.json" \
  --input-host-settings-candidate "$root/host-candidate.json" \
  --output "$root/plan.json" >"$root/plan-receipt.json"

set +e
"$bin" --json transaction apply \
  --plan "$root/plan.json" --backup-dir "$root/apply-backup" \
  --receipt "$root/apply-receipt.json" \
  --idempotency-key transaction-injected-post-write \
  --input-socket "$socket" --input-token "$token" \
  >"$root/apply.stdout" 2>"$root/apply.stderr"
apply_status=$?
set -e
if [ "$apply_status" -ne 6 ]; then
  echo "injected transaction failure returned status $apply_status, expected 6" >&2
  exit 1
fi

"$bin" --json device --transport bridge \
  --bridge-socket "$socket" --bridge-token "$token" config snapshot \
  --output "$root/config-after.json" >"$root/config-after-receipt.json"
"$bin" --json input --bridge-socket "$socket" --bridge-token "$token" \
  permission command snapshot --output "$root/host-after.json" \
  >"$root/host-after-receipt.json"

python3 - "$root" <<'PY'
import json
import pathlib
import stat
import sys

root = pathlib.Path(sys.argv[1])
load = lambda name: json.loads((root / name).read_text())
config_before = load("config-baseline.json")
host_before = load("host-baseline.json")
config_after = load("config-after.json")
host_after = load("host-after.json")
receipt = load("apply-receipt.json")
catalog = load("apply-backup/catalog.json")

assert receipt["operation"] == "apply"
assert receipt["status"] == "rolled-back"
assert "injected fixture post-write failure" in receipt["failure"]
assert '"rollbackPerformed":true' in receipt["failure"]
assert [item["id"] for item in receipt["mutations"]] == ["input-host-settings"]
assert [item["id"] for item in receipt["rollbackMutations"]] == [
    "input-host-settings",
]
assert config_after["revision"] == config_before["revision"]
assert host_after["revision"] == host_before["revision"]
assert catalog["operation"] == "apply"
assert stat.S_IMODE((root / "apply-backup").stat().st_mode) == 0o700
for path in (root / "apply-backup").rglob("*"):
    mode = stat.S_IMODE(path.stat().st_mode)
    assert mode == (0o700 if path.is_dir() else 0o600), (path, oct(mode))
assert "status rolled-back" in (root / "apply.stderr").read_text()
print("transaction_post_write_failure=observed")
print("transaction_provider_local_rollback=verified")
print("transaction_automatic_rollback=verified")
print("transaction_private_catalog_modes=verified")
PY
