# Compatibility and support policy

WorkLouderCTL uses evidence-based compatibility claims. A version is listed as
supported only after fixtures, automated checks, device readback, and rollback
verification cover the relevant operations.

## Current project stage

There is no released binary yet. The versions below are research baselines,
not a public support guarantee.

| Component | Research baseline | Current claim |
| --- | ---: | --- |
| Device | Work Louder Codex Micro | Initial target |
| Host OS | macOS | Initial target |
| Codex app | 26.727.51351 | Exact Tier 1 settings read/offline candidate adapter and service-only runtime recovery contract verified; external settings bridge integration pending |
| Codex Companion Bridge | Protocol v1 | Settings and six-slot Agent Key snapshot/CAS/apply/restore/rollback fixture verified |
| Work Louder Input | 0.17.3 | Earlier schema fixture |
| Work Louder Input | 0.18.0 | Exact bundled-kit live read adapter verified |
| Input Companion Bridge | Protocol v1 | Snapshot/CAS/apply/replay/restore/rollback fixture conformance verified; released Input writer pending |
| Codex Micro firmware | v0.6.0 | Live status/files/export read boundary verified |
| USB | Observed | Release verification pending |
| Bluetooth | Observed | Live read boundary verified; mutation pending |

The 2026-08-02 audit recorded Input updating from 0.17.3 to 0.18.0 during a
live session. Adapter selection therefore re-reads the installed and running
version immediately before planning and immediately before writing.

For Codex 26.727.51351, `codex-config-toml-read-v1` has live read-only evidence:
the five explicit Codex Micro settings validated, inherited layout defaults
were reconstructed, an atomic typed snapshot was reopened, and the source
`config.toml` SHA-256 was identical before and after. This claim does not cover
Codex settings mutation. Static inspection found the internal `settings-read`,
`settings-write`, `get-global-state`, and `set-global-state` handlers, while the
released build did not expose an external listener.

The same exact Codex build has a separate runtime-health boundary. A captured
`WRITE_FAILED` transition left the device enumerated and settings intact while
the service retained stale connection/topology Promises and lost both
`v.oai.hid` and `v.oai.rad` subscriptions. A service-only restart performed
while Input was paused restored `connected`, battery readback, both
subscriptions, and the Codex layer without restarting either window. The CLI
adapter verifies the app, `app.asar`, HID topology watcher, and Input Monitoring
permission module hashes before attaching; a changed byte disables this
contract until new evidence is frozen.

The isolated Codex Companion Bridge v1 fixture has cross-language evidence for
authenticated capability negotiation; frozen-definition settings snapshots;
source-SHA plus canonical-settings CAS; immutable backup; complete explicit-set
apply; and exact explicit/effective readback. It also verifies all six Agent Key
slots, command/Skill/task/keycap/empty assignment types, a separate canonical
global-state revision, complete-object apply, idempotent replay, stale-CAS
rejection, explicit restore, and automatic rollback after corrupt readback. Its
E2E path proves `recent -> custom -> recent`, brightness `100 -> 37 -> 100`,
auto-off `3-minutes -> 10-minutes -> 3-minutes`, and voice mode
`push-to-talk -> realtime -> push-to-talk`; restores the exact Agent Key revision;
and recovers the exact baseline settings source SHA-256. This is bridge
transaction evidence; released-Codex mutation begins when Codex installs the
reference integration and supplies exact settings and Agent Key replacers.

For Input 0.18.0 and Codex Micro firmware v0.6.0,
`input-bundled-device-kit-read-v1` has live read-only evidence over HID: the CLI
read status, listed `keymap.json` and `smart_actions.json`, exported their exact
bytes, matched device SHA-1 and host SHA-256, reopened the typed manifest and
files, and atomically published the bundle. The tested session used the
Bluetooth-reported connection (`isUsbConnection=false`). Input was gracefully
quit and reopened for each read. Cached `keymap.json` and `smart_actions.json`
remained byte-identical. Input may rewrite its own `input_storage.json` startup
metadata (`options.started` and Loki `meta.revision`/`meta.updated`) after a
reopen; a recursive comparison found no semantic configuration difference.
This boundary does not cover device, cache, or database mutation.

## Compatibility states

Each version adapter will use one of four explicit states:

- **supported** — fixtures and required hardware behaviors are verified;
- **read-only** — inspection is verified; mutations remain gated;
- **experimental** — an opt-in adapter exists with a documented test boundary;
- **unknown** — fields are preserved and inspection is allowed, while mutations
  wait for an adapter.

## Version policy

Before a mutation, WorkLouderCTL will detect and record:

- device identity and transport;
- firmware version;
- Input application version;
- Codex application version for Tier 1 inspection;
- device-file schema version;
- device file list, sizes, and checksums;
- Input cache/database format adapter;
- every unknown field that must be preserved.

An Input update alone will not silently select the nearest adapter. Exact
matching or a verified compatibility range is required for writes.

The Companion Bridge uses capability negotiation instead of matching Input's
internal bundle hash. Once a released Input build includes protocol v1, internal
device-kit and GUI updates remain behind Input's stable bridge adapter.
The current isolated cross-language fixture additionally verifies exact base64
snapshot bytes, device SHA-1, host SHA-256, an independently recomputed
deterministic revision, and a live compare-and-swap preflight. This is bridge
contract evidence rather than a released Input 0.18.0 capability claim.
The fixture writer also verifies complete apply readback, idempotent replay
without a second write, stale-CAS rejection, explicit restore, and automatic
rollback after a corrupt readback. These are transaction-engine claims; they do
not extend the Input 0.18.0 hardware boundary until an exact writer adapter and
real-device restore test pass.

## Adding support

New compatibility claims require:

1. sanitized before/after fixtures;
2. a deterministic parser/serializer round trip;
3. semantic validation coverage;
4. a no-op apply result with no byte changes;
5. one intentional modification with exact device readback;
6. a verified rollback to the original checksums;
7. the tested boundary recorded in this document.

See the [frozen 2026-08-02 audit](research/2026-08-02-codex-micro-audit.md)
for current hashes, state authorities, protocol observations, and claim limits.
