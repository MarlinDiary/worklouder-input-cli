# Companion architecture

WorkLouderCTL is designed as a transaction-safe companion to Work Louder Input.
The CLI does not treat one cached JSON document as the whole system.

## State authorities

The initial Codex Micro model has multiple authorities:

| Authority | Responsibility |
|---|---|
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
│ Device transport + file RPC  │ Input cache/database adapter│
└──────────────────────────────┴─────────────────────────────┘
```

## Planned write transaction

1. Discover the exact Input installation and connected device.
2. Read device status, files, sizes, and checksums.
3. Read Input cache and database state.
4. Parse with an exact version adapter and preserve unknown fields.
5. Validate references, limits, permissions, and the target behavior.
6. Produce a deterministic plan and structural diff.
7. Recheck hashes to detect a concurrent edit.
8. Coordinate and pause Input.
9. Create private immutable backups.
10. Write changed device files in dependency-safe order.
11. Read back and compare bytes, decoded JSON, and checksums.
12. Atomically synchronize Input cache and database.
13. Reopen Input and emit one verification record.
14. Restore every authority if a mutation or synchronization step fails.

## Design rules

- read-only by default;
- no mutation for unknown versions;
- no hidden AI-only write route;
- deterministic JSON output and typed exit statuses;
- unknown fields survive parse/serialize cycles;
- all claims name the tested Input and firmware boundary;
- every successful mutation leaves a runnable rollback.
