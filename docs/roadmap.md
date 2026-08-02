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

- [x] `worklouderctl version`
- [x] `worklouderctl doctor`
- [x] `worklouderctl input inspect`
- [x] `worklouderctl input export`
- [x] `worklouderctl input config snapshot`
- [x] `worklouderctl codex doctor`
- [x] `worklouderctl codex inspect`
- [x] `worklouderctl codex export`
- [x] `worklouderctl tier explain`
- [x] `worklouderctl device status`
- [x] `worklouderctl device files`
- [x] `worklouderctl device export`
- [x] Read-only Input process coordination and automatic reopen
- [x] Input Companion Bridge v1 contract and authenticated Unix-socket client
- [x] Input-main reference server and Input 0.18.0 service adapter
- [x] One-call Input main-process integration and release conformance command
- [x] Cross-language bridge status/files/export conformance test
- [x] Revisioned config snapshot and live compare-and-swap validation
- [x] Fixture-verified bridge apply/restore, idempotent retry, and auto rollback
- [ ] Integrate the bridge adapter into an official Input release
- [x] Structural `validate` and `diff`
- [x] JSON output
- [ ] Typed exit statuses

## M2 — Codex-native configuration beta

- [x] Exact Codex 26.727.51351 read contract and settings-schema adapter
- [x] `settings-read` and `settings-write` bridge client/reference integration
- [ ] Integrate the Codex Companion Bridge into a released Codex build
- [x] Agent source and single-tap behavior offline candidates
- [x] Six Agent Key task/command/Skill/keycap assignment snapshot and validation
- [ ] Six Agent Key assignment mutation commands
- [x] Six Command Key slots get/set/reset offline candidates
- [ ] voice mode
- [ ] dial modes and custom gestures
- [ ] joystick directions and Skills
- [ ] global brightness and auto-off
- [x] Codex settings backup, apply, readback, restore, and automatic rollback fixture
- [ ] Codex settings structural diff command

## M3 — Input semantic configuration beta

- [x] Strict offline snapshot validation and deterministic candidate rehash
- [x] Profile list/show/create/duplicate/rename/select/delete candidates
- [x] Layer list/show/create/duplicate/rename/delete/move and 24-bit RGB metadata candidates
- [ ] Live active-layer selection
- [x] Existing keys and frozen Input 0.18.0 assignment tokens
- [x] Encoder rotation and press assignments
- [x] Existing joystick sector assignments
- [x] Action and Multi Action reference validation plus profile usage synchronization
- [x] Layer backlight/underglow inspection, updates, and apply-to-all
- [x] Actions/macros list/show/create/rename/delete and event CRUD/reorder
- [x] Action group metadata, ordered member CRUD, and Input orphan cascade
- [x] Multi Action field CRUD, timing, groups, and reference cascade
- [x] Linked-app/AppSense list/show/link/update/unlink candidates and fixture transaction
- [x] Smart Action typed CRUD, groups, physical bindings, and delete cascade candidates

## M4 — Full cross-authority transaction

- [x] Smart Actions and groups (offline candidate and current-cache format verified)
- [x] Input cache read adapter with bridge-equivalent semantic snapshot/revision
- [ ] Input database adapter
- [ ] Input process coordination
- [x] Codex settings-bridge protocol, client, and reference process integration
- [ ] Released Codex process integration
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
- capability discovery for new Codex/Input versions
- delegated firmware update and recovery orchestration through Input
