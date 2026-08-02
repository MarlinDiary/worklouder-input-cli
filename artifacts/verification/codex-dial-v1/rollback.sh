#!/bin/sh
set -eu
repo=${1:-.}
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
patch=${2:-$script_dir/codex-dial.patch}
base=68534b6874ddfda5fb5be5157e49b1aeff2865c7
git -C "$repo" apply --check --reverse "$patch"
git -C "$repo" apply --reverse "$patch"
git -C "$repo" diff --check
if git -C "$repo" cat-file -e "$base^{commit}" 2>/dev/null; then
  git -C "$repo" diff --quiet "$base" -- . ':(exclude)artifacts/verification/codex-dial-v1'
fi
printf '%s\n' "rollback=verified" "base=$base"
