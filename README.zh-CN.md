# WorkLouderCTL：Work Louder Input 与 Codex Micro 的 CLI

**面向 Work Louder Input 与 Codex Micro 的开源 Companion CLI。**

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

它不会和 Input 同时争抢设备，而是将 Input 保留为 GUI，并为用户、脚本和 AI 提供可复现的命令行工作流：

```text
读取当前状态 → 生成计划与 diff → 备份 → 写入 → readback 验证 → 同步 Input → 恢复或完成
```

## 计划中的命令

```console
worklouderctl doctor
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
- 研究基线：Work Louder Input 0.17.3
- firmware fixture：Codex Micro v0.6.0

以上是当前研究基线；正式支持范围会通过版本适配器、fixtures 和真实硬件 readback 明确记录。

## 与 Input 的关系

WorkLouderCTL 第一阶段是 **Input Companion CLI**：

- Input 继续作为可视化编辑器；
- CLI 负责自动化、diff、批量配置、备份、验证和恢复；
- GUI 修改后，CLI 读取最新状态；
- CLI 修改后，同步 Input 的 cache/database，再重新打开 Input。

未来可以继续发展独立 driver，但它不阻塞 Companion CLI 的首个版本。

## 当前进度

仓库目前处于产品定义与 fixture 准备阶段。可安装 binary、Homebrew formula 和完整命令将在经过硬件验证后发布。

## 项目独立性

WorkLouderCTL 是独立社区项目，与 Work Louder 或 OpenAI 没有官方隶属或背书关系。相关产品名称归各自所有者所有。

## License

[MIT](LICENSE)
