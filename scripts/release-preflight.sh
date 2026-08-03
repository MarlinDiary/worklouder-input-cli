#!/usr/bin/env bash
set -uo pipefail

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

version=$(sed -nE 's/^version = "([^"]+)"$/\1/p' Cargo.toml | head -1)
failures=0

if [[ ${1:-} == "-h" || ${1:-} == "--help" ]]; then
  printf 'Usage: %s [vVERSION]\n' "$0"
  exit 0
fi
if (($# > 1)); then
  printf 'Usage: %s [vVERSION]\n' "$0" >&2
  exit 2
fi
tag=${1:-v$version}

pass() {
  printf 'PASS  %s\n' "$1"
}

fail() {
  printf 'FAIL  %s\n' "$1" >&2
  failures=$((failures + 1))
}

if [[ ! "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$ ]]; then
  fail "tag has invalid version syntax: $tag"
elif [[ "$tag" != "v$version" ]]; then
  fail "tag $tag does not match Cargo version v$version"
else
  pass "tag matches Cargo version: $tag"
fi

if [[ -z $(git status --porcelain) ]]; then
  pass "working tree is clean"
else
  fail "working tree has uncommitted changes"
fi

head=$(git rev-parse HEAD)
remote_main=$(git ls-remote --exit-code origin refs/heads/main 2>/dev/null || true)
remote_head=${remote_main%%[[:space:]]*}
if [[ -n "$remote_head" && "$head" == "$remote_head" ]]; then
  pass "HEAD matches origin/main: ${head:0:12}"
elif [[ -z "$remote_head" ]]; then
  fail "origin/main did not resolve"
else
  fail "HEAD does not match origin/main"
fi

if git rev-parse --verify --quiet "refs/tags/$tag" >/dev/null; then
  fail "local tag already exists: $tag"
else
  pass "local tag is available: $tag"
fi

remote_tag=$(git ls-remote --exit-code --tags origin "refs/tags/$tag" 2>/dev/null)
remote_tag_status=$?
case "$remote_tag_status" in
  0) fail "remote tag already exists: $tag" ;;
  2) pass "remote tag is available: $tag" ;;
  *) fail "remote tag lookup failed: $tag" ;;
esac

for command in gh jq; do
  if command -v "$command" >/dev/null 2>&1; then
    pass "$command is installed"
  else
    fail "$command is required"
  fi
done

if command -v gh >/dev/null 2>&1 && command -v jq >/dev/null 2>&1; then
  repository=$(gh repo view --json nameWithOwner --jq .nameWithOwner 2>/dev/null || true)
  if [[ -n "$repository" ]]; then
    pass "GitHub repository resolved: $repository"
  else
    fail "GitHub repository did not resolve"
  fi

  required_secrets=(
    APPLE_DEVELOPER_ID_CERTIFICATE_P12_BASE64
    APPLE_DEVELOPER_ID_CERTIFICATE_PASSWORD
    APPLE_SIGNING_IDENTITY
    APPLE_ID
    APPLE_TEAM_ID
    APPLE_APP_SPECIFIC_PASSWORD
    HOMEBREW_TAP_DEPLOY_KEY
  )
  if secrets=$(gh secret list --app actions --json name --jq '.[].name' 2>/dev/null); then
    for secret in "${required_secrets[@]}"; do
      if grep -Fqx "$secret" <<<"$secrets"; then
        pass "Actions secret exists: $secret"
      else
        fail "Actions secret is missing: $secret"
      fi
    done
  else
    fail "Actions secret names could not be read"
  fi

  protection=$(gh api "repos/$repository/branches/main/protection" 2>/dev/null || true)
  if [[ -n "$protection" ]] &&
    jq -e '.required_status_checks.strict == true' <<<"$protection" >/dev/null; then
    pass "main requires a current branch before merging"
  else
    fail "main strict status checks are not enabled"
  fi
  for context in 'Format, test, and lint' 'Rust 1.61 test'; do
    if jq -er '.required_status_checks.contexts[]' <<<"$protection" 2>/dev/null |
      grep -Fqx "$context"; then
      pass "main requires status check: $context"
    else
      fail "main is missing status check: $context"
    fi
  done

  run=$(gh run list --workflow CI --branch main --commit "$head" --limit 1 \
    --json databaseId,status,conclusion,headSha,url 2>/dev/null || true)
  if jq -e --arg head "$head" \
    'length == 1 and .[0].headSha == $head and .[0].status == "completed" and .[0].conclusion == "success"' \
    <<<"$run" >/dev/null 2>&1; then
    url=$(jq -r '.[0].url' <<<"$run")
    pass "CI succeeded for HEAD: $url"
  else
    fail "CI has not succeeded for HEAD"
  fi
fi

if ((failures > 0)); then
  printf 'NOT_READY failures=%d tag=%s head=%s\n' "$failures" "$tag" "$head" >&2
  exit 1
fi

printf 'READY tag=%s head=%s\n' "$tag" "$head"
