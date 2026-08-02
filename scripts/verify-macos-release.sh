#!/bin/sh
set -eu

repo=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
archive=${1:?usage: verify-macos-release.sh ARCHIVE}
root=$(mktemp -d /tmp/worklouderctl-macos-release.XXXXXX)
cleanup() { rm -rf "$root"; }
trap cleanup EXIT INT TERM

"$repo/scripts/verify-release-archive.py" "$archive" --execute >"$root/archive.txt"
tar -xzf "$archive" -C "$root"
binary=$(find "$root" -type f -path '*/bin/worklouderctl' -print -quit)
manifest=$(find "$root" -type f -name manifest.json -print -quit)
test -n "$binary"
test -n "$manifest"
signature_state=$(python3 -c \
  'import json,sys; print(json.load(open(sys.argv[1]))["signatureState"])' \
  "$manifest")
if [ "$signature_state" != unsigned ]; then
  codesign --verify --strict --verbose=2 "$binary"
fi
cat "$root/archive.txt"
printf '%s\n' "release_macos_signature=$signature_state" "release_macos=verified"
