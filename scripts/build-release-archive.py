#!/usr/bin/env python3
"""Build a deterministic, self-describing WorkLouderCTL release archive."""

import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import tarfile
import uuid


KIND = "worklouderctl-release-archive"
SCHEMA_VERSION = 1


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def validate_notarization_result(path):
    if path is None:
        raise SystemExit("accepted notarization result is required")
    if not path.is_file() or path.is_symlink():
        raise SystemExit("notarization result must be a regular non-symlink file")
    try:
        result = json.loads(path.read_text())
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise SystemExit("notarization result was not valid JSON") from error
    submission_id = result.get("id") if isinstance(result, dict) else None
    try:
        uuid.UUID(submission_id)
    except (AttributeError, TypeError, ValueError) as error:
        raise SystemExit("notarization result had an invalid submission id") from error
    if result.get("status") != "Accepted":
        raise SystemExit("notarization result was not accepted")
    return result


def add_bytes(archive, path, data, mode, mtime):
    info = tarfile.TarInfo(path)
    info.size = len(data)
    info.mode = mode
    info.mtime = mtime
    info.uid = 0
    info.gid = 0
    info.uname = ""
    info.gname = ""
    archive.addfile(info, io.BytesIO(data))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--notarization-result", type=Path)
    parser.add_argument(
        "--signature-state",
        choices=[
            "unsigned",
            "ad-hoc",
            "apple-development",
            "developer-id",
            "developer-id-notarized",
        ],
        default="unsigned",
    )
    args = parser.parse_args()
    if args.target not in {"aarch64-apple-darwin", "x86_64-apple-darwin"}:
        raise SystemExit("release target was unsupported")

    repo = Path(__file__).resolve().parent.parent
    cargo = (repo / "Cargo.toml").read_text()
    version = next(
        line.split('"')[1]
        for line in cargo.splitlines()
        if line.startswith("version = ")
    )
    binary = args.binary.resolve()
    if not binary.is_file() or not os.access(binary, os.X_OK):
        raise SystemExit(f"binary is missing or not executable: {binary}")
    reported = subprocess.run(
        [str(binary), "version"], check=True, text=True, capture_output=True
    )
    expected = f"worklouderctl {version}\n"
    if reported.stdout != expected or reported.stderr:
        raise SystemExit("binary version output did not match Cargo.toml")
    if args.signature_state != "unsigned":
        subprocess.run(
            ["codesign", "--verify", "--strict", "--verbose=2", str(binary)],
            check=True,
        )
        signature = subprocess.run(
            ["codesign", "-d", "--verbose=4", str(binary)],
            text=True,
            capture_output=True,
            check=True,
        )
        if args.signature_state == "ad-hoc":
            if "Signature=adhoc" not in signature.stderr:
                raise SystemExit("binary was not ad-hoc signed")
        else:
            expected_authority = (
                "Authority=Apple Development:"
                if args.signature_state == "apple-development"
                else "Authority=Developer ID Application:"
            )
            if expected_authority not in signature.stderr:
                raise SystemExit("binary signing authority did not match signature state")
    if args.signature_state == "developer-id-notarized":
        if (
            "Timestamp=" not in signature.stderr
            or "flags=0x10000(runtime)" not in signature.stderr
        ):
            raise SystemExit(
                "notarized binary lacked a secure timestamp or hardened runtime"
            )
        validate_notarization_result(args.notarization_result)
    elif args.notarization_result is not None:
        raise SystemExit("notarization result requires developer-id-notarized state")

    root = f"worklouderctl-v{version}-{args.target}"
    sources = [
        ("bin/worklouderctl", binary, 0o755),
        ("completions/worklouderctl.bash", repo / "completions/worklouderctl.bash", 0o644),
        ("completions/_worklouderctl", repo / "completions/_worklouderctl", 0o644),
        ("completions/worklouderctl.fish", repo / "completions/worklouderctl.fish", 0o644),
        ("LICENSE", repo / "LICENSE", 0o644),
        ("README.md", repo / "README.md", 0o644),
    ]
    files = []
    payloads = []
    for relative, source, mode in sources:
        data = source.read_bytes()
        payloads.append((relative, data, mode))
        files.append(
            {
                "path": relative,
                "size": len(data),
                "sha256": sha256(data),
                "mode": f"{mode:04o}",
            }
        )
    manifest = {
        "schemaVersion": SCHEMA_VERSION,
        "kind": KIND,
        "version": version,
        "target": args.target,
        "signatureState": args.signature_state,
        "files": files,
    }
    manifest_bytes = (json.dumps(manifest, indent=2) + "\n").encode()
    payloads.append(("manifest.json", manifest_bytes, 0o644))
    payloads.sort(key=lambda item: item[0].encode())

    args.output.mkdir(parents=True, exist_ok=True)
    archive_path = args.output / f"{root}.tar.gz"
    checksum_path = archive_path.with_suffix(archive_path.suffix + ".sha256")
    if archive_path.exists() or checksum_path.exists():
        raise SystemExit(f"release output already exists: {archive_path}")
    mtime = int(os.environ.get("SOURCE_DATE_EPOCH", "0"))
    tar_bytes = io.BytesIO()
    with tarfile.open(fileobj=tar_bytes, mode="w", format=tarfile.USTAR_FORMAT) as archive:
        for relative, data, mode in payloads:
            add_bytes(archive, f"{root}/{relative}", data, mode, mtime)
    with archive_path.open("wb") as output:
        with gzip.GzipFile(filename="", mode="wb", fileobj=output, mtime=0) as compressed:
            compressed.write(tar_bytes.getvalue())
    digest = sha256(archive_path.read_bytes())
    checksum_path.write_text(f"{digest}  {archive_path.name}\n")
    print(archive_path)
    print(checksum_path)
    print(f"archive_sha256={digest}")


if __name__ == "__main__":
    main()
