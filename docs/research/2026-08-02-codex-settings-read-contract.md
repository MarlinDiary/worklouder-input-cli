# Codex settings read contract (26.727.51351)

This note freezes the observed Codex Micro settings boundary used by the first
read-only Codex adapter. The machine-readable contract is
[`spec/codex-settings-26.727.51351.json`](../../spec/codex-settings-26.727.51351.json).

## Frozen application evidence

- App bundle: `/Applications/ChatGPT.app`
- Bundle version: `26.727.51351`
- ASAR: `Contents/Resources/app.asar`
- ASAR SHA-256:
  `a529edd72e10b08931c0d695b5e3e6a0be7f51874610dafc04f578436ab7d74d`

The renderer posts JSON parameters to `vscode://codex/{method}`. The native
`settings-read` handler returns `filePath`, explicit `settings`,
`effectiveSettings`, and `definitions`. The `settings-write` handler validates
each supplied value, updates the settings store, flushes it, and returns the
explicit and effective settings.

## Storage mapping

The settings store reads the `[desktop]` table in `$CODEX_HOME/config.toml`,
where `CODEX_HOME` defaults to `~/.codex`. Writes are delegated to the Codex
configuration client with paths shaped as `desktop.<key>`; this is not a
Chromium LevelDB store.

The initial adapter is deliberately named `codex-config-toml-read-v1`. It reads
only keys with the `codex-micro-` prefix, fills the five frozen defaults for an
effective view, and never serializes unrelated Codex configuration. A later
write adapter should use the running bridge or the equivalent Codex
configuration transaction rather than editing TOML text in place.

The implemented offline candidate adapter consumes that typed snapshot for
Agent source, single-tap mode, and all six Command Key slots. It validates the
embedded definitions/effective view, preserves unknown prefixed settings,
records a deterministic settings revision plus the original source SHA-256,
and leaves online mutation to the versioned `settings-write` transaction.

## Capture and compatibility rules

1. Hash the source file before and after reading it. Reject a moving capture.
2. Parse TOML and extract only the Codex Micro settings subtree.
3. Validate known settings against the frozen versioned contract.
4. Preserve unknown `codex-micro-*` keys in the explicit settings view.
5. Warn when the installed app version differs from the frozen version; strict
   doctor mode promotes that warning to a failing status.
6. Export through a sibling staging file, atomically rename it, reopen it, and
   compare the typed result with the planned snapshot.

No captured user setting values are committed in this research artifact.
