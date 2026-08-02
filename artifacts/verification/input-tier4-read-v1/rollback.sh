#!/bin/sh
set -eu
repo=${1:-.}
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
patch=${2:-$script_dir/input-tier4-read.patch}
base=51610680ce7b6ba86d9dafe60135e0be42cb0630
git -C "$repo" apply --check --reverse "$patch"
git -C "$repo" apply --reverse "$patch"
git -C "$repo" diff --check
if git -C "$repo" cat-file -e "$base^{commit}" 2>/dev/null; then
  git -C "$repo" diff --quiet "$base" -- . ':(exclude)artifacts/verification/input-tier4-read-v1'
fi
printf '%s\n' "rollback=verified" "base=$base"
