#!/bin/sh
set -eu

REPOSITORY="MarlinDiary/worklouder-input-cli"
INSTALLER_VERSION="1"
version=${WORKLOUDERCTL_VERSION:-}
prefix=${WORKLOUDERCTL_INSTALL_PREFIX:-"$HOME/.local"}

usage() {
  cat <<EOF
Install an official signed and notarized WorkLouderCTL release.

Usage: install.sh [--version VERSION] [--prefix PREFIX]
       install.sh --help

Options:
  --version VERSION  Install a specific release (default: latest)
  --prefix PREFIX    Installation prefix (default: \$HOME/.local)
  -h, --help         Show this help

Environment:
  WORKLOUDERCTL_VERSION           Same as --version
  WORKLOUDERCTL_INSTALL_PREFIX    Same as --prefix
  WORKLOUDERCTL_RELEASE_BASE_URL  HTTPS asset directory override for testing

Installer contract: v$INSTALLER_VERSION
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --version)
      [ "$#" -ge 2 ] || { echo "error: --version requires a value" >&2; exit 2; }
      version=$2
      shift 2
      ;;
    --prefix)
      [ "$#" -ge 2 ] || { echo "error: --prefix requires a value" >&2; exit 2; }
      prefix=$2
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

[ "$(uname -s)" = "Darwin" ] || {
  echo "error: official WorkLouderCTL binaries currently support macOS only" >&2
  exit 1
}

for command in curl shasum tar codesign install mktemp sed grep diff sort; do
  command -v "$command" >/dev/null 2>&1 || {
    echo "error: required command is missing: $command" >&2
    exit 1
  }
done

if [ -z "$version" ]; then
  latest_url="https://github.com/$REPOSITORY/releases/latest"
  effective_url=$(curl -fsSL --retry 3 -o /dev/null -w '%{url_effective}' "$latest_url")
  effective_url=${effective_url%/}
  version=${effective_url##*/}
  version=${version#v}
fi

if ! printf '%s\n' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+([-.+][0-9A-Za-z.-]+)?$'; then
  echo "error: invalid release version: $version" >&2
  exit 2
fi

case "$(uname -m)" in
  arm64) target="aarch64-apple-darwin" ;;
  x86_64) target="x86_64-apple-darwin" ;;
  *)
    echo "error: unsupported macOS architecture: $(uname -m)" >&2
    exit 1
    ;;
esac

asset="worklouderctl-v$version-$target.tar.gz"
root="worklouderctl-v$version-$target"
base_url=${WORKLOUDERCTL_RELEASE_BASE_URL:-"https://github.com/$REPOSITORY/releases/download/v$version"}
base_url=${base_url%/}

tmp=$(mktemp -d "${TMPDIR:-/tmp}/worklouderctl-install.XXXXXX")
cleanup() {
  rm -f "$tmp/$asset" "$tmp/$asset.sha256" "$tmp/archive-files" \
    "$tmp/expected-files" "$tmp/archive-types"
  rm -rf "$tmp/$root" 2>/dev/null || true
  rmdir "$tmp" 2>/dev/null || true
}
trap cleanup EXIT HUP INT TERM

echo "Downloading WorkLouderCTL $version for $target..."
curl -fL --retry 3 --proto '=https' --tlsv1.2 \
  -o "$tmp/$asset" "$base_url/$asset"
curl -fL --retry 3 --proto '=https' --tlsv1.2 \
  -o "$tmp/$asset.sha256" "$base_url/$asset.sha256"

sidecar_file=$(awk 'NF == 2 { print $2 }' "$tmp/$asset.sha256")
[ "$sidecar_file" = "$asset" ] || {
  echo "error: checksum sidecar names an unexpected asset" >&2
  exit 1
}
(cd "$tmp" && shasum -a 256 -c "$asset.sha256")

tar -tzf "$tmp/$asset" > "$tmp/archive-files"
if grep -Eq '(^/|(^|/)\.\.(/|$))' "$tmp/archive-files"; then
  echo "error: release archive contains an unsafe path" >&2
  exit 1
fi

cat > "$tmp/expected-files" <<EOF
$root/LICENSE
$root/README.md
$root/bin/worklouderctl
$root/completions/_worklouderctl
$root/completions/worklouderctl.bash
$root/completions/worklouderctl.fish
$root/manifest.json
EOF
sort "$tmp/archive-files" -o "$tmp/archive-files"
sort "$tmp/expected-files" -o "$tmp/expected-files"
if ! diff -u "$tmp/expected-files" "$tmp/archive-files"; then
  echo "error: release archive inventory does not match the install contract" >&2
  exit 1
fi

tar -tvzf "$tmp/$asset" | sed -E 's/^([^[:space:]]+).*/\1/' > "$tmp/archive-types"
if grep -Ev '^-[-rwxStTs]+$' "$tmp/archive-types" >/dev/null; then
  echo "error: release archive contains a non-regular-file entry" >&2
  exit 1
fi

tar -xzf "$tmp/$asset" -C "$tmp"
manifest="$tmp/$root/manifest.json"
grep -Eq '"kind"[[:space:]]*:[[:space:]]*"worklouderctl-release-archive"' "$manifest" || {
  echo "error: unexpected release manifest kind" >&2
  exit 1
}
grep -Eq '"version"[[:space:]]*:[[:space:]]*"'"$version"'"' "$manifest" || {
  echo "error: release manifest version mismatch" >&2
  exit 1
}
grep -Eq '"target"[[:space:]]*:[[:space:]]*"'"$target"'"' "$manifest" || {
  echo "error: release manifest target mismatch" >&2
  exit 1
}
grep -Eq '"signatureState"[[:space:]]*:[[:space:]]*"developer-id-notarized"' "$manifest" || {
  echo "error: release is not declared signed and notarized" >&2
  exit 1
}

binary="$tmp/$root/bin/worklouderctl"
codesign --verify --strict --verbose=2 "$binary"
actual_version=$($binary version)
[ "$actual_version" = "worklouderctl $version" ] || {
  echo "error: installed binary reported an unexpected version: $actual_version" >&2
  exit 1
}

mkdir -p \
  "$prefix/bin" \
  "$prefix/share/bash-completion/completions" \
  "$prefix/share/zsh/site-functions" \
  "$prefix/share/fish/vendor_completions.d"
install -m 0755 "$binary" "$prefix/bin/worklouderctl"
install -m 0644 "$tmp/$root/completions/worklouderctl.bash" \
  "$prefix/share/bash-completion/completions/worklouderctl"
install -m 0644 "$tmp/$root/completions/_worklouderctl" \
  "$prefix/share/zsh/site-functions/_worklouderctl"
install -m 0644 "$tmp/$root/completions/worklouderctl.fish" \
  "$prefix/share/fish/vendor_completions.d/worklouderctl.fish"

installed_version=$($prefix/bin/worklouderctl version)
[ "$installed_version" = "worklouderctl $version" ] || {
  echo "error: post-install version check failed" >&2
  exit 1
}

echo "Installed $installed_version to $prefix/bin/worklouderctl"
case ":$PATH:" in
  *":$prefix/bin:"*) ;;
  *) echo "Add $prefix/bin to PATH before invoking worklouderctl." ;;
esac
