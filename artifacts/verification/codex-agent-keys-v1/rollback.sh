#!/bin/sh
set -eu
repo=${1:-.}
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
patch=${2:-$script_dir/codex-agent-keys.patch}
base=af5c7dd0bf077509e6ae0a4817a72ff395624f3f

git -C "$repo" apply --check --reverse "$patch"
git -C "$repo" apply --reverse "$patch"
git -C "$repo" diff --check
if git -C "$repo" cat-file -e "$base^{commit}" 2>/dev/null; then
  git -C "$repo" diff --quiet "$base" -- . \
    ':(exclude)artifacts/verification/codex-agent-keys-v1'
fi
printf '%s\n' "rollback=verified" "base=$base"
