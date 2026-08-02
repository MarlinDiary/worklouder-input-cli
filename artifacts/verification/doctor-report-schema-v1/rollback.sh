#!/bin/sh
set -eu
repo=${1:-.}
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
patch=${2:-$script_dir/doctor-report-schema.patch}
base=a1c3e0d6c4a9d413f6ad60b67adc8c1191a22155
feature=1668f349dc6f3097f2d7e831c51e757e4b059268
git -C "$repo" apply --check --reverse "$patch"
git -C "$repo" apply --reverse "$patch"
git -C "$repo" diff --check
if git -C "$repo" cat-file -e "$base^{commit}" 2>/dev/null; then
  git -C "$repo" diff --quiet "$base" -- .     ':(exclude)artifacts/verification/doctor-report-schema-v1'
fi
printf '%s\n' "rollback=verified" "base=$base" "feature=$feature"
