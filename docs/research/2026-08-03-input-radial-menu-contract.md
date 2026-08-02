# Input 0.18.0 radial-menu configuration contract

Static inspection of the exact installed Input 0.18.0 ASAR shows that the
radial menu has no independent persisted settings. Input's `WindowService`
builds its initial data from the selected profile/layer joystick sectors plus
the complete Action, Multi Action, and Smart Action collections. The host
window remains an Input-owned runtime.

The device sends `kb.radial` with angle, distance, profile-array index,
layer-array index, operation, and optionally the active sector. Operation zero
hides the window; other operations show or update it. Input refuses to
initialize a radial window with fewer than two sectors and automatically hides
an inactive window after three seconds.

The renderer resolves a sector's display identity as follows:

- `KA_A<ID>`: Action `name`, optional `color`, and optional `icon`;
- `KA_M<ID>`: Multi Action `name`, optional `color`, and optional `icon`;
- `SA_<ID>`: Smart Action `name`, optional `color`, and optional `icon`;
- `KC_*` / `KI_*`: the host-OS HID label map;
- `KV_*`: the released vendor placeholder label `1`.

WorkLouderCTL exposes the configuration result without opening Input's window:

```console
worklouderctl radial show --input CONFIG.json [--profile PROFILE_ID] --layer LAYER_ID
```

`radial show` validates the complete snapshot, returns ordered sector angles,
assignments, kinds, and resolved resource labels. Mutation is already complete
through `layer joystick sector add/delete` and `control set --control
joystick:SECTOR`; deployment continues through the complete Input bridge
apply/readback/restore transaction. This keeps configuration parity separate
from reimplementing Input's overlay runtime.

Frozen bundle names and SHA-256 values are recorded in
[`spec/input-radial-menu-0.18.0.json`](../../spec/input-radial-menu-0.18.0.json).
