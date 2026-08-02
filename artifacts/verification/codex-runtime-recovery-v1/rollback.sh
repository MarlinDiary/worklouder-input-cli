#!/bin/sh
set -eu
repo=${1:-.}
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
patch=${2:-$script_dir/codex-runtime-recovery.patch}
base=0977dcec9b35660064efeb5dae5faba1485c068c

git -C "$repo" apply --check --reverse "$patch"
git -C "$repo" apply --reverse "$patch"
git -C "$repo" diff --check
if git -C "$repo" cat-file -e "$base^{commit}" 2>/dev/null; then
  git -C "$repo" diff --quiet "$base" -- .     ':(exclude)artifacts/verification/codex-runtime-recovery-v1'
fi
printf '%s\n' "rollback=verified" "base=$base"
