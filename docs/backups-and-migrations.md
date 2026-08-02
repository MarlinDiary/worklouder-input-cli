# Backup inspection and migration

`worklouderctl backup` reopens supported artifacts through the same strict
readers used by apply and restore. It does not infer validity from a filename
or from JSON syntax alone.

```sh
worklouderctl --json backup inspect --input ARTIFACT
worklouderctl --json backup migration-plan --input ARTIFACT
```

Supported inputs are:

- coordinated transaction plans, receipts, and private backup catalogs;
- a complete transaction backup directory containing `catalog.json`;
- Codex settings and Codex Agent Key snapshots;
- Input configuration and host-settings snapshots;
- Input preset catalogs;
- Input firmware/reset/recovery plans and verified mutation receipts;
- Input and device export bundles; and
- private sanitized Input log bundles.

For a transaction receipt, inspection validates the receipt, copied plan,
private catalog, artifact paths, revisions, authority entries, file modes, and
every recorded baseline/candidate digest. A successful apply receipt reports a
restore command template. A restore receipt or already rolled-back apply does
not advertise another restore.

`migration-plan` first performs the same complete verification. All published
artifact formats are currently schema version 1, so the result is
`migrationRequired=false`, `supported=true`, and `action=none`. A future schema
will add an explicit version-pair migration; unknown kinds or versions stop
with typed `invalid-data` rather than copying or partially rewriting a backup.

## Storage and process boundary

Input cache/database synchronization is owned by the Input bridge's injected
`configurationWriter` and `ApplicationService` adapters. WorkLouderCTL never
edits `input_storage.json` or a LokiJS collection directly. Mutations run
inside Input's serialized bridge and existing device session; the separate
direct device-kit transport remains read-only and requires Input to be closed
or explicitly restarted. This lets Input releases retain driver, database,
queue, and runtime ownership while the CLI retains plan, CAS, verification,
and rollback semantics.
