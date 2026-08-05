# Companion architecture

WorkLouderCTL is designed as a transaction-safe companion to Work Louder Input.
The CLI does not treat one cached JSON document as the whole system.

The authority boundary is defined in the [configuration tier model](tier-model.md):
Tier 1 uses Codex authorities; Tier 2 device bytes use Input under a bounded
provider lease and restore the requested final owner; Tier 3 and Tier 4
host/runtime operations stay delegated to Input.

## Provider strategy

WorkLouderCTL is a configuration control plane, not a hardware-driver project.
It delegates device transport, BLE/HID behavior, firmware flashing, Codex-aware
actions, and Input host actions to the currently installed Codex/Input builds.

Provider selection follows this order:

1. record the current RPC-verified owner and use Input as the device-byte
   authority for status/files/export/configuration and transaction postflight;
2. restore a pre-existing Codex owner and require comm/API plus HID/joystick
   subscriptions and a bounded RPC probe;
3. use the running Codex app for Tier 1 through the versioned
   [Codex Companion Bridge](codex-companion-bridge.md);
4. use the running Input app for Input-only host and operational authorities
   through the versioned
   [Input Companion Bridge](companion-bridge.md);
5. use the exact installed app's bundled device kit where a headless provider
   entry point is verified;
6. use coordinated file adapters only for state that the app itself persists;
7. preserve new/unknown fields and expose them through raw inspect/export;
8. disable typed writes for a changed schema until its adapter fixtures pass.

This keeps firmware, transport fixes, new device support, and runtime behavior
with the upstream applications while the CLI owns planning, validation, diff,
automation, verification, and rollback.

Each bridge is the target transport for its authority. It exposes an
allowlisted JSON-RPC surface over a private Unix socket and dispatches through
the owning application's existing services. The CLI negotiates named
capabilities, so Codex/Input can update internal implementations and GUIs without exposing those
implementation details as the public automation API.

## Implemented Codex Companion Bridge path

The `codex-companion-bridge-v1` client authenticates to the running Codex main
process over a user-only Unix socket. Its snapshot adapter delegates to Codex's
`settings-read` and `get-global-state` handlers, validates the exact frozen
definitions and all six Agent Key slots, and produces offline-editor-compatible
snapshots. When Codex injects complete explicit-setting replacement, the bridge
advertises settings apply/restore and serializes source-SHA plus settings-revision
CAS, immutable backup, exact explicit/effective readback, idempotency, restore,
and automatic rollback. A separate injected Agent Key replacer advertises
complete six-slot global-state apply/restore with its own revision CAS, backup,
idempotency, exact readback, restore, and rollback. The cross-language fixture
verifies `recent -> custom -> recent`, global brightness, auto-off, and voice mode
apply/readback/restore, every Agent Key assignment type, exact Agent Key revision
recovery, and exact source SHA recovery. For Codex 26.730.61309 the CLI's
exact-version/hash-gated installer supplies the external listener and captures
the existing connected `CodexMicroService`. Tier 1 settings and focus forwarding
stay in Codex. Device bytes use Input's authoritative bridge under a serialized
lease, then restore a fully subscribed and RPC-probed Codex service.

## Implemented Codex runtime recovery path

`codex-companion-runtime-v1` is a persistent bridge capability for liveness,
sharing the authenticated Codex main-process socket used by Tier 1. It verifies
the exact Codex 26.730.61309 app and native-module hashes and reads the captured
`CodexMicroService` without opening a per-command inspector or sending a process
signal. Status requires a connected device, live comm/API, settled
reconnect/topology Promises, and HID plus joystick subscriptions. Recovery
pauses Input when it is running, invalidates only the stale service attempt,
invokes the released service's bounded stop/start path, resumes Input after full
readback, and checks a post-resume stability window. It never patches the app or
replaces the Work Louder device kit.

## Implemented Input Companion Bridge path

The `input-companion-bridge-v1` client authenticates to the running Input main
process over a user-only Unix socket. The reference server advertises exact
capabilities and serializes all device operations through an injected Input
service adapter. Status, file list, file read, exact-byte export, device SHA-1,
host SHA-256, typed readback, atomic publication, revisioned configuration
snapshots, and live compare-and-swap validation pass a cross-language
conformance test. The same fixture verifies immutable pre-mutation backup,
apply, session-scoped idempotent replay, stale-revision rejection, complete
readback, restore, and automatic rollback after a failed readback. Write
capabilities appear only when Input injects a verified complete-set
`configurationWriter`; read-only integrations do not advertise them. The
repository also packages a one-call Electron main-process integration and a
read-only release conformance command. For Input 0.18.0 the CLI's exact-release
installer supplies this bridge/writer around Input's own connected services;
changed versions or hashes remain read-only until a matching adapter is
verified.

## Implemented direct compatibility path

The `input-bundled-device-kit-read-v1` adapter is the first implemented live
device provider. It launches the installed Input Electron binary with
`ELECTRON_RUN_AS_NODE=1` and loads that same bundle's
`@worklouder/wl-device-kit`. No transport package is copied into WorkLouderCTL.

Input and the headless reader share one JSON-RPC stream. The default
`require-closed` mode reports contention without changing process state. The
explicit `restart` mode asks Input to quit gracefully, waits for the main
process to stop, performs the read, and reopens the exact app path afterward.
Input can run normally without a renderer window, so coordination tracks its
main process. If Input briefly rejects a quit while initializing, the adapter
retries the same graceful request at bounded intervals and never
force-terminates Input. Status and list operations expose typed JSON;
export additionally validates safe relative paths, device SHA-1, host SHA-256,
exact size, typed manifest readback, file readback, and atomic publication.

## State authorities

The initial Codex Micro model has multiple authorities:

| Authority | Responsibility |
| --- | --- |
| Codex settings and runtime | Tier 1 Agent/Command assignments, Codex-aware dial/joystick/voice behavior, and task-state lighting |
| Device `keymap.json` | Deployed profiles, layers, controls, Actions, Multi Actions, lighting, and linked apps |
| Device `smart_actions.json` | Deployed Smart Action definitions and groups |
| Input cache | Local copies and device-file checksums |
| Input database | Editable definitions, selected state, app settings, and command permission |

Updating only one authority risks stale GUI state or a later overwrite. The
transaction engine therefore snapshots, validates, writes, verifies, and
synchronizes them as one operation.

## Provider lifecycle command

`worklouderctl provider` exposes exact-release bridge installation and device
ownership as one typed CLI family rather than requiring direct invocation of
repository scripts. Its embedded runtime is only an orchestrator: it verifies
the installed app versions and hashes, attaches to their loopback inspectors,
delegates to the existing Codex/Input services, closes the inspector, and
validates the returned JSON. It contains no HID driver, firmware protocol, or
host-action implementation. Handoffs retain the existing serialized lock,
single-owner invariant, automatic reacquisition rollback, and hidden Input
launch path.

## Architecture layers

```text
┌────────────────────────────────────────────────────────────┐
│ CLI / JSON / future agent protocol                         │
├────────────────────────────────────────────────────────────┤
│ Semantic model: profile, layer, control, action/multi/group│
├────────────────────────────────────────────────────────────┤
│ Plan + diff + validation + compatibility policy            │
├────────────────────────────────────────────────────────────┤
│ Transaction: backup, conflict check, apply, verify, restore│
├──────────────────────────────┬─────────────────────────────┤
│ Codex settings/IPC adapter   │ Input/device state adapters │
└──────────────────────────────┴─────────────────────────────┘
```

## Implemented guarded write transaction

The transaction core executes planning, CAS, backup, apply, readback,
synchronization, and rollback against the packaged reference providers. A
released Codex/Input integration supplies live discovery, runtime coordination,
and the same complete-set writer boundary; it does not introduce a second
transaction or a direct database/device write route. Physical device effects
remain provider-owned. The release boundary records live apply, readback,
exact restore, and connection continuity independently from fixture coverage.

1. Classify every requested field by tier and authority.
2. Discover the exact Codex/Input installations, runtimes, and connected device.
3. Read device status, files, sizes, and checksums.
4. Read Input cache and database state.
5. Parse with an exact version adapter and preserve unknown fields.
6. Validate references, limits, permissions, and the target behavior.
7. Produce a deterministic plan and structural diff.
8. Recheck hashes to detect a concurrent edit.
9. Coordinate the Codex and/or Input runtime selected by the plan.
10. Create private immutable backups.
11. Write one changed file directly, or require the provider/firmware atomic
    multi-file transaction before writing any member of a multi-file change.
12. Read back and compare bytes, decoded JSON, and checksums.
13. Atomically synchronize Codex settings and/or Input cache/database.
14. Refresh or reopen affected runtimes and emit one verification record.
15. Restore every authority if a mutation or synchronization step fails.

## Design rules

- read-only by default;
- Tier 1 writes require an exact Codex settings adapter;
- no replacement driver, BLE/HID stack, firmware flasher, or host runtime;
- every plan names its tier and state authority;
- no mutation for unknown versions;
- no hidden AI-only write route;
- deterministic JSON output and typed exit statuses;
- unknown fields survive parse/serialize cycles;
- all claims name the tested Input and firmware boundary;
- every successful mutation leaves a runnable rollback.
