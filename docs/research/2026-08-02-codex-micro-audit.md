# Codex Micro and Input audit — 2026-08-02

This audit freezes the research boundary used to design the first
WorkLouderCTL adapters. It intentionally separates official documentation,
installed-package evidence, live device state, and inference.

## Snapshot

| Component | Observed value | Evidence type |
| --- | --- | --- |
| Host | macOS, Apple Silicon | live inventory |
| Device | Codex Micro, USB/BLE capable | live HID inventory and official specification |
| VID/PID | `0x303A` / `0x8360` | live HID inventory |
| Firmware | `v0.6.0` | `sys.version` read after Input reconnect |
| Codex app | `26.727.51351` build `6119` | bundle metadata and extracted `package.json` |
| Input app | `0.18.0` | live bundle plus extracted `package.json` |
| Codex ASAR SHA-256 | `a529edd72e10b08931c0d695b5e3e6a0be7f51874610dafc04f578436ab7d74d` | frozen application artifact |
| Input ASAR SHA-256 | `8e530188bc693ca1b9950bdc0515adfc349a3563e1841fe61ff2d692dc6b2da8` | frozen application artifact |

The Input application updated from 0.17.3 to 0.18.0 during the audit. This is
direct evidence that adapter selection must use the current on-disk and
runtime version immediately before a mutation.

## Live state frozen at reconnect

Input reconnected at `2026-08-02 14:52:30` local time and returned:

```text
sys.version = v0.6.0
keymap.json       size=3294 sha1=b06f68b953688a8b16384e389c9ec7a91bfe1b11
smart_actions.json size=41 sha1=a99ff018aabf9a0436eaa0984d982567b8f55b04
```

The cached `keymap.json` SHA-256 was
`803779884034f0268339f3b227b550db043068403e5024cc5270bc229467a6c6`.
It matched the previously frozen hardware baseline byte-for-byte.

The keymap contained one profile, two layers, eleven Actions/macros, zero
Multi Actions, and zero linked applications. The separate Smart Action file
contained an empty list.

## Rollback incident and result

An unlabeled Input control was touched while inventorying the GUI. The UI
briefly switched the visible layer legend from Mac to Windows and was switched
back immediately. Input attempted two writes:

```text
fs.writebin -> task aborted
fs.writebin -> Request timed out
```

Verification after a graceful Input restart showed the original device file
size and SHA-1 again, while the local cache remained byte-identical to the
original SHA-256. No firmware update or intentional keymap change was applied.

This incident validates three requirements for the CLI:

1. visual state is insufficient as a write result;
2. chunked RPC timeout needs a fresh device file-list/readback check;
3. Input process coordination is mandatory before a companion write.

## Codex-native findings

The installed Codex package bundles `@worklouder/device-kit-oai` and a nested
Work Louder device kit. Static call sites show direct HID topology detection,
Agent/Command events, analog events, and task-aware lighting. Codex therefore
owns an application-specific runtime rather than only a set of ordinary
keyboard shortcuts.

The complete Tier 1 setting, slot, voice, dial, joystick, keycap, command, and
lighting inventory is captured in [Configuration reference](../configuration-reference.md).

## Input findings

Input 0.18.0 exposes Keymap, Preset, and Setup surfaces. The inspected Codex
Micro model includes profiles, six-layer controls, 171 basic-key choices,
Actions, advanced macro events, Multi Actions, per-layer lighting, linked apps,
Smart Actions, Cheat Sheet, radial UI, firmware operations, and local
cache/database state.

Input's main process includes focused-application observation and host
notifications for command, application, URL, Cheat Sheet, and radial behavior.
This creates a material distinction between device-resident Tier 2 behavior
and Input-running Tier 3 behavior.

## State authorities

| Authority | Observed responsibility | Transaction requirement |
| --- | --- | --- |
| Codex settings store | Tier 1 assignments and behavior | Inspect separately; preserve Codex authority |
| Codex runtime | Agent/task resolution and reactive RGB | Behavioral doctor checks |
| Device `keymap.json` | Tier 2 profiles, layers, controls, Actions, Multi Actions, lighting, links | Backup, exact write, readback |
| Device `smart_actions.json` | Smart Action definitions/references | Dependency-safe write and readback |
| Input device cache | local file copy and checksums | atomic synchronization |
| Input JSON database | device records, definitions, selected state, permissions | exact adapter plus atomic synchronization |
| Input host runtime | AppSense, Smart Actions, Cheat Sheet, radial windows | reopen and behavioral verification |

## Protocol surface observed

Read-only and file operations observed in the Input runtime include
`sys.version`, `device.status`, `fs.list`, `fs.readbin`, and `fs.writebin`.
Host calls include `host.focused_app`; device notifications cover Smart Action,
Cheat Sheet, and radial behavior. Codex uses a separate vendor notification
surface described in the configuration reference.

Method names alone are research evidence rather than a stable public protocol.
Every adapter must detect method shape, chunking rules, timeouts, and response
checksums against exact app/firmware fixtures.

## Compatibility conclusions

- Codex 26.727.51351 + firmware v0.6.0: Tier 1 inspection baseline.
- Input 0.17.3 + firmware v0.6.0: earlier schema fixture.
- Input 0.18.0 + firmware v0.6.0: current read-only research baseline.
- Input 0.18.0 semantic writes: pending a sanitized no-op round trip, one
  intentional hardware change, literal readback, and verified rollback.
- Firmware v0.6.1: offered by the GUI but excluded from this audit.

## Development decisions

1. Ship `doctor` and export/diff before semantic mutation.
2. Make `tier` a first-class field in the capability registry and every plan.
3. Keep Tier 1 editing in Codex; expose open-settings and diagnostics first.
4. Build exact Input 0.17.3 and 0.18.0 adapters rather than a nearest-version
   fallback.
5. Coordinate Input, lock all authorities, recheck hashes, then write.
6. Treat a timeout as unknown post-state until a fresh device readback.
7. Verify emitted behavior, not labels or GUI toasts.
8. Gate firmware and destructive reset under separate operational commands.

## Official sources checked

- [Codex Micro setup](https://worklouder.cc/openai-micro-setup): Codex-native
  configuration, Input customization, six layers, AppSense, communication
  modes, and reset behavior.
- [Codex Micro product page](https://worklouder.cc/codex-micro): direct Codex
  integration, joystick Skills, dial reasoning control, RGB, hardware, and
  Input-backed extra layers.
- [Input 0.18.0 release](https://github.com/worklouder/input-releases/releases/tag/v0.18.0):
  ChatGPT layer, Smart Actions, Cheat Sheet, GIF support, and release scope.
