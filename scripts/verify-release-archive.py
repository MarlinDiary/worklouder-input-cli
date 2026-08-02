#!/usr/bin/env python3
"""Verify a WorkLouderCTL release archive without trusting archive paths."""

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import subprocess
import tarfile
import tempfile


EXPECTED_PATHS = {
    "bin/worklouderctl": "0755",
    "completions/worklouderctl.bash": "0644",
    "completions/_worklouderctl": "0644",
    "completions/worklouderctl.fish": "0644",
    "LICENSE": "0644",
    "README.md": "0644",
}


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("archive", type=Path)
    parser.add_argument("--expected-target")
    parser.add_argument("--execute", action="store_true")
    args = parser.parse_args()
    archive_path = args.archive.resolve()
    if not archive_path.is_file() or archive_path.is_symlink():
        raise SystemExit("archive must be a regular non-symlink file")
    checksum_path = archive_path.with_suffix(archive_path.suffix + ".sha256")
    checksum = checksum_path.read_text().split()
    if checksum != [sha256(archive_path.read_bytes()), archive_path.name]:
        raise SystemExit("archive checksum sidecar did not match")

    with tarfile.open(archive_path, "r:gz") as archive:
        members = archive.getmembers()
        if not members:
            raise SystemExit("archive was empty")
        roots = set()
        by_relative = {}
        for member in members:
            path = PurePosixPath(member.name)
            if path.is_absolute() or ".." in path.parts or len(path.parts) < 2:
                raise SystemExit(f"unsafe archive path: {member.name}")
            if not member.isfile() or member.issym() or member.islnk():
                raise SystemExit(f"archive member was not a regular file: {member.name}")
            roots.add(path.parts[0])
            relative = str(PurePosixPath(*path.parts[1:]))
            if relative in by_relative:
                raise SystemExit(f"duplicate archive path: {relative}")
            by_relative[relative] = member
        if len(roots) != 1 or set(by_relative) != set(EXPECTED_PATHS) | {"manifest.json"}:
            raise SystemExit("archive root or file inventory was invalid")
        manifest = json.loads(archive.extractfile(by_relative["manifest.json"]).read())
        if (
            set(manifest)
            != {"schemaVersion", "kind", "version", "target", "signatureState", "files"}
            or not isinstance(manifest.get("files"), list)
            or manifest.get("schemaVersion") != 1
            or manifest.get("kind") != "worklouderctl-release-archive"
            or manifest.get("signatureState")
            not in {
                "unsigned",
                "apple-development",
                "developer-id",
                "developer-id-notarized",
            }
        ):
            raise SystemExit("release manifest header was invalid")
        if args.expected_target and manifest.get("target") != args.expected_target:
            raise SystemExit("release target differed from expectation")
        records = {record["path"]: record for record in manifest["files"]}
        if (
            len(records) != len(manifest["files"])
            or set(records) != set(EXPECTED_PATHS)
            or any(
                set(record) != {"path", "size", "sha256", "mode"}
                for record in manifest["files"]
            )
        ):
            raise SystemExit("release manifest inventory was invalid")
        for relative, expected_mode in EXPECTED_PATHS.items():
            member = by_relative[relative]
            data = archive.extractfile(member).read()
            record = records[relative]
            if (
                record.get("mode") != expected_mode
                or member.mode != int(expected_mode, 8)
                or record.get("size") != len(data)
                or record.get("sha256") != sha256(data)
            ):
                raise SystemExit(f"release file verification failed: {relative}")
        version = manifest.get("version")
        target = manifest.get("target")
        if roots != {f"worklouderctl-v{version}-{target}"}:
            raise SystemExit("release archive root did not match its manifest")

        if args.execute:
            with tempfile.TemporaryDirectory(prefix="worklouderctl-release-verify-") as root:
                binary = Path(root) / "worklouderctl"
                binary.write_bytes(archive.extractfile(by_relative["bin/worklouderctl"]).read())
                binary.chmod(0o755)
                result = subprocess.run(
                    [str(binary), "version"], check=True, text=True, capture_output=True
                )
                if result.stdout != f"worklouderctl {version}\n" or result.stderr:
                    raise SystemExit("packaged binary version execution failed")
    print(f"release_archive={archive_path.name}")
    print(f"release_target={manifest['target']}")
    print(f"release_signature_state={manifest['signatureState']}")
    print("release_archive=verified")


if __name__ == "__main__":
    main()
