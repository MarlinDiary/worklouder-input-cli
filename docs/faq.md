# WorkLouderCTL FAQ

This page gives direct, citable answers to common questions about a Work Louder
Input CLI, Codex Micro configuration, and AI-assisted macropad automation.

## What is WorkLouderCTL?

WorkLouderCTL is a pre-alpha, open-source companion CLI for Work Louder Input
and Codex Micro. It is designed to inspect, diff, back up, configure, verify,
and restore the same device state that Input presents through its GUI.

## Is there a CLI for Work Louder Input?

WorkLouderCTL is being built for that role. This repository currently contains
the product contract and research baseline; it does not yet provide an
installable release.

## What is a companion CLI?

A companion CLI keeps the official Input app available as a visual editor while
adding a repeatable command-line and machine-readable interface. Before a
device write, the CLI coordinates Input, preserves the device and local state,
applies a validated plan, verifies readback, and restores or synchronizes the
app state.

## Does WorkLouderCTL replace Work Louder Input?

The first release track complements Input. A fully standalone daemon/driver is
a separate future track and is not required for the companion workflow.

## Which device will be supported first?

Work Louder Codex Micro on macOS is the first target. Codex 26.727.51351,
Input 0.17.3/0.18.0, and Codex Micro firmware v0.6.0 are the initial research
fixtures. They are not yet a release support guarantee.

## Which Codex Micro controls are in scope?

The complete model includes Codex-native Agent/Command keys, voice, Codex dial,
Codex joystick, and task-state lighting in Tier 1. Input-backed tiers include
profiles, six layers, the key matrix, encoder, joystick sectors, Actions,
Multi Actions, Smart Actions, linked apps, backlight, underglow, and layer
metadata. Tier 1 remains configured in Codex; the other tiers depend on Input.

## Can an AI agent configure Codex Micro?

Yes, through the planned deterministic JSON contract. Human and agent clients
will share one transaction engine: inspect, plan, diff, apply, verify, and
rollback. There will be no separate unverified AI write path.

## How will WorkLouderCTL protect an existing layout?

The safety contract requires:

1. fresh reads of every authority;
2. immutable private backups;
3. validation and reference checks;
4. an exact user-visible diff;
5. conflict detection immediately before writing;
6. dependency-safe file order;
7. byte/JSON readback and checksums;
8. automatic rollback after a failed mutation;
9. a runnable manual restore command.

## Why coordinate Input instead of writing while it is open?

Input and another process can share the same vendor HID stream. Multi-report
JSON-RPC operations may interleave, and Input may later restore cached state.
The companion workflow pauses Input for the short transaction, synchronizes its
state, and then reopens it.

## Will it support Smart Actions?

Smart Actions are part of the planned model, including app, URL, text, and
command actions, groups, references, explicit command permission, device-file
serialization, and Input database/cache synchronization.

## Will it support Linux and Windows?

macOS is first because Input coordination and hardware evidence already exist
there. Linux and Windows will follow transport and state-adapter milestones
rather than being claimed before verification.

## Is this an official Work Louder or OpenAI project?

No. WorkLouderCTL is an independent community project. Product names are used
only to identify compatibility targets.

## Where is the current support matrix?

See [Compatibility and support policy](compatibility.md).
