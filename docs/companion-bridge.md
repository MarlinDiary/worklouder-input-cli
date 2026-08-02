# Input Companion Bridge

The Input Companion Bridge is the primary long-term transport for
WorkLouderCTL. Input keeps the only live device session. The CLI sends bounded,
versioned requests to Input instead of loading a second copy of the device kit.

```text
worklouderctl
    │ JSON-RPC 2.0 over a private Unix socket
    ▼
Input main process
    │ existing services and device queue
    ▼
Codex Micro
```

The frozen machine-readable contract is
[`spec/input-companion-bridge-v1.json`](../spec/input-companion-bridge-v1.json).

## Why this is the primary design

- Input remains the authority for HID/BLE discovery, firmware compatibility,
  RPC ordering, cached configuration, and host actions.
- Input and the CLI never race on the device JSON-RPC stream.
- GUI and CLI actions share Input's existing queue and state model.
- WorkLouderCTL does not ship a replacement driver or copied device kit.
- Input updates can add capabilities without changing protocol version 1.
- The CLI negotiates capabilities rather than depending on minified class names
  or fixed `app.asar` offsets.

The bundled-device-kit reader remains a compatibility and recovery path while
released Input builds do not contain the bridge. It is not the target write
architecture.

## Input 0.18.0 evidence

Static inspection of the installed Input 0.18.0 main process found existing
internal handlers for device discovery, status, file list/read, and config
apply. Input also disconnects every device from its `before-quit` handler.

Those handlers are Electron `ipcMain` routes. Input 0.18.0 does not publish an
external Unix socket, local HTTP listener, or command-bearing second-instance
route. An external CLI therefore needs a small bridge registered inside the
Input main process to reuse the already connected session.

## Discovery and authentication

On macOS, Input owns both files under its existing user-data directory:

```text
~/Library/Application Support/input/worklouderctl-bridge-v1.sock
~/Library/Application Support/input/worklouderctl-bridge-v1.token
```

The socket and token are user-only (`0600`). The token contains at least 32
random bytes and is presented only in the first `bridge.hello` request. The
server authenticates before dispatching any other method. It never listens on
TCP, evaluates shell text, or accepts arbitrary Electron channel names.

Tests and nonstandard installations can override discovery with
`WORKLOUDERCTL_BRIDGE_SOCKET` and `WORKLOUDERCTL_BRIDGE_TOKEN_FILE`.

## Handshake

Each connection begins with one JSON-RPC request:

```json
{"jsonrpc":"2.0","id":"1","method":"bridge.hello","params":{"protocolVersion":1,"token":"TOKEN","client":{"name":"worklouderctl","version":"0.1.0"}}}
```

Input returns its bridge version, Input version, session ID, and exact
capability list. Every later request is rejected unless its capability was
advertised.

## Read methods

Protocol version 1 defines:

- `bridge.health`
- `device.list`
- `device.status`
- `device.files.list`
- `device.files.read`
- `device.config.snapshot`
- `device.config.validate`

File content uses base64 inside JSON. Device SHA-1 and host SHA-256 remain
separate fields so a CLI export can independently verify both authorities.

## Mutation methods

Configuration apply and restore are capability-gated:

- `device.config.apply`
- `device.config.restore`

Every mutation carries a device ID, expected revision, unique idempotency key,
and complete typed input. Input serializes mutations on its existing device
queue. It snapshots before apply, checks the revision immediately before the
first write, reads back after the write, and restores the snapshot if any
authority fails to synchronize.

This makes the GUI and CLI two clients of the same transaction owner rather
than two independent writers.

## Update compatibility

Protocol and application versions are independent:

1. Input ships bridge protocol version 1.
2. New Input releases preserve version 1 methods and add named capabilities.
3. Breaking wire changes use a new protocol version and a new socket name.
4. WorkLouderCTL selects behavior from the handshake capability list.
5. Unknown response fields are preserved or ignored according to the method
   schema; missing required capabilities stop the requested operation.

An Input release can change its internal services, device kit, or GUI without
changing the CLI as long as its bridge adapter continues to satisfy the stable
contract.

## Integration boundary

The bridge belongs in Input's main process immediately after its service
container is initialized. Its adapter calls the same service objects used by
the existing Electron handlers; it does not import private minified symbols
from the packaged application.

WorkLouderCTL will maintain a reference server and conformance suite. Input
maintains only the small adapter from stable bridge method names to its current
service container.
