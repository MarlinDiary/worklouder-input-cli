# Codex and Input configuration parity matrix

This matrix is the acceptance contract for “replace Codex + Input configuration
with one CLI.” A row reaches **parity** only when the CLI can read, validate,
diff, write, read back, and roll back the corresponding GUI configuration.

Codex-aware commands continue to execute in Codex and Input host actions
continue to execute in Input. Driver/runtime replacement is outside this
parity contract.

## Status vocabulary

- **researched** — schema and call path are inventoried;
- **adapter-pending** — typed implementation and fixtures are next;
- **read-verified** — exact-version read and readback passed; writes remain pending;
- **candidate-verified** — a strict offline editor preserves unknown content,
  rehashes the full snapshot, and passes apply/readback/restore against the
  isolated transaction fixture;
- **verified** — exact-version read/write/readback/rollback passed;
- **parity** — the CLI covers every observed control in that GUI surface.

## Tier 1 — Codex configuration

| GUI surface | Configuration coverage | Planned command family | Current status |
| --- | --- | --- | --- |
| Agent source | pinned, recent, priority, custom | `codex agent-source get/set` | strict candidate plus bridge `recent -> custom -> recent` apply/readback/restore fixture verified |
| Agent Keys | `AG00`–`AG05`, task/command/keycap/Skill/empty | `codex agent-key snapshot/get/set/clear/apply/restore` | strict candidate, all assignment types, CAS, idempotency, stale rejection, exact readback, restore, and rollback fixture verified |
| Agent tap behavior | single-tap focus toggle | `codex agent-key tap-mode get/set` | strict offline candidate verified; generic complete-settings bridge transaction verified |
| Command Keys | six logical slots, command/Skill/keycap/reset | `codex command-key get/set/reset` | strict offline candidate and reset verified; generic complete-settings bridge transaction verified |
| Voice button | push-to-talk, Voice Chat | `codex voice get/set` | strict offline candidate plus `push-to-talk -> realtime -> push-to-talk` bridge apply/readback/restore fixture verified |
| Dial mode | composer, reasoning, scroll, custom | `codex dial mode get/set` | researched |
| Dial gestures | left, right, click, long press | `codex dial gesture get/set/clear` | researched |
| Joystick | up, right, down, left command/Skill/empty | `codex joystick get/set/clear` | researched |
| Lighting brightness | integer 0–100 | `codex lighting brightness get/set` | strict offline candidate plus `100 -> 37 -> 100` bridge apply/readback/restore fixture verified |
| Lighting auto-off | off, 30s, 1m, 3m, 10m, 30m, 1h | `codex lighting auto-off get/set` | strict offline candidate plus `3-minutes -> 10-minutes -> 3-minutes` bridge apply/readback/restore fixture verified |
| Layout reset | installed-build default layout | `codex reset layout` | researched |
| Full object | export, validate, diff, apply, restore | `codex config snapshot/diff/apply/restore` | settings-only structural diff plus authenticated snapshot/CAS/apply/readback/restore/rollback fixture verified; released Codex integration pending |
| Runtime health | connected control plane, settled reconnect, HID/joystick subscriptions | `codex runtime status/recover` | exact 26.727.51351 bundle contract, live failed-state capture and service-only recovery verified; CDP transport and CLI surface regression verified |

### Tier 1 adapter

Codex 26.727.51351 exposes `settings-read`, `settings-write`, and global-state
handlers via the native `vscode://codex/` bridge. The reference Codex Companion
Bridge delegates to those handlers, freezes the settings and Agent Key schemas,
and advertises writes only with injected complete explicit-setting replacement.
Its transaction preserves every unmodified setting, compares source and
canonical settings revisions, verifies exact explicit/effective readback, and
restores the pre-mutation snapshot after any failure. The inspected released
Codex build does not publish the external Unix socket, so released integration
remains separate from fixture transaction evidence.

The separate `codex-node-inspector-runtime-v1` adapter covers runtime health,
not configuration mutation. It is pinned to the exact app version plus
`app.asar` and native permission/topology module hashes. Recovery coordinates
with the running Input process, restarts only the released `CodexMicroService`,
and treats missing HID or joystick subscriptions and never-settled reconnect
Promises as failures even when the USB interface still enumerates.

## Tier 2 — Input device configuration

| GUI surface | Configuration coverage | Planned command family | Current status |
| --- | --- | --- | --- |
| Profiles | create, duplicate, rename, select, delete, up to six | `profile` | full lifecycle candidate-verified; profile create included in combined fixture apply/readback/restore |
| Layers | create, duplicate, rename, reorder, delete, color, up to six | `layer` | lifecycle and ordering candidate-verified; layer create included in combined fixture transaction; live active-layer selection pending |
| Basic keys | all frozen Input 0.18.0 device tokens | `control list/show/set` | candidate-verified |
| Layer/profile keys | normal/temp layers and profiles 1–6 | `control list/show/set` | candidate-verified |
| Actions | simple/advanced events, delays, groups | `action` | list/show/create/rename/delete, event CRUD/reorder, and group metadata/member/orphan-cascade candidate-verified |
| Multi Actions | tap, double tap, hold, tap-hold, timing, groups | `multi-action` | complete field CRUD plus group metadata/member/orphan-cascade candidate-verified; fixture apply/readback/restore verified |
| Encoder | counter-clockwise, clockwise, click | `control list/show/set` | candidate/apply/restore fixture-verified |
| Joystick sectors | existing radial sectors and targets | `control list/show/set` | candidate-verified; sector CRUD pending |
| Backlight | effect, brightness, speed, magic, color, apply-to-all | `layer lighting` | candidate-verified; fixture apply/readback/restore verified |
| Underglow | effect, brightness, speed, magic, color, apply-to-all | `layer lighting` | candidate-verified; fixture apply/readback/restore verified |
| AppSense links | list/show, application identity, link, update, unlink | `appsense` | candidate and current-cache schema verified; fixture apply/readback/restore verified; live focus transition tracked separately |
| Presets | inspect, preview, install | `preset` | researched |
| Full object | cache capture, export, snapshot, validate, diff, apply, restore | `input config snapshot`, `device export`, `device config snapshot/validate/apply/restore` | cache adapter is byte-exact and revision-equivalent to the bridge core; transaction fixture verified; released writer pending |

## Tier 3 — Input host configuration

| GUI surface | Configuration coverage | Planned command family | Current status |
| --- | --- | --- | --- |
| Smart Actions | text, command, URL, application | `smart-action` | typed list/show/create/set/delete, `SA_<ID>` control binding, and reference cascade candidate-verified against current Input 0.18.0 cache bytes |
| Smart Action groups | create, rename, move, delete | `smart-action group` | metadata and ordered member CRUD, empty groups, and container-only delete candidate-verified |
| Command permission | explicit host command toggle | `input permission command` | researched |
| Cheat Sheet | show, hold, hide, toggle assignments | `cheat-sheet` | researched |
| Radial menu | sectors, labels, referenced actions | `radial` | researched |
| AppSense runtime | focus observer and layer transition | `appsense test` | researched |

## Tier 4 — Input operations

| GUI surface | Configuration coverage | Planned command family | Current status |
| --- | --- | --- | --- |
| Device setup | identity, transport, battery, firmware | `doctor`, `device status` | read-verified |
| Input permissions | Input Monitoring and Accessibility | `input permissions` | researched |
| Firmware check | compatible release and requirements | `firmware check` | researched |
| Firmware update | delegate download/USB flash to Input, then verify readback | `input firmware update` | adapter-pending |
| Reset settings | full backup, reset, post-state | `device reset` | researched |
| Logs | collect and sanitize diagnostic bundle | `logs collect` | researched |
| Recovery | restore files or firmware recovery | `device recover` | adapter-pending |

## Cross-authority acceptance test

One end-to-end parity fixture must combine:

1. a Tier 1 Agent/Command/dial/joystick/lighting change;
2. a Tier 2 profile/layer/key/action/lighting change;
3. a Tier 3 Smart Action and AppSense change;
4. a dry-run Tier 4 firmware or reset plan;
5. one unified diff and backup catalog;
6. ordered Codex and Input coordination;
7. exact settings/device/cache/database readback;
8. observed key, host action, layer transition, and lighting behavior;
9. one rollback restoring all original hashes and behaviors.
