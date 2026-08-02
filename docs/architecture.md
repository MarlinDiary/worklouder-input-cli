# Companion architecture

WorkLouderCTL is designed as a transaction-safe companion to Work Louder Input.
The CLI does not treat one cached JSON document as the whole system.

The authority boundary is defined in the [configuration tier model](tier-model.md):
Tier 1 uses Codex authorities, while Tier 2 and above use Input authorities.
The CLI provides full configuration coverage across both sides.

## Provider strategy

WorkLouderCTL is a configuration control plane, not a hardware-driver project.
It delegates device transport, BLE/HID behavior, firmware flashing, Codex-aware
actions, and Input host actions to the currently installed Codex/Input builds.

Provider selection follows this order:

1. use the running app's supported settings/IPC/RPC surface;
2. use the exact installed app's bundled device kit where a headless provider
   entry point is verified;
3. use coordinated file adapters only for state that the app itself persists;
4. preserve new/unknown fields and expose them through raw inspect/export;
5. disable typed writes for a changed schema until its adapter fixtures pass.

This keeps firmware, transport fixes, new device support, and runtime behavior
with the upstream applications while the CLI owns planning, validation, diff,
automation, verification, and rollback.

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

## Planned layers

```text
┌────────────────────────────────────────────────────────────┐
│ CLI / JSON / future agent protocol                         │
├────────────────────────────────────────────────────────────┤
│ Semantic model: profile, layer, control, action, lighting  │
├────────────────────────────────────────────────────────────┤
│ Plan + diff + validation + compatibility policy            │
├────────────────────────────────────────────────────────────┤
│ Transaction: backup, conflict check, apply, verify, restore│
├──────────────────────────────┬─────────────────────────────┤
│ Codex settings/IPC adapter   │ Input/device state adapters │
└──────────────────────────────┴─────────────────────────────┘
```

## Planned write transaction

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
11. Write changed device files in dependency-safe order.
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
