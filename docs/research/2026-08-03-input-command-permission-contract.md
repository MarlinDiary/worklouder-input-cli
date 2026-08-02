# Input command permission contract (Input 0.18.0)

Input 0.18.0 stores the host command gate in the LokiJS database
`input_storage.json`, in the first record of the `app_settings` collection as
the boolean `smartActionCmdEnabled`. This is separate from the device-owned
`smart_actions.json` command definition.

The exact installed main bundle exposes `common-get-app-settings` and
`common-send-app-settings`. Both the Smart Action command editor and the
Settings surface read the field from the app-settings DTO, mutate only that
field, and submit the complete DTO. The storage service uses `findOne`, merges
the DTO with `Object.assign`, updates the record, and saves the Loki database.
New app settings default the permission to `false`.

The main-process handler for `kb.sa.exec` checks the same field before reading
the command payload. A false value returns immediately; a true value keeps
execution under Input's existing host runtime.

WorkLouderCTL therefore treats this field as Tier 3 host configuration. The
CLI reads a frozen `input_storage.json` file and emits a strict offline
candidate while preserving every unrelated collection, record, field, and
Loki metadata value. It does not edit the running database. Applying the
candidate remains an Input Companion Bridge app-settings transaction so Input
retains database/runtime authority.

Frozen source hashes and the exact storage, renderer, and runtime rules are in
[`spec/input-command-permission-0.18.0.json`](../../spec/input-command-permission-0.18.0.json).
