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
| `RecognitionSettings` | 总开关 `recognition_enabled` + 卡片列表，落盘为 `recognition_settings.json` |
| `RecognitionCard` | 触发来源、激活方式、效果配置、冷却、识色探针 |
| `RecognitionActivation` | RegionWatch / ColorWatch 的激活方式：`always` / `onceHotkey` / `timedHotkey` |
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
| `timedHotkey` | 按激活快捷键后在 `durationMs` 内识别，成功一次即停 |

## 效果执行

`effects::execute` 在锁内构建执行计划，释放锁后按固定顺序执行：

1. 音频效果入队到 `player::AudioCommand::Play`
2. 按键效果通过共享 `input_simulation::press_hotkey_once`
3. 点击效果通过共享 `input_simulation::click_points`

点击目标规则：

- `customRegion`：点击自定义区域中心。
- RegionWatch `recognitionRegion`：点击模板命中中心。
- ColorWatch `recognitionRegion`：只在显式选择的 probe 命中时点击；未命中则跳过点击，其他效果照常执行。

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
- Overlay mode：`?mode=recognition-overlay`。
- 持久化：`recognition_settings.json`；旧 `audio_settings.json` 自动迁移。
- Profile snapshot：字段 `recognition`；旧 `audio` 字段通过 serde alias 迁移。
