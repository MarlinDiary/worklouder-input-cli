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

## Release conformance

Once the main process has installed the bridge, verify authentication,
permissions, protocol, session identity, health, and required capabilities:

```sh
node companion/conformance.mjs \
  --require device.status.v1 \
  --require device.files.list.v1 \
  --require device.files.read.v1
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
export, and dual-hash readback.
