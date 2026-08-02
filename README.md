# WorkLouderCTL — Work Louder Input CLI for Codex Micro

<p align="center">
  <strong>The open-source full-configuration CLI for Codex Micro, Codex, and Work Louder Input.</strong>
</p>

<p align="center">
  <a href="README.zh-CN.md">简体中文</a> ·
  <a href="docs/faq.md">FAQ</a> ·
  <a href="docs/compatibility.md">Compatibility</a> ·
  <a href="docs/architecture.md">Architecture</a> ·
  <a href="docs/roadmap.md">Roadmap</a>
</p>

<p align="center">
  <img alt="Project status: pre-alpha" src="https://img.shields.io/badge/status-pre--alpha-F59E0B">
  <img alt="Target platform: macOS first" src="https://img.shields.io/badge/platform-macOS%20first-111827">
  <img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-22C55E">
</p>

> [!IMPORTANT]
> **Project status: pre-alpha.** This repository currently contains the product
> specification and research baseline. There is no installable release yet.
> Commands and capabilities described below are the target contract unless they
> are explicitly marked as verified.

## The short answer

Looking for a **Work Louder Input CLI**, a **Codex Micro command-line
configurator**, or a safe way for an **AI agent to configure a Work Louder
macropad**? WorkLouderCTL is being built for that job.

WorkLouderCTL is an unofficial, open-source full-configuration CLI. Its product
contract is configuration parity with both the Codex Micro settings page in
Codex and every Codex Micro surface in Work Louder Input. The GUIs become
optional for configuration; Codex and Input may still provide the live runtime
for Codex-aware actions, AppSense, Smart Actions, and reactive lighting.

WorkLouderCTL does **not** implement a replacement keyboard driver, BLE/HID
stack, firmware protocol, or host-action runtime. It delegates those jobs to
the installed Codex and Input versions, so vendor updates remain available.

## Why a full-configuration CLI?

The official Input app is useful for visual editing. A CLI adds workflows that
are difficult to make repeatable in a GUI:

- review a complete configuration as text;
- generate and approve an exact diff before a write;
- back up profiles, layers, keymaps, and Smart Actions;
- apply the same layout reproducibly;
- verify device readback and checksums;
- roll back a failed or unwanted change;
- let scripts and AI agents use a stable, machine-readable interface.

WorkLouderCTL is designed to **replace both configuration GUIs while
cooperating with their runtimes**. A guarded mutation coordinates Codex and
Input, preserves every state authority, writes only after validation, verifies
the result, and refreshes or reopens the affected runtime.

## Planned command experience

The intended binary name is `worklouderctl`:

```console
worklouderctl doctor
worklouderctl codex export
worklouderctl codex agent-source set priority
worklouderctl codex command-key set ACT06 --command toggleFastMode
worklouderctl codex joystick set up --skill SKILL_ID
worklouderctl codex lighting set --brightness 80 --auto-off 10-minutes
worklouderctl input inspect
worklouderctl device status
worklouderctl profile list
worklouderctl layer show 2
worklouderctl plan layout.yaml
worklouderctl apply layout.yaml
worklouderctl backup list
worklouderctl backup restore BACKUP_ID
```

These examples document the planned interface; they are not released commands
yet. Follow the [roadmap](docs/roadmap.md) for implementation status.

## Target capabilities

| Area | Target scope |
| --- | --- |
| Codex-native configuration | Agent Keys, Command Keys, voice mode, dial, joystick, Skills, global lighting, and reset |
| Device discovery | Codex Micro status, firmware, active profile/layer, USB and Bluetooth transport details |
| Profiles and layers | List, create, rename, select, diff, import, and export |
| Controls | Keys, rotary encoder, encoder press, and planar joystick sectors |
| Actions | Keycodes, Actions/macros, Multi Actions, Smart Actions, groups, and reference validation |
| Lighting | Layer backlight, underglow, effects, colors, brightness, and speed |
| AppSense | Linked applications and focused-app layer selection |
| Safety | Plan-first writes, immutable backups, conflict detection, exact readback, checksums, and rollback |
| Automation | Deterministic JSON output, typed exit codes, and an agent-friendly contract |

## Safety model

A mature write path must treat Codex, Input, and the device as synchronized
but distinct authorities:

```text
inspect current state
        ↓
validate + produce a plan and diff
        ↓
coordinate and pause Input
        ↓
back up Codex settings + device files + Input cache/database
        ↓
write changed files in dependency-safe order
        ↓
read back bytes/JSON + verify checksums
        ↓
atomically update the selected Codex/Input authorities
        ↓
refresh runtimes or roll everything back
```

Unknown Input, firmware, or schema versions will default to inspection until a
version adapter and fixture coverage exist.

## Initial compatibility target

The first implementation target is intentionally narrow:

- **Device:** Work Louder Codex Micro
- **Host:** macOS
- **Codex inspection baseline:** Codex 26.727.51351
- **Input schema fixtures:** Work Louder Input 0.17.3 and 0.18.0
- **Firmware fixture baseline:** Codex Micro v0.6.0

These versions describe the current research fixtures, not a released support
guarantee. See the [compatibility policy](docs/compatibility.md), the vendor's
[Codex Micro product page](https://worklouder.cc/codex-micro), and the official
[Codex Micro setup guide](https://worklouder.cc/openai-micro-setup).

## Project status

| Milestone | Status |
| --- | --- |
| SEO/AIO and product documentation foundation | Complete |
| Sanitized research fixtures and schema model | Planned |
| Read-only `doctor`, `inspect`, `status`, and `export` | Planned |
| Semantic profile/layer/control commands | Planned |
| Verified device mutation and rollback | Planned |
| Input cache/database and Smart Actions synchronization | Planned |
| Signed macOS release and Homebrew installation | Planned |

## Frequently asked questions

### Is there a CLI for Work Louder Input?

WorkLouderCTL is being developed as an open-source full-configuration CLI for
Codex, Work Louder Input, and Codex Micro. The repository is currently
pre-alpha and does not yet publish an installable binary.

### Does WorkLouderCTL replace the Codex and Input configuration GUIs?

Yes. Full configuration parity is the product target. Codex and Input remain
the driver/runtime providers for features that execute inside those apps. A
replacement driver/runtime is outside the project contract.

### Does it support Codex Micro?

Codex Micro on macOS is the first planned and researched target. Support claims
will be tied to exact Input and firmware versions with hardware readback.

### Can an AI agent configure Codex Micro?

That is a core design goal. The agent-facing interface will use the same
plan/apply/verify/rollback engine as the human CLI, with deterministic JSON and
no hidden write path.

Read the complete [FAQ](docs/faq.md).

## Documentation

- [Configuration tier model](docs/tier-model.md)
- [Complete configuration reference](docs/configuration-reference.md)
- [Configuration parity matrix](docs/configuration-parity.md)
- [2026-08-02 Codex Micro and Input audit](docs/research/2026-08-02-codex-micro-audit.md)
- [Frequently asked questions](docs/faq.md)
- [Compatibility and support policy](docs/compatibility.md)
- [Companion architecture](docs/architecture.md)
- [Product roadmap](docs/roadmap.md)
- [AI-readable project index](llms.txt)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)

## Contributing

The project is at the specification and fixture stage. Protocol findings,
sanitized configurations, compatibility evidence, and CLI design feedback are
welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a pull request.

## Independence

WorkLouderCTL is an independent community project. It is not affiliated with or
endorsed by Work Louder or OpenAI. Work Louder, Input, Codex Micro, Codex, and
other product names belong to their respective owners.

## License

[MIT](LICENSE)
