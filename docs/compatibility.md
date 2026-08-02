# Compatibility and support policy

WorkLouderCTL uses evidence-based compatibility claims. A version is listed as
supported only after fixtures, automated checks, device readback, and rollback
verification cover the relevant operations.

## Current project stage

There is no released binary yet. The versions below are research baselines,
not a public support guarantee.

| Component | Research baseline | Current claim |
|---|---:|---|
| Device | Work Louder Codex Micro | Initial target |
| Host OS | macOS | Initial target |
| Work Louder Input | 0.17.3 | Schema and behavior research baseline |
| Codex Micro firmware | v0.6.0 | Device read/write fixture baseline |
| USB | Observed | Release verification pending |
| Bluetooth | Observed | Release verification pending |

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
