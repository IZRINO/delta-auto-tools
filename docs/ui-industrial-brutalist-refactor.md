# 工业粗粝风 UI 重构文档

## 1. 目标

将桌面主界面从当前“战术白色操作台”升级为“Delta 战术工业白图纸控制台”：保留浅色工具属性，吸收工业粗粝风的硬边网格、瑞士工业印刷、高密度仪表和装备清单语义，让摩斯密码解析、计时器、连发器、攻略网站、账号管理、游戏数据与工具箱看起来属于同一个战术工业系统。

本次重构是视觉系统重肤，不改变工具行为、数据流、Tauri command、窗口入口或透明窗口核心约束。

## 2. 不改行为边界

以下内容必须保持现状：

- `src/App.tsx` 中 `overlayWindowModes` 的语义和 early-return 入口。
- 查询参数入口：`?mode=overlay`、`?mode=timer-display`、`?mode=counter-display`、`?mode=timer-position`、`?mode=counter-position`、`?mode=rapidfire-display`、`?mode=rapidfire-position`。
- Tauri command 名称与前端 `invoke` 调用参数。
- 原生窗口 label：`morse-overlay`、`timer-display`、`timer-position`、`counter-display`、`counter-position`、`rapidfire-display`、`rapidfire-position`、`strategy-content`。
- Morse 连续区域框选流程、autosave、热键录制暂停 / 恢复逻辑。
- 计时器 / 计数器独立总开关、运行态、透明窗口显示、位置设置和排序逻辑。
- 连发器 hold 热键状态机、启动抖动、最小间距、补齐触发、不追加策略、透明窗口行为。
- 攻略网站 `strategy-content` 子 WebView 的创建、销毁、bounds 同步、刷新和卸载清理生命周期。
- Delta 账号凭据边界、账号能力过滤、分批加载和版本号机制。

## 3. 视觉方向

采用“Delta 战术工业白图纸控制台”，不是全黑 CRT 终端，也不是营销页或普通后台。

核心语言：

- 浅色纸面：Canvas `#E8E4D8`，像战术地图和维护手册。
- 碳黑文字：Carbon Ink `#11120F`，避免纯黑。
- 硬边网格：1px / 2px 结构线、角标、分隔线、机械表格感。
- 工业高密度：小字号等宽元信息、紧凑行距、状态矩阵、装备清单式布局。
- Delta 语义：战术撤离、干员、武器库、载具、Operations / Warfare 的控制台感，只作为视觉隐喻，不新增功能。
- 单一通用强调色：Delta Hazard Orange `#C65A1E`。错误红只用于 destructive/error；令牌状态色只用于语义状态。

## 4. 开发顺序

### 4.1 文档确认

先写并确认两个文档：

- `docs/ui-industrial-brutalist-refactor.md`：范围、顺序、边界、验收。
- `docs/DESIGN.md`：语义设计系统、token、组件、布局、禁用项。

当前用户已确认执行计划，因此代码阶段可在文档落地且内容验证后继续。

### 4.2 全局 token

修改 `src/App.css`：

- 替换根色板为工业白图纸 token。
- 统一字体栈：显示 / 正文使用窄体工业 sans，数字和元信息使用 `JetBrains Mono Variable`。
- body 背景改为浅色纸面 + 机械网格 + 弱扫描线 / 蓝图结构线。
- 保留 `body[data-overlay-mode="true"]` 透明例外。
- 保留 `favorite-highlight`，但将 pulse 色改为唯一强调色或主 token。

### 4.3 共享视觉组件

集中修改 `src/components/app/app-ui.tsx`，避免页面级复制：

- `AppPage`：统一页面 padding、最大宽度、工业背景承载。
- `PageHero`：改为硬边图纸标题区，使用等宽 eyebrow、角标和信号指标。
- `SignalTile`：改为小型仪表格，数值等宽，图标像设备标记。
- `TacticalCard`：硬边 1px/2px border、低阴影或无阴影、active 状态使用橙色侧线。
- `SectionHeader`：像维护手册章节头，保留 action 区。
- `ControlTile` / `InlineControl`：统一为装备清单式浅色面。
- `TacticalEmptyState` / `AddCardButton` / `JsonPreBlock`：统一粗粝工业表面和等宽 JSON 展示。
- 新增 `CardToolbar`：承接卡片顶部按钮组、排序按钮、批量操作。
- 新增 `SurfaceToggleGroup`：承接 Tabs / ToggleGroup 外层工业底座。

### 4.4 shadcn 基础形状

只做低风险基础形状收紧：

- `src/components/ui/card.tsx`：降低圆角，去掉柔和营销卡片感。
- `src/components/ui/badge.tsx`：硬边、小写/等宽元信息友好，不破坏 variant API。

不新增第三方 UI 库，不引入 Motion / GSAP。

### 4.5 主壳层 Sidebar

只修改 `src/App.tsx` 主窗口视觉：

- 侧边栏变为工业导航轨，使用硬边、网格分隔、状态徽章。
- 保留 `tools` / `deltaTools` / `FavoritesSidebarGroup` / `renderToolPage` 行为。
- 不触碰 `overlayWindowModes` 和所有 `?mode=` early-return 分支。

### 4.6 页面重复样式收拢

只收拢重复视觉，不重写业务状态：

- `timer-page.tsx`：Tabs / ToggleGroup / CardHeader 使用 `SurfaceToggleGroup` / `CardToolbar`。
- `morse-panels.tsx`：区域摘要、验证卡、历史项使用 `ControlTile` / `InlineControl`。
- `rapidfire-page.tsx`：input / select / 数值卡背景 token 化，保留拖拽排序和上移下移兜底。
- `strategy-page.tsx`：工具条和内容宿主改为工业表面，保留 `strategy-content` 子 WebView 生命周期。
- `delta-toolbox-page.tsx`：折叠区域统一用 `TacticalCard` + 既有 `Collapsible`。
- `delta-account-selector.tsx`：空态使用 `InlineControl`。

### 4.7 验证与文档同步

- 运行 `bun run build`。
- 运行 `bun run test`。
- 若视觉规范实际落地，更新 `README.md` 和 `AGENTS.md` 中 UI 风格描述。
- 若形成长期设计决策且未来维护者会疑惑，新增 ADR；否则不新增。

## 5. 验收清单

### 自动验证

- `bun run build` 成功。
- `bun run test` 成功。

### 手动验收

在 `bun run tauri dev` 中检查：

- 主窗口 Sidebar、摩斯密码解析、计时器、计数器、连发器、攻略网站、账号管理、游戏数据、工具箱统一为浅色工业粗粝风。
- `?mode=overlay` 仍透明，不出现主窗口纸面背景。
- `?mode=timer-display`、`?mode=counter-display`、`?mode=rapidfire-display` 仍为透明 / 深色游戏叠加体验，置顶和点击穿透不被视觉重构破坏。
- `?mode=timer-position`、`?mode=counter-position`、`?mode=rapidfire-position` 仍可拖动定位并提交 / 取消。
- 攻略网站切换站点、手动刷新、自动刷新和页面卸载后不会残留遮挡主界面的 `strategy-content` 子 WebView。

## 6. 风险与控制

- 风险：页面级硬编码颜色太多，导致局部仍像旧 UI。控制：优先改 token 与共享组件，再定点收拢重复样式。
- 风险：透明窗口被主界面 token 误伤。控制：保留 overlay mode 背景透明，透明窗口只做必要硬边微调。
- 风险：为了视觉重构误改状态机。控制：页面清理只替换 className 和共享组件，禁止改 command、事件、窗口 label 和保存逻辑。
- 风险：高密度 UI 降低可读性。控制：正文保持 15px 基准，元信息使用等宽小字，交互目标维持最小 44px。