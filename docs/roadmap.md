# Roadmap

The roadmap prioritizes evidence and configuration safety over command count.

## M0 — Product and evidence foundation

- [x] Repository name and product positioning
- [x] SEO/AIO README and FAQ foundation
- [x] Compatibility and architecture policies
- [x] Tier 1 Codex / Tier 2+ Input authority contract
- [x] Codex 26.727.51351 and Input 0.18.0 feature inventory
- [x] Baseline checksums and reconnect verification record
- [x] Machine-readable capability registry
- [x] Full Codex + Input configuration parity contract
- [ ] Publish sanitized Input 0.17.3/0.18.0 and firmware v0.6.0 fixtures
- [ ] CLI command and configuration JSON schemas

## M1 — Read-only alpha

- [ ] `worklouderctl version`
- [ ] `worklouderctl doctor`
- [ ] `worklouderctl input inspect`
- [ ] `worklouderctl codex inspect`
- [ ] `worklouderctl codex export`
- [ ] `worklouderctl tier explain`
- [ ] `worklouderctl device status`
- [ ] `worklouderctl device files`
- [ ] `worklouderctl device export`
- [ ] Structural `show`, `validate`, and `diff`
- [ ] JSON output and typed exit statuses

## M2 — Codex-native configuration beta

- [ ] Exact Codex version and settings-schema adapter
- [ ] `settings-read` and `settings-write` bridge client
- [ ] Agent source and six Agent Key assignments
- [ ] six Command Key slots and reset
- [ ] voice mode
- [ ] dial modes and custom gestures
- [ ] joystick directions and Skills
- [ ] global brightness and auto-off
- [ ] Codex settings backup, diff, apply, readback, and rollback

## M3 — Input semantic configuration beta

- [ ] Profiles and active profile
- [ ] Layers and layer metadata
- [ ] Keys and keycodes
- [ ] Encoder rotation and press
- [ ] Joystick sectors
- [ ] Layer lighting
- [ ] Actions/macros and groups
- [ ] Multi Actions and reference validation
- [ ] Linked apps

## M4 — Full cross-authority transaction

- [ ] Smart Actions and groups
- [ ] Input cache adapter
- [ ] Input database adapter
- [ ] Input process coordination
- [ ] Codex process and settings-bridge coordination
- [ ] Private backup catalog
- [ ] Plan/apply conflict detection
- [ ] Exact readback and checksum verification
- [ ] Automatic and manual rollback

## M5 — Stable distribution

- [ ] Signed macOS binaries
- [ ] Homebrew formula
- [ ] Shell completions and generated command reference
- [ ] Compatibility matrix for every release
- [ ] Migration and backup inspection tools
- [ ] Agent-facing protocol over the same transaction core

## Later tracks

- Linux transport and packaging
- Windows transport and contention testing
- additional Work Louder devices through explicit adapters
- standalone user-session daemon
- firmware update and recovery tooling
