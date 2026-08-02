# Input joystick sector contract (Input 0.18.0)

Exact static inspection of `device_config_data-BMUs-0m4.js` shows two joystick
modes. `RADIAL` is editable; `JOYSTICK` is displayed as “coming soon” and its
selector is disabled. Switching an existing non-radial layout to `RADIAL`
seeds two sectors when fewer than two exist: `KI_X` followed by `KC_NONE`.

The radial editor permits 2 through 8 sectors. Add inserts `KC_NONE` immediately
before or after the selected sector. Delete is displayed only above two sectors.
Both operations pass the entire ordered list through the same deterministic
angle rebalancer.

The rebalancer pins sector zero to 45 degrees beginning at 67.5 degrees. The
remaining 315 degrees are divided equally across sectors 1 through N-1, with
both endpoints stored as normalized turns modulo 1. It preserves each retained
sector's assignment and unknown fields while replacing `a1` and `a2`.

## CLI contract

`layer joystick show` reports mode and ordered sectors. `layer joystick mode
set ... radial` follows the released seeding rule. `layer joystick sector add`
and `delete` enforce the released 2–8 limits, insertion/deletion indexes, and
angle algorithm. Assignment changes continue through `control set --control
joystick:INDEX`, so add itself uses the released `KC_NONE` value.

All mutations consume and publish offline semantic snapshots, preserve
untargeted files and unknown fields, synchronize Action/Multi Action usage, and
leave Input, its GUI, device state, and cache bytes unchanged. Applying a
candidate remains a separate Input Companion Bridge transaction.
