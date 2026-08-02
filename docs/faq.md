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
Input 0.18.0's bundled provider. It also generates strict offline candidates
and runs authenticated Codex/Input apply/readback/restore/rollback transactions
against isolated reference writers. Packaged releases and released-app writer
integration are later milestones.

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

Yes, through the deterministic JSON contract. Human and agent clients share
the same transaction model: inspect, plan, diff, apply, verify, and
rollback. There will be no separate unverified AI write path.

## Can it configure all six Codex Agent Keys?

Yes. The source-built CLI snapshots all six `AG00`-`AG05` assignments, reads or
edits one slot offline, supports command, Skill, task, keycap, and empty values,
then applies or restores the complete object through the Codex Companion Bridge.
The transaction uses a canonical global-state revision, immutable backup,
idempotent retry, stale-state rejection, exact readback, and automatic rollback.
Selecting those custom assignments remains a separate explicit
`codex-micro-agent-source=custom` settings transaction.

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

## How can the CLI coexist with Input while it is open?

The Input Companion Bridge keeps Input as the only owner of the device session.
CLI requests enter Input through a private authenticated Unix socket and use
the same service container and serialized device queue as GUI requests.

Write capabilities are advertised only when that Input version injects a
verified complete-configuration writer. The CLI then takes an immutable backup
and Input performs CAS, idempotent apply, full readback, and automatic rollback
inside the same serialized bridge session.

Until a released Input build includes that bridge, the direct compatibility
transport keeps `require-closed` as its default. Its explicit `restart` mode
performs the older graceful quit/read/reopen cycle.

## How can the CLI coexist with Codex while it is open?

The Codex Companion Bridge keeps the running Codex main process authoritative.
It delegates settings/global-state reads to Codex and advertises each mutation
capability only when Codex injects the corresponding exact complete-object
replacer. Settings use source-SHA plus settings-revision CAS; Agent Keys use a
separate global-state revision CAS. Both paths provide immutable backup, exact
readback, restore, and rollback over a private authenticated Unix socket. The
current Codex release contains the internal handlers but has not yet installed
this external listener; the repository includes the reference integration and
complete fixture E2E.

## Will it support Smart Actions?

Smart Action text, command, URL, and app definitions, groups, references,
bindings, and delete cascades are implemented as strict offline candidates.
Released Input writer and database synchronization remain separate milestones.

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
