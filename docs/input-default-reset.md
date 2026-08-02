# Input-owned default reset

WorkLouderCTL coordinates a complete reset while Input remains authoritative
for the default configuration. The CLI does not embed a device keymap, call a
programmer, or edit Input's renderer state.

## Plan

```sh
worklouderctl --json input reset plan \
  --plan reset-plan.json \
  --candidate reset-candidate.json
```

The authenticated bridge asks Input's injected default-configuration builder
for the exact connected device/layout default. It captures the current complete
configuration separately, verifies both snapshots, and freezes:

- Input app, device-kit, firmware, and layout versions;
- connected device identity and type;
- exact source and candidate revisions;
- candidate file count; and
- a canonical plan revision.

Planning is read-only. Both output files are atomically published and reopened
before success is reported.

## Apply and verify

```sh
worklouderctl --json input reset apply \
  --plan reset-plan.json \
  --candidate reset-candidate.json \
  --backup reset-before.json \
  --receipt reset-receipt.json \
  --expected-revision SOURCE_REVISION \
  --idempotency-key reset-2026-08-02-01

worklouderctl --json backup inspect --input reset-receipt.json
```

Apply reopens and validates the plan and candidate, checks the live revision
and rechecks the exact Input version immediately before the mutation,
captures a complete backup when one does not already exist, and reuses the
existing `device.config.apply` transaction. The bridge verifies exact candidate
readback. WorkLouderCTL then atomically publishes and reopens an immutable
receipt binding the plan, candidate, backup, source, target, device, layout,
Input version, and idempotency key.

A repeated request with the same idempotency key returns the same transaction
outcome. A stale source revision or any identity/version mismatch is rejected
before mutation.

## Rollback

The reset backup is a standard complete configuration snapshot:

```sh
worklouderctl --json device --transport bridge config restore \
  --input reset-before.json \
  --backup config-after-reset.json \
  --expected-revision RESET_TARGET_REVISION \
  --idempotency-key reset-rollback-01
```

Restore uses the same authenticated CAS transaction and exact readback as every
other configuration restore. The reset receipt's backup inspection includes
the runnable restore command and marks the artifact restorable.

## Verification boundary

`./scripts/test-reset-e2e.sh` starts only an isolated synthetic Input bridge. It
proves Input-owned candidate generation, source/candidate CAS, reuse of the
configuration transaction, idempotent replay, exact rollback to the source
revision, and receipt readback. It never opens, focuses, or automates the GUI.
