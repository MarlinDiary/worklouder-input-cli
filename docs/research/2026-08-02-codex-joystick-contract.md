# Codex joystick candidate contract (Codex 26.727.51351)

The Codex-native analog stick is stored under
`codex-micro-layout.analogStick` with four directions: `up`, `right`, `down`,
and `left`. Each direction is a command object, Skill object, or `null`.

The frozen defaults are plan-mode toggle, forward navigation, sidebar toggle,
and back navigation respectively. Released UI and runtime inspection shows that
these mappings are independent of the Input radial-menu/joystick-sector model;
they remain Tier 1 Codex actions resolved by the Codex runtime.

`codex joystick get/set/clear` validates the full frozen snapshot, reads or
changes one direction leaf, preserves the other three directions and unknown
layout fields, recomputes effective settings and the canonical revision, and
atomically publishes/reopens the candidate. Candidate generation stays
file-only; apply/readback/rollback remains a separate complete-settings Codex
Companion Bridge transaction.
