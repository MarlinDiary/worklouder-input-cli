# Sanitized compatibility fixtures

These fixtures are deterministic, synthetic minimum examples for adapter and
schema regression. They contain no exported user configuration, device serial,
account, application identity, log, credential, or firmware binary.

- `input/0.17.3/codex-micro-v0.6.0` freezes the legacy minimum keymap family.
- `input/0.18.0/codex-micro-v0.6.0` adds the separate Smart Action file and
  current layer-lighting shape.
- `firmware/codex-micro-v0.6.0` freezes a sanitized status DTO only.

Every directory has a manifest of exact sizes and SHA-256 digests. Regenerate
and verify the complete tree with:

```sh
./scripts/verify-sanitized-fixtures.sh
```

The 0.17.3 fixture is a structural compatibility boundary because its original
application bundle was superseded during research. The 0.18.0 read contract
has separate hash-pinned installed-package and live-device evidence in
`spec/` and `docs/research/`; this synthetic tree deliberately does not copy
the observed device configuration.
