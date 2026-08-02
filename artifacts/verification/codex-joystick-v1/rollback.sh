#!/bin/sh
set -eu
repo=${1:-.}
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
patch=${2:-$script_dir/codex-joystick.patch}
base=d7fb0964b171285ce5b84d9cd049fc129049f77c
git -C "$repo" apply --check --reverse "$patch"
git -C "$repo" apply --reverse "$patch"
git -C "$repo" diff --check
if git -C "$repo" cat-file -e "$base^{commit}" 2>/dev/null; then
  git -C "$repo" diff --quiet "$base" -- . ':(exclude)artifacts/verification/codex-joystick-v1'
fi
printf '%s\n' "rollback=verified" "base=$base"
