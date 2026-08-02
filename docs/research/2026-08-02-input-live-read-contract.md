# Input live device read contract (0.18.0)

This note freezes the provider boundary for the first live Codex Micro device
reader. The machine-readable contract is
[`spec/input-device-read-0.18.0.json`](../../spec/input-device-read-0.18.0.json).

## Bound provider

- Input bundle: `/Applications/input.app`
- Input version: `0.18.0`
- Input ASAR SHA-256:
  `8e530188bc693ca1b9950bdc0515adfc349a3563e1841fe61ff2d692dc6b2da8`
- Bundled `@worklouder/wl-device-kit`: `0.1.29`
- Device-kit entry SHA-256:
  `c0c25fce60054c8bee49e71dd31e2e38ffc2edade02a825191991bcd9f413729`

The CLI uses Input's own Electron runtime and bundled device kit. It does not
link a second HID implementation into the Rust binary. The adapter launches
Input's executable with `ELECTRON_RUN_AS_NODE=1`, loads the package through the
ASAR resolver, and calls its public `WLDeviceDiscovery`, `WLDeviceCommImpl`, and
`WLRPCApi` classes.

## Frozen read surface

| Operation | Device-kit method | Firmware RPC |
| --- | --- | --- |
| Firmware | `getFirmwareVersion` | `sys.version` |
| Status | `getDeviceStatus` | `device.status` |
| Files | `getFileList` | `fs.list` |
| File bytes | `readFileChunked` | `fs.readbin` |

Codex Micro discovery is restricted to device type `codex_micro`, Work Louder
USB vendor ID `12346`, product ID `33632`, and usage page `65280`. The provider
requires exactly one matching device.

## Process coordination

Input and a headless device-kit process consume the same JSON-RPC response
stream. Overlapping calls previously reproduced `InvalidInput`. The CLI
therefore defaults to `require-closed`. An explicit `restart` mode asks Input
to quit normally, waits for exit, performs the bounded read, and reopens Input
afterward. It never force-terminates the application.

## Live read evidence

The 0.18.0 provider successfully returned firmware/status and listed the live
device files. Before and after the probe, SHA-256 values for Input's cached
`keymap.json`, `smart_actions.json`, and `input_storage.json` were identical.
The Input application was reopened after the probe.

The public fixture records schemas and hashes only. User key assignments,
profile names, battery state, and device file contents are excluded.

## Export rules

1. Accept only safe relative device paths.
2. Read each listed file with the kit's 3,072-byte chunked reader.
3. Compare listed size and device SHA-1 with the read bytes.
4. Record a host SHA-256 for every exported file.
5. Publish a typed manifest and files through one atomic directory rename.
6. Reopen the manifest and every exported file before reporting success.
