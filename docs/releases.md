# macOS releases and Homebrew packaging

WorkLouderCTL's macOS release pipeline produces deterministic archives for
Apple Silicon and Intel Macs. A release archive contains the CLI, generated
Bash/Zsh/Fish completions, the license, the README, and a machine-readable
manifest with the SHA-256 and mode of every file.

The packaging pipeline is complete and fixture-verified. The repository is
still a **source alpha** until the first version tag is signed, notarized, and
published. Do not interpret a local unsigned or ad-hoc build as an official
signed release.

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
`developer-id-notarized`. A manual untagged workflow run produces unsigned test
artifacts and does not publish a GitHub release.

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
and Homebrew formula syntax/style together:

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

The workflow imports the certificate into an ephemeral keychain, signs both
architectures, submits each binary to Apple notarization, verifies the release
archives, builds and independently installs the Companion integration kit,
emits build-provenance attestations for all three packages, renders the
release-specific Homebrew formula, recomputes `SHA256SUMS`, and finally creates
the GitHub release. Missing credentials or any verification mismatch stops
publication.

## Homebrew formula

`packaging/homebrew/worklouderctl.rb.in` is the checksum-pinned formula
template. A tagged workflow renders `worklouderctl.rb` only after both release
archives have been built and verified. The generated formula installs the
binary and all three shell completions and runs `worklouderctl version` in its
test block.

After a formula has been published in a tap, install it with:

```console
brew tap OWNER/TAP
brew install worklouderctl
worklouderctl version
```

Until that first tap publication, use the source build or a verified archive;
there is no stable `brew install` command to advertise yet.

## Release checklist

1. Run `./scripts/release-preflight.sh vVERSION`. It verifies the clean/pushed
   commit, exact Cargo/tag match, tag availability, Apple secret names, main
   branch protection, and successful CI for the exact commit without printing
   secret values.
2. Keep the working tree clean and run every required gate in
   `spec/compatibility-matrix-v1.json`.
3. Before creating the tag, run the `Release` workflow manually on `main`. This
   exercises both native architecture package jobs with unsigned artifacts but
   intentionally skips publication.
4. Push an annotated version tag only after the preflight and manual package
   rehearsal pass.
5. Require both architecture jobs, notarization, archive verification, and
   provenance attestation to pass.
6. Download and independently verify both published archives and checksums.
7. Verify, install, import, and execute the Companion `.tgz` with
   `./scripts/test-companion-package.py`'s exact inventory/export boundary.
8. Install the generated formula into an isolated Homebrew prefix and run its
   test before promoting it to a stable tap.

Rollback for an unpublished release is deletion of the local output directory.
For a faulty published version, mark the GitHub release as withdrawn, remove
the formula version from the tap, and publish a new version; never replace an
existing checksum-pinned asset in place.
