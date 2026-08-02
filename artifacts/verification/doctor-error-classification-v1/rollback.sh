#!/bin/sh
set -eu
repo=${1:-.}
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
patch=${2:-$script_dir/doctor-error-classification.patch}
base=6c08cc3c02b5335f96938488ed5d59832480595a
feature=6074c7e4564f711c623858d6b736165eb0a47464
git -C "$repo" apply --check --reverse "$patch"
git -C "$repo" apply --reverse "$patch"
git -C "$repo" diff --check
if git -C "$repo" cat-file -e "$base^{commit}" 2>/dev/null; then
  git -C "$repo" diff --quiet "$base" -- .     ':(exclude)artifacts/verification/doctor-error-classification-v1'
fi
printf '%s\n' "rollback=verified" "base=$base" "feature=$feature"
