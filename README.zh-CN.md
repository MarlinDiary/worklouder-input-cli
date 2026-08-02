# WorkLouderCTL：Work Louder Input 与 Codex Micro 的 CLI

**覆盖 Codex App 与 Work Louder Input 全部配置能力的开源 CLI。**

[English](README.md) · [常见问题](docs/faq.md) ·
[兼容性](docs/compatibility.md) · [架构](docs/architecture.md) ·
[路线图](docs/roadmap.md)

> **当前状态：pre-alpha。** 仓库目前提供产品定义和研究基线，尚未发布可安装版本。下方命令代表目标接口。

## 简单来说，它是什么？

如果你正在搜索以下问题：

- Work Louder Input 有没有 CLI？
- 如何通过命令行配置 Codex Micro？
- AI Agent 能否安全修改 Work Louder Input 配置？
- 如何备份和恢复 Codex Micro 的 profiles、layers 和 keymap？

WorkLouderCTL 就是为这些场景设计的。

产品目标是让 GUI 对“配置”变为可选：Tier 1 通过 Codex settings adapter
完整读写，Tier 2 及以上通过 Input/device adapters 完整读写。Codex 和 Input
仍可作为 Codex-aware actions、AppSense、Smart Actions 与动态灯光的运行时。

```text
读取当前状态 → 生成计划与 diff → 备份 → 写入 → readback 验证 → 同步 Input → 恢复或完成
```

## 计划中的命令

```console
worklouderctl doctor
worklouderctl codex export
worklouderctl codex agent-source set priority
worklouderctl codex command-key set ACT06 --command toggleFastMode
worklouderctl codex joystick set up --skill SKILL_ID
worklouderctl input inspect
worklouderctl device status
worklouderctl profile list
worklouderctl layer show 2
worklouderctl plan layout.yaml
worklouderctl apply layout.yaml
worklouderctl backup restore BACKUP_ID
```

## 目标功能

- profiles 与六层 layouts；
- Codex Agent Keys、Command Keys、voice、dial、joystick、Skills 与全局灯光；
- 所有按键、旋钮和摇杆方向；
- keycodes、Actions、Multi Actions 与 Smart Actions；
- linked apps 与 AppSense；
- backlight、underglow、颜色与灯光效果；
- device、Input cache 与 Input database 的统一备份；
- plan-first 写入、精确 readback、checksum 与自动 rollback；
- 稳定 JSON 输出，方便 AI Agent 调用。

## 第一阶段兼容目标

- 设备：Work Louder Codex Micro
- 系统：macOS
- Codex 检查基线：Codex 26.727.51351
- Input schema fixtures：Work Louder Input 0.17.3 与 0.18.0
- firmware fixture：Codex Micro v0.6.0

以上是当前研究基线；正式支持范围会通过版本适配器、fixtures 和真实硬件 readback 明确记录。

## 与 Codex 和 Input 的关系

WorkLouderCTL 是 **Full-configuration CLI**：

- CLI 覆盖 Codex 与 Input 针对 Codex Micro 的全部配置项；
- Codex/Input GUI 可以继续使用，但不是完成配置的必需入口；
- Tier 1 写入 Codex settings authority，Tier 2+ 同步 device、Input cache/database；
- 需要宿主语义的动作仍由对应 runtime 执行并接受行为验证。

未来可以继续发展独立 driver/runtime；它与“完整替代配置 GUI”是两个独立目标。

## 当前进度

仓库目前处于产品定义与 fixture 准备阶段。可安装 binary、Homebrew formula 和完整命令将在经过硬件验证后发布。

配置边界已经确定：**Tier 1 使用 Codex 的设置模型与运行时；Tier 2 及以上
使用 Input 的设置模型与运行时；CLI 对两边都提供完整配置能力。** 详见
[Tier 模型](docs/tier-model.md)、
[完整配置参考](docs/configuration-reference.md) 与
[配置能力对等矩阵](docs/configuration-parity.md)、
[2026-08-02 深度审计](docs/research/2026-08-02-codex-micro-audit.md)。

## 项目独立性

WorkLouderCTL 是独立社区项目，与 Work Louder 或 OpenAI 没有官方隶属或背书关系。相关产品名称归各自所有者所有。

## License

[MIT](LICENSE)
