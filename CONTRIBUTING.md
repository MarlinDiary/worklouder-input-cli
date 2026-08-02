# Contributing to WorkLouderCTL

WorkLouderCTL is currently a source alpha with fixture and exact-release live
provider evidence. Contributions should strengthen the evidence, compatibility
model, transaction safety, provider integration, packaging, or command contract
without claiming support beyond the tested boundary.

## Good early contributions

- sanitized Input and device fixtures;
- schema and reference-validation tests;
- documented hardware observations with exact versions;
- provider discovery, parsing, and version-gated integration;
- deterministic CLI/JSON contract proposals;
- documentation corrections and translations.

## Evidence requirements

For behavior changes, include:

1. the exact Input, firmware, device, transport, and OS boundary;
2. a baseline that demonstrates the previous behavior;
3. the smallest focused change;
4. automated fixture checks;
5. hardware readback when the claim involves a device write;
6. rollback evidence for mutations.

Avoid committing personal Smart Action payloads, commands, application paths,
device identifiers, or unsanitized backups.

## Pull requests

- keep each pull request narrow;
- explain the user-visible behavior and compatibility boundary;
- update the compatibility policy or roadmap when the claim changes;
- use clear commit messages and include the validation commands/results.

Start with the commands exercised by CI:

```console
cargo fmt --check
cargo test --locked
cargo clippy --all-targets -- -D warnings
npm test --prefix companion
./scripts/verify-compatibility-matrix.sh
./scripts/test-release-packaging.sh
```

Run the relevant bridge or transaction E2E scripts for any changed authority.
Live-provider claims additionally require fresh version/hash detection,
pre-operation backups, exact readback, exact restore, and a documented final
ownership state. Keep private machine evidence outside the repository and
commit only deterministic sanitized fixtures.

## Dependency updates

Rust dependencies are deliberately pinned to preserve the declared Rust 1.61
minimum supported Rust version (MSRV). Apply Rust security updates individually
and require both the Rust 1.61 and current-stable CI jobs to pass. Major CLI or
manifest-format migrations, including Clap and TOML changes, are explicit
compatibility work rather than grouped automated upgrades.

All participation follows the [code of conduct](CODE_OF_CONDUCT.md).

By contributing, you agree that your contribution is licensed under the MIT
License.
