#!/bin/sh
set -eu
repo=${1:-.}
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
patch=${2:-$script_dir/macos-release-packaging.patch}
base=1329195
feature=3e8004a
git -C "$repo" apply --check --reverse "$patch"
git -C "$repo" apply --reverse "$patch"
git -C "$repo" diff --check
if git -C "$repo" cat-file -e "$base^{commit}" 2>/dev/null; then
  git -C "$repo" diff --quiet "$base" -- . \
    ':(exclude)artifacts/verification/macos-release-packaging-v1'
fi
printf '%s\n' "rollback=verified" "base=$base" "feature=$feature"
