# Live provider integration validation (2026-08-03)

This record freezes the first end-to-end validation against the installed
Codex `26.727.51351`, Work Louder Input `0.18.0`, device kit `0.1.28`, and a
USB Codex Micro initially running firmware `v0.6.0`. Both applications stayed running;
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
acquisition automatically reacquires the provider that was released. A request
for the current owner returns an idempotent result without re-enumerating HID,
preventing redundant native discovery cycles. Input `0.18.0` cannot safely
restart discovery after disposing its node-hid worker on the tested macOS 27
beta: that path trapped inside `IOHIDManager`. Input acquisition now starts a
fresh hidden Input process, installs the version- and hash-gated bridge, and
waits for one connected device. A failed acquisition quiesces Input before it
reacquires Codex, so an error does not leave two active device owners.

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

Input assigns a session-local `deviceId` after discovery. A second full
mutation proved reconnect portability: a snapshot captured as device `1` was
applied to live session `3`, read back as `#FE0000`, and restored from the old
baseline to `#FF0000`. The CLI resolves the current destination from a fresh
pre-mutation backup, while the bridge checks stable PID, device type, and layout
before accepting a snapshot from an earlier session. Overlay revision 3 hot-
replaced the adapter in the running Input process without restarting or
focusing the GUI.

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
output artifact, and performed no mutation. The CLI firmware command therefore
did not run a flash. A later official Input startup independently applied its
selected firmware update: Input's log records flash progress through `100%`,
`Flash ended`, reset completion, and `sys.version` readback `v0.6.1`. That
startup behavior is separate from the CLI capability result above.

## Provider handoff verification

Both directions were exercised:

1. Codex released all comm/API/HID/joystick subscriptions; Input restarted
   discovery and connected one device; `worklouderctl device status` returned
   firmware `v0.6.0` during the first handoff pass.
2. Input stopped polling, cleared discovery state, and disconnected its device;
   Codex reacquired USB with comm/API plus HID and joystick subscriptions.

Final state is Codex-owned: lifecycle `started`, device `connected` over USB,
comm/API present, and both input subscriptions present. After the stress pass,
Input was stopped and its complete official Developer ID bundle was restored
and signature-verified. `provider-handoff.mjs input` performs the coordinated
release/fresh-process/bridge path when Input authority is next required. The
device firmware at final readback is `v0.6.1` following Input's startup update.

## Regression and packaging boundary

The final counts and package hashes are recorded in the private machine-readable
verification record generated after the complete test and packaging pass.

The exact private evidence bundle contains the source snapshots, mutation
receipts, pre-apply/pre-restore backups, post-state snapshots, capability-gate
errors, provider handoff records, and independent checksum results. It is kept
outside the repository because device configuration and focused-application
state are user data.

## Subsequent ownership hardening

Repeated handoffs after the first successful matrix exposed two host-runtime
boundaries on this macOS 27 beta machine: Input 0.18.0 can trap in its native
`node-hid` worker during re-enumeration, and either app can invoke its own
`start()` again after a successful release. The follow-up implementation:

- binds every inspector session to the exact PID and executable and waits for
  the loopback port to be released;
- retains a local event-loop handle and deadline for every CDP command;
- runs fresh Input providers under a user-scoped `launchctl` job;
- installs a reversible `start()` suppression lease on the non-owner; and
- quiesces a relaunched Input process before reacquiring Codex after a failed
  Input acquisition.

The final recovery stopped Input, restored the official Input 0.18.0 app bundle
from its verified release archive, passed strict deep code-signature validation,
and left Codex as the only owner: USB connected with comm/API, HID, and joystick
subscriptions present. These stability observations do not change the earlier
successful apply/readback/restore evidence or imply that the CLI flashed
firmware.
