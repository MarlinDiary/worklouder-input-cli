# Input Companion Bridge integration kit

This directory is the executable reference integration for an Input-owned
Companion Bridge. Input keeps its existing device session; `worklouderctl`
connects to the private bridge socket and never starts a competing device-kit
session.

## One-call main-process installation

Install the bridge after Electron is ready and Input's service container has
created `devicesCommManager`:

```js
import { installInputCompanionBridge } from "@worklouder/input-companion-bridge-reference";

const companionBridge = await installInputCompanionBridge({
  app,
  services: {
    devicesCommManager,
    applicationService,
    analyticsService,
    presetCatalogAuthority: {
      async listPresets() {
        // Return Input's saved-first/default-second merged preset DTO array.
      },
    },
    configurationWriter: {
      async replaceConfiguration({ device, files, operation, targetRevision }) {
        // Route the complete file set through Input's existing queue and
        // synchronize every Input-owned state authority before returning.
      },
    },
  },
  deviceKitVersion: DEVICE_KIT_VERSION,
  bridgeVersion: "0.1.0",
});
```

The integration derives discovery paths from `app.getPath("userData")`, creates
the Unix socket and token with mode `0600`, maps bridge calls to the already
connected device objects, and removes its listener and socket during Input's
normal `before-quit` lifecycle. Input's existing quit handler remains the owner
of device disconnection.

`services.devicesCommManager` must provide `getDevices()`. Each connected device
uses the same `info`, `isConnected()`, and `rpcService` object already consumed
by Input's main-process handlers. No minified class names or packaged offsets
cross this integration boundary.

`configurationWriter` is optional. When it is absent, the bridge advertises
only read, snapshot, and validation capabilities. When present, it must replace
the complete configuration file set through Input's existing serialized device
queue and finish any cache/database synchronization before returning. Only then
does the bridge advertise `device.config.apply.v1` and
`device.config.restore.v1`.

When `applicationService` provides Input's existing `getAppSettings()` and
`saveAppSettings()` methods, the integration also advertises
`input.host-settings.snapshot.v1`, `input.host-settings.apply.v1`, and
`input.host-settings.restore.v1`. It reuses the current model instance,
replaces the complete three-boolean DTO through Input, runs the existing
analytics-consent refresh when available, and verifies CAS/readback/rollback.
It does not edit Input's LokiJS file directly. A custom `hostSettingsAuthority`
with `readSettings()` and `replaceSettings()` can be injected by later Input
versions.

When `presetCatalogAuthority.listPresets()` is present, the integration
advertises `input.presets.snapshot.v1`. The provider returns Input's complete
merged preset DTO array; the adapter clones it through bounded JSON,
recursively key-sorts it for the catalog revision, and exposes only the
read-only snapshot method. WorkLouderCTL does not read or edit the LokiJS
`presets` collection directly.

When Input supplies `nativeService.getWindowInFocus()` plus its existing
`focusAppService`, the integration advertises `input.appsense.runtime.v1`.
This read-only method returns collector/registration state, the native focused
application, the last payload forwarded to firmware, and the selected device
profile/layer status. It does not focus an application or open an Input window.

`permissionsAuthority.readStatus()`, `firmwareAuthority.readStatus()`, and
`logsAuthority.readLogs()` enable the optional Tier 4 read capabilities
`input.permissions.status.v1`, `input.firmware.status.v1`, and
`input.logs.snapshot.v1`. The one-call integration derives them from
`ApplicationService.checkAppPermissions`, `DeviceFlashService`, and
`WindowService.getWindowsLogs` when those exact released services are supplied.
Logs are bounded and sanitized by the adapter before transport. Firmware status
is read-only; flashing requires a separate high-level Input-owned authority.

The reference adapter owns the surrounding transaction: validate every byte
and digest, capture a pre-mutation snapshot, compare the live revision, invoke
the writer, read back the complete revision, and automatically restore and
verify the pre-mutation snapshot after any writer or readback failure.

## Release conformance

Once the main process has installed the bridge, verify authentication,
permissions, protocol, session identity, health, and required capabilities:

```sh
node companion/conformance.mjs \
  --require device.status.v1 \
  --require device.files.list.v1 \
  --require device.files.read.v1 \
  --require device.config.apply.v1 \
  --require device.config.restore.v1 \
  --require input.host-settings.snapshot.v1 \
  --require input.host-settings.apply.v1 \
  --require input.host-settings.restore.v1 \
  --require input.presets.snapshot.v1 \
  --require input.appsense.runtime.v1 \
  --require input.permissions.status.v1 \
  --require input.firmware.status.v1 \
  --require input.logs.snapshot.v1
```

Nonstandard paths can be supplied with `--socket` and `--token`, or with
`WORKLOUDERCTL_BRIDGE_SOCKET` and `WORKLOUDERCTL_BRIDGE_TOKEN_FILE`. The command
is read-only: it performs `bridge.hello` followed by `bridge.health`.

Run the full reference suite before each Input release:

```sh
npm --prefix companion test
./scripts/test-bridge-e2e.sh
```

The first command checks server, adapter, lifecycle, authentication, and path
ownership. The second starts an isolated fixture and verifies the Node
conformance command plus the Rust CLI handshake, live status, file list, exact
export, dual-hash readback, apply, idempotent replay, stale-revision rejection,
restore, and final revision recovery. It also verifies host-settings snapshot,
offline command-permission candidate generation with analytics-field
preservation, apply/replay/readback, and restore. It also verifies preset
catalog revision, filtering, metadata, preview decode, install reference
remapping, candidate validation, apply/readback, and restore. All mutation
conformance runs against isolated fixture authorities.
