# Codex Micro configuration reference

This is the research inventory for the first WorkLouderCTL compatibility
adapter. It records what exists, where it is authoritative, and how it must be
verified. The tested snapshot is documented in
[the 2026-08-02 audit](research/2026-08-02-codex-micro-audit.md).

## Tier 1: Codex settings

### Settings adapter surface

The inspected Codex build reads settings through `settings-read` and writes a
partial settings object through `settings-write`. Renderer calls reach the
native host through the `vscode://codex/<method>` bridge and then invalidate
the `get-settings` query. The Codex Micro UI uses the same setting definitions
and setter for agent source, single-tap behavior, layout, brightness, and
auto-off.

WorkLouderCTL therefore targets the versioned settings bridge for online
transactions. The implemented `codex-config-toml-read-v1` adapter reads the
`[desktop]` table in `$CODEX_HOME/config.toml`, validates only keys with the
`codex-micro-` prefix, and preserves unknown prefixed keys. It hashes the source
before and after capture, recursively fills inherited defaults, and excludes
all unrelated Codex configuration from its output. Offline mutation still
requires separate round-trip and restart fixtures. Raw Chromium LevelDB
mutation is not part of the configuration path.

The current read-only command surface is:

```console
worklouderctl codex doctor [--strict] [--config PATH] [--app PATH]
worklouderctl codex inspect [--config PATH] [--app PATH]
worklouderctl codex export --output FILE [--config PATH] [--app PATH]
```

### Persisted setting keys

The installed Codex app exposes these Codex Micro setting records:

| Setting | Observed default | Meaning |
| --- | ---: | --- |
| `codex-micro-agent-source` | `recent` | Source/order for Agent Key assignments |
| `codex-micro-single-tap-agent-keys` | `false` | Single tap brings the assigned task forward |
| `codex-micro-layout` | schema version `1` | Command-key, dial, joystick, and voice assignments |
| `codex-micro-lighting-brightness` | `100` | Global Tier 1 lighting intensity |
| `codex-micro-lighting-auto-off` | `3-minutes` | Idle auto-off duration |

Agent source values observed in the UI are `pinned`, `recent`, `priority`, and
`custom`. Custom Agent Key assignments may reference a task, command/shortcut,
keycap, or skill.

### Physical slots

| Surface | Logical IDs |
| --- | --- |
| Agent Keys | `AG00`, `AG01`, `AG02`, `AG03`, `AG04`, `AG05` |
| Command Keys | `ACT06`, `ACT07`, `ACT08`, `ACT09`, `ACT10_ACT11`, `ACT12` |
| Analog joystick | `up`, `right`, `down`, `left` |
| Dial gestures | `left`, `right`, `click`, `longPress` |

`ACT10_ACT11` is one double-width logical slot backed by two physical switch
signals. The observed default Command Keycaps are `FAST`, `APPR`, `REJ`,
`SPLIT`, `MIC`, and `CODEX`.

### Codex-aware behaviors

- Agent Keys: task assignment plus idle, thinking, complete, needs-input,
  error, and unassigned lighting states.
- Voice button modes: `push-to-talk` and `realtime`/Voice Chat.
- Dial modes: `composer-navigation`, `reasoning`, `conversation-scroll`, and
  `custom`.
- Default joystick commands: plan-mode toggle, forward, sidebar toggle, and
  back.
- Custom dial and joystick assignments: Codex command, skill, or empty.
- Long-pressing the dial opens Codex Micro settings.

### Codex command/keycap catalog

The inspected build contains 38 keycap identifiers:

```text
FAST APPR REJ SPLIT MIC CODEX BUG OAI TERM DWN DEL NEW NAV MAGIC
DIFF PLAY GIT BRCH BRANCH MRG PR PAINT LAB PARTY TIME MIND+ MIND-
EMPT1 EMPT2 EMPT3 EMPT4 SETUP FOLD UPL APPS YOLO YEET EMPT5
```

Their actions cover fast mode, approval/decline, task fork, voice input,
submit, feedback, OpenAI docs, terminal, copy-conversation Markdown, archive,
new task, browser, pin, review, environment action, Git/branch/PR operations,
photo/file attachment, settings, side chat, task management, reasoning level,
folder open, Skills, and custom composer commands.

### Codex lighting protocol surface

The installed Work Louder integration uses vendor notifications including:

| Method | Role inferred from call sites |
| --- | --- |
| `v.oai.hid` | Agent/Command key HID events |
| `v.oai.rad` | Analog/radial input events |
| `v.oai.thstatus` | Per-task status lighting |
| `v.oai.rgbcfg` | Key and ambient RGB configuration |

Observed lighting effects are off, solid, snake, rainbow, breath, gradient,
and shallow breath. Auto-off values are off, 30 seconds, 1/3/10/30 minutes,
and 1 hour.

## Tier 2: Input device configuration

### Implemented live read surface

The primary transport uses the authenticated Input Companion Bridge. The CLI
client and Input-main reference server expose:

```console
worklouderctl bridge status
worklouderctl device --transport bridge status
worklouderctl device --transport bridge files [--path PATH] [--recursive]
worklouderctl device --transport bridge export --output DIRECTORY
worklouderctl device --transport bridge config snapshot --output SNAPSHOT.json
worklouderctl device --transport bridge config validate --input SNAPSHOT.json
worklouderctl device --transport bridge config apply \
  --input CANDIDATE.json --backup PRE_APPLY.json
worklouderctl device --transport bridge config restore \
  --input ORIGINAL.json --backup PRE_RESTORE.json
```

Input owns the connected session and serializes requests through its existing
service container. It reports firmware, active profile/layer, battery/charging
state, device identity, transport, file names, sizes, and device checksums.
Export reads raw file bytes through the bridge, verifies device SHA-1 plus host
SHA-256, and atomically publishes a typed bundle accepted by `config validate`
and `config diff`. The config snapshot command adds exact base64 content and a
deterministic full-configuration revision. Validation recomputes sizes, both
digests, and the revision; `--expected-revision` enables a live read-only
compare-and-swap preflight.

Apply and restore use the same complete snapshot envelope. The CLI creates or
reopens an immutable pre-mutation backup, while Input owns serialized CAS,
idempotency, complete-set replacement, readback, and automatic rollback. These
mutation behaviors pass the isolated cross-language writer fixture; an
installed Input version must advertise the corresponding capabilities before
the commands run against a device.

Input 0.18.0 does not yet ship the bridge. The verified direct compatibility
path remains available as:

```console
worklouderctl device --transport direct [--input-mode require-closed|restart] status
```

### Root model

The observed `keymap.json` shape contains:

```text
version
activeProfileId
profiles[]
  id, name
  layers[]
    id, name, color, layout, lights
  macrosUsed[]
  multiActionsUsed[]
multiActions[]
macros[]
macrosGroups[]
multiActionsGroups[]
linkedApps[]
deviceSpecificConfig?          # device/model dependent
```

Input 0.18.0 also models `smartActions` and `smartActionGroups`. The connected
Codex Micro stores them in a separate `smart_actions.json` file.

### Profiles and layers

- profile create, rename, select, and delete;
- layer create, rename, select, delete, and application link;
- six normal layer selectors and six temporary layer selectors;
- six profile selectors;
- Mac/Windows label mode;
- touch-sensor layer cycling and AppSense automatic switching.

The frozen Codex Micro model has a maximum of six profiles and six layers per
profile. Other device models still require their own versioned limits.

The first semantic CLI slice operates on a complete revisioned snapshot rather
than a partial keymap fragment:

```sh
worklouderctl profile list --input SNAPSHOT.json
worklouderctl profile show --input SNAPSHOT.json --id ID
worklouderctl profile create --input SNAPSHOT.json --name NAME --output CANDIDATE.json
worklouderctl profile duplicate --input SNAPSHOT.json --id ID \
  [--name NAME] --output CANDIDATE.json
worklouderctl profile delete --input SNAPSHOT.json --id ID --output CANDIDATE.json
worklouderctl profile select --input SNAPSHOT.json --id ID --output CANDIDATE.json
worklouderctl profile rename --input SNAPSHOT.json --id ID --name NAME --output CANDIDATE.json
worklouderctl layer list --input SNAPSHOT.json [--profile ID]
worklouderctl layer show --input SNAPSHOT.json [--profile ID] --id ID
worklouderctl layer create --input SNAPSHOT.json [--profile ID] \
  --name NAME --output CANDIDATE.json
worklouderctl layer duplicate --input SNAPSHOT.json [--profile ID] \
  --id ID [--name NAME] --output CANDIDATE.json
worklouderctl layer delete --input SNAPSHOT.json [--profile ID] \
  --id ID --output CANDIDATE.json
worklouderctl layer move --input SNAPSHOT.json [--profile ID] \
  --id ID --to INDEX --output CANDIDATE.json
worklouderctl layer rename --input SNAPSHOT.json [--profile ID] \
  --id ID --name NAME --output CANDIDATE.json
worklouderctl layer color --input SNAPSHOT.json [--profile ID] \
  --id ID --color '#RRGGBB' --output CANDIDATE.json
worklouderctl layer lighting show --input SNAPSHOT.json [--profile ID] --id ID
worklouderctl layer lighting set --input SNAPSHOT.json [--profile ID] \
  --id ID --zone backlight --effect breath --brightness 0.5 \
  --speed 0.5 --magic 0.5 --color '#RRGGBB' [--apply-to-all] \
  --output CANDIDATE.json
worklouderctl appsense list --input SNAPSHOT.json
worklouderctl appsense show --input SNAPSHOT.json --id APP_ID
worklouderctl appsense link --input SNAPSHOT.json [--profile ID] \
  --layer ID --name NAME [--process BUNDLE_ID] [--path APP_PATH] \
  --output CANDIDATE.json
worklouderctl appsense set --input SNAPSHOT.json --id APP_ID \
  [--name NAME] [--process BUNDLE_ID|--clear-process] \
  [--path APP_PATH|--clear-path] --output CANDIDATE.json
worklouderctl appsense unlink --input SNAPSHOT.json [--profile ID] \
  --layer ID --output CANDIDATE.json
worklouderctl control list --input SNAPSHOT.json [--profile ID] --layer ID
worklouderctl control show --input SNAPSHOT.json [--profile ID] \
  --layer ID --control key:ROW:COLUMN
worklouderctl control set --input SNAPSHOT.json [--profile ID] \
  --layer ID --control encoder:INDEX:press --assignment KC_MUTE \
  --output CANDIDATE.json
```

These commands validate the entire snapshot offline, preserve unknown fields
and non-keymap bytes, rewrite `keymap.json` as compact ordered JSON, update its
size/SHA-1/SHA-256, and recompute the full configuration revision. Candidate
files are atomically published and reopened. Apply and rollback remain separate
explicit transaction steps through the Input-owned bridge.

Profile and layer object IDs are stable CLI identifiers allocated from the
largest existing ID plus one. Input 0.18.0's persisted `activeProfileId` is
instead a zero-based profile-array index; list/show report both values and
`profile select` resolves the object ID before writing the index. Deletion
keeps that index valid. A newly created profile contains the protected Codex
layer template with layer ID zero and no per-layer lights.

A layer is protected when any layout assignment begins with `KV_OAI_`. The
protected layer stays at position zero and is excluded from duplicate, delete,
move-to/from-zero, and per-layer lighting edits. New normal layers use the
frozen Codex Micro empty layout and copy the last layer's lighting when present.
Duplicate performs a deep copy with a new ID and removes `linkedAppId` so the
copy does not silently inherit an AppSense binding.

The observed Input 0.18.0 layer `color` is a 24-bit RGB integer. The CLI accepts
`#RRGGBB`, `0xRRGGBB`, or decimal `0..16777215`, stores the integer, and reports
both the integer and normalized uppercase `colorHex`. This field is layer
metadata; backlight and underglow objects remain separate lighting surfaces.

### Assignable controls

- 13 switches, including the double-width Codex Command Key surface;
- encoder counter-clockwise, clockwise, and click;
- planar joystick sectors/radial assignments;
- touch-sensor behavior where exposed by the device adapter.

The implemented physical IDs are `key:ROW:COLUMN`,
`encoder:INDEX:ccw|cw|press`, and `joystick:SECTOR`. List/show report the exact
device token and classify it as `basic`, `internal`, `action`, `multiAction`,
or `vendor`. Set modifies only an existing physical slot; joystick sector
angles remain byte-for-byte equivalent in the semantic document.

`spec/input-assignment-tokens-0.18.0.json` freezes 184 `KC_*` tokens and 43
`KI_*` tokens recovered from the Input 0.18.0 ASAR, plus `KA_A<ID>` and
`KA_M<ID>` reference formats. Reference IDs must exist in root `macros` or
`multiActions`; profile usage arrays are recomputed in Input's string order.
The catalog records the source ASAR SHA-256 and the observed Codex Micro
`[2,4,4,3]` key matrix. Existing `KV_*` assignments are accepted for strict
read/preservation but rejected as new user assignments.

### Basic keys

The inspected Input UI exposes 171 basic-key choices across:

- alphanumeric and glyph keys;
- modifiers, arrows, navigation, editing, and locks;
- `F1`–`F24`;
- number-pad keys;
- media, volume, power, sleep, stop, and brightness;
- Layer 1–6 and temporary Layer 1–6;
- Profile 1–6;
- Cheat Sheet show, hold, hide, and toggle.

### Actions/macros

Simple mode records a chord or phrase. Advanced mode represents ordered key
press, release, click, and delay events. An Action carries an ID, name, optional
color, and event list; groups are separate referenced collections.

Validation must cover balanced modifier release, referenced IDs, maximum event
counts, permitted keycodes, and preserved ordering. Hardware verification must
observe the emitted chord rather than relying on the displayed Action name.

The current offline Action interface is:

```sh
worklouderctl action list --input SNAPSHOT.json
worklouderctl action show --input SNAPSHOT.json --id ID
worklouderctl action create --input SNAPSHOT.json --name NAME --output CANDIDATE.json
worklouderctl action rename --input SNAPSHOT.json --id ID --name NAME --output CANDIDATE.json
worklouderctl action event add --input SNAPSHOT.json --id ID \
  --assignment KC_C --type press --delay 0 --output CANDIDATE.json
worklouderctl action event set --input SNAPSHOT.json --id ID --index INDEX \
  [--assignment TOKEN] [--type release|press|click] [--delay MILLISECONDS] \
  --output CANDIDATE.json
worklouderctl action event delete --input SNAPSHOT.json --id ID \
  --index INDEX --output CANDIDATE.json
worklouderctl action event move --input SNAPSHOT.json --id ID \
  --from INDEX --to INDEX --output CANDIDATE.json
worklouderctl action delete --input SNAPSHOT.json --id ID --output CANDIDATE.json
worklouderctl action group list --input SNAPSHOT.json
worklouderctl action group show --input SNAPSHOT.json --id GROUP_ID
worklouderctl action group create --input SNAPSHOT.json --name NAME \
  --action ACTION_ID [--action ACTION_ID] [--color '#RRGGBB'] [--tag TAG] \
  --output CANDIDATE.json
worklouderctl action group set --input SNAPSHOT.json --id GROUP_ID \
  [--name NAME] [--color '#RRGGBB'|--clear-color] \
  [--tag TAG|--clear-tags] --output CANDIDATE.json
worklouderctl action group member add --input SNAPSHOT.json --id GROUP_ID \
  --action ACTION_ID --output CANDIDATE.json
worklouderctl action group member remove --input SNAPSHOT.json --id GROUP_ID \
  --action ACTION_ID --output CANDIDATE.json
worklouderctl action group member move --input SNAPSHOT.json --id GROUP_ID \
  --from INDEX --to INDEX --output CANDIDATE.json
worklouderctl action group delete --input SNAPSHOT.json --id GROUP_ID \
  [--keep-members] --output CANDIDATE.json
```

`spec/input-actions-0.18.0.json` freezes the ASAR-derived model. Action IDs use
Input's last-action-ID-plus-one allocation. Event types are release `0`, press
`1`, and click `2`; delay is an integer from `0` through `9999` milliseconds.
A sole deleted event resets to `{act:1,delay:0,kc:"KC_NONE"}`, matching the
editor's one-event invariant.

Action deletion is referentially complete across existing layer keys,
encoders, joystick sectors, other Action events, four Multi Action branches,
Action groups, and profile usage arrays. References become `KC_NONE`; empty
Action groups are removed; `macrosUsed` is recomputed in Input string order.
The candidate still preserves unrelated file bytes and unknown JSON fields.

Stored groups keep ordered, unique `actionIds`, optional tags, and optional
color. New group IDs use the maximum existing group ID plus one, matching
Input's import path rather than assuming the array is sorted. Default group
deletion matches the Input 0.18.0 GUI: members that occur in no other stored
group are deleted with the normal Action cascade; shared members survive.
`--keep-members` removes only the group object. Member removal rejects a
resulting empty group so the snapshot remains valid.

### Multi Actions

The UI offers tap, double tap, hold, and tap-then-hold branches. The device
fields are `kcOnTap`, `kcOnDoubleTap`, `kcOnHold`, and `kcOnTapHold`, with `tt`
for the tapping term. Input 0.18.0 creates a Multi Action with four `KC_NONE`
assignments and a `250 ms` tapping term; its GUI displays that term as fixed.

```sh
worklouderctl multi-action list --input SNAPSHOT.json
worklouderctl multi-action show --input SNAPSHOT.json --id ID
worklouderctl multi-action create --input SNAPSHOT.json [--name NAME] \
  [--color '#RRGGBB'] [--icon ICON] --output CANDIDATE.json
worklouderctl multi-action set --input SNAPSHOT.json --id ID \
  [--name NAME] [--color '#RRGGBB'|--clear-color] \
  [--icon ICON|--clear-icon] [--tap TOKEN] [--double-tap TOKEN] \
  [--hold TOKEN] [--tap-hold TOKEN] [--tapping-term MILLISECONDS] \
  --output CANDIDATE.json
worklouderctl multi-action delete --input SNAPSHOT.json --id ID \
  --output CANDIDATE.json
worklouderctl multi-action group list --input SNAPSHOT.json
worklouderctl multi-action group show --input SNAPSHOT.json --id GROUP_ID
worklouderctl multi-action group create --input SNAPSHOT.json --name NAME \
  --multi-action ID [--multi-action ID] --output CANDIDATE.json
worklouderctl multi-action group set --input SNAPSHOT.json --id GROUP_ID \
  [--name NAME] [--color '#RRGGBB'|--clear-color] \
  [--tag TAG|--clear-tags] --output CANDIDATE.json
worklouderctl multi-action group member add --input SNAPSHOT.json --id GROUP_ID \
  --multi-action ID --output CANDIDATE.json
worklouderctl multi-action group member remove --input SNAPSHOT.json --id GROUP_ID \
  --multi-action ID --output CANDIDATE.json
worklouderctl multi-action group member move --input SNAPSHOT.json --id GROUP_ID \
  --from INDEX --to INDEX --output CANDIDATE.json
worklouderctl multi-action group delete --input SNAPSHOT.json --id GROUP_ID \
  [--keep-members] --output CANDIDATE.json
```

`spec/input-multi-actions-0.18.0.json` freezes the model and source ASAR hash.
Multi Action IDs use Input's last-resource-ID-plus-one allocation; group IDs
use maximum-ID-plus-one. New branch assignments must exist in the frozen token
catalog and self-reference is rejected. Delete replaces matching physical,
Action-event, and nested Multi Action tokens with `KC_NONE`, removes group
membership, drops empty groups, and recomputes `multiActionsUsed`. Multi Action
group delete applies the same orphan/shared-member rule as Action groups.

### Per-layer lighting

Backlight and underglow each carry effect, brightness, speed, magic, and color.
The inspected effect list is off, solid, snake, rainbow, breath, and gradient.
Brightness, speed, and magic are normalized numbers in `0..1`; color is stored
as a 24-bit integer. The default Codex Micro lighting is solid white backlight
at `1/0.5/1` and rainbow white underglow at `1/0.55/1`. `layer lighting set`
updates only supplied fields. `--apply-to-all` copies the selected zone to all
layers in the profile and initializes both default zones on a layer that had no
`lights` object, matching the inspected Input 0.18.0 mapping behavior.

### Linked applications / AppSense

A linked application is stored in root `linkedApps` as
`{id,name,process,path}`; the selected layer stores the same ID in
`linkedAppId`. IDs follow Input 0.18.0's first missing nonnegative integer rule,
which is intentionally different from the maximum-plus-one profile, layer,
Action, and group rules. A record needs a non-empty `process` or `path`.
On macOS, `process` is the application bundle identifier; automatic detection
normally stores an empty path and derives a `-mac` name.

`appsense list/show` return the stored identity plus all profile/layer
bindings. `link` creates the root record and layer reference together. `set`
can rename or deliberately update detected identity fields while preventing an
empty identity. `unlink` removes the selected layer reference and removes the
root record when that was its only binding. This shared-reference guard keeps
the generated candidate referentially valid even though the Input GUI normally
creates one binding per record.

The frozen Input delete behavior is also preserved: deleting a layer or
profile does not garbage-collect now-unreferenced `linkedApps`, while
duplicating a layer omits `linkedAppId`. Snapshot validation permits an
unreferenced root record but rejects a layer reference to a missing record.

Input's host focus service polls once per second and sends changed
`appName/process/path` data to the device. The device firmware owns identity
matching and the actual layer transition; the algorithm is not present in the
Input renderer or main process. Therefore candidate/apply/readback/restore
proves configuration parity, while full runtime parity additionally needs an
A/B focus transition and device-status read.

## Tier 3: Input host configuration

### Smart Actions

Input 0.18.0 advertises and implements:

- type text;
- run a command;
- open a URL;
- launch an application;
- name, group, search, reuse, and bind actions.

The offline `smart-action` editor now validates and preserves the separate
`smart_actions.json` authority. It supports list/show/create/set/delete for all
four typed payloads and `smart-action group` metadata plus ordered member CRUD.
Action keys use canonical `SA_<ID>` names; new IDs start at 1 and use
maximum-plus-one. Groups start at ID 0, may be empty, and container deletion
does not delete member actions. A Smart Action deletion replaces physical
`SA_<ID>` assignments with `KC_NONE` and removes group membership while keeping
the group valid. Unknown root, record, and payload fields survive edits, and a
Smart-only candidate leaves the exact `keymap.json` bytes unchanged.

`control set` accepts an existing `SA_<ID>` only for physical controls. Action
events and Multi Action branches retain their own frozen assignment grammar.
The CLI reports command definitions with `requiresCommandPermission: true`;
definition editing does not toggle the host permission.

The Input database has a `smartActionCmdEnabled` permission field. Command
execution must remain explicitly enabled and must be reported separately from
device deployment.

### Cheat Sheet and radial menu

Input hosts windows for Cheat Sheet and the joystick radial menu. Cheat Sheet
combines Keys, Actions, Multi Actions, and Smart Actions for the active layer.
The device emits show/hide/toggle notifications while Input renders the result.

## Tier 4: operational configuration

- application and firmware version discovery;
- device identity, transport, battery, profile, and layer status;
- device file list/read/write RPC;
- Input logs and input-monitoring permissions;
- firmware download, USB flashing, progress, and recovery;
- full settings reset;
- migration and preset installation.

These operations use distinct policy gates from normal semantic edits.
The installed Input updater/flasher remains the implementation provider. The
CLI invokes and observes that provider rather than owning a second firmware or
transport implementation.

## Verification levels

Each capability records one of four evidence levels:

1. **official** — described by an official Work Louder source;
2. **static** — present in the installed Codex/Input package;
3. **state** — present in a frozen cache/device readback;
4. **behavior** — exercised with literal output and post-state verification.

A public write-support claim requires all four levels for the exact version
adapter and a verified rollback.
