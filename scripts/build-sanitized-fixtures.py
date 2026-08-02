#!/usr/bin/env python3
"""Build deterministic, identity-free Input and firmware fixtures."""

import argparse
import base64
import hashlib
import json
import struct
from pathlib import Path


README = """# Sanitized compatibility fixtures

These fixtures are deterministic, synthetic minimum examples for adapter and
schema regression. They contain no exported user configuration, device serial,
account, application identity, log, credential, or firmware binary.

- `input/0.17.3/codex-micro-v0.6.0` freezes the legacy minimum keymap family.
- `input/0.18.0/codex-micro-v0.6.0` adds the separate Smart Action file and
  current layer-lighting shape.
- `firmware/codex-micro-v0.6.0` freezes a sanitized status DTO only.

Every directory has a manifest of exact sizes and SHA-256 digests. Regenerate
and verify the complete tree with:

```sh
./scripts/verify-sanitized-fixtures.sh
```

The 0.17.3 fixture is a structural compatibility boundary because its original
application bundle was superseded during research. The 0.18.0 read contract
has separate hash-pinned installed-package and live-device evidence in
`spec/` and `docs/research/`; this synthetic tree deliberately does not copy
the observed device configuration.
"""


def json_bytes(value):
    return (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode()


def config_revision(files):
    digest = hashlib.sha256(b"worklouder-input-config-revision-v1\0")
    for relative_path, content in sorted(files.items()):
        path_bytes = relative_path.encode()
        digest.update(struct.pack(">I", len(path_bytes)))
        digest.update(path_bytes)
        digest.update(struct.pack(">Q", len(content)))
        digest.update(content)
    return digest.hexdigest()


def snapshot(input_version, files):
    records = []
    for relative_path, content in sorted(files.items()):
        records.append(
            {
                "relativePath": relative_path,
                "size": len(content),
                "deviceChecksumSha1": hashlib.sha1(content).hexdigest(),
                "sha256": hashlib.sha256(content).hexdigest(),
                "dataBase64": base64.b64encode(content).decode(),
            }
        )
    return {
        "schemaVersion": 1,
        "kind": "worklouder-input-config-snapshot",
        "revisionAlgorithm": "sha256:path-u32be-path-bytes-size-u64be-content-v1",
        "revision": config_revision(files),
        "deviceId": f"sanitized-codex-micro-{input_version}",
        "files": records,
    }


def keymap(input_version):
    layer = {
        "id": 0,
        "name": "Sanitized Base",
        "color": 15595263,
        "layout": {
            "keymap": [["KC_A", "KC_B", "KC_C", "KC_D"]],
            "encoders": [],
            "joystick": {"type": "VENDOR", "sectors": []},
        },
    }
    if input_version == "0.18.0":
        layer["lights"] = {
            "backlight": {
                "effect": "solid",
                "brightness": 1.0,
                "speed": 0.5,
                "magic": 0.0,
                "color": 15595263,
            },
            "underglow": {
                "effect": "off",
                "brightness": 0.0,
                "speed": 0.5,
                "magic": 0.0,
                "color": 15595263,
            },
        }
    return {
        "version": 1,
        "activeProfileId": 0,
        "linkedApps": [],
        "macros": [],
        "macrosGroups": [],
        "multiActions": [],
        "multiActionsGroups": [],
        "profiles": [
            {
                "id": 0,
                "name": "Sanitized Profile",
                "layers": [layer],
            }
        ],
    }


def write(path, content):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content)


def file_record(root, path):
    content = path.read_bytes()
    return {
        "relativePath": path.relative_to(root).as_posix(),
        "size": len(content),
        "sha256": hashlib.sha256(content).hexdigest(),
    }


def build_input_fixture(root, input_version):
    fixture = root / "input" / input_version / "codex-micro-v0.6.0"
    files = {"keymap.json": json_bytes(keymap(input_version))}
    if input_version == "0.18.0":
        files["smart_actions.json"] = json_bytes(
            {"version": 1, "smartActions": {}, "smartActionGroups": []}
        )
    for relative_path, content in files.items():
        write(fixture / "device-files" / relative_path, content)
    write(fixture / "config-snapshot.json", json_bytes(snapshot(input_version, files)))
    tracked = sorted((fixture / "device-files").glob("*.json")) + [
        fixture / "config-snapshot.json"
    ]
    manifest = {
        "schemaVersion": 1,
        "kind": "worklouderctl-sanitized-input-fixture",
        "fixtureClass": "synthetic-structural",
        "inputVersion": input_version,
        "firmwareVersion": "v0.6.0",
        "deviceType": "codex_micro",
        "containsUserConfiguration": False,
        "claim": (
            "deterministic minimum schema fixture; it is not an export of a user device"
        ),
        "files": [file_record(fixture, path) for path in tracked],
    }
    write(fixture / "manifest.json", json_bytes(manifest))


def build_firmware_fixture(root):
    fixture = root / "firmware" / "codex-micro-v0.6.0"
    status = {
        "schemaVersion": 1,
        "kind": "worklouderctl-device-status",
        "adapter": "sanitized-fixture-v1",
        "inputAppVersion": "0.18.0",
        "deviceKitVersion": "0.1.29",
        "device": {
            "devicePid": "SANITIZED_PID",
            "deviceType": "codex_micro",
            "layoutType": "universal",
            "connectionType": "hid",
            "isUsbConnection": False,
        },
        "status": {
            "firmwareVersion": "v0.6.0",
            "selectedProfileIndex": 0,
            "selectedLayerIndex": 1,
            "batteryPercentage": 100,
            "isCharging": False,
        },
        "warnings": ["synthetic status fixture; no device identity or live power state"],
    }
    status_path = fixture / "status.json"
    write(status_path, json_bytes(status))
    manifest = {
        "schemaVersion": 1,
        "kind": "worklouderctl-sanitized-firmware-fixture",
        "fixtureClass": "synthetic-status",
        "firmwareVersion": "v0.6.0",
        "containsFirmwareBinary": False,
        "containsDeviceIdentity": False,
        "files": [file_record(fixture, status_path)],
    }
    write(fixture / "manifest.json", json_bytes(manifest))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        raise SystemExit(f"destination already exists: {args.output}")
    write(args.output / "README.md", README.encode())
    build_input_fixture(args.output, "0.17.3")
    build_input_fixture(args.output, "0.18.0")
    build_firmware_fixture(args.output)


if __name__ == "__main__":
    main()
