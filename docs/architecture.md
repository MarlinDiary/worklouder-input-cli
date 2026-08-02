# Companion architecture

WorkLouderCTL is designed as a transaction-safe companion to Work Louder Input.
The CLI does not treat one cached JSON document as the whole system.

The authority boundary is defined in the [configuration tier model](tier-model.md):
Tier 1 is configured in Codex, while Tier 2 and above depend on Input.

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
│ Device transport + file RPC  │ Input cache/database adapter│
└──────────────────────────────┴─────────────────────────────┘
```

## Planned write transaction

1. Classify every requested field by tier and authority.
2. Discover the exact Codex/Input installations and connected device.
3. Read device status, files, sizes, and checksums.
4. Read Input cache and database state.
5. Parse with an exact version adapter and preserve unknown fields.
6. Validate references, limits, permissions, and the target behavior.
7. Produce a deterministic plan and structural diff.
8. Recheck hashes to detect a concurrent edit.
9. Coordinate and pause Input.
10. Create private immutable backups.
11. Write changed device files in dependency-safe order.
12. Read back and compare bytes, decoded JSON, and checksums.
13. Atomically synchronize Input cache and database.
14. Reopen Input and emit one verification record.
15. Restore every authority if a mutation or synchronization step fails.

## Design rules

- read-only by default;
- Tier 1 remains Codex-authored unless an exact Codex adapter is verified;
- every plan names its tier and state authority;
- no mutation for unknown versions;
- no hidden AI-only write route;
- deterministic JSON output and typed exit statuses;
- unknown fields survive parse/serialize cycles;
- all claims name the tested Input and firmware boundary;
- every successful mutation leaves a runnable rollback.
