# WorkLouderCTL FAQ

This page gives direct, citable answers to common questions about a Work Louder
Input CLI, Codex Micro configuration, and AI-assisted macropad automation.

## What is WorkLouderCTL?

WorkLouderCTL is a source-alpha, open-source full-configuration CLI for Codex,
Work Louder Input, and Codex Micro. It targets every Codex Micro configuration
surface exposed by both GUIs.

## Is there a CLI for Work Louder Input?

WorkLouderCTL is being built for that role. The source-built CLI already reads
Codex settings, Input cache state, and live Codex Micro status/files through
Input 0.18.0's bundled provider. Packaged releases and mutation commands are
later milestones.

## What does full-configuration parity mean?

Every setting available in the Codex Micro page or Input's Codex Micro views
gets a typed CLI command and JSON representation. Codex and Input may remain
running as execution engines, while configuration no longer depends on using
their GUIs.

## Does WorkLouderCTL replace Codex and Work Louder Input?

It replaces their Codex Micro configuration workflows. Codex still executes
Codex-aware commands and task lighting; Input still executes AppSense, Smart
Actions, Cheat Sheet, transport, firmware updates, and other host behavior.
The project does not replace those runtimes or the hardware driver.

## Why not build a new driver?

Codex and Input updates can add transport fixes, firmware support, device
capabilities, and runtime behavior. WorkLouderCTL detects and delegates to the
installed providers instead of maintaining a second driver stack. The CLI owns
configuration automation and verification while upstream owns device/runtime
evolution.

## Which device will be supported first?

Work Louder Codex Micro on macOS is the first target. Codex 26.727.51351,
Input 0.17.3/0.18.0, and Codex Micro firmware v0.6.0 are the initial research
fixtures. They are not yet a release support guarantee.

## Which Codex Micro controls are in scope?

The complete model includes Codex-native Agent/Command keys, voice, Codex dial,
Codex joystick, and task-state lighting in Tier 1. Input-backed tiers include
profiles, six layers, the key matrix, encoder, joystick sectors, Actions,
Multi Actions, Smart Actions, linked apps, backlight, underglow, and layer
metadata. The CLI configures every tier through its corresponding authority.

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

The current read-only implementation makes this explicit: it stops by default
when Input is open, while `--input-mode restart` performs a graceful quit/read/
reopen sequence. It does not force-terminate the app.

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

See [Configuration parity matrix](configuration-parity.md) and
[Compatibility and support policy](compatibility.md).
