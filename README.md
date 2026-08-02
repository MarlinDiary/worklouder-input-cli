# WorkLouderCTL — Work Louder Input CLI for Codex Micro

<p align="center">
  <strong>The open-source companion CLI for Work Louder Input and Codex Micro.</strong>
</p>

<p align="center">
  <a href="README.zh-CN.md">简体中文</a> ·
  <a href="docs/faq.md">FAQ</a> ·
  <a href="docs/compatibility.md">Compatibility</a> ·
  <a href="docs/architecture.md">Architecture</a> ·
  <a href="docs/roadmap.md">Roadmap</a>
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

WorkLouderCTL is an unofficial, open-source companion to the Work Louder Input
app. Its goal is to inspect, plan, diff, back up, apply, verify, and restore
device configurations while keeping Input's GUI, cache, database, and the
device's persisted files synchronized.

## Why a companion CLI?

The official Input app is useful for visual editing. A CLI adds workflows that
are difficult to make repeatable in a GUI:

- review a complete configuration as text;
- generate and approve an exact diff before a write;
- back up profiles, layers, keymaps, and Smart Actions;
- apply the same layout reproducibly;
- verify device readback and checksums;
- roll back a failed or unwanted change;
- let scripts and AI agents use a stable, machine-readable interface.

WorkLouderCTL is designed to **cooperate with Input**, rather than race it for
the device. A guarded mutation will coordinate the app, preserve every state
authority, write only after validation, verify the result, and reopen Input.

## Planned command experience

The intended binary name is `worklouderctl`:

```console
worklouderctl doctor
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
|---|---|
| Device discovery | Codex Micro status, firmware, active profile/layer, USB and Bluetooth transport details |
| Profiles and layers | List, create, rename, select, diff, import, and export |
| Controls | Keys, rotary encoder, encoder press, and planar joystick sectors |
| Actions | Keycodes, Actions/macros, Multi Actions, Smart Actions, groups, and reference validation |
| Lighting | Layer backlight, underglow, effects, colors, brightness, and speed |
| AppSense | Linked applications and focused-app layer selection |
| Safety | Plan-first writes, immutable backups, conflict detection, exact readback, checksums, and rollback |
| Automation | Deterministic JSON output, typed exit codes, and an agent-friendly contract |

## Safety model

A mature write path must treat the device and Input as multiple synchronized
authorities:

```text
inspect current state
        ↓
validate + produce a plan and diff
        ↓
coordinate and pause Input
        ↓
back up device files + Input cache + Input database
        ↓
write changed files in dependency-safe order
        ↓
read back bytes/JSON + verify checksums
        ↓
atomically update Input state
        ↓
reopen Input or roll everything back
```

Unknown Input, firmware, or schema versions will default to inspection until a
version adapter and fixture coverage exist.

## Initial compatibility target

The first implementation target is intentionally narrow:

- **Device:** Work Louder Codex Micro
- **Host:** macOS
- **Research baseline:** Work Louder Input 0.17.3
- **Firmware fixture baseline:** Codex Micro v0.6.0

These versions describe the current research fixtures, not a released support
guarantee. See the [compatibility policy](docs/compatibility.md), the vendor's
[Codex Micro product page](https://worklouder.cc/codex-micro), and the official
[Codex Micro setup guide](https://worklouder.cc/openai-micro-setup).

## Project status

| Milestone | Status |
|---|---|
| SEO/AIO and product documentation foundation | Complete |
| Sanitized research fixtures and schema model | Planned |
| Read-only `doctor`, `inspect`, `status`, and `export` | Planned |
| Semantic profile/layer/control commands | Planned |
| Verified device mutation and rollback | Planned |
| Input cache/database and Smart Actions synchronization | Planned |
| Signed macOS release and Homebrew installation | Planned |

## Frequently asked questions

### Is there a CLI for Work Louder Input?

WorkLouderCTL is being developed as an open-source companion CLI for Work Louder
Input. The repository is currently pre-alpha and does not yet publish an
installable binary.

### Does WorkLouderCTL replace the Input app?

The first product is a companion: Input remains available as the visual editor,
while WorkLouderCTL provides repeatable inspection, automation, verification,
and rollback. A standalone driver is a possible later track.

### Does it support Codex Micro?

Codex Micro on macOS is the first planned and researched target. Support claims
will be tied to exact Input and firmware versions with hardware readback.

### Can an AI agent configure Codex Micro?

That is a core design goal. The agent-facing interface will use the same
plan/apply/verify/rollback engine as the human CLI, with deterministic JSON and
no hidden write path.

Read the complete [FAQ](docs/faq.md).

## Documentation

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
