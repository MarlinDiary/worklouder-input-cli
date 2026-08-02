# Delegated firmware update

`worklouderctl input firmware update` coordinates a complete update without
reimplementing Work Louder's driver, release selector, bootloader transport, or
device programmer. Those operations remain one high-level Input-owned
`firmwareOperationsAuthority.updateFirmware` call.

## Frozen workflow

```sh
worklouderctl --json input firmware plan \
  --device DEVICE_ID --output firmware-plan.json

worklouderctl --json input firmware update \
  --plan firmware-plan.json \
  --backup firmware-config-backup.json \
  --receipt firmware-update-receipt.json \
  --expected-revision CONFIG_REVISION \
  --idempotency-key UPDATE_KEY
```

The plan binds the selected release, device identity, USB state, complete live
configuration revision, and seven ordered phases. The update command rejects a
non-ready or modified plan, snapshots the complete configuration before calling
Input, and requires that the backup, plan, and live CAS revision agree.

The injected Input authority owns these phases:

1. `backup-configuration`
2. `download-input-selected-release`
3. `enter-bootloader`
4. `flash-with-input-device-programmer`
5. `reconnect-original-device`
6. `restore-changed-configuration`
7. `verify-firmware-and-configuration`

The bridge serializes the mutation, authenticates it, supports idempotent replay,
and observes firmware plus configuration again after Input returns. A provider
error is treated as ambiguous: matching postflight may confirm completion;
otherwise the response is a typed `recoveryRequired` failure. The CLI publishes
its success receipt atomically, reopens it, and validates it against both the
immutable plan and complete backup.

`./scripts/test-firmware-update-e2e.sh` exercises USB readiness, plan/live CAS,
complete backup, Input delegation, update postflight, immutable receipt
inspection, and same-key replay against an isolated synthetic bridge fixture. It
does not open or control the Input GUI.
