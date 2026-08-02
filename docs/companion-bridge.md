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

## Installing the exact-release overlays

For the currently frozen Codex and Input releases, the repository contains
no-restart installers and an explicit device-owner handoff:

```sh
node scripts/install-codex-live-bridge.mjs
node scripts/install-input-live-bridge.mjs
node scripts/provider-handoff.mjs status
node scripts/provider-handoff.mjs input # Input owns device configuration
node scripts/provider-handoff.mjs codex # Codex owns hardware actions
```

The two applications can stay open, but only one provider owns the physical
HID session at a time. Host-only bridge methods remain available while the
other provider owns the device. Every attach uses the loopback inspector only,
checks the exact release identity, and closes the inspector afterward. See the
[live validation record](research/2026-08-03-live-provider-integration-validation.md)
for reversible mutation and bidirectional handoff evidence.

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
- `input.host-settings.snapshot`
- `input.presets.snapshot`
- `input.appsense.runtime`
- `input.permissions.status`
- `input.firmware.status`
- `input.firmware.plan`
- `input.firmware.update`
- `input.logs.snapshot`

File content uses base64 inside JSON. Device SHA-1 and host SHA-256 remain
separate fields so a CLI export can independently verify both authorities.

`device.config.snapshot` captures the recursive file set with a list-read-list
consistency check. The response contains exact base64 bytes, each file's device
SHA-1 and host SHA-256, and a deterministic configuration revision. The
revision hashes this prefix and then every path-sorted file:

```text
"worklouder-input-config-revision-v1\0"
u32be(path byte length) || utf8(path)
u64be(content byte length) || content bytes
```

`device.config.validate` recomputes every size, digest, and revision. With an
`expectedRevision`, it also takes a fresh live snapshot and performs a read-only
compare-and-swap preflight. WorkLouderCTL exposes these methods as:

Protocol v1 bounds a snapshot at 4,096 files, 16 MiB per file, and 32 MiB of
decoded content; request and response lines are each capped at 64 MiB.

`input.host-settings.snapshot` is an independent host authority. It returns the
complete `showedAnalyticsPopUp`, `analyticsConsented`, and
`smartActionCmdEnabled` DTO plus a deterministic SHA-256 revision. It has no
device ID and does not read or write device configuration files.

`input.presets.snapshot` is a second read-only host authority. Input supplies
its saved-first/default-second merged preset DTO array; the bridge recursively
sorts object keys for a deterministic SHA-256 revision and publishes the
complete catalog without exposing the underlying LokiJS collection.

`input.appsense.runtime` is a read-only Input-owned runtime authority. It
returns the current native focused application, the last focus payload Input
forwarded to firmware, collector/device-registration state, and the device's
raw selected profile/layer status. The one-call integration delegates to
Input 0.18.0 `nativeService.getWindowInFocus`, `focusAppService`, and the
existing device session; the CLI does not synthesize focus or control a GUI.

`input.permissions.status` delegates to Input's installed permission service and
reports the exact platform meaning of its single boolean. `input.firmware.status`
delegates update compatibility and release selection to Input's installed
firmware service but never flashes. `input.firmware.plan` additionally freezes
the exact configuration revision, Input-selected release, USB readiness, and
the seven ordered renderer workflow phases. `input.firmware.update` is exposed
only by an injected high-level Input authority; it applies plan/config CAS,
idempotent serialization, a long-running timeout, exact firmware/config
postflight, and a recovery-required outcome without implementing the flasher.
`input.logs.snapshot` returns at most 5,000
newest in-memory renderer entries, with home paths, email addresses, and common
credential forms redacted before bridge transport. Each capability is advertised
only when its matching Input-owned authority is available.

`input.reset.plan` calls an injected Input-owned default-configuration builder.
It freezes the Input version, connected device identity, device-kit and firmware
versions, layout version, exact source revision, and the complete candidate
revision. The read-only plan does not change renderer or device state. Applying
the plan reuses `device.config.apply`: WorkLouderCTL captures a complete backup,
enforces source CAS, sends the exact Input-produced candidate, verifies readback,
publishes an immutable receipt, and supports exact restore through the normal
configuration transaction. The CLI rechecks the exact Input version immediately
before apply. No default keymap is duplicated in the CLI.

`input.recovery.plan` and `input.recovery.apply` appear only when Input injects
`recoveryAuthority.readStatus()` and `recoveryAuthority.recoverFirmware()`.
Planning accepts a previously captured complete configuration snapshot and
freezes its revision together with the exact Input/device-kit versions,
Input-detected bootloader identity, and Input-selected release. Apply rechecks
that plan, delegates programmer and reconnect to Input, then restores the exact
snapshot through `device.config.apply.v1`. Final firmware and configuration
revisions must match the frozen targets. The bridge does not expose a second
driver, bootloader transport, programmer, or firmware downgrade path.

```sh
worklouderctl input config snapshot --output config-snapshot.json
worklouderctl device config snapshot --output config-snapshot.json
worklouderctl device config validate --input config-snapshot.json
worklouderctl device config validate --input config-snapshot.json \
  --expected-revision REVISION
worklouderctl input permissions
worklouderctl input firmware check
worklouderctl input firmware plan --output firmware-plan.json
worklouderctl input firmware update --plan firmware-plan.json \
  --backup firmware-config-before.json --receipt firmware-update.json \
  --expected-revision CONFIG_REVISION --idempotency-key UPDATE_KEY
worklouderctl input reset plan --plan reset-plan.json \
  --candidate reset-candidate.json
worklouderctl input reset apply --plan reset-plan.json \
  --candidate reset-candidate.json --backup reset-before.json \
  --receipt reset-receipt.json --expected-revision CONFIG_REVISION \
  --idempotency-key RESET_KEY
worklouderctl input recovery plan --backup config-before-recovery.json \
  --plan recovery-plan.json
worklouderctl input recovery apply --plan recovery-plan.json \
  --backup config-before-recovery.json --receipt recovery-receipt.json \
  --idempotency-key RECOVERY_KEY
worklouderctl input logs collect --output input-log-bundle
```

The first command reads Input's current cache with no bridge/device transport
and emits the standard semantic snapshot. The live bridge command may add
device/runtime metadata; for identical cached/device bytes, both share the same
`deviceId`, files, hashes, and revision. A later bridge apply still performs a
fresh CAS check against the live device revision.

## Offline semantic candidates

WorkLouderCTL can turn a valid complete snapshot into a complete profile/layer
candidate without opening a bridge connection:

```sh
worklouderctl profile list --input config-snapshot.json
worklouderctl profile show --input config-snapshot.json --id 0
worklouderctl profile create --input config-snapshot.json \
  --name Work --output profile-create-candidate.json
worklouderctl profile duplicate --input config-snapshot.json \
  --id 0 --name 'Work Copy' --output profile-copy-candidate.json
worklouderctl profile rename --input config-snapshot.json \
  --id 0 --name Work --output candidate.json
worklouderctl profile select --input config-snapshot.json \
  --id 7 --output profile-select-candidate.json
worklouderctl layer show --input config-snapshot.json --profile 0 --id 1
worklouderctl layer create --input config-snapshot.json \
  --profile 0 --name Build --output layer-create-candidate.json
worklouderctl layer duplicate --input config-snapshot.json \
  --profile 0 --id 1 --name 'Build Copy' --output layer-copy-candidate.json
worklouderctl layer rename --input config-snapshot.json \
  --profile 0 --id 1 --name Build --output candidate.json
worklouderctl layer color --input config-snapshot.json \
  --profile 0 --id 1 --color '#EDF6FF' --output color-candidate.json
worklouderctl layer lighting set --input config-snapshot.json \
  --profile 0 --id 1 --zone backlight --effect breath --brightness 0.5 \
  --color '#EDF6FF' --apply-to-all --output lighting-candidate.json
worklouderctl appsense list --input config-snapshot.json
worklouderctl appsense show --input config-snapshot.json --id 0
worklouderctl appsense link --input config-snapshot.json \
  --profile 0 --layer 1 --name 'Codex-mac' --process com.openai.codex \
  --output appsense-candidate.json
worklouderctl appsense set --input appsense-candidate.json --id 0 \
  --name 'Codex Desktop' --output appsense-renamed-candidate.json
worklouderctl appsense unlink --input appsense-candidate.json \
  --profile 0 --layer 1 --output appsense-unlinked-candidate.json
worklouderctl appsense test --expected-process com.openai.codex \
  --expected-profile-index 0 --expected-layer-index 2 --timeout-ms 5000
worklouderctl control list --input config-snapshot.json --profile 0 --layer 1
worklouderctl control show --input config-snapshot.json --profile 0 --layer 1 \
  --control encoder:0:press
worklouderctl control set --input config-snapshot.json --profile 0 --layer 1 \
  --control encoder:0:press --assignment KC_MUTE --output control-candidate.json
worklouderctl action list --input config-snapshot.json
worklouderctl action show --input config-snapshot.json --id 3
worklouderctl action event set --input config-snapshot.json --id 3 --index 0 \
  --assignment KC_X --type click --delay 200 --output action-candidate.json
worklouderctl action delete --input config-snapshot.json --id 3 \
  --output action-delete-candidate.json
worklouderctl action group create --input config-snapshot.json --name Shortcuts \
  --action 3 --action 4 --output action-group-candidate.json
worklouderctl multi-action show --input config-snapshot.json --id 1
worklouderctl multi-action set --input config-snapshot.json --id 1 \
  --tap KC_A --double-tap KC_B --hold KC_C --tap-hold KC_D \
  --output multi-action-candidate.json
worklouderctl multi-action group create --input config-snapshot.json --name Gestures \
  --multi-action 1 --output multi-action-group-candidate.json
worklouderctl smart-action create --input config-snapshot.json --name 'Insert text' \
  --type text --text 'hello' --color '#EDF6FF' --output smart-text-candidate.json
worklouderctl smart-action create --input smart-text-candidate.json --name 'Open URL' \
  --type url --url 'https://example.com' --output smart-url-candidate.json
worklouderctl smart-action group create --input smart-url-candidate.json --name Launchers \
  --smart-action 1 --smart-action 2 --output smart-group-candidate.json
worklouderctl control set --input smart-group-candidate.json --profile 0 --layer 1 \
  --control key:0:0 --assignment SA_1 --output smart-bound-candidate.json
worklouderctl cheat-sheet bindings --input smart-bound-candidate.json \
  --profile 0 --layer 1
worklouderctl cheat-sheet bind --input smart-bound-candidate.json \
  --profile 0 --layer 1 --control encoder:0:press toggle \
  --output cheat-sheet-candidate.json
worklouderctl input preset snapshot --output preset-catalog.json
worklouderctl preset list --catalog preset-catalog.json --device codex_micro \
  --layout universal --os mac
worklouderctl preset preview --catalog preset-catalog.json --id 9002 \
  --output preset-preview.png
worklouderctl preset install --input config-snapshot.json \
  --catalog preset-catalog.json --id 9002 --profile 0 \
  --output preset-candidate.json
```

Before publishing, the editor independently verifies every file's canonical
base64, size, SHA-1, SHA-256, safe unique path, and the path-framed full
revision. It preserves unknown envelope/record/keymap fields and the exact
bytes of files outside `keymap.json`. The resulting candidate can then be sent
to `device config apply`; candidate generation itself never calls Input.

## Mutation methods

Configuration apply and restore are capability-gated:

- `device.config.apply`
- `device.config.restore`
- `input.host-settings.apply`
- `input.host-settings.restore`

Every mutation carries an expected revision, unique idempotency key, and
complete typed input; device configuration mutations also carry a device ID.
Input serializes mutations in its main process. It snapshots before apply,
checks the revision immediately before the first write, reads back after the
write, and restores the snapshot if any authority fails to synchronize.

This makes the GUI and CLI two clients of the same transaction owner rather
than two independent writers.

The CLI always captures or reopens an immutable pre-mutation backup before it
sends a mutation. Repeating the exact command with the same backup and
idempotency key returns the cached Input-session result without a second write.
Reusing the key with a different operation, device, expected revision, or
target revision is rejected.

```sh
worklouderctl device config apply \
  --input candidate.json --backup pre-apply.json \
  --expected-revision REVISION --idempotency-key RETRY_KEY

worklouderctl device config restore \
  --input original.json --backup pre-restore.json \
  --expected-revision CURRENT_REVISION --idempotency-key RESTORE_KEY

worklouderctl input permission command apply \
  --input host-settings-enabled.json --backup host-settings-before.json \
  --expected-revision REVISION --idempotency-key HOST_RETRY_KEY

worklouderctl input permission command restore \
  --input host-settings-original.json --backup host-settings-current.json \
  --expected-revision CURRENT_REVISION --idempotency-key HOST_RESTORE_KEY
```

Input advertises these methods only when its version adapter injects a
`configurationWriter.replaceConfiguration` implementation. The writer must
replace the complete file set through Input's own serialized queue and finish
its state synchronization before returning. The bridge then performs a fresh
full snapshot and revision readback. A writer/readback failure triggers a full
pre-mutation restore plus another revision readback.

Host-setting methods are advertised separately when Input injects its
application-settings authority. They replace the complete three-boolean DTO
through `ApplicationService`, including the existing analytics-consent refresh,
and use an independent revision/idempotency domain.

The reference writer and cross-language fixture verify this transaction model.
Enabling it for an installed Input release remains a separate version-adapter
and real-device rollback milestone.

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

WorkLouderCTL maintains the executable reference pieces in this repository:

- `companion/input-main-bridge.mjs` — authenticated, allowlisted, serialized
  Unix-socket JSON-RPC server;
- `companion/input-main-adapter.mjs` — adapter over Input's existing
  `devicesCommManager`, per-device `rpcService`, and application-settings
  and preset-catalog authorities;
- `companion/input-main-integration.mjs` — one-call Electron main-process
  installation with Input-owned discovery and lifecycle cleanup;
- `companion/conformance.mjs` — read-only release conformance command;
- `companion/input-main-bridge.test.mjs` — authentication, dispatch, and service
  adapter tests;
- `scripts/test-bridge-e2e.sh` — Rust CLI handshake, status, file list, exact
  export, semantic profile/layer/lighting/AppSense/control/Action/Multi Action/group
  inspection, lifecycle/CRUD/cascade candidates, independent candidate rehash,
  apply/readback, idempotent retry, stale-CAS rejection, restore, host command
  permission `false -> true -> false`, preset catalog/list/show/preview/install,
  preset apply/readback/restore, and dual-hash conformance test.
- `scripts/test-recovery-e2e.sh` — backup-bound bootloader plan, delegated
  programmer/reconnect, exact configuration restore, receipt inspection, and
  idempotent replay fixture.

Input maintains only the small adapter from stable bridge method names to its
current service container. Integration creates the adapter after Input's
service container is initialized, starts the bridge under `app.getPath("userData")`,
and stops it from Input's existing quit cleanup. Input 0.18.0 is a static
adapter boundary; the installed release itself remains unchanged.

The exact one-call integration and release-check commands are documented in
[`companion/README.md`](../companion/README.md).
