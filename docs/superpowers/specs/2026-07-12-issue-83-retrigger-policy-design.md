# Issue #83 识别重复触发策略设计

## 背景

`v0.18.1` 将 `always` 常驻 RegionWatch / ColorWatch 改为上升沿触发：目标持续命中时只触发一次，明确未命中后重新武装。用户反馈该行为不应默认作用于所有卡片，且实际使用中仍会连续触发。

本设计增加卡片级可选策略，并提高“目标消失”判定抗抖能力。

## 目标与非目标

### 目标

1. 为常驻识图/识色增加“目标消失后再触发”开关。
2. 开关默认关闭，旧配置和新卡片保持 cooldown 周期触发行为。
3. 开关开启后，持续命中只触发一次；连续 2 次未命中后才重新武装。
4. RegionWatch 与 ColorWatch 使用相同状态机。
5. 截图失败不计入未命中，不因采集故障重新触发。

### 非目标

- 不改变 `onceHotkey`、`timedHotkey` activation session 语义。
- 不新增 Tauri command、事件名、事件 payload、查询参数或原生窗口 label。
- 不改音频 Single/Combo/Random 的效果语义。
- 不把未命中次数暴露为用户配置项。

## 用户行为

开关字段命名：`retriggerAfterDisappear`，中文标签为“目标消失后再触发”。

开关仅在 `triggerMode` 为 `regionWatch` 或 `colorWatch` 且 `activationMode` 为 `always` 时显示。字段仍随卡片保存；切换到其他 activation mode 时不删除字段。

| 开关 | 持续命中 | 未命中后的重新触发 |
| --- | --- | --- |
| 关闭（默认） | 每次 cooldown 到期可触发 | 不需要重新武装 |
| 开启 | 同一命中区间只触发一次 | 连续 2 次未命中后，下一次命中可触发 |

轮询始终按 `watchPollIntervalMs` 截图与匹配。cooldown 只控制实际效果触发，不跳过轮询。

## 数据模型与兼容性

### Rust

`RecognitionCard` 增加：

```rust
#[serde(default)]
pub retrigger_after_disappear: bool,
```

字段放在 watcher 配置附近。`RecognitionCard` 的 `Default`、normalize 和序列化保持默认 `false`。旧 JSON 缺字段时由 serde 补 `false`，无需迁移脚本。

### TypeScript

`RecognitionCard`、`RecognitionCardForm`、`DEFAULT_RECOGNITION_CARD` 同步增加字段。`settingsToForm` 对缺失值使用 `false`，`parseSettingsForm` 始终输出布尔值，确保保存后类型稳定。

Profile 继续使用现有 recognition snapshot，不新增外层结构。

## 状态机

扩展现有 `MatchGate`，由每个 watcher 实例独立持有：

```text
CooldownRepeat:
  Matched -> cooldown 到期时触发
  NotMatched / CaptureFailed -> 不改变周期触发语义

AfterDisappear:
  idle + Matched -> 触发，进入 matched
  matched + Matched -> 不触发，保持 matched
  matched + NotMatched -> misses = misses + 1
  misses >= 2 -> idle，重新武装
  任意状态 + CaptureFailed -> 保持当前状态
```

开启策略下，重新出现发生在 cooldown 内也消费本次上升沿但不触发；目标必须再次完成“连续 2 次未命中 → 命中”序列。这样不会在 cooldown 结束时补发旧命中。

效果执行失败仍消费本次触发，不循环重试。

全局开关或识别模块开关关闭时，watcher 暂停轮询，gate 状态保留；恢复后继续当前会话状态。配置保存导致 watcher 重启时视为新监听会话，gate 重新初始化。

## 实现边界

### 前端

- `src/components/app/recognition-types.ts`
  - 增加 card/form 字段和默认值。
- `src/components/app/recognition-utils.ts`
  - 增加双向转换，处理旧配置缺省值。
- `src/components/app/recognition-page.tsx`
  - 在“激活方式”区域增加 Switch，仅 `always` + RegionWatch/ColorWatch 显示。

### 后端

- `src-tauri/src/recognition/types.rs`
  - 增加 serde camelCase 字段和默认值。
- `src-tauri/src/recognition/watcher/manager.rs`
  - 传递策略到两个常驻 watcher。
  - 将 `MatchGate` 扩展为 cooldown 重复触发和消失后重触发两种策略。
  - `NotMatched` 仅在开启策略时累计，连续 2 次才重新武装。
  - `CaptureFailed` 不累计。
- `droid-wiki/features/recognition.md`
  - 更新默认行为、开关语义和重新武装规则。

不修改 `lib.rs` command 注册、capabilities、events、Profile 外层协议。

## 测试设计

### Rust

为 gate 和 watcher 步进逻辑增加测试：

1. 默认关闭时首次命中、cooldown 内抑制、cooldown 到期持续命中可触发。
2. 开启时持续命中只触发一次。
3. 开启时一次未命中不足以重新武装。
4. 开启时连续两次未命中后重新出现可触发。
5. 截图失败不改变命中状态或未命中计数。
6. cooldown 内重新出现不补触发。
7. RegionWatch 与 ColorWatch 都使用策略字段。

### Vitest

1. 缺失字段的 settings 转 form 结果为 `false`。
2. 缺失字段的 form 转 settings 结果为 `false`。
3. `true` / `false` 双向 round-trip 保持一致。
4. `always` 与非 `always` 卡片均保留字段，只有 `always` 常驻识别显示 UI 开关。

### 验收标准

- 旧配置升级后不改变原有 cooldown 周期触发行为。
- 开启开关的卡片持续命中不会重复执行音频、按键或点击效果。
- 单次识别抖动不会重新触发；连续两个轮询周期未命中后再次出现才触发。
- #83 中“依旧会一直触发”场景有回归测试覆盖。
- `bun run test`、`bun run build`、`cargo test --manifest-path src-tauri/Cargo.toml`、`cargo check --manifest-path src-tauri/Cargo.toml` 通过。
- `codegraph sync` 完成，wiki 与代码行为一致。

## 发布与 Issue 流程

实现完成后先运行完整验证，再从实际 diff 提炼中文 commit。回复 #83 说明开关默认关闭、两次未命中重新武装和验证版本；等待用户确认，不直接关闭 Issue。
