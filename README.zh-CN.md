# WorkLouderCTL：Work Louder Input 与 Codex Micro 的 CLI

**覆盖 Codex App 与 Work Louder Input 全部配置能力的开源 CLI。**

[English](README.md) · [常见问题](docs/faq.md) ·
[兼容性](docs/compatibility.md) · [架构](docs/architecture.md) ·
[路线图](docs/roadmap.md)

> **当前状态：source alpha。** 仓库现在可构建真实的 `worklouderctl`：已经支持
> provider 诊断、Codex Micro 设置检查/导出、Input 只读检查/原字节导出、
> live device status/files、双哈希验证导出、带 revision 的 bridge snapshot、
> live CAS 校验、离线 profile/layer/control/Action candidate 生成、fixture 验证的
> apply/restore transaction、结构化 diff、JSON 输出和 shell completion。
> 尚未发布打包版本；bridge transaction 已通过隔离 writer fixture
> 验证，真实设备写入仍以 Input writer adapter 与硬件 rollback 验证为启用条件。

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
CLI 不实现新的键盘 driver、BLE/HID stack、firmware protocol 或 host-action
runtime，而是调用当前已安装的 Codex/Input provider，从而继续获得上游更新。

```text
读取当前状态 → 生成计划与 diff → 备份 → 写入 → readback 验证 → 同步 Input → 恢复或完成
```

## 构建和使用当前 CLI

最低 Rust 版本为 1.61：

```console
git clone https://github.com/MarlinDiary/worklouder-input-cli.git
cd worklouder-input-cli
cargo build --release --locked
./target/release/worklouderctl doctor
```

当前已经实现：

```console
worklouderctl version
worklouderctl tier list
worklouderctl tier explain 1
worklouderctl capability list --tier 2
worklouderctl doctor [--strict]
worklouderctl codex doctor [--strict]
worklouderctl codex inspect
worklouderctl codex export --output CODEX_SNAPSHOT.json
worklouderctl input inspect [--device DEVICE_ID]
worklouderctl input export --output BACKUP_DIRECTORY
worklouderctl bridge status
worklouderctl device --transport bridge status
worklouderctl device --transport bridge files --recursive
worklouderctl device --transport bridge export --output DEVICE_BACKUP
worklouderctl device --transport bridge config snapshot --output CONFIG.json
worklouderctl device --transport bridge config validate --input CONFIG.json
worklouderctl device --transport bridge config apply --input CONFIG.json --backup BEFORE.json
worklouderctl device --transport bridge config restore --input BEFORE.json --backup CURRENT.json
worklouderctl profile list --input CONFIG.json
worklouderctl profile show --input CONFIG.json --id PROFILE_ID
worklouderctl profile select --input CONFIG.json --id PROFILE_ID --output CANDIDATE.json
worklouderctl profile rename --input CONFIG.json --id PROFILE_ID --name NAME --output CANDIDATE.json
worklouderctl layer list --input CONFIG.json [--profile PROFILE_ID]
worklouderctl layer show --input CONFIG.json [--profile PROFILE_ID] --id LAYER_ID
worklouderctl layer rename --input CONFIG.json [--profile PROFILE_ID] --id LAYER_ID --name NAME --output CANDIDATE.json
worklouderctl layer color --input CONFIG.json [--profile PROFILE_ID] --id LAYER_ID --color '#RRGGBB' --output CANDIDATE.json
worklouderctl control list --input CONFIG.json [--profile PROFILE_ID] --layer LAYER_ID
worklouderctl control show --input CONFIG.json [--profile PROFILE_ID] --layer LAYER_ID --control key:ROW:COLUMN
worklouderctl control set --input CONFIG.json [--profile PROFILE_ID] --layer LAYER_ID --control encoder:INDEX:press --assignment KC_MUTE --output CANDIDATE.json
worklouderctl action list --input CONFIG.json
worklouderctl action show --input CONFIG.json --id ACTION_ID
worklouderctl action create --input CONFIG.json --name NAME --output CANDIDATE.json
worklouderctl action rename --input CONFIG.json --id ACTION_ID --name NAME --output CANDIDATE.json
worklouderctl action event add --input CONFIG.json --id ACTION_ID --assignment KC_C --type press --delay 0 --output CANDIDATE.json
worklouderctl action event set --input CONFIG.json --id ACTION_ID --index 0 --assignment KC_C --type click --delay 200 --output CANDIDATE.json
worklouderctl action event delete --input CONFIG.json --id ACTION_ID --index 0 --output CANDIDATE.json
worklouderctl action event move --input CONFIG.json --id ACTION_ID --from 1 --to 0 --output CANDIDATE.json
worklouderctl action delete --input CONFIG.json --id ACTION_ID --output CANDIDATE.json
worklouderctl action group list --input CONFIG.json
worklouderctl action group create --input CONFIG.json --name NAME --action ACTION_ID --output CANDIDATE.json
worklouderctl action group member move --input CONFIG.json --id GROUP_ID --from 1 --to 0 --output CANDIDATE.json
worklouderctl action group delete --input CONFIG.json --id GROUP_ID [--keep-members] --output CANDIDATE.json
worklouderctl multi-action list --input CONFIG.json
worklouderctl multi-action show --input CONFIG.json --id MULTI_ACTION_ID
worklouderctl multi-action create --input CONFIG.json --name NAME --output CANDIDATE.json
worklouderctl multi-action set --input CONFIG.json --id MULTI_ACTION_ID --tap KC_A --double-tap KC_B --hold KC_C --tap-hold KC_D --tapping-term 250 --output CANDIDATE.json
worklouderctl multi-action delete --input CONFIG.json --id MULTI_ACTION_ID --output CANDIDATE.json
worklouderctl multi-action group create --input CONFIG.json --name NAME --multi-action MULTI_ACTION_ID --output CANDIDATE.json
worklouderctl multi-action group delete --input CONFIG.json --id GROUP_ID [--keep-members] --output CANDIDATE.json
worklouderctl device --transport direct --input-mode require-closed status
worklouderctl config validate BACKUP_DIRECTORY
worklouderctl config diff BASE CANDIDATE
worklouderctl --json input inspect
worklouderctl completion bash|zsh|fish
```

`codex inspect` 只读取 Codex `config.toml` 中 `[desktop]` 表的五个
`codex-micro-*` 设置，按照 Codex 26.727.51351 的冻结契约校验，并在 effective
view 中递归补齐继承的默认值。`codex export` 原子发布并重新打开 typed JSON
snapshot；两者都不会序列化其他 Codex 设置。

`input inspect` 同样全程只读；`input export` 把源文件原字节复制到原子发布的
目录，并在 `manifest.json` 记录 size 与 SHA-256。它不会暂停 Input，也不会
写入设备。

`device` 的首选 transport 是
[Input Companion Bridge](docs/companion-bridge.md)：CLI 通过私有 Unix socket
认证，由正在运行的 Input main process 使用现有 device session 执行请求。
`--transport auto` 会在 socket 和 token 出现后优先选择 bridge。

目前已发布的 Input 0.18.0 尚未包含 bridge，因此
`--transport direct` 保留为只读兼容路径：它复用已安装 Input 内置的 device
kit，不附带第二套 driver。`--input-mode require-closed` 只报告占用状态；
显式选择 `--input-mode restart` 才会请求 Input 优雅退出并在读取后重新打开，
且不执行 force-kill。
`device export` 对每个文件核对 device SHA-1 与 host SHA-256，重新读取
typed manifest 和文件后，再原子发布目录。
bridge 专用的 `device config snapshot` 还会保存精确 base64 字节和确定性
revision；`device config validate` 会重算 size、双哈希与 revision，配合
`--expected-revision REVISION` 可对 live device 做只读 CAS 预检。
`device config apply/restore` 会创建或复用 immutable backup，并由 Input 执行
CAS、session-scoped idempotency、完整 revision readback 与 automatic rollback。
只有运行中的 Input 注入已验证 writer 时，bridge 才会公布这两个写能力；当前
跨语言证据来自隔离 reference writer。

`profile`、`layer`、`control`、`action` 与 `multi-action` 是离线 semantic editor：先严格验证 snapshot 内每个
size、SHA-1、SHA-256、canonical base64、safe path、keymap ID 与完整 revision，
再只修改请求的 semantic field，保留未知 JSON 字段和其他文件的原字节，重算
受影响的哈希与 revision，原子发布并重新打开 candidate。这个阶段不连接 Input
或设备；candidate 再交给现有 `device config apply` transaction。

物理 control 使用稳定 ID：`key:ROW:COLUMN`、
`encoder:INDEX:ccw|cw|press`、`joystick:SECTOR`。`control set` 校验冻结的
Input 0.18.0 assignment grammar，支持 `KC_*`、`KI_*` 以及已存在的
`KA_A<ID>` Action / `KA_M<ID>` Multi Action 引用。现有 `KV_*` vendor token
按 read-only 类型读取和保留；可写 assignment 来自 catalog 与有效引用。引用发生变化时，candidate
会按照 Input 的顺序同步 `macrosUsed` 与 `multiActionsUsed`，再完成全 snapshot
rehash、原子发布和 reopen readback。

`action` 已冻结 Input 0.18.0 的 Action 模型：ID 采用相同的 last-ID-plus-one
分配规则；event 保留有序的 `release(0)`、`press(1)`、`click(2)` 与
`0..9999 ms` delay；新 Action 使用 Input 默认的 `KC_NONE` press event。
删除采用完整 cascade，同步处理 layer controls、其他 Action events、Multi Action
branches、groups 与 profile `macrosUsed`，每个被移除的引用落为 `KC_NONE`。

`action group` 与 `multi-action group` 已覆盖 list/show/create、name/color/tags
更新、有序 member add/remove/move 和 delete。默认 group delete 与 Input 0.18.0
一致：只属于该 stored group 的 member 会连同完整引用 cascade 一起删除；共享 member
保留。`--keep-members` 只移除 group container。Group ID 按实际观察到的
maximum-ID-plus-one 规则分配。

`multi-action` 已覆盖 `tap`、`double-tap`、`hold`、`tap-hold` 四个 assignment，
以及 name、color、icon、tapping term。新 Multi Action 使用四个 `KC_NONE` 与
`250 ms` 默认值；删除时同步清理 physical controls、Action events、嵌套 Multi
Action、groups 和 profile usage 引用。

仓库已经包含可执行的 Input-main reference server、Input 0.18.0 service
adapter、认证测试，以及 Rust CLI 跨语言 conformance test：

```console
node --test companion/input-main-bridge.test.mjs
./scripts/test-bridge-e2e.sh
```

## 其余计划中的写入命令

```console
worklouderctl codex agent-source set priority
worklouderctl codex command-key set ACT06 --command toggleFastMode
worklouderctl codex joystick set up --skill SKILL_ID
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

独立 driver/runtime 明确不属于本项目目标；CLI 只替代配置入口和配置工作流。

## 当前进度

仓库目前处于 transaction source-alpha 阶段。Codex TOML read adapter 的
`doctor`、`inspect`、`export`，以及 Input 0.18.0 live device 的 `status`、
`files`、双哈希 `export`、Companion Bridge v1 contract/client/reference server、
revisioned config snapshot、live CAS 预检、fixture apply/restore/rollback 和
Input 自动恢复，以及 profile list/select/rename 与 layer list/rename 的严格离线
candidate 生成、profile/layer show、layer 24-bit RGB color candidate，keys、
encoder gestures、已有 joystick sectors 的 control list/show/set，以及 Action
list/show/create/rename/delete 和 event add/set/delete/move candidate 已经实现；Input
release 集成、可安装 binary、Homebrew formula、
Codex live settings bridge 写入与完整写入命令将在对应 transaction 和真实硬件
readback 验证后发布。

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
