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
- [x] Publish deterministic sanitized Input 0.17.3/0.18.0 and firmware v0.6.0 fixtures with manifest verification
- [x] CLI command, doctor, configuration, transaction, and error JSON Schema registry

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
- [x] Structural `validate` and `diff`
- [x] JSON output
- [x] Typed exit statuses and JSON error envelopes

## M2 — Codex-native configuration beta

- [x] Exact Codex 26.727.51351 read contract and settings-schema adapter
- [x] `settings-read` and `settings-write` bridge client/reference integration
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
- [x] Input-owned cache/database adapter contract through configuration writer and ApplicationService; no direct LokiJS writes
- [x] Input process coordination through the in-process serialized bridge; direct compatibility reads retain explicit quit/reopen
- [x] Codex settings-bridge protocol, client, and reference process integration
- [x] Private backup catalog with atomic publication and mode-0700/0600 containment
- [x] Plan/apply all-authority CAS conflict detection and retry drift rejection
- [x] Exact provider readback plus coordinated all-authority postflight
- [x] Automatic reverse rollback and manual restore with roll-forward recovery
- [x] One full-parity fixture spanning all four tiers, unified diff, semantic post-state, observed fixture behavior, and exact rollback

## M5 — Stable distribution

- [x] Deterministic Apple Silicon and Intel archives with exact manifest, checksum, and executable verification
- [x] Explicit unsigned, ad-hoc, Apple Development, Developer ID, and notarized signature-state validation
- [x] Fail-closed Developer ID signing, Apple notarization, and build-provenance release workflow
- [x] Checksum-pinned Homebrew formula generation, syntax/style checks, isolated-prefix install, and execution test
- [x] Deterministic Companion integration bundle with exact inventory, installed exports, conformance binary, checksum, and provenance workflow
- [x] Deterministic Bash/Zsh/Fish completions and exhaustive generated command reference
- [x] Machine-readable compatibility matrix and Cargo-version gate for every release
- [x] Strict backup inspection and schema migration assessment tools
- [x] Shell-free agent JSON envelope over the same parser, typed statuses, and transaction core

## M6 — Delegated Input operations

- [x] Hash-pinned Input 0.18.0 permission, firmware-read, log, reset, and update-flow inventory
- [x] Optional permission, firmware status/plan, and sanitized log bridge authorities
- [x] Input-owned high-level firmware update authority with backup, USB/bootloader flash, reconnect, restore, and postflight
- [x] Input-owned complete default/reset candidate per device/layout/version through the existing configuration transaction, with immutable plan, idempotent apply, readback, and exact rollback
- [x] Bootloader recovery authority and exact post-recovery configuration restore, with immutable backup-bound plan, Input programmer delegation, receipt inspection, and idempotent replay

## External release gates

These gates change vendor or credential-owned state rather than this
repository. They are intentionally tracked separately from implementation:

- an official Input release adopts the packaged Input Companion Bridge and
  supplies its complete configuration writer;
- a released Codex build adopts the packaged Codex Companion Bridge and
  supplies exact settings and Agent Key replacers;
- the project owner provisions Developer ID/notarization credentials, creates
  the first version tag, verifies the published binaries, and promotes the
  generated formula to a stable Homebrew tap;
- official-provider USB mutation and physical-device rollback evidence is
  frozen after those released integrations exist.

## Later tracks

- Linux transport and packaging
- Windows transport and contention testing
- additional Work Louder devices through explicit adapters
- capability discovery for new Codex/Input versions
- signed Windows/Linux distribution after their provider boundaries exist
