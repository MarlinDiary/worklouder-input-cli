# Codex dial candidate contract (Codex 26.727.51351)

The frozen Codex layout stores the dial in two independent fields:

- `encoderMode`: `composer-navigation`, `reasoning`, `conversation-scroll`, or
  `custom`;
- `encoder`: `left`, `right`, `click`, and `longPress`, each holding a command,
  Skill, or `null`.

## Released behavior

The inspected Codex bundle changes `encoderMode` without rewriting the encoder
mapping. Its settings UI exposes the action editor only when the selected mode
is `custom`. At runtime, left/right rotation uses custom mappings only in that
mode. Click and long press likewise invoke the custom mapping only in custom
mode; otherwise click retains the selected built-in mode behavior and long
press opens Codex Micro settings.

## CLI contract

`codex dial mode get/set` reads or changes only `encoderMode` and preserves all
four gesture mappings. `codex dial gesture get` reports the effective command,
Skill, or empty mapping. `gesture set/clear` requires effective mode `custom`,
changes one gesture leaf, preserves the other three gestures and unknown layout
fields, validates the complete layout, recomputes effective settings and the
canonical settings revision, and atomically publishes/reopens the candidate.

Candidate generation is file-only. Applying the complete candidate remains a
separate Codex Companion Bridge transaction with source/revision CAS, readback,
automatic rollback, and explicit restore.
