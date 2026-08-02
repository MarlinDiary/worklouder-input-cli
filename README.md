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
> snapshots/CAS validation, offline profile/layer/control/Action candidate generation,
> fixture-verified apply/restore transactions, JSON output, and shell completions.
> There is no packaged release yet.
> The bridge transaction engine now verifies backup, apply, idempotent retry,
> readback, restore, and automatic rollback against an isolated writer fixture.
> Installed-device mutation remains gated on a released, hardware-verified
> Input writer adapter.

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
worklouderctl input inspect [--device DEVICE_ID]
worklouderctl input export --output BACKUP_DIRECTORY
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
worklouderctl profile select --input CONFIG.json --id PROFILE_ID --output CANDIDATE.json
worklouderctl profile rename --input CONFIG.json --id PROFILE_ID --name NAME --output CANDIDATE.json
worklouderctl layer list --input CONFIG.json [--profile PROFILE_ID]
worklouderctl layer show --input CONFIG.json [--profile PROFILE_ID] --id LAYER_ID
worklouderctl layer rename --input CONFIG.json [--profile PROFILE_ID] --id LAYER_ID --name NAME --output CANDIDATE.json
worklouderctl layer color --input CONFIG.json [--profile PROFILE_ID] --id LAYER_ID --color '#RRGGBB' --output CANDIDATE.json
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

`input inspect` is also read-only. `input export` copies the exact source bytes
into an atomically published directory and records each file's size and SHA-256
in `manifest.json`. It does not pause Input or write the device.

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
The bridge-only `device config snapshot` command additionally records exact
base64 bytes and a deterministic revision. `device config validate` recomputes
every size and digest; `--expected-revision REVISION` also checks the current
live revision as a read-only compare-and-swap preflight.
`device config apply/restore` create or reopen immutable backups and route a
complete-set transaction through Input with CAS, session-scoped idempotency,
full revision readback, and automatic rollback. These commands are advertised
only when the running Input version supplies a verified configuration writer;
the current cross-language evidence uses the isolated reference writer.

`profile`, `layer`, `control`, and `action` commands are offline semantic editors. They strictly
validate every embedded size, SHA-1, SHA-256, canonical base64 payload, safe
path, keymap ID, and the full snapshot revision before producing a new complete
candidate. A candidate preserves unknown JSON fields and unrelated file bytes,
updates only the requested semantic field, recomputes all affected hashes and
the revision, publishes atomically, and reopens the result. It does not contact
Input or the device. Apply it through the guarded bridge transaction:

```console
worklouderctl device --transport bridge config snapshot --output before.json
worklouderctl layer color --input before.json --profile 0 --id 1 \
  --color '#EDF6FF' --output candidate.json
worklouderctl device --transport bridge config apply \
  --input candidate.json --backup pre-apply.json \
  --expected-revision REVISION --idempotency-key layer-color-1
```

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
updated together, with each removed reference becoming `KC_NONE`. Group CRUD
remains a separate upcoming slice.

The repository includes an executable Input-main reference server, a service
adapter for the Input 0.18.0 service shape, authentication tests, and a
cross-language Rust CLI conformance test:

```console
node --test companion/input-main-bridge.test.mjs
./scripts/test-bridge-e2e.sh
```

## Additional planned mutation commands

The intended binary name is `worklouderctl`:

```console
worklouderctl codex agent-source set priority
worklouderctl codex command-key set ACT06 --command toggleFastMode
worklouderctl codex joystick set up --skill SKILL_ID
worklouderctl codex lighting set --brightness 80 --auto-off 10-minutes
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
| Provider `doctor` and Input `inspect`/exact-byte `export` | Complete |
| Bundle `validate` and structural `diff` | Complete |
| Codex settings contract and TOML `doctor`/`inspect`/`export` | Complete |
| Input 0.18.0 live `device status`/`files`/verified `export` | Complete |
| Read-only Input process coordination and automatic reopen | Complete |
| Companion Bridge v1 contract, CLI client, and reference server | Complete |
| Bridge config snapshot, deterministic revision, and live CAS preflight | Complete |
| Bridge apply/restore transaction and automatic rollback fixture | Complete |
| Companion Bridge integration in a released Input build | Upstream integration pending |
| Codex live settings-bridge write client | Planned |
| Semantic profile/layer candidates | List/show/select/rename/color implemented and fixture-verified |
| Semantic physical controls | List/show/set for keys, encoder gestures, and existing joystick sectors; candidate/apply/restore fixture-verified |
| Semantic Actions | List/show/create/rename/delete and event add/set/delete/move; cascade/apply/restore fixture-verified |
| Real-device mutation and rollback | Planned |
| Input cache/database and Smart Actions synchronization | Planned |
| Signed macOS release and Homebrew installation | Planned |

## Frequently asked questions

### Is there a CLI for Work Louder Input?

WorkLouderCTL is an open-source full-configuration CLI project for Codex, Work
Louder Input, and Codex Micro. The source alpha can inspect both apps,
read/export live Codex Micro state, generate validated profile/layer/control/Action candidates,
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
