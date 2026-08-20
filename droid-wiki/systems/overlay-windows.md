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
| `recognition-overlay` | 识别触发 | 监听区域、识色探针、自定义点击区域选择叠加窗 |
| 息屏（无 WebView） | 息屏 | 独立线程原生 Win32 视觉遮罩，不进 `?mode=`；只挡画面。息屏打开时识别改走 WGC + `WDA_EXCLUDEFROMCAPTURE` 透视遮罩；关闭时仍走 GDI。禁止在 WebView2 GUI 线程建窗 |

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
3. `PositionMoveQueue` 用 `requestAnimationFrame` 合并同帧坐标；最多一个 `xxx_position_moved` in-flight，调用使用 `log: false`
4. mouseup / Enter / Escape 先 `flush()` 最终坐标；`xxx_position_commit`（Enter 键）不会越过尚未完成的 moved invoke
5. `xxx_position_cancel`（Escape 键）放弃并关闭

位置状态机的核心逻辑在 [同步工具基座](sync-tool.md) 的 `apply_position_event` 中实现。

### 阶段 4 热路径对比（2026-07-16）

| 场景 | 改动前 | 改动后 |
|------|--------|--------|
| Rapidfire 1ms 更新 10 秒 | 10,000 次 count 更新逐次发完整 Bootstrap，结束再发 1 次 | 普通事件不超过 600 次，结束 final 额外 1 次；减少至少 94.0% |
| 同一帧 500 次位置 move | 500 次 `position_moved` invoke + 1,000 条 start/success 日志 | 1 次 invoke，0 条 start/success 日志 |
| Timer 代表性 tick payload | 798B 完整 Bootstrap | 273B runs payload，减少 65.8% |
| Timer debug 序列化 CPU proxy（100,000 次） | 7303ms | 2488ms，减少 65.9% |

CPU 数据是同机 debug 构建的 serde 序列化 microbenchmark，不等同于端到端整机 CPU profiler；它只量化本阶段移除 settings clone/序列化后的热点差异。

## 区域选择叠加窗

Morse 和识别触发使用全屏透明叠加窗（`morse-overlay` / `recognition-overlay`）拖拽选择屏幕区域。通过 `?mode=overlay`（Morse）或 `?mode=recognition-overlay`（识别触发）进入。

## 共享组件

前端跨工具复用 overlay 基础设施：

| 组件 | 文件 | 用途 |
|------|------|------|
| `SyncOverlayWindow` | `src/components/app/sync-overlay-window.tsx` | 计时器/计数器/连发器共享的显示/位置窗口包装 |
| `PositionMoveQueue` | `src/components/ui/position-move-queue.ts` | rAF latest-point 合并、单 in-flight 与最终坐标 flush barrier |
| `MorseOverlay` / `RegionSelectionOverlay` | `src/components/app/morse-overlay.tsx` | Morse 区域选择全屏叠加窗 |
| `RecognitionRegionOverlay` | `src/components/app/recognition-page.tsx` | 识别触发区域/探针/点击区域选择叠加窗 |

## 集成点

- [计时器](../features/timer.md)、[计数器](../features/counter.md)、[连发器](../features/rapidfire.md) 各自拥有显示窗口和位置窗口
- [Morse](../features/morse.md) 拥有区域选择叠加窗
- [识别触发](../features/recognition.md) 共享 overlay 流程用于监听区域、探针和点击区域选择
- `src-tauri/src/overlay_utils.rs` 提供共享的叠加窗创建与尺寸计算工具函数
- settings/结构变化通过 `state-changed`，运行态通过轻量 `runs-changed` 同时发到 `main` 和显示窗口

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
