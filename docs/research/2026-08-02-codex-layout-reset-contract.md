# Codex layout reset candidate contract (Codex 26.727.51351)

The released settings UI confirms reset through
`settings.codexMicro.keyboardLayout.resetConfirmation`, then invokes the same
settings setter used by the other Codex Micro controls with the installed
build's `cxt` default layout object. The target setting is
`codex-micro-layout`; Agent Key assignment storage and Agent Key source remain
separate.

The frozen default contains the six Command Key slots, four analog-stick
directions, four dial gestures, dial mode, and voice-button mode. Therefore an
exact GUI-equivalent reset replaces the complete layout object rather than
resetting individual leaves. Other `codex-micro-*` settings are preserved.

## CLI contract

`codex reset layout` accepts a complete offline settings snapshot, validates
its frozen definitions and effective view, and compares the effective layout
with the exact embedded installed-build default. A changed candidate replaces
only `/settings/codex-micro-layout`, recomputes effective settings and the
canonical settings revision, and atomically publishes/reopens the result. An
already-inherited default is a no-op and remains implicit.

Candidate generation is file-only. Applying it remains a separate Codex
Companion Bridge transaction with source/revision CAS, exact readback,
automatic rollback, and explicit restore.
