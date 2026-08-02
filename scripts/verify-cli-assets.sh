#!/bin/sh
set -eu

repo=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
tmp=$(mktemp -d "${TMPDIR:-/tmp}/worklouderctl-cli-assets.XXXXXX")
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

cargo build --locked --manifest-path "$repo/Cargo.toml" >/dev/null
python3 "$repo/scripts/render-cli-assets.py" \
  --binary "$repo/target/debug/worklouderctl" --output "$tmp/generated"

diff -u "$repo/completions/worklouderctl.bash" "$tmp/generated/completions/worklouderctl.bash"
diff -u "$repo/completions/_worklouderctl" "$tmp/generated/completions/_worklouderctl"
diff -u "$repo/completions/worklouderctl.fish" "$tmp/generated/completions/worklouderctl.fish"
diff -u "$repo/docs/command-reference.md" "$tmp/generated/docs/command-reference.md"

bash -n "$repo/completions/worklouderctl.bash"
zsh -n "$repo/completions/_worklouderctl"
grep -q '^## `worklouderctl transaction restore`$' "$repo/docs/command-reference.md"
grep -q '^## `worklouderctl agent execute`$' "$repo/docs/command-reference.md"

count=$(grep -c '^## `worklouderctl' "$repo/docs/command-reference.md")
printf '%s\n' \
  'completion_bash=syntax-verified' \
  'completion_zsh=syntax-verified' \
  'completion_fish=generated' \
  "command_reference_paths=$count" \
  'cli_assets=deterministic'
