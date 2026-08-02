# Input 0.18.0 Cheat Sheet assignment contract

Static inspection of the exact installed Input 0.18.0 build identifies four
Cheat Sheet assignments in the released Codex Micro key catalog:

| CLI behavior | Input token | Released label | Host notification |
| --- | --- | --- | --- |
| `show` | `KI_CS_SHOW` | Show Cheat-Sheet | `kb.cs.show` |
| `hold` | `KI_CS_SHOW_TMP` | Show Cheat-Sheet Hold | firmware-owned show/hide pair |
| `hide` | `KI_CS_HIDE` | Hide Cheat-Sheet | `kb.cs.hide` |
| `toggle` | `KI_CS_TOGGLE` | Toggle Cheat-Sheet | `kb.cs.toggle` |

Input exposes these tokens for Creator Micro V2 and Codex Micro at firmware
`0.5.0` or newer. The inspected device boundary is Codex Micro `v0.6.0`.
Show, hide, and toggle notifications are routed to `WindowService` operations
`1`, `0`, and `2`. Input initializes the Cheat Sheet from the current complete
device configuration plus zero-based profile/layer indices in advanced mode.

WorkLouderCTL treats this as configuration, not as a second host window runtime.
`cheat-sheet catalog` publishes the frozen mapping, `bindings` finds only these
four assignments in one layer, and `bind` changes one physical control token in
an offline complete snapshot. Deployment continues through the existing Input
bridge apply/readback/restore transaction. The CLI does not open or close Input
windows while producing a candidate.

Evidence hashes are frozen in
[`spec/input-cheat-sheet-0.18.0.json`](../../spec/input-cheat-sheet-0.18.0.json).
