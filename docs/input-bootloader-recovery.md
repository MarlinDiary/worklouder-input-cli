# Input-owned bootloader recovery

`worklouderctl input recovery` restores a Codex Micro that Input has detected in
bootloader mode. WorkLouderCTL does not implement a driver, bootloader
transport, release parser, or device programmer. The installed Input process
supplies one high-level `recoveryAuthority`; the CLI adds an immutable plan,
complete configuration backup binding, authenticated delegation, exact
postflight, receipt inspection, and retry semantics around it.

## Preserve configuration before recovery

Capture a complete snapshot while the normal device is still readable:

```sh
worklouderctl --json device --transport bridge config snapshot \
  --output configuration-before-recovery.json
```

Keep this file if a firmware operation reports `recoveryRequired`. A device
already exposed only as a bootloader may no longer provide the ordinary file
RPC needed to create a new complete snapshot.

## Freeze and apply recovery

```sh
worklouderctl --json input recovery plan \
  --backup configuration-before-recovery.json \
  --plan recovery-plan.json

worklouderctl --json input recovery apply \
  --plan recovery-plan.json \
  --backup configuration-before-recovery.json \
  --receipt recovery-receipt.json \
  --idempotency-key RECOVERY_KEY
```

The plan binds all of the following:

- exact Input app and device-kit versions;
- original device ID, model, layout, connection identity, and prior firmware;
- Input-detected bootloader transport and identifier;
- Input-selected compatible firmware release;
- complete backup revision and file count;
- the six ordered recovery phases.

Planning is read-only. `ready` is false when Input cannot currently identify the
bootloader or select a release. Apply reconnects to the bridge and rejects Input
version drift, plan edits, backup edits, a reused idempotency key with different
content, and a changed bootloader/release plan before invoking the provider.

## Authority and verification

Input owns the first four phases:

1. `detect-input-bootloader`
2. `validate-input-selected-release`
3. `recover-with-input-device-programmer`
4. `reconnect-original-device`

After the normal device returns, WorkLouderCTL uses the existing complete
configuration transaction for:

5. `restore-exact-configuration`
6. `verify-firmware-and-configuration`

Success requires the recovered firmware to equal the frozen target and the
post-restore configuration revision to equal the original backup revision. An
ambiguous provider error may be accepted only when postflight independently
confirms the target firmware; configuration restore and exact final readback
still have to pass. Otherwise the bridge returns a typed recovery-required
failure and does not claim completion.

The success receipt is written atomically, reopened, and verified against its
plan and backup. Inspect either artifact with:

```sh
worklouderctl --json backup inspect --input recovery-plan.json
worklouderctl --json backup inspect --input recovery-receipt.json
```

## Rollback boundary

Recovery does not perform an automatic firmware downgrade. The preserved
configuration remains independently restorable through the normal transaction:

```sh
worklouderctl --json device --transport bridge config restore \
  --input configuration-before-recovery.json \
  --backup configuration-before-rollback.json \
  --expected-revision CURRENT_REVISION \
  --idempotency-key ROLLBACK_KEY
```

`./scripts/test-recovery-e2e.sh` proves plan/backup binding, Input programmer
delegation, post-reconnect exact configuration restore, same-key replay,
receipt inspection, and final firmware/config readback against an isolated
synthetic bridge fixture. It does not open or control the Input GUI.
