#!/bin/sh
set -eu

repo=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
root=$(mktemp -d /tmp/wlb-e2e.XXXXXX)
socket=$root/bridge.sock
token=$root/bridge.token
export_dir=$root/export
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
node "$repo/companion/conformance.mjs" \
  --socket "$socket" --token "$token" \
  --require device.status.v1 \
  --require device.files.list.v1 \
  --require device.files.read.v1 \
  >"$root/node-conformance.json"
"$bin" --json bridge --socket "$socket" --token "$token" status \
  >"$root/bridge-status.json"
"$bin" --json device --transport bridge \
  --bridge-socket "$socket" --bridge-token "$token" status \
  >"$root/device-status.json"
"$bin" --json device --transport bridge \
  --bridge-socket "$socket" --bridge-token "$token" files --recursive \
  >"$root/device-files.json"
"$bin" --json device --transport bridge \
  --bridge-socket "$socket" --bridge-token "$token" export \
  --output "$export_dir" >"$root/device-export.json"
"$bin" --json config validate "$export_dir" \
  >"$root/export-validation.json"

python3 - "$root" <<'PY'
import hashlib
import json
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
conformance = json.loads((root / "node-conformance.json").read_text())
bridge = json.loads((root / "bridge-status.json").read_text())
status = json.loads((root / "device-status.json").read_text())
files = json.loads((root / "device-files.json").read_text())
manifest = json.loads((root / "export" / "manifest.json").read_text())
validation = json.loads((root / "export-validation.json").read_text())

assert conformance["conformant"] is True
assert conformance["protocolVersion"] == 1
assert conformance["sessionId"] == bridge["sessionId"]
assert bridge["protocolVersion"] == 1
assert bridge["inputVersion"] == "0.18.0-fixture"
assert "device.files.read.v1" in bridge["capabilities"]
assert status["adapter"] == "input-companion-bridge-v1"
assert status["status"]["selectedLayerIndex"] == 2
assert len(files["files"]) == 2
assert manifest["adapter"] == "input-companion-bridge-v1"
assert validation["valid"] is True
for record in manifest["files"]:
    path = root / "export" / record["relativePath"]
    data = path.read_bytes()
    assert len(data) == record["size"]
    assert hashlib.sha1(data).hexdigest() == record["deviceChecksumSha1"]
    assert hashlib.sha256(data).hexdigest() == record["sha256"]

print("bridge_protocol=1")
print("node_conformance=verified")
print("bridge_transport=input-owned-session")
print("status_profile_layer=0/2")
print("exported_files=2")
print("sha1_sha256_readback=verified")
print("config_validation=verified")
PY
