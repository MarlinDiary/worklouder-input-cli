# Changelog

All notable changes to WorkLouderCTL are documented in this file. The project
uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1]

### Added

- `worklouderctl appsense relay install/status/test/sync/remove`, a persistent
  macOS focus relay with authenticated socket reuse, functional health state,
  bounded timeout retry, and serialized bridge recovery.
- Serialized provider leases with RPC health receipts, guarded configuration
  writes, readback, automatic restore, and persistent idempotency.
- Codex `26.730.61309` exact-release contracts, including independent ACT10 and
  ACT11 microphone keys and `codex voice separate-keys get/set/reset`.
- Persistent `codex.runtime.status.v1` and `codex.runtime.recover.v1` bridge
  capabilities replace per-command process-signal inspection; provider assets
  are content addressed and Codex ESM overlays use revisioned module roots.
- Owner-preserving `device status/files/export` and `device config
  snapshot/validate/apply/restore` commands. `--owner auto` now leases Input's
  authoritative configuration session and restores a pre-existing Codex owner.
- `transaction apply/restore --device-owner auto|codex|input`, allowing Input
  to perform device configuration while restoring the requested final owner.
- Public `appsense-relay-v1` and `codex-owner-config-v1` JSON Schemas.
- A checksum-, inventory-, manifest-, signature-, and version-verifying binary
  installer plus a stable `MarlinDiary/tap/worklouderctl` Homebrew distribution
  path with automatic formula publication from tagged releases.
- Independent Homebrew Tap CI that audits, installs, verifies the Developer ID
  signature, and tests the public formula on every change and on a weekly
  schedule.

### Fixed

- AppSense can now return from an application layer to the Codex layer without
  stopping Codex's device service or handing the USB session to Input.
- Device configuration now treats Input as the byte-authoritative provider and
  restores Codex automatically after status, export, validation, apply, restore,
  or any failed Input acquisition.
- Provider acquisition requires a real RPC response in addition to connection
  counters and subscriptions, preventing a `detected` or stale native handle
  from being reported as ready.
- Codex auto-updates fail closed on the pinned app/hash/chunk contract; the
  provider runtime is now adapted to `26.730.61309` and its new microphone-key
  layout without changing the installed application bundle.
- Repeated Codex runtime checks keep the GUI PID stable. A live
  `connection-failed` service recovered to `connected` and passed three
  consecutive subscription-health readbacks through the persistent bridge.
- AppSense relay timeouts retry the existing Codex service instead of
  reinstalling the bridge; genuine transport loss performs one serialized
  self-healing reinstall and leaves a diagnostic health record.
- `appsense relay test` now closes its one-shot persistent bridge client after
  success or failure, so the verification command exits immediately.
- AppSense focus forwarding and provider configuration now use serialized
  per-user locks, preventing frontmost-app events from racing provider leases;
  `lsappinfo` event payloads are no longer
  misinterpreted as timer delays.
- Multi-file device changes require the firmware transaction primitive and
  fail before the first write when it is absent. Failed writes first read back
  state and skip unnecessary rollback writes when the baseline is intact.
- Self-contained transaction restore resolves owner routing from the archived
  private plan, so restore still succeeds after the original plan inputs have
  been deleted.
- `config diff` now compares validated files embedded in Input configuration
  snapshots, so differently named `before.json` and `candidate.json` files
  report precise `/keymap.json` and `/smart_actions.json` changes.
- Input `control set` and the commands that delegate to it now keep the Codex
  protected layer read-only, preserving its reserved `KV_OAI_*` protocol
  assignments and routing customization to the Codex-native command family.
- The generated Homebrew formula now derives its version from the release URL,
  removing the redundant explicit `version` rejected by `brew audit --strict`.

### Security

- Device mutation idempotency bindings persist in a mode-`0700`
  directory and mode-`0600` atomic registry; a key cannot be reused for a
  different baseline, target, or operation.

### Verified boundary

- Rust unit/CLI tests, 49 Node bridge/runtime tests, JSON-contract checks, all
  provider fixtures, and the complete bridge/firmware/reset/recovery/four-
  authority transaction E2E suite pass for this release.
- Codex `26.730.61309` static release identity, settings schema, runtime chunks,
  subscribed provider recovery, RPC health gates, and automatic rollback to
  Codex are verified separately in the release verification record. The device
  configuration writer remains Input `0.18.0`.

## [0.1.0]

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

### Changed

- Promoted the public project status to a configuration-parity release,
  condensed both READMEs around real workflows, and aligned repository metadata,
  compatibility state, contributor guidance, security policy, and agent-facing
  documentation with the implemented feature boundary.

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

- Drivers, firmware programmers, device discovery, and runtime ownership remain
  delegated to the installed Codex and Input applications.
- Input `0.18.0` can trap in its native `node-hid` re-enumeration path on the
  tested macOS 27 beta. WorkLouderCTL uses serialized ownership leases, a fresh
  hidden Input process, quiescence checks, and rollback to contain that provider
  behavior.

[Unreleased]: https://github.com/MarlinDiary/worklouder-input-cli/commits/main
[0.1.1]: https://github.com/MarlinDiary/worklouder-input-cli/releases/tag/v0.1.1
[0.1.0]: https://github.com/MarlinDiary/worklouder-input-cli/releases/tag/v0.1.0
