# 工业粗粝风 UI 重构文档

## 1. 目标

桌面主界面采用 `DESIGN.md` 定义的 **Swiss Industrial Print × Declassified Tactical Control Board**：工业纸面、粗黑结构线、巨型模块标题、密集
telemetry、机械标签和单一航空红焦点。它不是白色 SaaS 后台、不是旧 Sidebar + 圆角 Card + Hero，也不是全黑 CRT。

本次重构只改变视觉框架与 JSX 布局，不改变工具行为、数据流、Tauri command、窗口入口或透明窗口核心约束。

## 2. 不改行为边界

以下内容必须保持现状：

- `src/App.tsx` 中 `overlayWindowModes` 的语义和 early-return 入口。
- 查询参数入口：`?mode=overlay`、`?mode=timer-display`、`?mode=counter-display`、`?mode=timer-position`、
  `?mode=counter-position`、`?mode=rapidfire-display`、`?mode=rapidfire-position`。
- Tauri command 名称与前端 `invoke` 调用参数。
- 原生窗口 label：`morse-overlay`、`timer-display`、`timer-position`、`counter-display`、`counter-position`、
  `rapidfire-display`、`rapidfire-position`、`strategy-content`。
- Morse 连续区域框选流程、autosave、热键录制暂停 / 恢复逻辑。
- 计时器 / 计数器独立总开关、运行态、透明窗口显示、位置设置和排序逻辑。
- 连发器 hold 热键状态机、启动抖动、最小间距、补齐触发、不追加策略、透明窗口行为。
- 攻略网站 `strategy-content` 子 WebView 的创建、销毁、bounds 同步、刷新和卸载清理生命周期。
- Delta 账号凭据边界、账号能力过滤、分批加载和版本号机制。

## 3. 已落地的视觉方向

核心 token 与规则：

- Paper `#F1EFE8`：主背景，旧纸 / 档案页。
- Bone `#DDD8CC`：次级底板、禁用区、表格隔行。
- Ink `#080808`：主文字、粗边框、结构块。
- Steel `#3B3B36` / Ash `#8A867B` / Line `#B9B2A4`：元信息、弱标签、工程纸网格。
- Alert Red `#E11919`：唯一通用强调色，只用于当前选择、危险动作、运行态和关键焦点。
- Warning Amber `#A36A00`、Valid Green `#3F6B2A`：只用于语义状态。
- Data Well `#141414`：JSON、原始响应、小型数据井和叠加窗文本底。
- `--radius: 0`：主窗口 90 度直角；禁止 pill、柔和圆角卡片、玻璃态、柔和阴影和渐变营销卡。

## 4. 实现结构

### 4.1 全局基底

`src/App.css` 负责：

- 工业纸面 token。
- 工程纸网格 + 低透明纸面噪声。
- 全局直角半径。
- `body[data-overlay-mode="true"]` 透明例外。
- token 状态色与收藏高亮动画。

### 4.2 主壳层

`src/App.tsx` 的桌面主窗口是三段式机械界面：

1. **Top Manifest Bar**：48px 顶栏，展示产品代号、当前模块、工具总数、窗口数和 Tauri 运行面。
2. **Left Index Rail**：240px 档案索引轨，工具项固定显示编号、英文代号、中文名；当前项黑底反白并带 Alert Red 标识；收藏显示
   `PINNED / <count>`。
3. **Main Work Grid**：页面内容进入 12 列工作网格。

Overlay / display / position 查询参数入口仍 early return，不经过主壳层。

### 4.3 共享视觉语义层

`src/components/app/app-ui.tsx` 保留导出名，但语义已重写：

- `AppPage`：12 列 Main Work Grid。
- `PageHero`：Macro Module Header，包含巨大模块标题、机器元信息与状态矩阵承载区。
- `SignalTile`：Status Matrix Cell。
- `TacticalCard`：FIELD UNIT 面板。
- `SectionHeader`：黑色机器标签条。
- `ControlTile` / `InlineControl`：硬边配置格。
- `JsonPreBlock`：深色 Data Well。
- `SurfaceToggleGroup`：硬边频道 / 拨档底座。

### 4.4 基础 UI 原语

`src/components/ui/*`
的常用原语已统一为工业机械样式：Button、Card、Badge、Input、Textarea、Tabs、ToggleGroup/Toggle、Dialog、Select、Dropdown、ContextMenu、Popover、Tooltip、Switch、Checkbox、Radio、Slider、Progress、Table、Alert。

行为层仍使用原 shadcn/Radix 组件；只改变 class。

### 4.5 页面工作台

已重排页面：

- Morse：MX-01 信号破译台；Selection / Workbench / Result / History 映射为四个 FIELD UNIT。
- Timer / Counter：任务时序板；双通道 Tabs、总控字段、透明窗口分组、计时卡片与计数卡片改为硬边配置行。
- Rapidfire：火控矩阵；全局发射设定、通道分组和 RF 卡片改为机械配置单元。
- Strategy：贴顶工业浏览器工具条 + `strategy-content` 宿主区；WebView 生命周期不变。
- Delta Accounts / Game / Toolbox / Favorites：状态矩阵、档案卡、Command Unit、Data Well。
- Delta Login / Data Card / Query Workbench / Placeholder：硬边 Dialog、二维码框、原始响应井和未开放单元。

## 5. 验收清单

### 自动验证

- `bun run build` 成功。
- `bun run test` 成功。

### 手动验收

在 `bun run tauri dev` 中检查：

- 主窗口 Top Manifest Bar、Left Index Rail、Morse、Timer/Counter、Rapidfire、Strategy、Delta Accounts、Delta Game、Delta
  Toolbox、Favorites 统一为浅色工业粗粝风。
- 每个工作台有巨大模块元素、高密度数据区和 Alert Red 焦点。
- `?mode=overlay` 仍透明，不出现主窗口纸面背景。
- `?mode=timer-display`、`?mode=counter-display`、`?mode=rapidfire-display` 仍为透明 / 深色游戏叠加体验，置顶和点击穿透不被视觉重构破坏。
- `?mode=timer-position`、`?mode=counter-position`、`?mode=rapidfire-position` 仍可拖动定位并提交 / 取消。
- 攻略网站切换站点、手动刷新、自动刷新和页面卸载后不会残留遮挡主界面的 `strategy-content` 子 WebView。

## 6. 长期维护规则

- 新页面先复用 `app-ui.tsx` 语义层，不复制一套自定义卡片体系。
- 不恢复旧 SidebarProvider 主壳层。
- 不恢复圆角 shadcn 默认卡片、柔和阴影、橙色旧主题或营销式 Hero。
- 主窗口不使用大面积半透明 / 毛玻璃；透明感仅属于游戏 overlay。
- 修改视觉时不得改 command、事件、窗口 label、保存逻辑或状态机。
