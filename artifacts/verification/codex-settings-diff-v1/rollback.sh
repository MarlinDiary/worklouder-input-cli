#!/bin/sh
set -eu
repo=${1:-.}
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
patch=${2:-$script_dir/codex-settings-diff.patch}
base=ae66d32e1157aa5592b09425245dd0af4396f059

git -C "$repo" apply --check --reverse "$patch"
git -C "$repo" apply --reverse "$patch"
git -C "$repo" diff --check
if git -C "$repo" cat-file -e "$base^{commit}" 2>/dev/null; then
  git -C "$repo" diff --quiet "$base" -- .     ':(exclude)artifacts/verification/codex-settings-diff-v1'
fi
printf '%s\n' "rollback=verified" "base=$base"
