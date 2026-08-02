# Agent protocol

WorkLouderCTL accepts a versioned JSON command envelope and executes it without
a shell. The command is parsed by the same Clap model and dispatched to the
same functions used by the human CLI, including the coordinated transaction
engine.

```json
{
  "schemaVersion": 1,
  "argv": ["worklouderctl", "backup", "inspect", "--input", "BACKUP"],
  "output": "json",
  "expectedExitStatuses": [0]
}
```

```sh
worklouderctl --json agent validate --input command.json
worklouderctl --json agent execute --input command.json
```

`validate` checks the exact envelope shape, rejects empty/NUL arguments,
deduplicates the accepted status contract, and inserts `--json` directly into
the argv array when JSON output was requested. It performs no command action.

`execute` parses that normalized argv in-process. It never invokes a shell and
rejects recursive `agent` envelopes. The outer process emits one
`worklouderctl-agent-execution` JSON result:

- `exitStatus` is the inner command's typed status;
- `success` means that status is zero;
- `accepted` means the status appeared in `expectedExitStatuses`;
- `stdout` contains parsed JSON or a text string; and
- `error` contains the typed runtime error, or a usage error for invalid argv.

The outer agent protocol call completes after writing this report, including
when the inner status was an expected error. Consumers decide success from
`accepted` and `exitStatus`, not only the wrapper process status.

The input and result schemas are embedded as `command-envelope-v1` and
`agent-execution-v1`. Mutation envelopes must still create offline candidates,
plan, apply with an idempotency key, inspect exact receipt/readback, and restore
through the normal transaction commands. There is no alternate mutation path.
