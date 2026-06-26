# 透明叠加窗

计时器、计数器和连发器各自使用透明叠加窗口浮在游戏上方，显示实时数据但不阻挡点击。这些窗口通过 Tauri 的 WebviewWindow API 创建，具有透明、置顶、点击穿透等特性。

## 窗口 label

| Label | 工具 | 用途 |
|-------|------|------|
| `timer-display` | 计时器 | 显示计时器卡片的倒计时/正计时与进度 |
| `timer-position` | 计时器 | 拖拽校准显示位置的窗口 |
| `counter-display` | 计数器 | 显示计数器当前值 |
| `counter-position` | 计数器 | 位置校准窗口 |
| `rapidfire-display` | 连发器 | 显示触发键到目标键映射与开火状态 |
| `rapidfire-position` | 连发器 | 位置校准窗口 |
| `morse-overlay` | Morse | 全屏透明区域选择叠加窗 |
| `audio-overlay` | 音频 | 区域/探针选择叠加窗 |

## 显示窗口

显示窗口（`*-display`）创建时具有以下属性：

- 透明背景（不套用主窗口的纸面风格）
- 无边框（no decorations）
- 始终置顶（always on top）
- 不显示在任务栏（skip taskbar）
- 点击穿透（鼠标事件传递给下方窗口）
- 可调宽度（计时器/计数器最小 320px，连发器 320-800px）
- 高度按启用的卡片数计算

前端通过 `src/App.tsx` 中的 `?mode=*-display` 查询参数渲染这些窗口，提前返回 overlay 内容而非主壳层。

## 位置窗口

位置窗口（`*-position`）是小型校准风格窗口，用户拖拽以定位显示窗口。可使用靶心/十字线视觉风格。通过 `?mode=*-position` 进入。

拖拽流程：

1. 前端调用 `xxx_begin_position_selection` 打开位置窗口
2. 用户拖拽窗口到目标屏幕位置
3. 拖拽过程中触发 `xxx_position_moved`，存储临时坐标
4. `xxx_position_commit`（Enter 键）保存坐标到设置并关闭窗口
5. `xxx_position_cancel`（Escape 键）放弃并关闭

位置状态机的核心逻辑在 [同步工具基座](sync-tool.md) 的 `apply_position_event` 中实现。

## 区域选择叠加窗

Morse 和音频使用全屏透明叠加窗（`morse-overlay` / `audio-overlay`）拖拽选择屏幕区域。通过 `?mode=overlay`（Morse）或 `?mode=audio-overlay`（音频）进入。使用 `oneshot` channel 将完成结果回传给调用方。

## 共享组件

前端跨工具复用 overlay 基础设施：

| 组件 | 文件 | 用途 |
|------|------|------|
| `SyncOverlayWindow` | `src/components/app/sync-overlay-window.tsx` | 计时器/计数器/连发器共享的显示/位置窗口包装 |
| `MorseOverlay` / `RegionSelectionOverlay` | `src/components/app/morse-overlay.tsx` | Morse 区域选择全屏叠加窗 |
| `AudioRegionOverlay` | `src/components/app/audio-page.tsx` | 音频区域/探针选择叠加窗 |

## 集成点

- [计时器](../features/timer.md)、[计数器](../features/counter.md)、[连发器](../features/rapidfire.md) 各自拥有显示窗口和位置窗口
- [Morse](../features/morse.md) 拥有区域选择叠加窗
- [音频触发器](../features/audio.md) 共享 overlay 流程用于区域/探针选择
- `src-tauri/src/overlay_utils.rs` 提供共享的叠加窗创建与尺寸计算工具函数
- 状态变更同时 emit 到 `main` 和显示窗口 label，使 overlay 实时更新

## 修改入口

- 新增工具的显示窗口：在工具模块中调用 `WebviewWindowBuilder` 创建透明窗口，设置 `?mode=` 查询参数
- 修改窗口样式：调整 `WebviewWindowBuilder` 的 flag 链
- 修改位置校准流程：复用 [同步工具基座](sync-tool.md) 的 `apply_position_event`

## 关键源文件

| 文件 | 用途 |
|------|------|
| `src-tauri/src/overlay_utils.rs` | 共享 overlay/位置窗口创建工具函数 |
| `src/components/app/sync-overlay-window.tsx` | 共享前端显示/位置窗口组件 |
| `src/App.tsx` | `?mode=` 查询参数分支进入 overlay 窗口 |
