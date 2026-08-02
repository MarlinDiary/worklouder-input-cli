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
- [x] CLI command, configuration, transaction, and error JSON Schema registry

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
- [x] Typed exit statuses and JSON error envelopes

## M2 — Codex-native configuration beta

- [x] Exact Codex 26.727.51351 read contract and settings-schema adapter
- [x] `settings-read` and `settings-write` bridge client/reference integration
- [ ] Integrate the Codex Companion Bridge into a released Codex build
- [x] Agent source and single-tap behavior offline candidates
- [x] Six Agent Key task/command/Skill/keycap assignment snapshot and validation
- [x] Six Agent Key offline get/set/clear plus bridge apply/restore transaction
- [x] Six Command Key slots get/set/reset offline candidates
- [x] voice mode
- [x] dial modes and command/Skill/empty custom gestures
- [x] joystick directions and command/Skill/empty candidates
- [x] exact installed-build whole-layout reset candidate
- [x] global brightness and auto-off
- [x] Codex settings backup, apply, readback, restore, and automatic rollback fixture
- [x] Codex settings-only structural diff with revisions and typed before/after values

## M3 — Input semantic configuration beta

- [x] Strict offline snapshot validation and deterministic candidate rehash
- [x] Profile list/show/create/duplicate/rename/select/delete candidates
- [x] Layer list/show/create/duplicate/rename/delete/move and 24-bit RGB metadata candidates
- [x] Input-owned active profile/layer runtime observation and AppSense expectation test fixture
- [x] Existing keys and frozen Input 0.18.0 assignment tokens
- [x] Encoder rotation and press assignments
- [x] Existing joystick sector assignments
- [x] Input 0.18.0 radial joystick mode, two-sector seed, sector add/delete limits, and exact angle rebalance
- [x] Action and Multi Action reference validation plus profile usage synchronization
- [x] Layer backlight/underglow inspection, updates, and apply-to-all
- [x] Actions/macros list/show/create/rename/delete and event CRUD/reorder
- [x] Action group metadata, ordered member CRUD, and Input orphan cascade
- [x] Multi Action field CRUD, timing, groups, and reference cascade
- [x] Linked-app/AppSense list/show/link/update/unlink candidates and fixture transaction
- [x] Smart Action typed CRUD, groups, physical bindings, and delete cascade candidates
- [x] Input host command permission snapshot/candidate/apply/restore transaction
- [x] Cheat Sheet show/hold/hide/toggle catalog and binding candidates
- [x] Preset catalog snapshot/list/show/preview and exact Input 0.18.0 install remapping; all 17 bundled defaults candidate-verified
- [x] Radial-menu sector inspection and Action/Multi Action/Smart Action label resolution; mutation reuses verified joystick/control transactions
- [x] Input-owned OS permission status with exact platform semantics
- [x] Input-owned compatible firmware release check without duplicated release parsing
- [x] Bounded sanitized Input log bundle with private modes and checksum readback

## M4 — Full cross-authority transaction

- [x] Smart Actions and groups (offline candidate and current-cache format verified)
- [x] Input cache read adapter with bridge-equivalent semantic snapshot/revision
- [ ] Input database adapter
- [ ] Input process coordination
- [x] Codex settings-bridge protocol, client, and reference process integration
- [ ] Released Codex process integration
- [x] Private backup catalog with atomic publication and mode-0700/0600 containment
- [x] Plan/apply all-authority CAS conflict detection and retry drift rejection
- [x] Exact provider readback plus coordinated all-authority postflight
- [x] Automatic reverse rollback and manual restore with roll-forward recovery

## M5 — Stable distribution

- [ ] Signed macOS binaries
- [ ] Homebrew formula
- [ ] Shell completions and generated command reference
- [ ] Compatibility matrix for every release
- [ ] Migration and backup inspection tools
- [ ] Agent-facing protocol over the same transaction core

## M6 — Delegated Input operations

- [x] Hash-pinned Input 0.18.0 permission, firmware-read, log, reset, and update-flow inventory
- [x] Optional permission, firmware status, and sanitized log bridge authorities
- [ ] Input-owned high-level firmware update authority with backup, USB/bootloader flash, reconnect, restore, and postflight
- [ ] Complete default/reset candidate per device/layout/version through the existing configuration transaction
- [ ] Bootloader recovery authority and exact post-recovery configuration restore

## Later tracks

- Linux transport and packaging
- Windows transport and contention testing
- additional Work Louder devices through explicit adapters
- capability discovery for new Codex/Input versions
- delegated firmware update and recovery orchestration through Input
