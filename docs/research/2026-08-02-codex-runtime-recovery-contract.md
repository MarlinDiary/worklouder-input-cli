# Codex Micro runtime recovery contract (Codex 26.727.51351)

This contract covers a released Codex failure mode where the device remains
enumerated and the settings remain valid, but the Codex Micro service stops
delivering every Tier 1 button event.

## Observed failed state

The installed service was captured in this state after a concurrent HID write
failure:

- `deviceState.status = detected`
- `comm != null` and `api != null`
- `connectPromise != null`
- `topologyReconciliationPromise != null`
- `unsubscribeHid = null`
- `unsubscribeJoystick = null`

The log boundary was equally explicit: `v.oai.hid` and `v.oai.rad` handlers were
removed after `WRITE_FAILED`, a reconnect began, but the two handlers and the
minute `device.status` read never returned. The device-side `keymap.json` SHA-1
still equaled the Input cache SHA-1, so rewriting the keymap was not indicated.

## Recovery boundary

The supported recovery is a strict, version-pinned companion operation:

1. require the exact installed app version and `app.asar` hash;
2. attach to the Codex main process on loopback through the Node inspector;
3. read the one live `CodexMicroService` instance;
4. pause the Input process without closing its window or changing its files;
5. invalidate the stale service attempt, clear only its settled-never Promise
   references, and call the released service's own `stop()`/`start()` path;
6. require `connected`, live comm/API, settled connect/topology Promises, and
   restored HID/joystick subscriptions;
7. resume Input, re-read the same healthy state, and close the inspector only
   when the CLI opened it.

No keymap, Codex config, Input cache, firmware, driver, or application bundle is
written. A changed Codex bundle requires a new frozen contract and new evidence.
