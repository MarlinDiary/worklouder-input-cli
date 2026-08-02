#!/bin/sh
set -eu
repo=${1:-.}
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
patch=${2:-$script_dir/codex-companion-bridge.patch}
base=1bc08b9ae41b1b7e557d4d8300a8148868b67370

git -C "$repo" apply --check --reverse "$patch"
git -C "$repo" apply --reverse "$patch"
git -C "$repo" diff --check
if git -C "$repo" cat-file -e "$base^{commit}" 2>/dev/null; then
  git -C "$repo" diff --quiet "$base" -- . \
    ':(exclude)artifacts/verification/codex-companion-bridge-v1'
fi
printf '%s\n' "rollback=verified" "base=$base"
