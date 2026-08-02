#!/bin/sh
set -eu

repo=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
target=${WORKLOUDERCTL_TARGET:-$(rustc -vV | sed -n 's/^host: //p')}
output=${1:-$repo/dist}
identity=${WORKLOUDERCTL_CODESIGN_IDENTITY:-}
target_dir=${CARGO_TARGET_DIR:-$repo/target}
binary=$target_dir/$target/release/worklouderctl

case "$target" in
  *-apple-darwin) ;;
  *) echo "target must be an Apple Darwin target: $target" >&2; exit 2 ;;
esac
if [ -e "$output" ]; then
  echo "release output already exists: $output" >&2
  exit 2
fi

cargo build --release --locked --target "$target" --manifest-path "$repo/Cargo.toml"
test -x "$binary"
"$repo/scripts/verify-cli-assets.sh"

mkdir -p "$output/staging"
signed_binary=$output/staging/worklouderctl
cp "$binary" "$signed_binary"
chmod 0755 "$signed_binary"
signature_state=unsigned
if [ -n "$identity" ]; then
  codesign --force --options runtime --sign "$identity" "$signed_binary"
  codesign --verify --strict --verbose=2 "$signed_binary"
  case "$identity" in
    *"Developer ID Application"*) signature_state=developer-id ;;
    *) signature_state=apple-development ;;
  esac
fi

SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH:-$(git -C "$repo" show -s --format=%ct HEAD)} \
  "$repo/scripts/build-release-archive.py" \
  --binary "$signed_binary" --target "$target" --output "$output" \
  --signature-state "$signature_state"
archive=$(find "$output" -maxdepth 1 -name '*.tar.gz' -print -quit)
test -n "$archive"
"$repo/scripts/verify-macos-release.sh" "$archive"
printf '%s\n' "release_staged_binary=$signed_binary" "release_signature_state=$signature_state"
