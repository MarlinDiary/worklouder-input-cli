# Codex Companion Bridge v1

The Codex Companion Bridge is WorkLouderCTL's stable Tier 1 automation boundary.
It keeps Codex as the settings, global-state, and runtime authority while giving
the CLI a versioned, authenticated transaction API.

## Release boundary

Static inspection of Codex 26.727.51351 found the native handlers
`settings-read`, `settings-write`, `get-global-state`, and `set-global-state`.
The renderer reaches native handlers with `POST vscode://codex/{method}`. The
inspected release does not publish an external listener. This repository ships
the reference main-process adapter and integration, not an `app.asar` patch.

The bridge is useful in two environments:

1. the isolated fixture, which verifies the complete CLI transaction today;
2. a Codex main process that installs `installCodexCompanionBridge` and supplies
   an exact settings replacer.

The adapter advertises mutation capabilities only when the integration supplies
complete explicit-setting replacement, including removal of keys that are
absent from the target.

## Discovery and authentication

Default paths:

```text
$HOME/Library/Application Support/Codex/worklouderctl-codex-bridge-v1.sock
$HOME/Library/Application Support/Codex/worklouderctl-codex-bridge-v1.token
```

Environment overrides:

```text
WORKLOUDERCTL_CODEX_BRIDGE_SOCKET
WORKLOUDERCTL_CODEX_BRIDGE_TOKEN_FILE
```

The Unix socket and token are owned by the current user and have mode `0600`.
The transport is newline-framed JSON-RPC 2.0. A connection begins with exactly
one authenticated `bridge.hello` request and negotiates named capabilities.

## Capabilities

| JSON-RPC method | Capability | Meaning |
| --- | --- | --- |
| `bridge.hello` | `bridge.handshake.v1` | authenticate and negotiate |
| `bridge.health` | `bridge.health.v1` | report bridge/Codex version and uptime |
| `codex.settings.snapshot` | `codex.settings.snapshot.v1` | read explicit/effective settings, definitions, source SHA, and canonical revision |
| `codex.settings.apply` | `codex.settings.apply.v1` | complete-set CAS apply with readback and rollback |
| `codex.settings.restore` | `codex.settings.restore.v1` | complete-set CAS restore with readback and rollback |
| `codex.agentKeys.snapshot` | `codex.agentKeys.snapshot.v1` | read and validate all six custom Agent Key slots |
| `codex.agentKeys.apply` | `codex.agentKeys.apply.v1` | complete six-slot global-state CAS apply |
| `codex.agentKeys.restore` | `codex.agentKeys.restore.v1` | complete six-slot global-state CAS restore |

## CLI workflow

```console
worklouderctl codex bridge inspect
worklouderctl codex config snapshot --output before.json
worklouderctl codex agent-source set \
  --input before.json custom --output candidate.json
worklouderctl codex config apply \
  --input candidate.json --backup pre-apply.json \
  --idempotency-key agent-source-custom-1
worklouderctl codex config restore \
  --input before.json --backup pre-restore.json \
  --idempotency-key agent-source-restore-1
worklouderctl codex agent-key assignments
worklouderctl codex agent-key snapshot --output agent-before.json
worklouderctl codex agent-key set \
  --input agent-before.json AG01 --command COMMAND_ID \
  --output agent-candidate.json
worklouderctl codex agent-key apply \
  --input agent-candidate.json --backup agent-pre-apply.json \
  --idempotency-key agent-key-command-1
worklouderctl codex agent-key restore \
  --input agent-before.json --backup agent-pre-restore.json \
  --idempotency-key agent-key-restore-1
```

Pass `--socket PATH --token PATH` after `codex bridge` or `codex config`, or
after `codex agent-key assignments`, to select an isolated bridge.

## Settings transaction

Every apply and restore request contains:

- `expectedSourceSha256` — compare-and-swap against the settings source bytes;
- `expectedSettingsRevision` — compare-and-swap against recursive-key-sorted
  explicit settings;
- `targetSettingsRevision` — independently recomputed from the candidate;
- `idempotencyKey` — a stable key for exact retries in the bridge session;
- complete `settings` and `effectiveSettings` objects.

The main-process adapter serializes operations and performs this sequence:

1. capture current source SHA, explicit settings, effective settings, and
   canonical revision;
2. compare both expected revisions before the first write;
3. preserve the complete pre-mutation snapshot;
4. replace the complete explicit Codex Micro setting set through Codex;
5. flush and capture a second snapshot;
6. compare exact explicit settings, effective settings, and target revision;
7. restore the pre-mutation explicit set and verify it after any mutation or
   readback error.

The Rust client creates or reopens an immutable backup before calling apply or
restore. Existing backup files enable retries without replacing the original.

## Agent Key snapshot

The global-state key is `codex-micro-custom-agent-assignments`. Slots are
`AG00` through `AG05`. Each value is one of:

- `null`;
- a command `{ "type": "command", "commandId": "..." }`;
- a Skill `{ "type": "skill", "skillName": "...", "skillPath": "..." }`;
- a task `{ "hostId": "...", "threadKey": "...", "title": "..." }`;
- a keycap `{ "keycapId": "..." }`.

The bridge normalizes all six slots, preserves unknown fields inside valid
assignment objects, and hashes recursive-key-sorted compact JSON with the frozen
`codex-agent-keys-revision-v1` framing.

Agent Key apply/restore takes `expectedGlobalStateRevision`,
`targetGlobalStateRevision`, `idempotencyKey`, and the complete six-slot
`assignments` object. Codex performs exact complete-object replacement through
`set-global-state`; the adapter snapshots, checks CAS, writes, snapshots again,
and compares the exact assignments and revision. A failed write/readback
restores and verifies the pre-mutation object. Mutation capabilities appear only
when the main-process integration injects `agentKeysWriter.replaceAssignments`.

The assignment object and `codex-micro-agent-source` setting are separate Codex
authorities. Use `codex agent-source set ... custom` plus `codex config apply`
when the custom slots should become the active Agent Key source. Agent Key
mutation itself does not silently change that setting.

## Reference integration

```js
import { installCodexCompanionBridge } from "./companion/index.mjs";

const bridge = await installCodexCompanionBridge({
  app,
  request: (method, params) => nativeRequest(method, params),
  settingsReplacer: {
    replaceSettings: ({ settings }) => replaceCompleteCodexMicroSettings(settings),
  },
  agentKeysWriter: {
    replaceAssignments: ({ key, assignments }) =>
      nativeRequest("set-global-state", { key, value: assignments }),
  },
});
```

Omit either writer for a snapshot-only integration of that authority. The
handshake excludes the corresponding apply/restore capabilities.

## Verification

```console
npm test --prefix companion
cargo test --all-targets
cargo build --release
WORKLOUDERCTL_BIN=./target/release/worklouderctl \
  ./scripts/test-codex-bridge-e2e.sh
```

The E2E test proves `recent -> custom -> recent`, global brightness
`100 -> 37 -> 100`, and lighting auto-off
`3-minutes -> 10-minutes -> 3-minutes`, plus voice mode
`push-to-talk -> realtime -> push-to-talk`; applies command/Skill/task/keycap/
empty Agent Key assignments; verifies idempotent replay and stale-CAS rejection;
restores the exact six-slot revision; and requires the restored settings source
SHA-256 to equal the baseline source SHA-256.
