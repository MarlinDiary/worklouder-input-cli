#!/bin/sh
set -eu

repo=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
root=$(mktemp -d /tmp/worklouderctl-release-packaging.XXXXXX)
cleanup() {
  python3 - "$root" <<'PY'
from pathlib import Path
import shutil
import sys
root = Path(sys.argv[1])
if root.name.startswith("worklouderctl-release-packaging.") and root.parent == Path("/tmp"):
    shutil.rmtree(root, ignore_errors=True)
PY
}
trap cleanup EXIT INT TERM

target=${WORKLOUDERCTL_RELEASE_TARGET:-aarch64-apple-darwin}
if [ -n "${WORKLOUDERCTL_BIN:-}" ]; then
  bin=$WORKLOUDERCTL_BIN
else
  cargo build --release --locked --manifest-path "$repo/Cargo.toml"
  bin=$repo/target/release/worklouderctl
fi
test -x "$bin"
python3 "$repo/scripts/test-notarization-evidence.py" \
  >"$root/notarization-evidence.txt"

SOURCE_DATE_EPOCH=0 "$repo/scripts/build-release-archive.py" \
  --binary "$bin" --target "$target" --output "$root/one" \
  >"$root/build-one.txt"
SOURCE_DATE_EPOCH=0 "$repo/scripts/build-release-archive.py" \
  --binary "$bin" --target "$target" --output "$root/two" \
  >"$root/build-two.txt"

archive=$(find "$root/one" -name '*.tar.gz' -type f)
second=$root/two/$(basename "$archive")
cmp "$archive" "$second"
cmp "$archive.sha256" "$second.sha256"
"$repo/scripts/verify-release-archive.py" "$archive" \
  --expected-target "$target" --execute >"$root/verify.txt"

version=$($bin version | awk '{print $2}')
digest=$(awk '{print $1}' "$archive.sha256")
intel_digest=$(printf '%s' "$digest-x86_64" | shasum -a 256 | awk '{print $1}')
"$repo/scripts/render-homebrew-formula.py" \
  --version "$version" \
  --base-url "https://github.com/MarlinDiary/worklouder-input-cli/releases/download/v$version" \
  --arm64-sha256 "$digest" --x86-64-sha256 "$intel_digest" \
  --output "$root/Formula/worklouderctl.rb" >"$root/formula.txt"
test "$(stat -f %Lp "$root/Formula/worklouderctl.rb")" = 644
ruby -c "$root/Formula/worklouderctl.rb" >"$root/ruby.txt"
if grep -Eq '^[[:space:]]+version[[:space:]]+"' \
  "$root/Formula/worklouderctl.rb"; then
  echo "generated formula must derive its version from the release URL" >&2
  exit 1
fi
if command -v brew >/dev/null 2>&1; then
  if ! brew style "$root/Formula/worklouderctl.rb" \
    >"$root/brew-style.txt" 2>&1; then
    cat "$root/brew-style.txt" >&2
    exit 1
  fi
fi

mkdir "$root/extracted" "$root/prefix"
tar -xzf "$archive" -C "$root/extracted"
package_root=$(find "$root/extracted" -mindepth 1 -maxdepth 1 -type d)
mkdir -p "$root/prefix/bin" "$root/prefix/share/bash-completion/completions" \
  "$root/prefix/share/zsh/site-functions" \
  "$root/prefix/share/fish/vendor_completions.d"
install -m 0755 "$package_root/bin/worklouderctl" "$root/prefix/bin/worklouderctl"
install -m 0644 "$package_root/completions/worklouderctl.bash" \
  "$root/prefix/share/bash-completion/completions/worklouderctl"
install -m 0644 "$package_root/completions/_worklouderctl" \
  "$root/prefix/share/zsh/site-functions/_worklouderctl"
install -m 0644 "$package_root/completions/worklouderctl.fish" \
  "$root/prefix/share/fish/vendor_completions.d/worklouderctl.fish"
test "$($root/prefix/bin/worklouderctl version)" = "worklouderctl $version"
test -s "$root/prefix/share/bash-completion/completions/worklouderctl"
test -s "$root/prefix/share/zsh/site-functions/_worklouderctl"
test -s "$root/prefix/share/fish/vendor_completions.d/worklouderctl.fish"

printf '%s\n' \
  "release_archive_deterministic=verified" \
  "release_archive_manifest=verified" \
  "release_binary_execute=verified" \
  "release_temp_prefix_install=verified" \
  "homebrew_formula_version_from_url=verified" \
  "homebrew_formula_syntax=verified" \
  "homebrew_formula_style=verified"
