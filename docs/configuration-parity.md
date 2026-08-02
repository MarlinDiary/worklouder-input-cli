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
| Agent source | pinned, recent, priority, custom | `codex agent-source get/set` | researched |
| Agent Keys | `AG00`–`AG05`, task/command/keycap/Skill/empty | `codex agent-key get/set/clear` | researched |
| Agent tap behavior | single-tap focus toggle | `codex agent-key tap-mode get/set` | researched |
| Command Keys | six logical slots, command/Skill/keycap/reset | `codex command-key get/set/reset` | researched |
| Voice button | push-to-talk, Voice Chat | `codex voice get/set` | researched |
| Dial mode | composer, reasoning, scroll, custom | `codex dial mode get/set` | researched |
| Dial gestures | left, right, click, long press | `codex dial gesture get/set/clear` | researched |
| Joystick | up, right, down, left command/Skill/empty | `codex joystick get/set/clear` | researched |
| Lighting brightness | integer 0–100 | `codex lighting brightness get/set` | researched |
| Lighting auto-off | off, 30s, 1m, 3m, 10m, 30m, 1h | `codex lighting auto-off get/set` | researched |
| Layout reset | installed-build default layout | `codex reset layout` | researched |
| Full object | export, validate, diff, apply, restore | `codex export/diff/apply/restore` | adapter-pending |

### Tier 1 adapter

Codex 26.727.51351 exposes `settings-read` and `settings-write` via the native
`vscode://codex/` bridge. The renderer sends partial setting updates and then
invalidates `get-settings`. The adapter must preserve every unmodified setting,
validate the Codex Micro schema, coordinate the running app, and confirm that
the runtime reloaded the new value.

## Tier 2 — Input device configuration

| GUI surface | Configuration coverage | Planned command family | Current status |
| --- | --- | --- | --- |
| Profiles | create, rename, select, delete | `profile` | candidate-verified for list/show/rename/select; create/delete pending |
| Layers | create, rename, select, delete, color, up to six | `layer` | candidate-verified for list/show/rename/color; create/select/delete pending |
| Basic keys | all frozen Input 0.18.0 device tokens | `control list/show/set` | candidate-verified |
| Layer/profile keys | normal/temp layers and profiles 1–6 | `control list/show/set` | candidate-verified |
| Actions | simple/advanced events, delays, groups | `action` | list/show/create/rename/delete and event CRUD/reorder candidate-verified; group CRUD pending |
| Multi Actions | tap, double tap, hold, tap-hold, timing | `multi-action` | researched |
| Encoder | counter-clockwise, clockwise, click | `control list/show/set` | candidate/apply/restore fixture-verified |
| Joystick sectors | existing radial sectors and targets | `control list/show/set` | candidate-verified; sector CRUD pending |
| Backlight | effect, brightness, speed, magic, color | `lighting backlight` | researched |
| Underglow | effect, brightness, speed, magic, color | `lighting underglow` | researched |
| AppSense links | application-to-layer mapping | `appsense` | researched |
| Presets | inspect, preview, install | `preset` | researched |
| Full object | export, snapshot, validate, diff, apply, restore | `device export`, `device config snapshot/validate/apply/restore` | transaction fixture verified; released writer pending |

## Tier 3 — Input host configuration

| GUI surface | Configuration coverage | Planned command family | Current status |
| --- | --- | --- | --- |
| Smart Actions | text, command, URL, application | `smart-action` | researched |
| Smart Action groups | create, rename, move, delete | `smart-action group` | researched |
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
