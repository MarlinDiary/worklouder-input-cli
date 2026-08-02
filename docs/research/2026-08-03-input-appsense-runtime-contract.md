# Input 0.18.0 AppSense runtime contract

Date: 2026-08-03 (NZST)

## Frozen source

| Artifact | SHA-256 |
| --- | --- |
| `/Applications/input.app/Contents/Resources/app.asar` | `8e530188bc693ca1b9950bdc0515adfc349a3563e1841fe61ff2d692dc6b2da8` |
| `dist-electron/main/index.js` | `7c16191956acb8d0d89c50eb940dc9b757bf6966b19c56499ca2d60d8743154a` |
| `node_modules/@worklouder/wl-device-kit/dist/index.js` | `c0c25fce60054c8bee49e71dd31e2e38ffc2edade02a825191991bcd9f413729` |

The installed package declares Input `0.18.0` and
`@worklouder/wl-device-kit ^0.1.28`; the extracted installed dependency is the
same device-kit authority already frozen by the repository's device read
contract.

## Observed service path

The minified main chunk contains these byte offsets in the frozen file:

| Offset | Observed symbol/behavior |
| ---: | --- |
| 137701 | `nativeService.getWindowInFocus()` runs `window-info-retriever.scpt` and returns `appName` plus macOS bundle ID as `process` |
| 140809 | focus service class; tracks `getFocusApp`, `focusAppDevices`, and `lastAppInFocus` |
| 142161 | changed `appName/process/path` is assigned to `lastAppInFocus` and sent to every focus-capable device |
| 142462 | GUI autodetect starts a 10-second switch watch and 5-second stabilization watch |

Codex Micro enables the `focusedApp` device feature. The device-kit bundle's
`sendFocusApp` begins at byte 252892 and calls JSON-RPC method
`host.focused_app` (literal at byte 252978). Input polls focus every 1,000 ms.
The firmware consumes the focus payload and owns linked-app matching plus the
actual layer transition.

The renderer's status model leaves `selectedProfileIndex` unchanged but stores
`selectedLayerIndex - 1`. The companion API therefore exposes the raw device
values and documents profile as zero-based and layer as firmware one-based.

## Companion contract

`input.appsense.runtime.v1` returns, in one serialized bridge request:

- current native `focusedApp`;
- focus service `lastForwardedApp`;
- whether the collector is active;
- the sorted focus-capable device ID set and selected-device membership;
- current device status including raw selected profile/layer indexes.

The one-call integration obtains these values from Input's injected
`nativeService`, `focusAppService`, and current device manager. It does not
start a second poller, send `host.focused_app`, focus an application, or own the
layer runtime.

`worklouderctl appsense test` waits for collector health, exact
`focusedApp == lastForwardedApp`, optional identity values, and optional raw
profile/layer indexes. A mismatch timeout is a typed conflict.

## Verification boundary

The isolated Input fixture proves bridge capability discovery, strict DTO
normalization, Rust decoding, positive focus/layer expectations, and negative
timeout classification. This verifies the complete CLI/provider contract.
Released Input does not yet install the external companion socket, so a real
A/B application focus transition and physical device layer readback remain the
released-integration gate. The firmware's internal identity-match algorithm is
not claimed because it is absent from the inspected Input main and renderer
chunks.
