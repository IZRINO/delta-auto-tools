# 识别触发

## 目的

识别触发（Recognition）把「触发来源」和「触发效果」拆开：卡片可以由快捷键、区域图像匹配或多区域识色触发，再按固定顺序执行音频、按键、点击效果。

典型用途：屏幕状态变化后播放提示音、按下指定快捷键、点击命中位置或自定义区域中心。

## 目录结构

```text
src-tauri/src/recognition/
├── mod.rs              # RecognitionLogic、Tauri commands、热键注册、settings normalize
├── types.rs            # RecognitionSettings / RecognitionCard / activation / effects / probes
├── settings.rs         # recognition_settings.json 读写；旧 audio_settings.json 迁移
├── effects.rs          # 音频→按键→点击效果执行器
├── player.rs           # rodio 音频播放线程
├── events.rs           # recognition://* 事件常量
└── watcher/
    ├── manager.rs      # watcher 生命周期、activation session
    ├── matching.rs     # 图像/颜色匹配
    └── capture.rs      # 截图和参考图读取
```

## 数据模型

| 类型 | 职责 |
|------|------|
| `RecognitionSettings` | 总开关 `recognition_enabled` + `card_groups` + 卡片列表，落盘为 `recognition_settings.json` |
| `RecognitionGroup` | 识别卡片分组，包含 `id`、`name`、`order`、`collapsed`、`enabled`；旧配置会补 `default-recognition-group` 且旧分组默认启用 |
| `RecognitionCard` | 分组归属 `group_id`、排序 `order`、触发来源、激活方式、效果配置、冷却、识色探针 |
| `RecognitionActivation` | RegionWatch / ColorWatch 的激活方式：`always` / `onceHotkey` / `timedHotkey`；Hotkey 来源不使用 activation |
| `RecognitionEffects` | 每卡最多一个音频效果、一个按键效果、一个点击效果 |
| `RecognitionAudioEffect` | 音频文件、Single/Combo/Random、音量、并发策略 |
| `RecognitionClickEffect` | 自定义区域中心或识别命中中心；ColorWatch 需显式选择 probe |

旧 `audioEnabled`、`audioFiles`、`playMode`、`volume`、`comboWindows`、`allowSimultaneous` 会在 `normalize_settings` 中迁移到新字段。

## 触发来源

1. **Hotkey**：`restart_hotkey_listeners` 在 scope `"recognition"` 注册触发热键，命中后调用 `effects::spawn_execute(...TriggerContext::Hotkey)`。
2. **RegionWatch**：`watcher::run_region_watcher` 轮询截图，与参考图做 RGB NCC 模板匹配；命中后传入模板中心坐标。
3. **ColorWatch**：`watcher::run_color_watcher` 轮询 probe，按 Average 或 AnyPixel 匹配目标色；命中后传入命中 probe 的中心坐标。

RegionWatch / ColorWatch 可选激活方式：

| 模式 | 行为 |
|------|------|
| `always` | 无激活快捷键，持续识别 |
| `onceHotkey` | 按激活快捷键后识别一次 |
| `timedHotkey` | 按激活快捷键后在 `durationMs` 内识别，命中 `triggerCount` 次或超时后停止 |

Hotkey 来源表示“快捷键直接触发效果”，不展示 activation 配置；`onceHotkey` / `timedHotkey` 只表示“快捷键激活区域/识色识别窗口”。

## 效果执行

`effects::execute` 在锁内构建执行计划，释放锁后按固定顺序执行：

1. 音频效果入队到 `player::AudioCommand::Play`
2. 按键效果通过共享 `input_simulation::press_hotkey_once`
3. 点击效果通过共享 `input_simulation::click_points`

点击目标规则：

- `customRegion`：点击自定义区域中心。
- `customRegion` 支持草稿态：启用点击效果但尚未框选区域时，配置可保存，便于打开框选 overlay；实际触发时没有点击坐标，会跳过点击。
- RegionWatch `recognitionRegion`：点击模板命中中心。
- ColorWatch `recognitionRegion`：只在显式选择的 probe 命中时点击；`anyPixel` 点击该 probe 内与目标色距离最小的实际命中像素，`average` 点击 probe 区域中心。指定 probe 未命中时跳过点击，其他效果照常执行。

## Tauri Commands

| 命令 | 作用 |
|------|------|
| `recognition_get_bootstrap` | 返回 settings + hotkey error |
| `recognition_save_settings` | normalize → 写盘 → 更新内存 → 重启热键/watcher → emit state → 更新 Profile |
| `recognition_set_hotkey_recording` | 热键录制期间暂停/恢复 recognition scope |
| `recognition_begin_region_selection` | 打开 `recognition-overlay-{cardId}` 框选监听区域、probe 区域或自定义点击区域 |
| `recognition_overlay_submit_selection` | 提交框选区域并重启 watcher |
| `recognition_overlay_cancel_selection` | 取消并关闭 overlay |
| `recognition_test_play` | 测试当前音频效果 |
| `recognition_test_match` | RegionWatch 匹配测试 |
| `recognition_test_color_match` | ColorWatch 匹配测试 |
| `recognition_read_reference_image` | 读取参考图 data URL |

## 事件

| 事件 | 说明 |
|------|------|
| `recognition://state-changed` | settings/bootstrap 更新 |
| `recognition://hotkey-triggered` | 快捷键触发效果执行完成 |
| `recognition://region-matched` | RegionWatch / ColorWatch 命中 |
| `recognition://hotkey-error` | 效果执行或热键错误 |

## 集成点

- 热键 scope：`recognition`，冲突策略 `ConflictPolicy::AllowHold`。
- Recognition 同一 scope 内允许多个启用卡片复用同一个监听热键；命中后会触发所有匹配卡片。
- Overlay mode：`?mode=recognition-overlay`。
- 持久化：`recognition_settings.json`；旧 `audio_settings.json` 自动迁移。
- Profile snapshot：字段 `recognition`；旧 `audio` 字段通过 serde alias 迁移。
## 当前行为补充

- `always` 常驻 RegionWatch / ColorWatch 按 `watchPollIntervalMs` 持续检查，但只在“未命中 → 命中”上升沿执行效果；目标持续命中不会重复触发，明确未命中后重新武装。截图失败不视为目标消失，`cooldownMs` 继续限制不同上升沿的最短触发间隔。
- 禁用卡片或禁用分组中的卡片可保存未完成草稿；重新启用时恢复快捷键、激活方式和效果完整性校验。
- `RecognitionActivation` 的 `timedHotkey` 支持 `triggerCount`，默认 `1`；会话在限时内命中 N 次或超时后结束。
- `RecognitionHotkeyEffect` 支持 `steps: [{ hotkey, delayMs }]` 序列；旧 `{ hotkey }` 配置会迁移为单步序列。
- 识别触发的监听热键、激活热键和按键效果热键支持字母、数字、F1-F24、方向键，以及 `,`、`.`、`;`、`/`、`\`、`[`、`]`、`-`、`=`、`+`、`` ` ``、`'` 等符号；配置以 ASCII 物理键持久化，录制中文/全角标点时会归一到对应物理键，例如 `，` -> `,`、`。` -> `.`。
- 按键效果执行顺序为 audio 入队、hotkey steps 逐步执行、click effect；每个 step 的 `delayMs` 在对应 hotkey 执行前等待。
- 全局开关关闭时，识别 scope 的热键与 RegionWatch / ColorWatch watcher 都被全局门控拦截；Recognition 页面会显示“全局开关关闭，识别触发不会响应”。
- 按键效果 step 属于输出动作，不参与 output-output 重复冲突校验；同一卡片或不同卡片可以复用输出按键。为防递归，step 不得等于任意已注册监听热键（Hotkey 触发热键或 RegionWatch / ColorWatch 激活热键）。
- 卡片支持分组、跨分组移动、组内排序和折叠。分组持久化字段为 `cardGroups`，卡片持久化字段为 `groupId` 和组内 `order`；跨分组移动后源分组和目标分组分别归一 `order`。
- 分组 `enabled=false` 时，组内卡片仍保留并可编辑，但不会注册 Hotkey listener、RegionWatch watcher、ColorWatch watcher，也不会继续执行已排队 activation session 的效果。
- 旧配置缺少分组字段时自动归入 `default-recognition-group`。
- 排查同输出按键问题时查看 `recognition` 日志：`注册识别监听热键` 确认 listener 注册，`准备执行触发效果` 和 `执行按键效果 step` 确认效果链已进入 input simulation。
