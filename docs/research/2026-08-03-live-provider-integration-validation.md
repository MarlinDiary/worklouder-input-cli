# Live provider integration validation (2026-08-03)

This record freezes the first end-to-end validation against the installed
Codex `26.727.51351`, Work Louder Input `0.18.0`, device kit `0.1.28`, and a
USB Codex Micro running firmware `v0.6.0`. Both applications stayed running;
the installers did not activate, restart, or navigate either GUI.

## Integration entrypoints

```sh
node scripts/install-codex-live-bridge.mjs
node scripts/install-input-live-bridge.mjs
node scripts/provider-handoff.mjs status
node scripts/provider-handoff.mjs input
node scripts/provider-handoff.mjs codex
```

Each installer verifies the exact application version and `app.asar` SHA-256
before attaching to a loopback inspector. Input additionally verifies the
loaded main-script SHA-256 and captures the released service container at an
exact breakpoint in its recurring discovery loop. The inspector is closed
after every operation. The bridges remain authenticated Unix sockets with
user-only socket/token modes.

`provider-handoff.mjs` is required because Codex and Input cannot
simultaneously own the vendor HID session. It stops discovery and disconnects
the releasing provider before acquiring the other provider. A failed
acquisition automatically reacquires the provider that was released. Input's
discovery cache is cleared during release so reacquisition emits a fresh
device event without restarting Input.

## Baseline, mutation, readback, restore

| Authority | Baseline | Temporary target | Modified readback | Restored readback |
| --- | --- | --- | --- | --- |
| Codex settings | source `e0ada13d...0392`, revision `04fc39c6...69a4`, brightness `100` | brightness `99` | source `871620f6...396`, revision `701b9d09...f734`, value `99` | original source SHA, original revision, value `100` |
| Codex Agent Keys | revision `20a6cd04...980d`, six empty slots | `AG05` keycap assignment | revision `1f6eb945...c601`, assignment type `keycap` | original revision, six empty slots |
| Input device configuration | revision `62a1c544...e075`, layer 1 `#FF0000` | layer 1 `#FE0000` | revision `5a5657fb...214a`, color `#FE0000` | original revision, color `#FF0000` |
| Input host settings | revision `08d213a6...039f`, command Smart Actions disabled | enabled | revision `79385c7d...f4bf`, enabled | original revision, disabled |

Every apply and restore used compare-and-swap revision gates, a stable
idempotency key, an immutable pre-operation backup, and a new live snapshot.
All four restore comparisons were exact. The final Codex settings source bytes
also reproduced the baseline SHA-256 exactly.

## Read-only live coverage

The live Input bridge advertised and exercised 18 capabilities:

- authenticated handshake and health;
- device list/status and file list/read;
- configuration snapshot/validate/apply/restore;
- host-settings snapshot/apply/restore;
- AppSense runtime state;
- Input Monitoring permission state;
- firmware status and plan;
- sanitized log snapshot.

Observed results:

- device file list/read returned `keymap.json` and `smart_actions.json`;
- Input Monitoring was granted;
- AppSense produced one matching live sample without changing focus;
- the radial menu resolved three sectors from the restored layer snapshot;
- the sanitized log bundle was reopened and both file size/SHA-256 entries
  were recomputed independently;
- Input selected firmware `v0.6.1` for current firmware `v0.6.0`; the frozen
  seven-phase plan was ready and bound to configuration revision
  `62a1c544...e075`.

Input `0.18.0` did not inject the optional preset, reset, firmware-update, or
recovery authorities. Each command exited `3` with code
`provider-unavailable`, named its missing negotiated capability, created no
output artifact, and performed no mutation. Firmware flashing was therefore
not run.

## Provider handoff verification

Both directions were exercised:

1. Codex released all comm/API/HID/joystick subscriptions; Input restarted
   discovery and connected one device; `worklouderctl device status` returned
   firmware `v0.6.0`.
2. Input stopped polling, cleared discovery state, and disconnected its device;
   Codex reacquired USB with comm/API plus HID and joystick subscriptions.

Final state is Codex-owned: lifecycle `started`, device `connected` over USB,
comm/API present, and both input subscriptions present. Input remains running
with its bridge and host authorities available, while its device discovery is
paused until `provider-handoff.mjs input` is called.

## Regression and packaging boundary

```text
cargo test --all-targets: 81 library + 32 CLI tests passed
npm test --prefix companion: 30 tests passed
scripts/test-companion-package.py: deterministic inventory/import/conformance passed
```

The exact private evidence bundle contains the source snapshots, mutation
receipts, pre-apply/pre-restore backups, post-state snapshots, capability-gate
errors, provider handoff records, and independent checksum results. It is kept
outside the repository because device configuration and focused-application
state are user data.
