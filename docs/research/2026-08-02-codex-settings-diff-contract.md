# Codex settings diff contract v1

`codex config diff` compares two complete, frozen-contract Codex settings
snapshots before an apply or restore transaction.

## Compared authority

The command validates both inputs with the existing Codex snapshot contract and
compares only their explicit `settings` objects. It intentionally excludes
snapshot transport metadata (`sourcePath`, source hash, installed version,
adapter, and warnings), frozen definitions, and derived `effectiveSettings`.
Those fields can differ without representing a requested Codex configuration
mutation; `effectiveSettings` is independently recomputed and checked while
each snapshot is validated.

## Output contract

The report contains:

- both input paths;
- both canonical settings revisions;
- an `identical` boolean;
- deterministic leaf changes rooted at `/settings`;
- each change's `added`, `removed`, or `changed` kind; and
- typed before/after JSON values when present.

Object keys use RFC 6901 JSON Pointer escaping and deterministic sorted order.
Arrays use stable numeric indices. Unknown settings fields remain visible
rather than being filtered through the currently known GUI schema.

## Operation boundary

The command is file-only and read-only. It opens neither bridge, app, device,
nor GUI, and it performs no settings write. The resulting revisions and paths
are suitable for human review or an agent-controlled apply preflight.
