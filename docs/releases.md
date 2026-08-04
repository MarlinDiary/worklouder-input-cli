# macOS releases and Homebrew packaging

WorkLouderCTL's macOS release pipeline produces deterministic archives for
Apple Silicon and Intel Macs. A release archive contains the CLI, generated
Bash/Zsh/Fish completions, the license, the README, and a machine-readable
manifest with the SHA-256 and mode of every file.

The packaging pipeline is complete and fixture-verified. The official
[`v0.1.1`](https://github.com/MarlinDiary/worklouder-input-cli/releases/tag/v0.1.1)
release publishes signed and notarized Apple Silicon and Intel archives. A
local unsigned or ad-hoc build remains distinct from an official signed release.

## Install an official release

The stable tap installs the formula with
[item-scoped trust on Homebrew 6](https://docs.brew.sh/Tap-Trust):

```console
brew tap MarlinDiary/tap
brew install MarlinDiary/tap/worklouderctl
worklouderctl version
```

The standalone installer downloads the correct architecture and checks the
sidecar checksum, exact inventory, manifest identity, `developer-id-notarized`
state, code signature, and reported version before writing the binary and
shell completions:

```console
curl -fsSLO https://raw.githubusercontent.com/MarlinDiary/worklouder-input-cli/main/install.sh
sh install.sh
~/.local/bin/worklouderctl version
```

The default prefix is `~/.local`. Run `sh install.sh --help` to pin a release
or choose another prefix.

The same tagged release also contains a deterministic Companion Bridge `.tgz`
integration kit. That package lets Codex/Input maintainers install the exact
reference adapters, one-call main-process integrations, and read-only
conformance executable without copying source files manually. It remains a
private release asset rather than an npm-registry publication.

## Verify a downloaded release

Download the archive and its adjacent `.sha256` file, then run:

```console
shasum -a 256 -c worklouderctl-vVERSION-TARGET.tar.gz.sha256
./scripts/verify-release-archive.py \
  worklouderctl-vVERSION-TARGET.tar.gz --expected-target TARGET --execute
./scripts/verify-macos-release.sh worklouderctl-vVERSION-TARGET.tar.gz
shasum -a 256 -c \
  worklouder-input-companion-bridge-reference-VERSION.tgz.sha256
```

`TARGET` is either `aarch64-apple-darwin` or `x86_64-apple-darwin`.
Verification checks the sidecar checksum, exact archive inventory, manifest,
per-file hashes and modes, target, CLI execution, and the declared macOS code
signature state.

Verify the integration package from a source checkout, or install the already
checksum-verified release tgz directly:

```console
./scripts/test-companion-package.py
npm install --ignore-scripts --no-audit --no-fund \
  ./worklouder-input-companion-bridge-reference-VERSION.tgz
./node_modules/.bin/input-companion-conformance --help
```

`private: true` blocks accidental npm-registry publication; the reviewed,
checksum-pinned GitHub release asset is the distribution mechanism.

## Signature states

The archive contract records one of five states; they are intentionally not
interchangeable:

| State | Meaning | Public distribution |
| --- | --- | --- |
| `unsigned` | No macOS code signature | Local testing only |
| `ad-hoc` | Local ad-hoc code signature | Local testing only |
| `apple-development` | Apple Development certificate | Development testing only |
| `developer-id` | Developer ID Application certificate | Signed but not notarized |
| `developer-id-notarized` | Developer ID Application plus accepted Apple notarization | Required for tagged releases |

The release workflow fails closed: a `v*` tag must exactly match the Cargo
version, have one matching Developer ID Application identity, pass signing and
notarization, and produce an archive whose signature state is
`developer-id-notarized`. That state requires the accepted JSON result emitted
by the exact `notarytool submit --wait` invocation; a declared state alone is
rejected. A manual untagged workflow run produces unsigned test artifacts by
default. Enable its `signed_validation` input to exercise the real Developer ID
import, signing, and Apple notarization path for both architectures without
creating a tag or publishing a GitHub release.

## Build a local archive

Build an unsigned archive using the host Rust target:

```console
./scripts/build-macos-release.sh ./dist-local
```

Build an ad-hoc signed archive without using a certificate:

```console
WORKLOUDERCTL_CODESIGN_IDENTITY=- \
  ./scripts/build-macos-release.sh ./dist-adhoc
```

For an Apple Development or Developer ID identity, pass its exact keychain
name in `WORKLOUDERCTL_CODESIGN_IDENTITY`. The script signs a staging copy; it
does not modify `target/.../release/worklouderctl`.

To test deterministic packaging, archive verification, local-prefix install,
and Homebrew formula version derivation, syntax, and style together:

```console
./scripts/test-release-packaging.sh
```

## Tagged release credentials

The `Release` GitHub Actions workflow requires these repository secrets for a
tagged release:

- `APPLE_DEVELOPER_ID_CERTIFICATE_P12_BASE64`
- `APPLE_DEVELOPER_ID_CERTIFICATE_PASSWORD`
- `APPLE_SIGNING_IDENTITY`
- `APPLE_ID`
- `APPLE_TEAM_ID`
- `APPLE_APP_SPECIFIC_PASSWORD`
- `HOMEBREW_TAP_DEPLOY_KEY`

The workflow imports the certificate into an ephemeral keychain, signs both
architectures, submits each binary to Apple notarization, verifies the release
archives, builds and independently installs the Companion integration kit,
emits build-provenance attestations for all three packages, renders the
release-specific Homebrew formula, recomputes `SHA256SUMS`, and finally creates
the GitHub release. It then pushes the checksum-pinned formula with the scoped
`MarlinDiary/homebrew-tap` deploy key after `brew audit --strict` passes.
Missing credentials or any verification mismatch stops the affected
publication step.

## Homebrew formula

`packaging/homebrew/worklouderctl.rb.in` is the checksum-pinned formula
template. A tagged workflow renders `worklouderctl.rb` only after both release
archives have been built and verified. The generated formula installs the
binary and all three shell completions and runs `worklouderctl version` in its
test block.

The tap has an independent macOS workflow that runs Ruby syntax, `brew style`,
`brew audit --strict`, a clean formula install, Developer ID signature
verification, and `brew test` on every push and pull request and once weekly.

The generated formula is published to
[`MarlinDiary/homebrew-tap`](https://github.com/MarlinDiary/homebrew-tap). Use
the fully qualified name so Homebrew trusts only this formula rather than every
current and future item in the tap:

```console
brew tap MarlinDiary/tap
brew install MarlinDiary/tap/worklouderctl
worklouderctl version
```

## Release checklist

1. Run `./scripts/release-preflight.sh vVERSION`. It verifies the clean/pushed
   commit, exact Cargo/tag match, tag availability, Apple secret names, main
   branch protection, and successful CI for the exact commit without printing
   secret values.
2. Keep the working tree clean and run every required gate in
   `spec/compatibility-matrix-v1.json`.
3. Before creating the tag, run the `Release` workflow manually on `main` with
   `signed_validation` enabled. This exercises both native architecture package
   jobs, repository secrets, Developer ID signing, Apple notarization, archive
   verification, and attestations while intentionally skipping publication.
4. Push an annotated version tag only after the preflight and manual package
   rehearsal pass.
5. Require both architecture jobs, notarization, archive verification, and
   provenance attestation to pass.
6. Download and independently verify both published archives and checksums.
7. Verify, install, import, and execute the Companion `.tgz` with
   `./scripts/test-companion-package.py`'s exact inventory/export boundary.
8. Confirm the release workflow committed the generated formula to the stable
   tap, then run `brew install MarlinDiary/tap/worklouderctl` and `brew test
   MarlinDiary/tap/worklouderctl` from a clean tap state.

Rollback for an unpublished release is deletion of the local output directory.
For a faulty published version, mark the GitHub release as withdrawn, remove
the formula version from the tap, and publish a new version; never replace an
existing checksum-pinned asset in place.
