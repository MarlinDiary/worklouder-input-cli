# Input operational read contract (Input 0.18.0)

This note freezes the first non-mutating Tier 4 surface recovered from the exact
installed Input 0.18.0 build. Inspection and all fixture verification were done
without opening, focusing, or closing Input windows.

## Permission authority

Input's `common-check-app-permissions` handler calls
`ApplicationService.checkAppPermissions(devicePaths)`, which constructs the
installed device kit's `WLPermissions`. On macOS that class calls the bundled
native addon's `hasInputMonitoringPermission()` and returns one boolean. No
separate Accessibility check exists in this released path. On Linux the same
class checks read/write access for each supplied HID node; other platforms
return true. `input permissions` reports this exact platform-dependent meaning
instead of inferring broader macOS consent state.

## Firmware read authority

The connected-device manager obtains `sys.version`, then
`DeviceFlashService.checkForFwUpdates(currentVersion, deviceType)`. When an
update exists, `getLatestFwRelease` returns Input's selected `.bin` release with
`version`, `fetchedAt`, `changeLog`, and `downloadUrl`. `input firmware check`
uses only those Input-owned services and the existing connected session. The
CLI neither downloads nor parses release feeds itself.

The renderer's complete update sequence is separately recorded in
`spec/input-operations-0.18.0.json`: device-file backup, Input-selected download,
bootloader transition/discovery, Input/WLDeviceProgrammer flash, original PID
rediscovery, backup restore, and post-state readback. Input 0.18.0 does not
expose this sequence as one main-process method. A future mutation bridge must
therefore receive one injected high-level Input-owned authority; the CLI will
not duplicate the programmer or transport.

## Diagnostic log authority

Input's Help menu reads the `WindowService.getWindowsLogs()` in-memory ring,
which is capped at 5,000 renderer entries. The bridge snapshots a caller-bounded
suffix and redacts user-home prefixes, email addresses, and common credential/device-identifier
forms before any response leaves Input. `input logs collect` independently
validates the sanitized DTO and atomically publishes a private `0700` bundle
containing `0600` JSON/text files plus a checksum manifest; every file and the
manifest are reopened after publication.

## Reset boundary

The Setup “Reset settings” control resets renderer atoms and enters the default
layout flow. Static inspection found no standalone Input main-process reset
service. It is not represented as a generic device erase call. A later reset
mutation must be expressed as a complete configuration candidate and use the
existing snapshot/CAS/readback/rollback transaction, or be supplied by a new
Input-owned high-level authority.
