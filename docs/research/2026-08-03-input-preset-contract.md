# Input preset contract (Input 0.18.0)

This note freezes the preset behavior recovered from the exact installed Input
0.18.0 ASAR without opening or controlling the GUI.

## Catalog

Input merges user-saved presets from the LokiJS `presets` collection before 17
bundled defaults. The main process exposes the saved collection through
`localStorageGetPresets` and `localStorageSavePreset`. The renderer owns the
bundled catalog. A preset DTO contains metadata and image fields plus one layer,
Action/Multi Action definitions, and their groups.

The renderer filters the merged catalog by exact device type, keyboard layout
type, and OS (`0` for macOS, `1` for Windows). Search is a case-insensitive
substring match over the preset name and tags. The installed catalog contains
IDs `9001..9016` plus the literal Windows Affinity ID `90017`; the CLI must
preserve that observed value rather than normalizing it.

## Install

Install is offered only while the selected profile has fewer than six layers.
Input clones the preset content, appends preset tags to imported group tags,
deduplicates Action/Multi Action definitions using the exact equality fields in
`spec/input-presets-0.18.0.json`, assigns new IDs after the current last ID,
remaps `KA_` and `KM_` references, deduplicates groups, assigns the cloned layer
`max(layer.id) + 1`, appends it, and selects the appended layer in renderer
state. Device persistence remains the ordinary complete keymap transaction.

The preset DTO has no Smart Action fields in this release. The generic import
resolver can handle Smart Actions for other import surfaces, but preset install
passes only the layer, Actions, Action groups, Multi Actions, and Multi Action
groups.

## Active-layer boundary

The released service returns `selectedLayerIndex` through `device.status`, and
preset install updates the renderer's selected-layer atom. Static inspection
found no persisted active-layer field and no device RPC setter for selecting a
layer. Therefore “live active-layer selection” is a runtime observation or a
layer-key/AppSense behavior, not an additional layer configuration mutation.

## CLI boundary

Catalog reads belong behind an injected Input-owned preset provider. Preset
installation produces a strict offline complete configuration candidate and
uses the existing snapshot/CAS/apply/readback/rollback transaction. The CLI
does not edit `input_storage.json` directly and does not bundle the multi-
megabyte preview images as source code.
