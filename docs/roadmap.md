# Roadmap

The roadmap prioritizes evidence and configuration safety over command count.

## M0 — Product and evidence foundation

- [x] Repository name and product positioning
- [x] SEO/AIO README and FAQ foundation
- [x] Compatibility and architecture policies
- [ ] Sanitized Input 0.17.3 and firmware v0.6.0 fixtures
- [ ] Baseline checksums and verification records
- [ ] CLI contract and JSON schemas

## M1 — Read-only alpha

- [ ] `worklouderctl version`
- [ ] `worklouderctl doctor`
- [ ] `worklouderctl input inspect`
- [ ] `worklouderctl device status`
- [ ] `worklouderctl device files`
- [ ] `worklouderctl device export`
- [ ] Structural `show`, `validate`, and `diff`
- [ ] JSON output and typed exit statuses

## M2 — Semantic configuration beta

- [ ] Profiles and active profile
- [ ] Layers and layer metadata
- [ ] Keys and keycodes
- [ ] Encoder rotation and press
- [ ] Joystick sectors
- [ ] Layer lighting
- [ ] Actions/macros and groups
- [ ] Multi Actions and reference validation
- [ ] Linked apps

## M3 — Full Input companion transaction

- [ ] Smart Actions and groups
- [ ] Input cache adapter
- [ ] Input database adapter
- [ ] Input process coordination
- [ ] Private backup catalog
- [ ] Plan/apply conflict detection
- [ ] Exact readback and checksum verification
- [ ] Automatic and manual rollback

## M4 — Stable distribution

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
