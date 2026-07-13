# Issues #81-#83 识别触发修复设计

## 目标

本设计修复三个识别触发问题：

1. #81：识色 `anyPixel` 点击效果应点击实际命中像素，而非 probe 区域中心。
2. #82：禁用卡片或禁用分组中的未完成草稿不得阻断其他卡片保存。
3. #83：`always` 常驻识图/识色只在目标从未命中变为命中时触发；目标持续存在时不得重复触发，明确消失后才重新武装。

不新增持久化字段，不改 Tauri command、事件名、查询参数、原生窗口 label 或 Profile 数据格式。

## 根因

### #81 命中坐标丢失

`scan_region_for_color` 只返回命中数量、最近颜色和颜色距离。`match_color_probes` 继续把结果压缩为命中 probe 索引。Watcher 构建 `TriggerContext::Color` 时只能使用 probe 区域中心，因此 `effects::click_point_for_effect` 无法取得实际命中像素。

### #82 禁用草稿仍被严格校验

前端 `parseSettingsForm` 对全部卡片执行快捷键、激活快捷键和效果完整性校验。Rust `validate_settings` 同样遍历全部卡片。两层都未区分运行态卡片与禁用草稿，任一禁用草稿缺少必填项都会使整份 settings 保存失败。

### #83 常驻 watcher 使用电平触发

RegionWatch 和 ColorWatch 常驻循环虽然按 `watchPollIntervalMs` 截图，并通过 `cooldownMs` 限流，但匹配持续为真时会在每次冷却结束后再次触发。当前循环没有记录上一次匹配状态，也没有等待目标消失后重新武装。

## 方案选择

采用局部修复方案：扩展现有颜色匹配结果、按运行态过滤严格校验、为两个常驻 watcher 增加最小边沿状态。

不采用统一 `TriggerGate` 重构。该抽象会同时触及 activation session，超出 #81-#83 范围。不采用 Combo 效果层抑制，因为重复触发来自 watcher，且按键、点击效果也必须遵守相同触发语义。

## #81 实际命中像素点击

### 匹配结果

`PixelScanResult` 增加最近像素局部坐标。AnyPixel 扫描记录容差内颜色距离最小的像素：

- 非零最小距离需要完成区域扫描，确保选择全局最小值。
- 距离为 `0` 时已达到理论最小值，可以立即结束。
- 多个像素距离相同时保留扫描顺序中的第一个，保证结果确定。

坐标继续通过单目标结果、probe 聚合结果和 `ColorMatchResult` 向上传递。probe 内配置多个目标颜色时，只从已命中的目标中选择颜色距离最小者：`Any` 和 `All` 使用相同选点规则。

### 屏幕坐标

Watcher 将局部像素坐标加上 probe 区域原点，生成屏幕绝对坐标，再写入 `TriggerContext::Color`。点击效果仍按 `color_probe_index` 选择指定 probe。

- `anyPixel`：点击与目标色距离最小的实际命中像素。
- `average`：没有单像素位置，保持点击 probe 区域中心。
- 指定 probe 未命中或没有坐标：跳过点击，其他效果继续执行。

## #82 禁用草稿保存

### 运行态判定

卡片同时满足以下条件才执行运行必填校验：

- `card.enabled == true`
- 所属分组存在且 `group.enabled == true`；旧配置或缺失分组按既有 normalize 规则归入默认启用分组

### 前端解析

`parseSettingsForm` 先建立分组启用状态，再把 `strictRuntimeValidation` 传给 `parseCardForm`。

所有卡片仍需满足可序列化的基本结构与数值范围，例如名称、轮询间隔、阈值、冷却和音量必须可解析。只有运行必填项在禁用草稿中放宽：

- Hotkey 来源可保存空触发快捷键。
- 非 Always activation 可保存空激活快捷键。
- 已打开但未配置完成的效果可保留草稿数据。
- 可暂时没有可执行效果。

用户启用卡片或所属分组时恢复严格校验；不完整配置继续阻止保存并显示现有错误。

### Rust 兜底

`validate_settings` 只对运行态卡片执行快捷键、activation、效果完整性和可执行效果数量校验。Rust 层必须保留该限制，防止绕过前端直接调用 command 写入启用但不可运行的配置。

## #83 常驻识别边沿触发

### 状态转换

RegionWatch 和 ColorWatch 的 `always` 循环各维护 `was_matched`：

| 上轮状态 | 本轮结果 | 行为 |
|---|---|---|
| 未命中 | 未命中 | 保持待命 |
| 未命中 | 命中 | 形成上升沿；检查 cooldown 后决定是否触发，并进入已命中状态 |
| 已命中 | 命中 | 不触发，保持已命中状态 |
| 已命中 | 未命中 | 重新武装 |

截图失败不是明确未命中，不改变 `was_matched`。效果执行失败仍消费本次上升沿，避免每个 poll 重试并形成错误风暴。

### interval 与 cooldown

每个 `watchPollIntervalMs` 都执行一次截图和匹配。不能像当前实现一样在 cooldown 未结束时跳过截图，否则 watcher 无法观察目标消失与重新出现。

`cooldownMs` 只限制不同上升沿之间的最短触发间隔。若新目标在 cooldown 内出现，该上升沿被抑制，但 watcher 仍进入已命中状态；目标必须再次消失并重新出现，才产生下一次上升沿。

本次只修改 `always` 常驻 watcher。`onceHotkey` 和 `timedHotkey` activation session 不在 #83 范围内。

Single、Random、Combo 共用上述触发语义。不能只在 Combo 音频效果中抑制重复，否则同一触发源下不同效果会产生不一致行为。

## 错误处理与兼容性

- 缺少点击坐标时只跳过点击效果，不阻断音频或按键效果。
- 禁用草稿保持可编辑、可持久化；启用时进行完整校验。
- 截图失败保持匹配锁定状态，不把采集故障解释为目标消失。
- 不新增 serde 字段，不需要配置迁移。
- 不改现有命令注册、capability、事件 payload 或前端类型的持久化字段。

## 测试设计

### 前端 Vitest

- 禁用卡片缺触发快捷键和音频文件时可通过 `parseSettingsForm`。
- 禁用分组中的启用卡片允许保存未完成草稿。
- 同一卡片或分组启用后恢复原有错误。
- 数值范围校验在禁用草稿中仍生效。

### Rust 单元测试

- AnyPixel 单像素命中返回局部坐标。
- 多个容差内像素选择颜色距离最小者。
- 相同距离使用稳定扫描顺序。
- probe 多目标 `Any`/`All` 从已命中目标中选择最小距离坐标。
- Color context 使用绝对命中坐标；Average 使用区域中心。
- 禁用卡片和禁用分组允许不完整草稿；启用配置仍拒绝。
- watcher 状态转换覆盖四种组合。
- 截图失败不重新武装。
- 持续命中不重复触发。
- 明确未命中后重新出现产生新上升沿。
- cooldown 内上升沿被抑制，但 poll 和状态更新继续执行。

### 完整验证

```powershell
bun run test
bun run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
codegraph sync
```

## 开发步骤

1. 添加 #81-#83 回归测试，先证明当前行为失败。
2. 修改前端解析与测试，支持禁用草稿。
3. 修改 Rust validation 与测试，按运行态卡片严格校验。
4. 扩展颜色扫描、单目标、probe 聚合和总匹配结果的坐标数据流。
5. 将局部坐标转换为屏幕坐标并接入点击效果；保留 Average 中心回退。
6. 提取可单测的 watcher 边沿状态转换。
7. 调整 RegionWatch、ColorWatch 常驻循环：始终按 interval 匹配，再执行边沿与 cooldown 门控。
8. 更新 `droid-wiki/features/recognition.md`。
9. 运行完整验证与 `codegraph sync`。
10. 从实际 diff 提炼中文 commit。Issue 回复处理结论后等待用户确认，不直接关闭。

## 预计改动文件

- `src/components/app/recognition-utils.ts`
- `src/components/app/recognition-utils.test.ts`
- `src-tauri/src/recognition/mod.rs`
- `src-tauri/src/recognition/effects.rs`
- `src-tauri/src/recognition/watcher/matching.rs`
- `src-tauri/src/recognition/watcher/manager.rs`
- `droid-wiki/features/recognition.md`
