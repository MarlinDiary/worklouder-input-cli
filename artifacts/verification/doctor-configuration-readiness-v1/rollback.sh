#!/bin/sh
set -eu
repo=${1:-.}
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
patch=${2:-$script_dir/doctor-configuration-readiness.patch}
base=1647b3a2c03615cb61b90eb336b5739d0dfaa384
feature=fbdbbafcb779a2b17ef95d041745dbe5a98097c7
git -C "$repo" apply --check --reverse "$patch"
git -C "$repo" apply --reverse "$patch"
git -C "$repo" diff --check
if git -C "$repo" cat-file -e "$base^{commit}" 2>/dev/null; then
  git -C "$repo" diff --quiet "$base" -- .     ':(exclude)artifacts/verification/doctor-configuration-readiness-v1'
fi
printf '%s\n' "rollback=verified" "base=$base" "feature=$feature"
