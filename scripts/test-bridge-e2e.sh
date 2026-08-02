#!/bin/sh
set -eu

repo=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
root=$(mktemp -d /tmp/wlb-e2e.XXXXXX)
socket=$root/bridge.sock
token=$root/bridge.token
export_dir=$root/export
config_snapshot=$root/config-snapshot.json
candidate_snapshot=$root/config-candidate.json
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
  --require device.config.snapshot.v1 \
  --require device.config.validate.v1 \
  --require device.config.apply.v1 \
  --require device.config.restore.v1 \
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
"$bin" --json device --transport bridge \
  --bridge-socket "$socket" --bridge-token "$token" config snapshot \
  --output "$config_snapshot" >"$root/config-snapshot-receipt.json"
revision=$(python3 -c \
  'import json,sys; print(json.load(open(sys.argv[1]))["revision"])' \
  "$config_snapshot")
"$bin" --json device --transport bridge \
  --bridge-socket "$socket" --bridge-token "$token" config validate \
  --input "$config_snapshot" --expected-revision "$revision" \
  >"$root/config-bridge-validation.json"
python3 - "$config_snapshot" "$candidate_snapshot" <<'PY'
import base64
import hashlib
import json
import struct
import sys

source, destination = sys.argv[1:]
snapshot = json.load(open(source))
for record in snapshot["files"]:
    data = base64.b64decode(record["dataBase64"], validate=True)
    if record["relativePath"] == "keymap.json":
        value = json.loads(data)
        value["bridgeMutation"] = "e2e"
        data = json.dumps(value, separators=(",", ":")).encode()
    record["size"] = len(data)
    record["deviceChecksumSha1"] = hashlib.sha1(data).hexdigest()
    record["sha256"] = hashlib.sha256(data).hexdigest()
    record["dataBase64"] = base64.b64encode(data).decode()
h = hashlib.sha256(b"worklouder-input-config-revision-v1\0")
for record in sorted(snapshot["files"], key=lambda item: item["relativePath"].encode()):
    path = record["relativePath"].encode()
    data = base64.b64decode(record["dataBase64"], validate=True)
    h.update(struct.pack(">I", len(path)))
    h.update(path)
    h.update(struct.pack(">Q", len(data)))
    h.update(data)
snapshot["revision"] = h.hexdigest()
with open(destination, "w") as output:
    json.dump(snapshot, output, separators=(",", ":"))
    output.write("\n")
PY
candidate_revision=$(python3 -c \
  'import json,sys; print(json.load(open(sys.argv[1]))["revision"])' \
  "$candidate_snapshot")
"$bin" --json device --transport bridge \
  --bridge-socket "$socket" --bridge-token "$token" config apply \
  --input "$candidate_snapshot" --backup "$root/pre-apply.json" \
  --expected-revision "$revision" --idempotency-key e2e-apply-1 \
  >"$root/config-apply.json"
"$bin" --json device --transport bridge \
  --bridge-socket "$socket" --bridge-token "$token" config apply \
  --input "$candidate_snapshot" --backup "$root/pre-apply.json" \
  --expected-revision "$revision" --idempotency-key e2e-apply-1 \
  >"$root/config-apply-replay.json"
set +e
"$bin" --json device --transport bridge \
  --bridge-socket "$socket" --bridge-token "$token" config apply \
  --input "$candidate_snapshot" --backup "$root/stale-attempt-backup.json" \
  --expected-revision "$revision" --idempotency-key e2e-apply-stale \
  >"$root/config-apply-stale.json" 2>"$root/config-apply-stale.err"
stale_status=$?
set -e
[ "$stale_status" -ne 0 ]
printf '%s\n' "$stale_status" >"$root/config-apply-stale.status"
"$bin" --json device --transport bridge \
  --bridge-socket "$socket" --bridge-token "$token" config snapshot \
  --output "$root/post-apply.json" >"$root/post-apply-receipt.json"
"$bin" --json device --transport bridge \
  --bridge-socket "$socket" --bridge-token "$token" config restore \
  --input "$config_snapshot" --backup "$root/pre-restore.json" \
  --expected-revision "$candidate_revision" --idempotency-key e2e-restore-1 \
  >"$root/config-restore.json"
"$bin" --json device --transport bridge \
  --bridge-socket "$socket" --bridge-token "$token" config snapshot \
  --output "$root/post-restore.json" >"$root/post-restore-receipt.json"

python3 - "$root" <<'PY'
import hashlib
import base64
import json
import pathlib
import struct
import sys

root = pathlib.Path(sys.argv[1])
conformance = json.loads((root / "node-conformance.json").read_text())
bridge = json.loads((root / "bridge-status.json").read_text())
status = json.loads((root / "device-status.json").read_text())
files = json.loads((root / "device-files.json").read_text())
manifest = json.loads((root / "export" / "manifest.json").read_text())
validation = json.loads((root / "export-validation.json").read_text())
snapshot = json.loads((root / "config-snapshot.json").read_text())
snapshot_receipt = json.loads(
    (root / "config-snapshot-receipt.json").read_text()
)
bridge_validation = json.loads(
    (root / "config-bridge-validation.json").read_text()
)
candidate = json.loads((root / "config-candidate.json").read_text())
apply = json.loads((root / "config-apply.json").read_text())
replay = json.loads((root / "config-apply-replay.json").read_text())
pre_apply = json.loads((root / "pre-apply.json").read_text())
post_apply = json.loads((root / "post-apply.json").read_text())
pre_restore = json.loads((root / "pre-restore.json").read_text())
restore = json.loads((root / "config-restore.json").read_text())
post_restore = json.loads((root / "post-restore.json").read_text())

assert conformance["conformant"] is True
assert conformance["protocolVersion"] == 1
assert conformance["sessionId"] == bridge["sessionId"]
assert bridge["protocolVersion"] == 1
assert bridge["inputVersion"] == "0.18.0-fixture"
assert "device.files.read.v1" in bridge["capabilities"]
assert "device.config.snapshot.v1" in bridge["capabilities"]
assert "device.config.validate.v1" in bridge["capabilities"]
assert "device.config.apply.v1" in bridge["capabilities"]
assert "device.config.restore.v1" in bridge["capabilities"]
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

assert snapshot["schemaVersion"] == 1
assert snapshot["kind"] == "worklouder-input-config-snapshot"
assert snapshot["deviceId"] == "fixture-device"
revision_hash = hashlib.sha256()
revision_hash.update(b"worklouder-input-config-revision-v1\0")
for record in sorted(snapshot["files"], key=lambda item: item["relativePath"].encode()):
    path = record["relativePath"].encode()
    data = base64.b64decode(record["dataBase64"], validate=True)
    assert len(data) == record["size"]
    assert hashlib.sha1(data).hexdigest() == record["deviceChecksumSha1"]
    assert hashlib.sha256(data).hexdigest() == record["sha256"]
    revision_hash.update(struct.pack(">I", len(path)))
    revision_hash.update(path)
    revision_hash.update(struct.pack(">Q", len(data)))
    revision_hash.update(data)
assert revision_hash.hexdigest() == snapshot["revision"]
assert snapshot_receipt["revision"] == snapshot["revision"]
assert snapshot_receipt["fileCount"] == 2
assert bridge_validation["valid"] is True
assert bridge_validation["revision"] == snapshot["revision"]
assert bridge_validation["liveRevision"] == snapshot["revision"]
assert candidate["revision"] != snapshot["revision"]
assert pre_apply["revision"] == snapshot["revision"]
assert apply["operation"] == "apply"
assert apply["changed"] is True
assert apply["idempotentReplay"] is False
assert apply["beforeRevision"] == snapshot["revision"]
assert apply["afterRevision"] == candidate["revision"]
assert replay["idempotentReplay"] is True
assert replay["afterRevision"] == candidate["revision"]
assert post_apply["revision"] == candidate["revision"]
assert (root / "config-apply-stale.status").read_text().strip() != "0"
assert "revision conflict" in (root / "config-apply-stale.err").read_text()
assert pre_restore["revision"] == candidate["revision"]
assert restore["operation"] == "restore"
assert restore["changed"] is True
assert restore["beforeRevision"] == candidate["revision"]
assert restore["afterRevision"] == snapshot["revision"]
assert post_restore["revision"] == snapshot["revision"]

print("bridge_protocol=1")
print("node_conformance=verified")
print("bridge_transport=input-owned-session")
print("status_profile_layer=0/2")
print("exported_files=2")
print("sha1_sha256_readback=verified")
print("config_validation=verified")
print("config_snapshot_revision=verified")
print("config_live_cas=verified")
print("config_apply_readback=verified")
print("config_idempotent_replay=verified")
print("config_stale_cas_rejected=verified")
print("config_restore_readback=verified")
PY
