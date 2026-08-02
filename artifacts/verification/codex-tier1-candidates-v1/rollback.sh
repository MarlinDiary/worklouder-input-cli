#!/bin/sh
set -eu
repo=${1:-.}
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
patch=${2:-$script_dir/codex-tier1-candidates.patch}
base=11520add1e086787c4dfad43e176eea489afa321

git -C "$repo" apply --check --reverse "$patch"
git -C "$repo" apply --reverse "$patch"
git -C "$repo" diff --check
if git -C "$repo" cat-file -e "$base^{commit}" 2>/dev/null; then
  git -C "$repo" diff --quiet "$base" -- . \
    ':(exclude)artifacts/verification/codex-tier1-candidates-v1'
fi
printf '%s\n' "rollback=verified" "base=$base"
