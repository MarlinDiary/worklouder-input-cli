# Exit statuses and machine-readable errors

Every runtime failure has one stable process status. With `--json`, the CLI
writes a `worklouderctl-error` v1 object to standard error and leaves standard
output empty. Clap keeps status `2` for command-line usage errors.

| Status | Code | Meaning |
| ---: | --- | --- |
| 0 | success | The requested read, candidate, apply, or restore completed |
| 1 | unexpected | An error did not match a narrower public class |
| 2 | usage | Clap rejected the command line |
| 3 | provider-unavailable | A required Codex/Input bridge or connected device was unavailable |
| 4 | invalid-data | An input, snapshot, candidate, schema, path, or value was invalid |
| 5 | conflict | A revision, source hash, plan artifact, or idempotent retry drifted |
| 6 | operation-rolled-back | The requested mutation failed and every applied change was restored |
| 7 | rollback-failed | The requested mutation failed and complete recovery was not verified |
| 8 | permission-denied | macOS or filesystem permissions blocked the operation |

Example error envelope:

```json
{
  "schemaVersion": 1,
  "kind": "worklouderctl-error",
  "code": "conflict",
  "exitStatus": 5,
  "message": "live input-config revision conflicted with the coordinated plan",
  "causes": []
}
```

Automation should branch on `exitStatus` or `code`, not parse `message`.
Transaction status `6` is distinct from success: original state is verified,
but the requested target state was not retained. Status `7` requires inspection
of the written transaction receipt and its private backup catalog.
