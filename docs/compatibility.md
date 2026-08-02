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
| Codex app | 26.727.51351 | Exact Tier 1 settings read adapter verified |
| Work Louder Input | 0.17.3 | Earlier schema fixture |
| Work Louder Input | 0.18.0 | Exact bundled-kit live read adapter verified |
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
Codex settings mutation.

For Input 0.18.0 and Codex Micro firmware v0.6.0,
`input-bundled-device-kit-read-v1` has live read-only evidence over HID: the CLI
read status, listed `keymap.json` and `smart_actions.json`, exported their exact
bytes, matched device SHA-1 and host SHA-256, reopened the typed manifest and
files, and atomically published the bundle. The tested session used the
Bluetooth-reported connection (`isUsbConnection=false`). Input was gracefully
quit and reopened for each read, and the three cached configuration-file
SHA-256 values were identical before and after. This boundary does not cover
device, cache, or database mutation.

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
