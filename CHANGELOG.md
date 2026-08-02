# Changelog

All notable changes to WorkLouderCTL are documented in this file. The project
uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-08-03

### Added

- A Rust CLI with stable human-readable output, deterministic JSON envelopes,
  typed exit statuses, generated Bash/Zsh/Fish completions, and an exhaustive
  generated command reference.
- Tier 1 Codex configuration for Agent source, Agent Keys, Command Keys, voice,
  lighting, dial, joystick, runtime recovery, layout reset, snapshots, diffs,
  compare-and-swap apply, readback, and exact restore.
- Tier 2-4 Input configuration for profiles, layers, colors, lighting, physical
  controls, Actions, Multi Actions, groups, Smart Actions, AppSense, presets,
  radial menus, host permissions, diagnostics, firmware planning, reset, and
  recovery delegation.
- Authenticated Codex and Input Companion Bridge contracts, reference adapters,
  exact-release live overlays, and coordinated provider handoff without
  activating or navigating either GUI.
- First-class `worklouderctl provider` commands that materialize the private
  embedded exact-release runtime and delegate bridge install/remove plus
  status/acquire/release/handoff without a shell.
- Four-authority transactions with immutable backups, revision gates,
  idempotency keys, postflight verification, automatic reverse rollback, and
  manual restore.
- Deterministic sanitized fixtures, JSON Schemas, a release compatibility
  matrix, and a shell-free agent execution protocol.
- Deterministic Apple Silicon and Intel archives, signature-state verification,
  fail-closed Developer ID signing/notarization, build-provenance attestations,
  a checksum-pinned Homebrew formula, and a deterministic Companion integration
  package.

### Security

- Disabled Clap 3's optional terminal-color feature, removing the unmaintained
  `atty` dependency affected by `GHSA-g98v-hv3f-hcfr` while preserving the
  declared Rust 1.61 minimum version and deterministic help output.
- Updated release actions to their Node.js 24 generations and pinned every
  external GitHub Action to an immutable full commit with a checked version
  annotation.

### Verified boundary

- The released Codex `26.727.51351` and Input `0.18.0` applications were
  live-validated with a USB Codex Micro: Codex settings, Agent Keys, Input
  device configuration, and Input host settings each completed
  apply/readback/exact-restore transactions.
- Provider ownership was exercised in both directions and recovered to a
  single Codex-owned USB/HID/joystick session. Input reconnect portability was
  verified across session-local device identifiers.
- Input `0.18.0` did not expose the optional preset, reset, firmware-update, or
  bootloader-recovery bridge authorities. Those commands fail closed when the
  capability is absent; their end-to-end behavior is covered by deterministic
  provider fixtures rather than the released Input writer.

### Known limitations

- Version `0.1.0` is a macOS/Codex Micro source alpha until its first signed and
  notarized GitHub release is published.
- Drivers, firmware programmers, device discovery, and runtime ownership remain
  delegated to the installed Codex and Input applications.
- Input `0.18.0` can trap in its native `node-hid` re-enumeration path on the
  tested macOS 27 beta. WorkLouderCTL uses serialized ownership leases, a fresh
  hidden Input process, quiescence checks, and rollback to contain that provider
  behavior.

[Unreleased]: https://github.com/MarlinDiary/worklouder-input-cli/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/MarlinDiary/worklouder-input-cli/releases/tag/v0.1.0
