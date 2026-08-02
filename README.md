# WorkLouderCTL — Work Louder Input CLI for Codex Micro

<p align="center">
  <strong>The open-source full-configuration CLI for Codex Micro, Codex, and Work Louder Input.</strong>
</p>

<p align="center">
  <a href="README.zh-CN.md">简体中文</a> ·
  <a href="docs/faq.md">FAQ</a> ·
  <a href="docs/compatibility.md">Compatibility</a> ·
  <a href="docs/architecture.md">Architecture</a> ·
  <a href="docs/roadmap.md">Roadmap</a>
</p>

<p align="center">
  <img alt="Project status: source alpha" src="https://img.shields.io/badge/status-source%20alpha-F59E0B">
  <img alt="Target platform: macOS first" src="https://img.shields.io/badge/platform-macOS%20first-111827">
  <img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-22C55E">
</p>

> [!IMPORTANT]
> **Project status: source alpha.** The repository now builds a working
> `worklouderctl` binary with provider diagnostics, Codex Micro settings
> inspection/export, Input inspection/exact-byte export, validation, structural
> diff, live device status/file reads, verified device export, revisioned bridge
> snapshots/CAS validation, offline profile/layer/AppSense/control/Action/Smart
> Action candidate generation, fixture-verified Input and Codex apply/restore
> transactions, six-slot Codex Agent Key candidates and transactions, JSON output, and
> shell completions.
> There is no packaged release yet.
> The bridge transaction engine now verifies backup, apply, idempotent retry,
> readback, restore, and automatic rollback against an isolated writer fixture.
> Released-app mutation remains gated on Codex/Input integrations that supply
> verified complete-set writers; hardware mutation additionally requires exact
> device rollback evidence.

## The short answer

Looking for a **Work Louder Input CLI**, a **Codex Micro command-line
configurator**, or a safe way for an **AI agent to configure a Work Louder
macropad**? WorkLouderCTL is being built for that job.

WorkLouderCTL is an unofficial, open-source full-configuration CLI. Its product
contract is configuration parity with both the Codex Micro settings page in
Codex and every Codex Micro surface in Work Louder Input. The GUIs become
optional for configuration; Codex and Input may still provide the live runtime
for Codex-aware actions, AppSense, Smart Actions, and reactive lighting.

WorkLouderCTL does **not** implement a replacement keyboard driver, BLE/HID
stack, firmware protocol, or host-action runtime. It delegates those jobs to
the installed Codex and Input versions, so vendor updates remain available.

## Why a full-configuration CLI?

The official Input app is useful for visual editing. A CLI adds workflows that
are difficult to make repeatable in a GUI:

- review a complete configuration as text;
- generate and approve an exact diff before a write;
- back up profiles, layers, keymaps, and Smart Actions;
- apply the same layout reproducibly;
- verify device readback and checksums;
- roll back a failed or unwanted change;
- let scripts and AI agents use a stable, machine-readable interface.

WorkLouderCTL is designed to **replace both configuration GUIs while
cooperating with their runtimes**. A guarded mutation coordinates Codex and
Input, preserves every state authority, writes only after validation, verifies
the result, and refreshes or reopens the affected runtime.

## Build and run the current CLI

The declared minimum Rust version is 1.61:

```console
git clone https://github.com/MarlinDiary/worklouder-input-cli.git
cd worklouder-input-cli
cargo build --release --locked
./target/release/worklouderctl doctor
```

Currently implemented commands:

```console
worklouderctl version
worklouderctl tier list
worklouderctl tier explain 1
worklouderctl capability list --tier 2
worklouderctl doctor [--strict]
worklouderctl codex doctor [--strict]
worklouderctl codex inspect
worklouderctl codex export --output CODEX_SNAPSHOT.json
worklouderctl codex bridge inspect
worklouderctl codex config snapshot --output CODEX_SNAPSHOT.json
worklouderctl codex config apply --input CODEX_CANDIDATE.json --backup CODEX_BEFORE.json
worklouderctl codex config restore --input CODEX_BEFORE.json --backup CODEX_CURRENT.json
worklouderctl codex agent-key assignments
worklouderctl codex agent-key snapshot --output AGENT_KEYS.json
worklouderctl codex agent-key get --input AGENT_KEYS.json AG00
worklouderctl codex agent-key set --input AGENT_KEYS.json AG01 --command COMMAND_ID --output AGENT_CANDIDATE.json
worklouderctl codex agent-key clear --input AGENT_CANDIDATE.json AG00 --output AGENT_CLEARED.json
worklouderctl codex agent-key apply --input AGENT_CLEARED.json --backup AGENT_BEFORE.json
worklouderctl codex agent-key restore --input AGENT_BEFORE.json --backup AGENT_CURRENT.json
worklouderctl codex agent-source get --input CODEX_SNAPSHOT.json
worklouderctl codex agent-source set --input CODEX_SNAPSHOT.json priority --output CODEX_CANDIDATE.json
worklouderctl codex agent-key tap-mode get --input CODEX_SNAPSHOT.json
worklouderctl codex agent-key tap-mode set --input CODEX_SNAPSHOT.json enabled --output CODEX_CANDIDATE.json
worklouderctl codex command-key get --input CODEX_SNAPSHOT.json ACT06
worklouderctl codex command-key set --input CODEX_SNAPSHOT.json ACT06 --keycap BUG --command COMMAND_ID --output CODEX_CANDIDATE.json
worklouderctl codex command-key reset --input CODEX_CANDIDATE.json ACT06 --output CODEX_RESET.json
worklouderctl codex dial mode get --input CODEX_SNAPSHOT.json
worklouderctl codex dial mode set --input CODEX_SNAPSHOT.json custom --output CODEX_DIAL.json
worklouderctl codex dial gesture set --input CODEX_DIAL.json left --command navigateBack --output CODEX_DIAL_LEFT.json
worklouderctl codex dial gesture set --input CODEX_DIAL_LEFT.json right --skill-name Review --skill-path /PATH/TO/SKILL.md --output CODEX_DIAL_RIGHT.json
worklouderctl codex dial gesture get --input CODEX_DIAL_RIGHT.json right
worklouderctl codex dial gesture clear --input CODEX_DIAL_RIGHT.json left --output CODEX_DIAL_CLEARED.json
worklouderctl codex joystick get --input CODEX_SNAPSHOT.json up
worklouderctl codex joystick set --input CODEX_SNAPSHOT.json up --skill-name Plan --skill-path /PATH/TO/SKILL.md --output CODEX_JOYSTICK_UP.json
worklouderctl codex joystick set --input CODEX_JOYSTICK_UP.json right --command navigateForward --output CODEX_JOYSTICK_RIGHT.json
worklouderctl codex joystick clear --input CODEX_JOYSTICK_RIGHT.json down --output CODEX_JOYSTICK_CLEARED.json
worklouderctl codex reset layout --input CODEX_JOYSTICK_CLEARED.json --output CODEX_LAYOUT_DEFAULT.json
worklouderctl codex config diff CODEX_SNAPSHOT.json CODEX_CANDIDATE.json
worklouderctl codex lighting brightness get --input CODEX_SNAPSHOT.json
worklouderctl codex lighting brightness set --input CODEX_SNAPSHOT.json 80 --output CODEX_BRIGHTNESS.json
worklouderctl codex lighting auto-off get --input CODEX_BRIGHTNESS.json
worklouderctl codex lighting auto-off set --input CODEX_BRIGHTNESS.json 10-minutes --output CODEX_LIGHTING.json
worklouderctl codex voice get --input CODEX_LIGHTING.json
worklouderctl codex voice set --input CODEX_LIGHTING.json realtime --output CODEX_VOICE.json
worklouderctl codex runtime status
worklouderctl codex runtime recover [--timeout-seconds 15]
worklouderctl input inspect [--device DEVICE_ID]
worklouderctl input export --output BACKUP_DIRECTORY
worklouderctl input config snapshot --output CONFIG.json [--device DEVICE_ID]
worklouderctl input permission command snapshot --output HOST_SETTINGS.json
worklouderctl input permission command get --input HOST_SETTINGS.json
worklouderctl input permission command set --input HOST_SETTINGS.json enabled --output HOST_SETTINGS_ENABLED.json
worklouderctl input permission command apply --input HOST_SETTINGS_ENABLED.json --backup HOST_SETTINGS_BEFORE.json
worklouderctl input permission command restore --input HOST_SETTINGS.json --backup HOST_SETTINGS_CURRENT.json
worklouderctl bridge status
worklouderctl device --transport bridge status
worklouderctl device --transport bridge files --recursive
worklouderctl device --transport bridge export --output DEVICE_BACKUP
worklouderctl device --transport bridge config snapshot --output CONFIG.json
worklouderctl device --transport bridge config validate --input CONFIG.json
worklouderctl device --transport bridge config apply --input CONFIG.json --backup BEFORE.json
worklouderctl device --transport bridge config restore --input BEFORE.json --backup CURRENT.json
worklouderctl profile list --input CONFIG.json
worklouderctl profile show --input CONFIG.json --id PROFILE_ID
worklouderctl profile create --input CONFIG.json --name NAME --output CANDIDATE.json
worklouderctl profile duplicate --input CONFIG.json --id PROFILE_ID --name NAME --output CANDIDATE.json
worklouderctl profile delete --input CONFIG.json --id PROFILE_ID --output CANDIDATE.json
worklouderctl profile select --input CONFIG.json --id PROFILE_ID --output CANDIDATE.json
worklouderctl profile rename --input CONFIG.json --id PROFILE_ID --name NAME --output CANDIDATE.json
worklouderctl layer list --input CONFIG.json [--profile PROFILE_ID]
worklouderctl layer show --input CONFIG.json [--profile PROFILE_ID] --id LAYER_ID
worklouderctl layer create --input CONFIG.json [--profile PROFILE_ID] --name NAME --output CANDIDATE.json
worklouderctl layer duplicate --input CONFIG.json [--profile PROFILE_ID] --id LAYER_ID --name NAME --output CANDIDATE.json
worklouderctl layer delete --input CONFIG.json [--profile PROFILE_ID] --id LAYER_ID --output CANDIDATE.json
worklouderctl layer move --input CONFIG.json [--profile PROFILE_ID] --id LAYER_ID --to INDEX --output CANDIDATE.json
worklouderctl layer rename --input CONFIG.json [--profile PROFILE_ID] --id LAYER_ID --name NAME --output CANDIDATE.json
worklouderctl layer color --input CONFIG.json [--profile PROFILE_ID] --id LAYER_ID --color '#RRGGBB' --output CANDIDATE.json
worklouderctl layer lighting show --input CONFIG.json [--profile PROFILE_ID] --id LAYER_ID
worklouderctl layer lighting set --input CONFIG.json [--profile PROFILE_ID] --id LAYER_ID --zone backlight --effect breath --brightness 0.5 --color '#RRGGBB' [--apply-to-all] --output CANDIDATE.json
worklouderctl layer joystick show --input CONFIG.json [--profile PROFILE_ID] --id LAYER_ID
worklouderctl layer joystick mode set --input CONFIG.json [--profile PROFILE_ID] --id LAYER_ID radial --output CANDIDATE.json
worklouderctl layer joystick sector add --input CONFIG.json [--profile PROFILE_ID] --id LAYER_ID --index INDEX --output CANDIDATE.json
worklouderctl layer joystick sector delete --input CONFIG.json [--profile PROFILE_ID] --id LAYER_ID --index INDEX --output CANDIDATE.json
worklouderctl appsense list --input CONFIG.json
worklouderctl appsense show --input CONFIG.json --id APP_ID
worklouderctl appsense link --input CONFIG.json [--profile PROFILE_ID] --layer LAYER_ID --name NAME [--process BUNDLE_ID] [--path APP_PATH] --output CANDIDATE.json
worklouderctl appsense set --input CONFIG.json --id APP_ID [--name NAME] [--process BUNDLE_ID|--clear-process] [--path APP_PATH|--clear-path] --output CANDIDATE.json
worklouderctl appsense unlink --input CONFIG.json [--profile PROFILE_ID] --layer LAYER_ID --output CANDIDATE.json
worklouderctl smart-action list --input CONFIG.json
worklouderctl smart-action show --input CONFIG.json --id SMART_ACTION_ID
worklouderctl smart-action create --input CONFIG.json --name NAME --type text --text TEXT --output CANDIDATE.json
worklouderctl smart-action set --input CONFIG.json --id SMART_ACTION_ID --type url --url URL --output CANDIDATE.json
worklouderctl smart-action delete --input CONFIG.json --id SMART_ACTION_ID --output CANDIDATE.json
worklouderctl smart-action group create --input CONFIG.json --name NAME --smart-action SMART_ACTION_ID --output CANDIDATE.json
worklouderctl smart-action group member move --input CONFIG.json --id GROUP_ID --from 1 --to 0 --output CANDIDATE.json
worklouderctl smart-action group delete --input CONFIG.json --id GROUP_ID --output CANDIDATE.json
worklouderctl control list --input CONFIG.json [--profile PROFILE_ID] --layer LAYER_ID
worklouderctl control show --input CONFIG.json [--profile PROFILE_ID] --layer LAYER_ID --control key:ROW:COLUMN
worklouderctl control set --input CONFIG.json [--profile PROFILE_ID] --layer LAYER_ID --control encoder:INDEX:press --assignment KC_MUTE --output CANDIDATE.json
worklouderctl action list --input CONFIG.json
worklouderctl action show --input CONFIG.json --id ACTION_ID
worklouderctl action create --input CONFIG.json --name NAME --output CANDIDATE.json
worklouderctl action rename --input CONFIG.json --id ACTION_ID --name NAME --output CANDIDATE.json
worklouderctl action event add --input CONFIG.json --id ACTION_ID --assignment KC_C --type press --delay 0 --output CANDIDATE.json
worklouderctl action event set --input CONFIG.json --id ACTION_ID --index 0 --assignment KC_C --type click --delay 200 --output CANDIDATE.json
worklouderctl action event delete --input CONFIG.json --id ACTION_ID --index 0 --output CANDIDATE.json
worklouderctl action event move --input CONFIG.json --id ACTION_ID --from 1 --to 0 --output CANDIDATE.json
worklouderctl action delete --input CONFIG.json --id ACTION_ID --output CANDIDATE.json
worklouderctl action group list --input CONFIG.json
worklouderctl action group create --input CONFIG.json --name NAME --action ACTION_ID --output CANDIDATE.json
worklouderctl action group set --input CONFIG.json --id GROUP_ID --name NAME --color '#RRGGBB' --tag TAG --output CANDIDATE.json
worklouderctl action group member add --input CONFIG.json --id GROUP_ID --action ACTION_ID --output CANDIDATE.json
worklouderctl action group member remove --input CONFIG.json --id GROUP_ID --action ACTION_ID --output CANDIDATE.json
worklouderctl action group member move --input CONFIG.json --id GROUP_ID --from 1 --to 0 --output CANDIDATE.json
worklouderctl action group delete --input CONFIG.json --id GROUP_ID [--keep-members] --output CANDIDATE.json
worklouderctl multi-action list --input CONFIG.json
worklouderctl multi-action show --input CONFIG.json --id MULTI_ACTION_ID
worklouderctl multi-action create --input CONFIG.json --name NAME --output CANDIDATE.json
worklouderctl multi-action set --input CONFIG.json --id MULTI_ACTION_ID --tap KC_A --double-tap KC_B --hold KC_C --tap-hold KC_D --tapping-term 250 --output CANDIDATE.json
worklouderctl multi-action delete --input CONFIG.json --id MULTI_ACTION_ID --output CANDIDATE.json
worklouderctl multi-action group create --input CONFIG.json --name NAME --multi-action MULTI_ACTION_ID --output CANDIDATE.json
worklouderctl multi-action group member move --input CONFIG.json --id GROUP_ID --from 1 --to 0 --output CANDIDATE.json
worklouderctl multi-action group delete --input CONFIG.json --id GROUP_ID [--keep-members] --output CANDIDATE.json
worklouderctl device --transport direct --input-mode require-closed status
worklouderctl config validate BACKUP_DIRECTORY
worklouderctl config diff BASE CANDIDATE
worklouderctl --json input inspect
worklouderctl completion bash|zsh|fish
```

`codex inspect` reads only the five `codex-micro-*` settings from the
`[desktop]` table in Codex's `config.toml`, validates them against the frozen
Codex 26.727.51351 contract, and recursively fills inherited defaults in the
effective view. `codex export` atomically publishes and reopens a typed JSON
snapshot. Neither command serializes unrelated Codex settings.

`codex agent-source`, `codex agent-key tap-mode`, `codex command-key`,
`codex dial`, `codex joystick`, `codex reset`, `codex lighting`, and `codex voice` are strict offline Tier 1 editors. They
validate the embedded frozen definitions,
recompute effective settings and a recursive-key-sorted revision, preserve
unknown `codex-micro-*` values, publish atomically, and reopen the result. Each
receipt carries `expectedSourceSha256` for the Codex `settings-write` CAS
transaction; candidate generation leaves the source TOML and Codex runtime
state unchanged. `codex config apply/restore` consumes those candidates through
the authenticated Codex Companion Bridge with source-SHA and canonical
settings-revision CAS, complete explicit-setting replacement, exact
explicit/effective readback, immutable backup, session idempotency, and
automatic rollback. `codex agent-key snapshot/get/set/clear` validates all six
slots and the command, Skill, task, keycap, and empty assignment shapes while
preserving untargeted unknown fields. `codex agent-key apply/restore` adds a
separate global-state revision CAS, immutable backup, idempotent retry, exact
six-slot readback, stale-CAS rejection, and automatic rollback. Assignment
storage and the `codex-micro-agent-source=custom` setting remain independently
controlled, so applying assignments never changes source ordering implicitly.

`codex dial mode get/set` covers composer navigation, reasoning effort,
conversation scrolling, and custom mode without rewriting gesture mappings.
In custom mode, `dial gesture get/set/clear` edits exactly one of `left`,
`right`, `click`, or `long-press` as a command, Skill, or empty mapping. Gesture
edits reject inactive non-custom snapshots, preserve the other three gestures
and unknown layout fields, and leave the source settings bytes unchanged.

`codex joystick get/set/clear` covers `up`, `right`, `down`, and `left`. Each
direction can store a Codex command, Skill, or empty mapping. Every candidate
changes one `analogStick` leaf, preserves the other three directions plus
unknown layout fields, and keeps the source settings bytes unchanged.

`codex reset layout` matches the released reset call path: it replaces the
complete `codex-micro-layout` with the exact installed-build default. Command
Keys, joystick, dial, and voice-button fields return to their frozen defaults;
Agent Key assignments, Agent source, lighting, unknown sibling settings, and
the source settings bytes are preserved.

`codex config diff BASE.json CANDIDATE.json` validates both frozen-contract
snapshots and compares only their explicit `settings`. Transport metadata,
warnings, definitions, and the derived effective view are excluded, while
unknown settings remain visible. Human output lists deterministic JSON Pointer
paths; `--json` also includes typed before/after values and both canonical
settings revisions. The command is file-only and never opens a bridge or app.

`codex runtime status` is the released-app Tier 1 liveness probe. It requires
the exact frozen Codex version and bundle hashes, attaches to the Codex main
process on loopback, and reads the one live `CodexMicroService`. Healthy means
`connected`, live comm/API objects, settled connection/topology Promises, and
active `v.oai.hid` plus `v.oai.rad` subscriptions. `codex runtime recover`
temporarily pauses the Input process without closing its window, restarts only
that Codex service, requires the complete healthy state, resumes Input, and
holds the same state for a post-resume stability window. It does not rewrite
Codex settings, Input caches, the device keymap, firmware, or either app bundle.
The loopback inspector is closed after use and a one-shot reattach handler is
installed for the next CLI invocation. A changed Codex bundle requires a newly
verified runtime contract.

The repository ships the reference Codex main-process adapter and Electron
integration. The inspected Codex 26.727.51351 release exposes its internal
`settings-read`, `settings-write`, and global-state handlers to its renderer but
does not publish the external socket. Live mutations therefore remain behind
the integration capability gate; the current end-to-end evidence uses the
isolated same-contract fixture. See the
[Codex Companion Bridge protocol](docs/codex-companion-bridge.md).

`input inspect` is also read-only. `input export` copies the exact source bytes
into an atomically published directory and records each file's size and SHA-256
in `manifest.json`. It does not pause Input or write the device.
`input config snapshot` reads the current Input cache without launching or
controlling Input, captures `keymap.json` plus optional `smart_actions.json`
byte-for-byte, excludes host-only `input_storage.json`, and publishes the same
validated snapshot/revision core consumed by every offline semantic editor.

The primary `device` transport is the
[Input Companion Bridge](docs/companion-bridge.md). It authenticates over a
private Unix socket and asks the running Input main process to execute requests
through Input's existing device session. `--transport auto` prefers this bridge
as soon as Input publishes its socket and token.

The current released Input 0.18.0 app does not include the bridge, so
`--transport direct` remains a read-only compatibility route. It loads the
exact device kit bundled with the installed Input app and ships no second
driver. `--input-mode require-closed` reports contention. The explicit
`--input-mode restart` route requests a graceful quit and reopens Input after
the read; it never force-terminates Input.
`device export` verifies the device SHA-1 and host SHA-256 for every file,
reopens the typed manifest and files, and atomically publishes the directory.
The live `device config snapshot` command adds bridge/device metadata around
the same exact base64 bytes and deterministic revision core. `device config
validate` recomputes every size and digest; `--expected-revision REVISION` also
checks the current live revision as a read-only compare-and-swap preflight.
`device config apply/restore` create or reopen immutable backups and route a
complete-set transaction through Input with CAS, session-scoped idempotency,
full revision readback, and automatic rollback. These commands are advertised
only when the running Input version supplies a verified configuration writer;
the current cross-language evidence uses the isolated reference writer.

`profile`, `layer`, `appsense`, `control`, `action`, `multi-action`, and
`smart-action` commands are offline semantic editors. They strictly
validate every embedded size, SHA-1, SHA-256, canonical base64 payload, safe
path, keymap ID, and the full snapshot revision before producing a new complete
candidate. A candidate preserves unknown JSON fields and unrelated file bytes,
updates only the requested semantic field, recomputes all affected hashes and
the revision, publishes atomically, and reopens the result. It does not contact
Input or the device. Apply it through the guarded bridge transaction:

```console
worklouderctl input config snapshot --output before.json
worklouderctl layer color --input before.json --profile 0 --id 1 \
  --color '#EDF6FF' --output candidate.json
worklouderctl device --transport bridge config apply \
  --input candidate.json --backup pre-apply.json \
  --expected-revision REVISION --idempotency-key layer-color-1
```

The apply-side live CAS compares `REVISION` with a fresh bridge snapshot, so a
cache snapshot that became stale is detected before the first device write.

`layer joystick show/mode/sector` matches Input 0.18.0's released radial
editor. It exposes the editable `RADIAL` mode, seeds the observed two-sector
default, inserts `KC_NONE`, enforces the 2–8 sector range, and recomputes every
`a1`/`a2` boundary with Input's fixed 45-degree first sector. Assignment changes
continue through `control set --control joystick:INDEX`.

Profile and layer lifecycle commands follow the frozen Input 0.18.0 Codex
Micro model: at most six profiles and six layers, maximum-ID-plus-one object
allocation, and a zero-based persisted `activeProfileId` index even though CLI
arguments and output use stable object IDs. The protected `KV_OAI_*` Codex
layer remains first and is not duplicated, deleted, reordered, or selected as
a direct lighting target. A normal layer duplicate drops `linkedAppId`; a new
layer copies the last layer's lighting. Backlight and underglow support `off`, `solid`, `snake`,
`rainbow`, `breath`, and `gradient`, normalized `0..1` brightness/speed/magic,
24-bit color, and zone-level `--apply-to-all`.

`appsense` manages Input 0.18.0's `linkedApps` records and each layer's
`linkedAppId`. New IDs use Input's first-missing-nonnegative rule; macOS
`process` is the bundle identifier, and at least one of `process` or `path`
must be non-empty. `list/show` include every profile/layer binding. Link, field
update, and unlink candidates are covered by the same complete-snapshot
validation and fixture apply/readback/restore transaction. Input and device
firmware remain responsible for observing focus and switching the live layer;
that runtime transition is tracked separately from configuration parity.

Physical controls use stable IDs: `key:ROW:COLUMN`,
`encoder:INDEX:ccw|cw|press`, and `joystick:SECTOR`. `control set` accepts the
frozen Input 0.18.0 assignment grammar (`KC_*`, `KI_*`, existing `KA_A<ID>`
Actions, and existing `KA_M<ID>` Multi Actions). Existing `KV_*` vendor tokens
are readable and preserved; writable assignments draw from the catalog and
validated references. When
an Action reference changes, the candidate synchronizes `macrosUsed` and
`multiActionsUsed` using Input's ordering before the normal full-snapshot
rehash and atomic readback.

`action` freezes Input 0.18.0's Action model: IDs use the same last-ID-plus-one
allocation, events retain ordered `release(0)`, `press(1)`, or `click(2)`
semantics and `0..9999 ms` delays, and a new Action starts with Input's
`KC_NONE` press event. Delete is a complete cascade: layer controls, other
Action events, Multi Action branches, groups, and profile `macrosUsed` are
updated together, with each removed reference becoming `KC_NONE`.

`action group` and `multi-action group` cover list/show/create, metadata and
tag updates, ordered member add/remove/move, and delete. The default group
delete matches Input 0.18.0: a member used only by that stored group is deleted
with the same full reference cascade, while shared members remain. Pass
`--keep-members` to remove only the group container. Group IDs use the observed
maximum-ID-plus-one import rule.

`multi-action` covers all four gesture assignments (`tap`, `double-tap`,
`hold`, and `tap-hold`), name, color, icon, and tapping term. New Multi Actions
use Input's four `KC_NONE` assignments and `250 ms` default. Deletion clears
every physical, Action-event, nested Multi Action, group, and profile-usage
reference before removing the resource.

`smart-action` covers Input 0.18.0 `TEXT_STEP`, `CMD_STEP`, `URL_STEP`, and
`APP_STEP` records in `smart_actions.json`, including typed payloads, optional
color/icon metadata, physical `SA_<ID>` bindings, and stored groups. New IDs
use Input's maximum-ID-plus-one rule and start at 1. Group IDs start at 0 and
empty groups are valid. Deleting a Smart Action clears every physical binding
to `KC_NONE` and removes group membership while preserving the group container.
Smart-only candidates preserve the exact `keymap.json` bytes. Command actions
report `requiresCommandPermission`; the host gate remains Input's
`smartActionCmdEnabled` setting and is not silently changed by definition CRUD.
`input permission command` snapshots all three Input host-setting booleans and
changes only this gate. Its bridge transaction preserves the analytics fields,
uses revision CAS and idempotency, verifies complete DTO readback, and restores
the pre-write settings automatically after a failed readback.

```console
worklouderctl input permission command snapshot --output host-settings.json
worklouderctl input permission command get --input host-settings.json
worklouderctl input permission command set --input host-settings.json enabled \
  --output host-settings-enabled.json
worklouderctl input permission command apply \
  --input host-settings-enabled.json --backup host-settings-before-apply.json \
  --expected-revision REVISION --idempotency-key enable-command-actions
worklouderctl input permission command restore \
  --input host-settings.json --backup host-settings-before-restore.json \
  --expected-revision CURRENT_REVISION --idempotency-key restore-command-actions
```

The CLI does not write `input_storage.json`; Input remains responsible for
persistence and command execution.

The repository includes an executable Input-main reference server, a service
adapter for the Input 0.18.0 service shape, authentication tests, and a
cross-language Rust CLI conformance test:

```console
node --test companion/input-main-bridge.test.mjs
./scripts/test-bridge-e2e.sh
./scripts/test-codex-bridge-e2e.sh
```

## Additional planned mutation commands

The intended binary name is `worklouderctl`:

```console
worklouderctl codex agent-source set --input codex.json priority --output codex-priority.json
worklouderctl codex command-key set --input codex-priority.json ACT06 \
  --command toggleFastMode --output codex-fast.json
worklouderctl codex joystick set up --skill SKILL_ID
worklouderctl plan layout.yaml
worklouderctl apply layout.yaml
worklouderctl backup list
worklouderctl backup restore BACKUP_ID
```

These examples document the remaining mutation interface. Follow the
[roadmap](docs/roadmap.md) for exact status.

## Target capabilities

| Area | Target scope |
| --- | --- |
| Codex-native configuration | Agent Keys, Command Keys, voice mode, dial, joystick, Skills, global lighting, and reset |
| Device discovery | Codex Micro status, firmware, active profile/layer, USB and Bluetooth transport details |
| Profiles and layers | List, create, rename, select, diff, import, and export |
| Controls | Keys, rotary encoder, encoder press, and planar joystick sectors |
| Actions | Keycodes, Actions/macros, Multi Actions, Smart Actions, groups, and reference validation |
| Lighting | Layer backlight, underglow, effects, colors, brightness, and speed |
| AppSense | Linked applications and focused-app layer selection |
| Safety | Plan-first writes, immutable backups, conflict detection, exact readback, checksums, and rollback |
| Automation | Deterministic JSON output, typed exit codes, and an agent-friendly contract |

## Safety model

A mature write path must treat Codex, Input, and the device as synchronized
but distinct authorities:

```text
inspect current state
        ↓
validate + produce a plan and diff
        ↓
route the transaction through Input's serialized bridge
        ↓
back up Codex settings + device files + Input cache/database
        ↓
write changed files in dependency-safe order
        ↓
read back bytes/JSON + verify checksums
        ↓
atomically update the selected Codex/Input authorities
        ↓
refresh runtimes or roll everything back
```

Unknown Input, firmware, or schema versions will default to inspection until a
version adapter and fixture coverage exist.

## Initial compatibility target

The first implementation target is intentionally narrow:

- **Device:** Work Louder Codex Micro
- **Host:** macOS
- **Codex inspection baseline:** Codex 26.727.51351
- **Input schema fixtures:** Work Louder Input 0.17.3 and 0.18.0
- **Firmware fixture baseline:** Codex Micro v0.6.0

These versions describe the current research fixtures, not a released support
guarantee. See the [compatibility policy](docs/compatibility.md), the vendor's
[Codex Micro product page](https://worklouder.cc/codex-micro), and the official
[Codex Micro setup guide](https://worklouder.cc/openai-micro-setup).

## Project status

| Milestone | Status |
| --- | --- |
| SEO/AIO and product documentation foundation | Complete |
| Sanitized research fixtures and schema model | Planned |
| Rust CLI foundation, tier/capability contract, and JSON output | Complete |
| Provider `doctor` and Input `inspect`/exact-byte `export`/semantic cache snapshot | Complete |
| Bundle `validate` and structural `diff` | Complete |
| Codex settings contract and TOML `doctor`/`inspect`/`export` | Complete |
| Codex Agent source/tap mode and Command Key offline candidates | Complete; Codex bridge apply/restore fixture verified |
| Input 0.18.0 live `device status`/`files`/verified `export` | Complete |
| Read-only Input process coordination and automatic reopen | Complete |
| Companion Bridge v1 contract, CLI client, and reference server | Complete |
| Bridge config snapshot, deterministic revision, and live CAS preflight | Complete |
| Bridge apply/restore transaction and automatic rollback fixture | Complete |
| Companion Bridge integration in a released Input build | Upstream integration pending |
| Codex Companion Bridge client/reference integration | Complete; released Codex integration pending |
| Semantic profile/layer candidates | Full lifecycle, selection, ordering, color, and lighting candidate-verified; combined profile-create/layer-create/lighting apply/readback/restore fixture-verified |
| Semantic physical controls | List/show/set for keys and encoder gestures; joystick mode, 2–8-sector lifecycle, assignments, and exact angle rebalance; candidate-verified with Input cache hashes unchanged |
| Semantic Actions | List/show/create/rename/delete and event add/set/delete/move; cascade/apply/restore fixture-verified |
| Real-device mutation and rollback | Planned |
| Smart Action definitions, groups, bindings, and cascade | Candidate-verified against current Input 0.18.0 cache bytes; released writer pending |
| Input cache read adapter | Complete; byte-exact bridge-equivalent semantic snapshot |
| Input database synchronization | Planned |
| Signed macOS release and Homebrew installation | Planned |

## Frequently asked questions

### Is there a CLI for Work Louder Input?

WorkLouderCTL is an open-source full-configuration CLI project for Codex, Work
Louder Input, and Codex Micro. The source alpha can inspect both apps,
read/export live Codex Micro state, generate validated profile/layer/lighting/control/Action/Multi Action/Smart Action/group candidates,
and exercise apply/restore against the isolated bridge writer; packaged
binaries and released-Input writer integration are upcoming.

### Does WorkLouderCTL replace the Codex and Input configuration GUIs?

Yes. Full configuration parity is the product target. Codex and Input remain
the driver/runtime providers for features that execute inside those apps. A
replacement driver/runtime is outside the project contract.

### Does it support Codex Micro?

Codex Micro on macOS is the first target. Input 0.18.0 and firmware v0.6.0 have
live read-only evidence for status, file listing, exact file export, Input
restart, and unchanged cached configuration. Mutation claims remain tied to
separate write/readback/rollback evidence.

### Can an AI agent configure Codex Micro?

That is a core design goal. The agent-facing interface will use the same
plan/apply/verify/rollback engine as the human CLI, with deterministic JSON and
no hidden write path.

Read the complete [FAQ](docs/faq.md).

## Documentation

- [Configuration tier model](docs/tier-model.md)
- [Complete configuration reference](docs/configuration-reference.md)
- [Configuration parity matrix](docs/configuration-parity.md)
- [2026-08-02 Codex Micro and Input audit](docs/research/2026-08-02-codex-micro-audit.md)
- [Codex settings read contract](docs/research/2026-08-02-codex-settings-read-contract.md)
- [Input live device read contract](docs/research/2026-08-02-input-live-read-contract.md)
- [Input Companion Bridge protocol](docs/companion-bridge.md)
- [Codex Companion Bridge protocol](docs/codex-companion-bridge.md)
- [Frequently asked questions](docs/faq.md)
- [Compatibility and support policy](docs/compatibility.md)
- [Companion architecture](docs/architecture.md)
- [Product roadmap](docs/roadmap.md)
- [AI-readable project index](llms.txt)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)

## Contributing

The project is at the transaction source-alpha stage. Protocol findings,
sanitized configurations, compatibility evidence, CLI implementations, and
design feedback are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) before
opening a pull request.

## Independence

WorkLouderCTL is an independent community project. It is not affiliated with or
endorsed by Work Louder or OpenAI. Work Louder, Input, Codex Micro, Codex, and
other product names belong to their respective owners.

## License

[MIT](LICENSE)
