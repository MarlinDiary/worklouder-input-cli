#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
tmp=$(mktemp -d "${TMPDIR:-/tmp}/worklouderctl-installer-test.XXXXXX")
cleanup() {
  rm -f \
    "$tmp/prefix/bin/worklouderctl" \
    "$tmp/prefix/share/bash-completion/completions/worklouderctl" \
    "$tmp/prefix/share/zsh/site-functions/_worklouderctl" \
    "$tmp/prefix/share/fish/vendor_completions.d/worklouderctl.fish"
  rmdir "$tmp/prefix/bin" \
    "$tmp/prefix/share/bash-completion/completions" \
    "$tmp/prefix/share/bash-completion" \
    "$tmp/prefix/share/zsh/site-functions" \
    "$tmp/prefix/share/zsh" \
    "$tmp/prefix/share/fish/vendor_completions.d" \
    "$tmp/prefix/share/fish" \
    "$tmp/prefix/share" \
    "$tmp/prefix" "$tmp" 2>/dev/null || true
}
trap cleanup EXIT HUP INT TERM

version=${WORKLOUDERCTL_TEST_VERSION:-0.1.0}
WORKLOUDERCTL_VERSION=$version \
WORKLOUDERCTL_INSTALL_PREFIX="$tmp/prefix" \
  "$repo_root/install.sh"

test "$("$tmp/prefix/bin/worklouderctl" version)" = "worklouderctl $version"
test -x "$tmp/prefix/bin/worklouderctl"
test -s "$tmp/prefix/share/bash-completion/completions/worklouderctl"
test -s "$tmp/prefix/share/zsh/site-functions/_worklouderctl"
test -s "$tmp/prefix/share/fish/vendor_completions.d/worklouderctl.fish"
codesign --verify --strict --verbose=2 "$tmp/prefix/bin/worklouderctl"

printf 'PASS installer version=%s prefix=%s\n' "$version" "$tmp/prefix"
