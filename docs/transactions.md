# Cross-authority transactions

`worklouderctl transaction` coordinates one immutable change across the four
configuration authorities used by Codex Micro:

| Apply order | Authority | Tier | Runtime owner |
| ---: | --- | ---: | --- |
| 1 | Input host settings | 3 | Input |
| 2 | Device configuration | 2 | Current Codex or Input owner |
| 3 | Codex Agent Keys | 1 | Codex |
| 4 | Codex settings | 1 | Codex |

Restore uses the reverse order. WorkLouderCTL delegates every write to the
installed app's authenticated companion bridge; it does not implement a driver
or a second device runtime. `--device-owner auto` is the default and preserves
the exclusive current owner. `--device-owner codex|input` selects an explicit
route but still does not perform a handoff.

## 1. Create and inspect a plan

Capture each baseline from its live authority, produce strict offline
candidates, then bind any one to four complete baseline/candidate pairs into a
canonical plan:

```console
worklouderctl --json transaction plan \
  --codex-settings-base codex-base.json \
  --codex-settings-candidate codex-candidate.json \
  --codex-agent-keys-base agent-keys-base.json \
  --codex-agent-keys-candidate agent-keys-candidate.json \
  --input-config-base input-base.json \
  --input-config-candidate input-candidate.json \
  --input-host-settings-base host-base.json \
  --input-host-settings-candidate host-candidate.json \
  --output plan.json

worklouderctl --json transaction show --input plan.json
```

The plan contains each authority's baseline and target revision, content
SHA-256 values, routing metadata, and structural JSON changes. Plan creation and
apply reject incomplete pairs, unknown fields, duplicate authorities, changed
artifacts, stale live revisions, and mismatched Codex source bytes.

## 2. Apply with one rollback boundary

```console
worklouderctl --json transaction apply \
  --device-owner auto \
  --plan plan.json \
  --backup-dir apply-backup \
  --receipt apply-receipt.json \
  --idempotency-key configure-workstation-1
```

Before the first write, the CLI snapshots every live authority into a staging
directory, verifies every planned CAS value, copies every candidate and the
plan, and atomically publishes a mode-`0700` catalog whose files are mode
`0600`. The catalog is self-contained: restore still works after the original
plan and its input artifacts are removed.

Each provider performs its own apply and exact readback. The coordinator then
runs a second all-authority postflight. If a later provider fails, earlier
writes are restored in reverse order. A failure still writes a receipt with
`status: rolled-back` or `status: rollback-failed` before the command exits with
typed status `6` or `7`. See [exit statuses](exit-statuses.md).

For Codex-owned device configuration, one changed device file uses the existing
provider write. A change spanning multiple files requires the firmware's
multi-file transaction primitive; if unavailable, the operation stops before
the first file write. Persistent idempotency binds each key to the exact
operation, baseline revision, and target revision.

An exact idempotent retry returns the existing receipt only after revalidating
the private catalog and current live state. State drift makes the retry fail
instead of reporting stale success.

Pass explicit bridge paths when not using their default locations:

```console
worklouderctl --json transaction apply \
  --plan plan.json --backup-dir apply-backup \
  --receipt apply-receipt.json --idempotency-key configure-workstation-1 \
  --codex-socket CODEX_SOCKET --codex-token CODEX_TOKEN \
  --input-socket INPUT_SOCKET --input-token INPUT_TOKEN
```

## 3. Restore

```console
worklouderctl --json transaction restore \
  --device-owner auto \
  --apply-receipt apply-receipt.json \
  --backup-dir restore-backup \
  --receipt restore-receipt.json \
  --idempotency-key configure-workstation-1-restore
```

Restore first verifies the successful apply receipt, copied plan, catalog
hashes, private paths, and every live target revision. It captures a second
private recovery catalog before restoring. A failed restore rolls already
restored authorities forward to their applied state; successful restore and
idempotent retry both require exact all-authority postflight readback.

Keep the apply receipt and the complete `apply-backup` directory together. Do
not edit catalog JSON or artifacts: hashes, containment, file types, and private
permissions are validated on reopen.

## Verification fixtures

```console
./scripts/test-transaction-e2e.sh
./scripts/test-transaction-rollback-e2e.sh
```

The first fixture proves four-authority plan/apply/readback/restore, retry,
drift rejection, private catalog permissions, and reverse rollback after an
injected fourth-provider failure. The second injects a post-write Input provider
failure and proves provider-local rollback plus coordinator rollback of the
already-applied host authority. Both run against isolated fixture processes and
do not open or control GUI windows.
