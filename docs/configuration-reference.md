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

The official setup guide describes a maximum of six layers. Profile and layer
counts still require device-specific validation rather than a global constant.

### Assignable controls

- 13 switches, including the double-width Codex Command Key surface;
- encoder counter-clockwise, clockwise, and click;
- planar joystick sectors/radial assignments;
- touch-sensor behavior where exposed by the device adapter.

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

### Multi Actions

The UI offers tap, double tap, hold, and tap-then-hold branches. Timing includes
a tapping term and hold duration. Every branch references a basic key, Action,
Multi Action-compatible target, or Smart Action accepted by the exact device
adapter.

### Per-layer lighting

Backlight and underglow each carry effect, brightness, speed, magic, and color.
The inspected effect list is off, solid, snake, rainbow, breath, and gradient.
Input can apply a lighting choice across all layers.

### Linked applications / AppSense

A linked application associates a desktop application with a layer. Input's
host process watches the focused application and requests the corresponding
layer. Verification needs an A/B focus transition plus a device-status read,
not only the saved link record.

## Tier 3: Input host configuration

### Smart Actions

Input 0.18.0 advertises and implements:

- type text;
- run a command;
- open a URL;
- launch an application;
- name, group, search, reuse, and bind actions.

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
