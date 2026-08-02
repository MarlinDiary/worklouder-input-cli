#!/usr/bin/env python3
"""Render the release-specific Homebrew formula from verified checksums."""

import argparse
from pathlib import Path
import re


SHA256 = re.compile(r"^[0-9a-f]{64}$")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--base-url", required=True)
    parser.add_argument("--arm64-sha256", required=True)
    parser.add_argument("--x86-64-sha256", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+(?:[-+][0-9A-Za-z.-]+)?", args.version):
        raise SystemExit("version was invalid")
    if not args.base_url.startswith("https://") or args.base_url.endswith("/"):
        raise SystemExit("base URL must be HTTPS without a trailing slash")
    if not SHA256.fullmatch(args.arm64_sha256) or not SHA256.fullmatch(args.x86_64_sha256):
        raise SystemExit("release checksum was invalid")
    repo = Path(__file__).resolve().parent.parent
    template = (repo / "packaging/homebrew/worklouderctl.rb.in").read_text()
    rendered = (
        template.replace("@VERSION@", args.version)
        .replace("@BASE_URL@", args.base_url)
        .replace("@ARM64_SHA256@", args.arm64_sha256)
        .replace("@X86_64_SHA256@", args.x86_64_sha256)
    )
    if "@" in rendered:
        raise SystemExit("formula contained an unresolved template token")
    if args.output.exists():
        raise SystemExit(f"formula destination already exists: {args.output}")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(rendered)
    args.output.chmod(0o644)
    print(args.output)


if __name__ == "__main__":
    main()
