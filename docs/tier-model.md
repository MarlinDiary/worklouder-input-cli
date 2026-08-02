# Configuration tier model

This document is the product boundary for WorkLouderCTL. It separates Codex
integration from Input-backed configuration so that one tool never overwrites
state owned by another.

## Normative boundary

> **Tier 1 uses the Codex configuration model and runtime. Every other tier
> uses Work Louder Input authorities. WorkLouderCTL configures all tiers.**

WorkLouderCTL may inspect and diagnose all tiers. Mutation policy is authority
aware:

| Tier | Name | Configuration authority | CLI mutation policy |
| --- | --- | --- | --- |
| 1 | Codex-native | Codex settings plus Codex runtime | Full transactional reads and writes through an exact Codex adapter |
| 2 | Input device configuration | Input plus device files | Transactional companion writes after exact-version validation |
| 3 | Input host automations | Input host runtime plus Input database | Transactional companion writes with permission and host-runtime checks |
| 4 | Input operations | Input updater, flasher, transport and recovery services | Diagnose first; explicit, separately gated operational commands |

The Tier 2–4 split is a WorkLouderCTL taxonomy derived from the observed state
authorities. It does not claim to be a vendor-defined tier system.

## Tier 1 — Codex-native

Tier 1 covers controls whose meaning is implemented by Codex rather than a
generic keyboard shortcut:

- six Agent Keys (`AG00`–`AG05`) and their task assignments;
- six Command Key slots (`ACT06`, `ACT07`, `ACT08`, `ACT09`,
  `ACT10_ACT11`, `ACT12`);
- Codex commands and skills assigned to command keys, the dial, or joystick;
- Agent Key source ordering and single-tap behavior;
- voice-button behavior;
- Codex-aware dial modes and custom dial gestures;
- Codex-aware joystick directions;
- Codex task-state lighting, global brightness, and auto-off policy.

Codex defines both the saved settings and the live semantics. It consumes vendor
HID events, resolves task and command state, and sends reactive lighting
messages. A generic Input keymap cannot reproduce this complete runtime.

### Tier 1 invariant

The CLI must never route a Tier 1 edit through Input merely because the same
physical control is visible there. It uses a versioned Codex settings adapter:

```text
worklouderctl codex inspect
worklouderctl codex doctor
worklouderctl codex export
worklouderctl codex diff CONFIG
worklouderctl codex agent-source get|set
worklouderctl codex agent-key get|set|clear AG00
worklouderctl codex command-key get|set|reset ACT06
worklouderctl codex dial get|set|reset
worklouderctl codex joystick get|set|reset
worklouderctl codex voice get|set
worklouderctl codex lighting get|set
worklouderctl codex apply CONFIG
worklouderctl codex restore BACKUP_ID
```

The inspected Codex build exposes `settings-read` and `settings-write` through
its native settings bridge. The adapter uses that validated bridge when Codex
is running and a separately verified storage adapter for coordinated offline
transactions. Direct Chromium LevelDB editing is excluded from the design.

## Tier 2 — Input device configuration

Tier 2 is device-persisted configuration edited by Input:

- profiles and active profile;
- up to six layers and active layer;
- switches, encoder rotation/click, and joystick sectors;
- basic keycodes, layer keys, temporary-layer keys, and profile keys;
- Actions/macros and Action groups;
- Multi Actions and Multi Action groups;
- per-layer backlight and underglow;
- linked applications/AppSense layer bindings;
- device-specific configuration fields.

The observed deployment files are `keymap.json` and `smart_actions.json`.
Input also keeps local cache and database representations, so a device-only
write is incomplete from the companion CLI's perspective.

## Tier 3 — Input host automations

Tier 3 needs Input running on the computer:

- Smart Actions: text, command, URL, and application launch;
- focused-application observation used by AppSense;
- Cheat Sheet show, hold, hide, and toggle controls;
- radial-menu presentation and selection;
- host permission for command-running Smart Actions;
- other model-specific host widgets and media or wallpaper services.

These features combine device references with desktop behavior. Verification
must therefore test the emitted device event and the host-side result.

## Tier 4 — Input operations

Tier 4 contains lifecycle operations with a larger failure radius:

- Input discovery, version inspection, logs, and permissions;
- transport and contention diagnosis;
- firmware availability checks;
- USB-only firmware update and recovery flows;
- factory/profile reset and repair;
- backup catalog, restore, and migration between exact adapters.

Firmware changes and destructive resets are separate commands from normal
configuration apply. They always require a fresh backup and explicit target
version.

Firmware and recovery commands delegate to the installed Input updater/flasher;
the CLI adds planning, artifact verification, logging, and post-state readback.
It does not reimplement the firmware or transport protocol.

## Cross-tier conflict rule

The Codex-native layer and Input layers share one physical device and vendor
communication channel. WorkLouderCTL therefore follows these rules:

1. detect Codex, Input, competing input-monitoring tools, transport, and exact
   versions before any write;
2. classify every requested field by tier before generating a plan;
3. split a cross-tier plan into explicit ordered transactions with one combined
   diff and rollback boundary;
4. coordinate Codex for Tier 1 and Input for Tier 2 or Tier 3 transactions;
5. preserve unknown fields byte-for-byte in every authority;
6. refresh/reopen the affected apps and verify settings plus runtime behavior;
7. leave a runnable rollback record for every mutation.

## Primary sources

- [Codex Micro setup](https://worklouder.cc/openai-micro-setup)
- [Codex Micro product page](https://worklouder.cc/codex-micro)
- [Input 0.18.0 release](https://github.com/worklouder/input-releases/releases/tag/v0.18.0)
