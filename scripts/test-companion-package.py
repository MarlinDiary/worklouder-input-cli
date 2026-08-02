#!/usr/bin/env python3
"""Build, install, reopen, and verify the deterministic Companion integration kit."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import tempfile


EXPECTED_FILES = {
    "package/LICENSE": 0o644,
    "package/README.md": 0o644,
    "package/codex-main-adapter.mjs": 0o644,
    "package/codex-main-bridge.mjs": 0o644,
    "package/codex-main-integration.mjs": 0o644,
    "package/conformance.mjs": 0o755,
    "package/index.mjs": 0o644,
    "package/input-main-adapter.mjs": 0o644,
    "package/input-main-bridge.mjs": 0o644,
    "package/input-main-integration.mjs": 0o644,
    "package/package.json": 0o644,
}

EXPECTED_EXPORTS = {
    "BRIDGE_PROTOCOL_VERSION",
    "BridgeError",
    "CODEX_AGENT_KEYS_MUTATION_KIND",
    "CODEX_AGENT_KEYS_SNAPSHOT_KIND",
    "CODEX_AGENT_KEYS_STATE_KEY",
    "CODEX_AGENT_KEY_SLOTS",
    "CODEX_BRIDGE_PROTOCOL_VERSION",
    "CODEX_SETTINGS_REVISION_ALGORITHM",
    "CODEX_SETTINGS_SNAPSHOT_KIND",
    "CODEX_SETTINGS_SNAPSHOT_SCHEMA_VERSION",
    "CONFIG_REVISION_ALGORITHM",
    "CONFIG_SNAPSHOT_KIND",
    "CONFIG_SNAPSHOT_SCHEMA_VERSION",
    "CodexBridgeError",
    "HOST_SETTINGS_KIND",
    "HOST_SETTINGS_REVISION_ALGORITHM",
    "HOST_SETTINGS_SCHEMA_VERSION",
    "PRESET_CATALOG_KIND",
    "PRESET_CATALOG_REVISION_ALGORITHM",
    "PRESET_CATALOG_SCHEMA_VERSION",
    "agentKeysRevision",
    "canonicalJson",
    "createCodexMainAdapter",
    "createInputMainAdapter",
    "hostSettingsRevision",
    "inspectInputCompanionBridge",
    "installCodexCompanionBridge",
    "installInputCompanionBridge",
    "presetCatalogRevision",
    "settingsRevision",
    "startCodexCompanionBridge",
    "startInputCompanionBridge",
}


def run(*args, cwd=None):
    return subprocess.run(
        args,
        cwd=cwd,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def pack(package, destination):
    destination.mkdir()
    result = run(
        "npm",
        "pack",
        str(package),
        "--pack-destination",
        str(destination),
        "--json",
    )
    report = json.loads(result.stdout)
    if len(report) != 1:
        raise SystemExit("npm pack returned an unexpected report count")
    archive = destination / report[0]["filename"]
    if not archive.is_file():
        raise SystemExit("npm pack did not create the reported archive")
    return archive


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--output",
        type=Path,
        help="also publish the verified tgz and SHA-256 sidecar into this directory",
    )
    args = parser.parse_args()
    repo = Path(__file__).resolve().parent.parent
    package = repo / "companion"
    cargo = (repo / "Cargo.toml").read_text()
    version = re.search(r'^version = "([^"]+)"$', cargo, re.MULTILINE).group(1)
    metadata = json.loads((package / "package.json").read_text())
    if metadata["version"] != version:
        raise SystemExit("Companion package and CLI versions differed")
    if metadata.get("private") is not True or metadata.get("license") != "MIT":
        raise SystemExit("Companion package publication/license boundary changed")

    with tempfile.TemporaryDirectory(prefix="worklouderctl-companion-package.") as raw:
        root = Path(raw)
        first = pack(package, root / "one")
        second = pack(package, root / "two")
        if first.read_bytes() != second.read_bytes():
            raise SystemExit("Companion package was not byte-for-byte deterministic")
        digest = sha256(first)

        with tarfile.open(first, "r:gz") as archive:
            files = {
                member.name: member.mode
                for member in archive.getmembers()
                if member.isfile()
            }
            if files != EXPECTED_FILES:
                raise SystemExit(f"Companion package inventory changed: {files!r}")
            # The exact regular-file allowlist above makes extraction bounded
            # while retaining compatibility with Python versions before the
            # tarfile extraction-filter argument was added.
            archive.extractall(root / "extracted")

        extracted = root / "extracted/package"
        if (extracted / "LICENSE").read_bytes() != (repo / "LICENSE").read_bytes():
            raise SystemExit("Companion package license differed from the repository")
        help_result = run("node", str(extracted / "conformance.mjs"), "--help")
        expected_help = (
            "usage: input-companion-conformance [--socket PATH] [--token PATH] "
            "[--require CAPABILITY]...\n"
        )
        if help_result.stdout != expected_help:
            raise SystemExit(
                "Companion conformance executable help changed: "
                f"{help_result.stdout!r} stderr={help_result.stderr!r}"
            )

        install = root / "install"
        run(
            "npm",
            "install",
            "--ignore-scripts",
            "--no-audit",
            "--no-fund",
            "--prefix",
            str(install),
            str(first),
        )
        import_result = run(
            "node",
            "--input-type=module",
            "-e",
            "const m=await import('@worklouder/input-companion-bridge-reference');"
            "console.log(JSON.stringify(Object.keys(m).sort()))",
            cwd=install,
        )
        exports = set(json.loads(import_result.stdout))
        if exports != EXPECTED_EXPORTS:
            raise SystemExit(f"Companion package exports changed: {sorted(exports)!r}")

        if args.output:
            args.output.mkdir(parents=True, exist_ok=True)
            destination = args.output / first.name
            sidecar = destination.with_name(destination.name + ".sha256")
            if destination.exists() or sidecar.exists():
                raise SystemExit("Companion package output already exists")
            shutil.copyfile(first, destination)
            destination.chmod(0o644)
            sidecar.write_text(f"{digest}  {destination.name}\n")
            sidecar.chmod(0o644)
            print(f"companion_package={destination}")
            print(f"companion_package_sha256={digest}")

    print("companion_package_deterministic=verified")
    print("companion_package_inventory=verified")
    print("companion_package_install_import=verified")
    print("companion_conformance_binary=verified")


if __name__ == "__main__":
    main()
