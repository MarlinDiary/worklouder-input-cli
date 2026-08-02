#!/bin/sh
set -eu
repo=${1:-.}
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
patch=${2:-$script_dir/doctor-authenticated-readiness-e2e.patch}
base=00677df4c99c76eb47901da968d3351e82900089
feature=edd1e4d9636cd7bbba3d2db919fab65b2a5f0705
git -C "$repo" apply --check --reverse "$patch"
git -C "$repo" apply --reverse "$patch"
git -C "$repo" diff --check
if git -C "$repo" cat-file -e "$base^{commit}" 2>/dev/null; then
  git -C "$repo" diff --quiet "$base" -- .     ':(exclude)artifacts/verification/doctor-authenticated-readiness-e2e-v1'
fi
printf '%s\n' "rollback=verified" "base=$base" "feature=$feature"
