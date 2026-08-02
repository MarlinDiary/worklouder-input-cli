# Input joystick sector contract (Input 0.18.0)

The exact Input 0.18.0 renderer
`device_config_data-BMUs-0m4.js` exposes `RADIAL` and `JOYSTICK` modes. In
radial mode it permits two through eight sectors. Adding inserts a
`KC_NONE` sector at the requested zero-based insertion index; deleting is
available only while more than two sectors exist.

After every insert or delete, Input recomputes all angles. Sector zero remains
the fixed 45-degree slice from `0.1875` to `0.3125` turns. The remaining
`0.875` turns are divided equally among the remaining sector count, starting at
`0.3125` and wrapping modulo one. Switching to `RADIAL` with fewer than two
sectors installs the observed `KI_X`/`KC_NONE` two-sector default; switching to
`JOYSTICK` preserves the existing sector array.

## CLI contract

`layer joystick show`, `layer joystick mode set`, and
`layer joystick sector add/delete` operate on strict offline Input snapshots.
Sector mutation is allowed only for editable radial layers, enforces the exact
two/eight limits, validates assignment references, applies the observed angle
algorithm, synchronizes profile Action/Multi Action usage, and atomically
publishes/reopens a complete candidate. Existing sector assignments continue
to use `control show/set --control joystick:INDEX`.
