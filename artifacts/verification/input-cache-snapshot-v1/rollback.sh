#!/bin/sh
set -eu
repo=${1:-.}
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
patch=${2:-$script_dir/input-cache-snapshot.patch}
base=63882cf5923a5c8a9ef200e1fae215e36009793e

git -C "$repo" apply --check --reverse "$patch"
git -C "$repo" apply --reverse "$patch"
git -C "$repo" diff --check
if git -C "$repo" cat-file -e "$base^{commit}" 2>/dev/null; then
  git -C "$repo" diff --quiet "$base" -- . \
    ':(exclude)artifacts/verification/input-cache-snapshot-v1'
fi
printf '%s\n' "rollback=verified" "base=$base"
