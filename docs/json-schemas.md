# JSON Schemas

WorkLouderCTL embeds its public automation schemas in the binary. This keeps
the documents version-matched with the command that consumes or emits the
artifacts.

```console
worklouderctl --json schema list
worklouderctl --json schema show agent-execution-v1
worklouderctl --json schema show backup-inspection-v1
worklouderctl --json schema show command-envelope-v1
worklouderctl --json schema show configuration-v1
worklouderctl --json schema show doctor-report-v1
worklouderctl --json schema show input-operations-v1
worklouderctl --json schema show provider-lifecycle-v1
worklouderctl --json schema show release-archive-v1
worklouderctl --json schema show transaction-v1
worklouderctl --json schema show error-v1
```

`schema show` writes the schema document itself, rather than wrapping it in a
second response object. The checked-in sources live under `spec/schemas/` and
use JSON Schema Draft 2020-12.

| Registry name | Contract |
| --- | --- |
| `agent-execution-v1` | validation and execution results for shell-free agent calls |
| `backup-inspection-v1` | verified artifact kind, restore availability, and migration requirements |
| `command-envelope-v1` | shell-free `argv`, output mode, and accepted exit statuses |
| `configuration-v1` | Codex settings, Codex Agent Keys, Input device configuration, and Input host settings snapshots |
| `doctor-report-v1` | global provider health, checks, discovered devices, and authenticated configuration readiness |
| `transaction-v1` | coordinated plan, transaction receipt, and private backup catalog |
| `error-v1` | typed JSON error written to stderr |
| `input-operations-v1` | Input permission/firmware status, immutable firmware/reset/recovery plans, verified receipts, and sanitized diagnostic log bundle |
| `provider-lifecycle-v1` | exact-release bridge install/remove and provider status/acquire/release/handoff results |
| `release-archive-v1` | deterministic macOS archive target, signature state, file modes, sizes, and SHA-256 records |

Schemas are additive within a version. A field removal, changed meaning, or
stricter accepted value creates a new registry name and `$id`. Unknown
configuration kinds and newer schema versions remain inspection-only until a
matching adapter and fixture are present.

## Agent invocation

Automation should construct an argv array and execute it without a shell. Add
`--json` before the subcommand whenever the selected command supports JSON.
Status `0` means success; statuses `1` through `8` follow
[`exit-statuses.md`](exit-statuses.md). A runtime error uses `error-v1` on
stderr. Clap syntax errors retain status `2` and their human usage text.

Mutation agents use one path only:

1. create offline candidates;
2. create and reopen a coordinated `transaction plan`;
3. call `transaction apply` with an immutable idempotency key;
4. inspect the receipt and exact provider readback;
5. call `transaction restore` with a new idempotency key when rollback is
   requested.

This protocol uses the same transaction core as the human CLI. It does not
introduce a daemon, a second device session, or a hidden mutation channel.
