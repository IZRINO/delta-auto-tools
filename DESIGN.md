---
name: 三角洲行动工具
description: 两套制式并行。夜航黑标是默认生产壳；战地控制台仍可切换。overlay 不换皮。
colors:
  tracer-red: "oklch(48% 0.21 25)"
  tracer-red-content: "oklch(100% 0 281.288)"
  shell-copper: "oklch(42% 0.06 48)"
  tracer-yellow: "oklch(82% 0.189 84.429)"
  tracer-yellow-content: "oklch(27% 0.077 45.635)"
  barrel-grey: "oklch(21.5% 0 261.692)"
  breech-grey: "oklch(18.8% 0 264.665)"
  chalk: "oklch(96% 0.003 264.542)"
  gunmetal-void: "oklch(26% 0 0)"
  ash-neutral: "oklch(44% 0.017 285.786)"
  signal-cyan: "oklch(60% 0.126 221.723)"
  signal-green: "oklch(64% 0.2 131.684)"
  signal-orange: "oklch(66% 0.179 58.318)"
  signal-crimson: "oklch(58% 0.253 17.585)"
  m-blue-light: "#0066b1"
  m-blue-dark: "#1c69d4"
  m-red: "#e22718"
  blackmark-canvas-night: "#000000"
  blackmark-surface-night: "#0d0d0d"
  blackmark-elevated-night: "#1a1a1a"
  blackmark-hair-night: "#3c3c3c"
  blackmark-ink-night: "#ffffff"
  blackmark-body-night: "#bbbbbb"
  blackmark-muted-night: "#7e7e7e"
  blackmark-canvas-day: "#f1f1f1"
  blackmark-surface-day: "#ffffff"
  blackmark-hair-day: "#c8c8c8"
  blackmark-ink-day: "#111111"
  blackmark-body-day: "#4a4a4a"
  blackmark-success: "#0fa336"
  blackmark-warning: "#f4b400"
  blackmark-electric: "#0653b6"
typography:
  display:
    fontFamily: "Segoe UI Variable Display, Segoe UI, system-ui, sans-serif"
    fontSize: "3rem"
    fontWeight: 600
    lineHeight: 1
    fontVariation: "tabular-nums"
  headline:
    fontFamily: "Segoe UI Variable Display, Segoe UI, system-ui, sans-serif"
    fontSize: "2.25rem"
    fontWeight: 600
    lineHeight: 1.25
  title:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, sans-serif"
    fontSize: "1rem"
    fontWeight: 600
    lineHeight: 1.5
  body:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, sans-serif"
    fontSize: "0.875rem"
    fontWeight: 400
    lineHeight: 1.625
  label:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, sans-serif"
    fontSize: "0.75rem"
    fontWeight: 400
    lineHeight: 1.5
  caption:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, sans-serif"
    fontSize: "0.6875rem"
    fontWeight: 400
    lineHeight: 1.5
  readout:
    fontFamily: "JetBrains Mono Variable, Cascadia Mono, Consolas, monospace"
    fontSize: "1rem"
    fontWeight: 600
    letterSpacing: "0.025em"
    fontVariation: "tabular-nums"
  blackmark-display:
    fontFamily: "Noto Sans SC, Inter Variable, Segoe UI, sans-serif"
    fontSize: "3.5rem"
    fontWeight: 700
    lineHeight: 1
    letterSpacing: "-0.02em"
  blackmark-body:
    fontFamily: "Noto Sans SC, Inter Variable, Segoe UI, sans-serif"
    fontSize: "1rem"
    fontWeight: 300
    lineHeight: 1.625
  blackmark-label:
    fontFamily: "Noto Sans SC, Inter Variable, Segoe UI, sans-serif"
    fontSize: "0.75rem"
    fontWeight: 700
    letterSpacing: "0.14em"
  blackmark-readout:
    fontFamily: "JetBrains Mono Variable, Cascadia Mono, Consolas, monospace"
    fontSize: "2rem"
    fontWeight: 600
    letterSpacing: "0.025em"
    fontVariation: "tabular-nums"
rounded:
  none: "0rem"
  field: "0.5rem"
  box: "0.5rem"
  selector: "2rem"
  blackmark-none: "0rem"
  blackmark-full: "9999px"
spacing:
  hair: "0.125rem"
  tight: "0.5rem"
  base: "0.75rem"
  card: "1rem"
  roomy: "1.25rem"
  blackmark-spec: "1.5rem"
  blackmark-dock: "0.375rem"
  blackmark-hero: "2.5rem"
components:
  button-primary:
    backgroundColor: "{colors.tracer-red}"
    textColor: "{colors.tracer-red-content}"
    rounded: "{rounded.field}"
    height: "2.5rem"
    padding: "0 1rem"
    typography: "{typography.label}"
  button-primary-hover:
    backgroundColor: "oklch(63% 0.234 24.700)"
    textColor: "{colors.tracer-red-content}"
  button-outline:
    backgroundColor: "transparent"
    textColor: "{colors.chalk}"
    rounded: "{rounded.field}"
    height: "2.5rem"
    padding: "0 1rem"
  button-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.chalk}"
    rounded: "{rounded.field}"
  card-surface:
    backgroundColor: "{colors.breech-grey}"
    textColor: "{colors.chalk}"
    rounded: "{rounded.box}"
    padding: "1.25rem"
  well-inset:
    backgroundColor: "{colors.barrel-grey}"
    textColor: "{colors.chalk}"
    rounded: "{rounded.box}"
    padding: "1rem"
  input-field:
    backgroundColor: "{colors.barrel-grey}"
    textColor: "{colors.chalk}"
    rounded: "{rounded.field}"
    height: "2rem"
    typography: "{typography.body}"
  badge-code:
    backgroundColor: "{colors.tracer-red}"
    textColor: "{colors.tracer-red-content}"
    rounded: "{rounded.selector}"
    size: "1.25rem"
  toggle-on:
    backgroundColor: "{colors.tracer-red}"
    rounded: "{rounded.selector}"
  overlay-readout:
    backgroundColor: "oklch(0% 0 0 / 0.2)"
    textColor: "#ffffff"
    rounded: "0.375rem"
    typography: "{typography.readout}"
    padding: "0.5rem 0.75rem"
  blackmark-button-primary:
    backgroundColor: "transparent"
    textColor: "{colors.blackmark-ink-night}"
    rounded: "{rounded.blackmark-none}"
    height: "3rem"
    padding: "0 2rem"
    typography: "{typography.blackmark-label}"
  blackmark-button-primary-hover:
    backgroundColor: "{colors.blackmark-ink-night}"
    textColor: "{colors.blackmark-canvas-night}"
  blackmark-spec:
    backgroundColor: "{colors.blackmark-surface-night}"
    textColor: "{colors.blackmark-ink-night}"
    rounded: "{rounded.blackmark-none}"
    padding: "{spacing.blackmark-spec}"
  blackmark-dock-active:
    backgroundColor: "{colors.blackmark-ink-night}"
    textColor: "{colors.blackmark-canvas-night}"
    rounded: "{rounded.blackmark-none}"
    height: "3rem"
    padding: "0 1rem 0 0.75rem"
---
<!-- IMPECCABLE_BODY_HEAD -->
# Design System: 三角洲行动工具

<!-- 本文档面向生成新页面的 AI agent。主窗口有两条视觉线路；overlay 是第三条表面，不跟随主窗口换世界。 -->

## Overview

**Creative North Star: "两套制式（Two Issued Kits）"**

同一产品发两套主窗口视觉世界，设置里切换，禁止混用。

**World A — 战地控制台 The Field Console** 是可切换的第一条视觉世界。深色枪管金属底，内容开凿成比背景更暗的凹槽，一道 1px 弹壳铜切口勾边。语气冷静、紧密、可信。数字和状态自己说话。这不是 SaaS 仪表盘，没有圆角柔光卡片。红色只出现在真正需要注意力的地方。配色主题 `olive-amber` / `valentine` / `arctic-blue` 只给这一世界换 28 个 token，不发明新世界。

**World B — 夜航黑标 Night-ops Black Mark** 是默认生产壳。视觉权威是 `blackmark-demo.html` + `src/blackmark-demo.tsx`。BMW M 的语法翻成 Operate：纯黑夜航或浅灰日间、直角、Noto Sans SC 700 对 300、4px 三色条只做身份、白描边主按钮、底部居中悬浮图标 dock。背景是碳纤加一道细展厅扫光，不是铺满蓝红。不得用换 valentine 色冒充。

**Overlay — 游戏读数仪表** 是第三条表面，不属任何主窗口世界。游戏画面才是底。黑纱玻璃、JetBrains Mono、白字。无论主窗口走 A 还是 B，overlay 的 `?mode=` 窗保持现有玻璃读数：点击穿透、不抢焦点、不遮准星。禁止把黑标碳纤、三色条、巨型标题带进 overlay。

**Key Characteristics:**

- 双主窗口线路并行：夜航黑标（默认生产）与战地控制台。设置切换，同一屏禁止混语法。
- Overlay 独立：白边玻璃读数，不跟随黑标，不跟随配色主题的圆角/铜边。
- 两条线路都零投影。战地用凹槽深度；黑标用发丝线 + 色阶反转（选中项夜航白底/日间黑底）。
- 读数一律 JetBrains Mono + tabular-nums，两条线路共用。
- 战地配色主题三套共享 28 token；黑标有自己的夜航/日间变量，不进那 28 个 key。

## Colors

**The Dual Line Rule.** 主窗口一次只渲染一个世界。战地 token 不得画黑标；黑标变量不得画战地。配色主题三套只服务战地。Overlay 不吃黑标色。

### World A — 战地控制台

默认主题为 `valentine`（黑红）。下面的描述与值均以 valentine 为准；三套主题在同一 token key 下换值，不新增 key。

### Primary

- **曳光红 Tracer Red**（`oklch(48% 0.21 25)`）：唯一强调色。用于主按钮、失败/紧急、占用键鼠、favorite 高亮描边。配白色文字 `oklch(100% 0 281.288)`，对比 ≥4.5:1。不铺边框、不铺导航图标。

### Secondary

- **曳光弹黄 Tracer Yellow**（`oklch(82% 0.189 84.429)`）：次级功能色，不是第二强调色。用于 overlay 计时进度条填充、`badge-secondary`、SaveStateBadge 的「已保存」态。配深色文字 `oklch(27% 0.077 45.635)`。

### Neutral

- **枪管灰 Barrel Grey**（`oklch(21.5% 0 261.692)`）：`base-100`，页面底色。整个界面的「表盘面板」。
- **膛底灰 Breech Grey**（`oklch(18.8% 0 264.665)`）：`base-200`，比底色更暗。卡片、Rail、Header、Dialog 都用它——容器是开凿进表盘的凹槽，不是浮起的台面。
- **粉笔白 Chalk**（`oklch(96% 0.003 264.542)`）：`base-content`，正文与前景文字。
- **弹壳铜 Shell Copper**（`oklch(42% 0.06 48)`）：`base-300`，全局 1px 边框与分隔线。低饱和铜缝，不是警报红。
- **炮膛黑 Gunmetal Void**（`oklch(26% 0 0)`）：`accent` 槽位，作深色底腔（tooltip、JsonPreBlock 等 mockup 容器）。
- **灰烬中性 Ash Neutral**（`oklch(44% 0.017 285.786)`）：`neutral`，tooltip 背景、stat 描述等次级表面。
- **信号青 Signal Cyan**（`oklch(60% 0.126 221.723)`）：`info`，信息态。
- **信号绿 Signal Green**（`oklch(64% 0.2 131.684)`）：`success`，令牌有效、保存成功、可执行。
- **信号橙 Signal Orange**（`oklch(66% 0.179 58.318)`）：`warning`，临期令牌、即将到达阈值的警告。注意与曳光弹黄同族不同位：yellow 是功能色，orange 是状态色。
- **信号绯 Signal Crimson**（`oklch(58% 0.253 17.585)`）：`error`，过期令牌、失败、全局总开关关闭横幅。

### Named Rules

**The One Voice Rule.** 曳光红是唯一主色，任何一屏的曳光红面积不超过约 10%。它的稀缺性就是它的意义——主按钮、当前激活项、状态灯心跳。大面积铺红即降级为噪音。禁止把 `primary` / `error` 当 base 表面上的正文字色（小字对比不够）；读数用 `base-content`，失败用 `alert-error` 或 error 底 + `error-content`。

**The Copper Seam Rule.** 默认边框色就是弹壳铜（`base-300`）。不要另发明灰边框，也不要把边框做成曳光红；一道 1px 铜缝就是全部边界语言。

**The Yellow Is Not A Second Accent Rule.** 曳光弹黄只出现在 overlay 进度条、次级 badge 和「已保存」提示。不得用它做主按钮、链接或大面积背景；它和曳光红同框时，红永远压黄。

### World B — 夜航黑标

主色不是一块填色，是白（夜航）或黑（日间）的字与描边。三色条 `{colors.m-blue-light}` `{colors.m-blue-dark}` `{colors.m-red}` 只做 4px 身份标记（顶栏下、标题下、选中 dock 顶缘），永不做按钮填充或页面底。

#### Primary

- **夜航墨 / 日间纸**（`{colors.blackmark-ink-night}` / `{colors.blackmark-ink-day}`）：正文、描边按钮、选中反转底。CTA 是「墨色描边的空心矩形」，hover 才填满反转。

#### Identity (not CTA)

- **M 蓝浅** `{colors.m-blue-light}` `#0066b1`、**M 蓝深** `{colors.m-blue-dark}` `#1c69d4`、**M 红** `{colors.m-red}` `#e22718`：只出现在三色条与扫光刃口。

#### Neutral

- **夜航画布** `{colors.blackmark-canvas-night}` `#000`：页面底。
- **夜航表面** `{colors.blackmark-surface-night}` `#0d0d0d`：规格格。
- **夜航发丝** `{colors.blackmark-hair-night}` `#3c3c3c`：1px 边。
- **夜航正文辅** `{colors.blackmark-body-night}` `#bbbbbb`：说明文字。
- **日间画布** `{colors.blackmark-canvas-day}` `#f1f1f1`；**日间表面** `#fff`；**日间墨** `#111`。日间是结构反转，不是另一套色相。

#### Semantic

- **警告** `{colors.blackmark-warning}` `#f4b400`：需人工检查。
- **成功** `{colors.blackmark-success}` `#0fa336`。
- **错误** `{colors.m-red}` 仅用于失败切口（表格左 2px inset），不是大面积底。

**The Stripe Is Identity Rule.** 三色条不是主按钮、不是进度条、不是卡片左边线装饰。出现位置：顶栏下 4px 通栏、标题下 4px 短条（约 7rem）、选中 dock 顶 3px。

**The No Flood Wash Rule.** 背景禁止铺满蓝红线性渐变。允许碳纤织纹 + 一道细展厅扫光（刃口透明度约 8–10%）。扫光变宽或颜色变饱和即跑偏。

## Typography

两条线路字族不同，读数共用。

### World A — 战地控制台

**Display Font:** Segoe UI Variable Display（回退 Segoe UI → system-ui）
**Body Font:** Segoe UI Variable Text（回退 Segoe UI → system-ui）
**Readout/Mono Font:** JetBrains Mono Variable（回退 Cascadia Mono → Consolas）

**Character:** 界面用 Windows 原生可变字体，冷静、合规、不抢戏；一切「读数」切到 JetBrains Mono 且开 tabular-nums。正文字重固定在 400 / 600 两档，标题 600，不做 300/700/800 的花样。

### Hierarchy

- **Display**（600, 3rem, lh 1, tabular-nums）：MacroNumber 巨数，单页最多一处，用于真正需要「一眼读数」的量（如剩余时间、总计数）。
- **Headline**（600, 1.25rem, lh 1.25）：MacroHeader 页面标题 `text-xl`，每页一个。
- **Title**（600, 1rem, lh 1.5）：CardTitle / SectionHeader / DialogTitle `text-base`。
- **Body**（400, 0.875rem, lh 1.625）：正文 `text-sm`，行宽限 `max-w-[64ch]`，长文本一律 truncate 防溢出。桌面密集工具正当例外，不要「修正」到 1rem。
- **Label**（400, 0.75rem, lh 1.5）：标签、元信息、按钮文字 `text-xs`，次级内容用 `text-base-content/60`。
- **Caption**（400, 0.6875rem, lh 1.5）：密集元信息唯一小级 `text-caption`。时间轴行、档案次级、收藏格子标注。禁止再散落任意 px/rem。
- **Readout**（600, 1rem, tracking 0.025em, tabular-nums）：overlay 读数、快捷键录制框、ConfigRow 数值列、DataWell/JsonPreBlock 全文。暗底补偿只加在 readout 的 `letterSpacing: 0.025em`，界面字不引入第三档字重。

### Named Rules

**The Two Weight Rule.** 战地控制台只用 400 与 600。标题与正文靠字号和色阶（`/60`、`/70`）分层，不靠字重堆叠。此条不约束黑标。

**The Readout Is Mono Rule.** 凡是用户要「读数」的地方——时间、计数、坐标、快捷键、状态词——必须是等宽字体并开 tabular-nums。比例字体的数字逐帧跳动即视为 bug。两条线路都遵守。

### World B — 夜航黑标

**Display / Body Font:** Noto Sans SC（简体 300 与 700，自托管子集 `src/fonts/blackmark/`）。拉丁回退 Inter Variable。禁止用系统雅黑冒充标题。
**Readout:** JetBrains Mono Variable 600 + tabular-nums + tracking 0.025em。

**Character:** 中文标题与正文同一家族，靠 700 / 300 的落差当编辑签名。按钮与 dock 标签 700、字距 0.12–0.14em、大写（中文无大小写，字距仍在）。禁止引入第三档字重 400/500 去「调和」。

- **Display**（700, 3.5rem, lh 1）：当前工具名，每页一个。右侧可有同文水印约 6% 不透明度。
- **Body**（300, 1rem, lh 1.625）：说明句，max 约 46–60ch。
- **Label**（700, 0.75rem, tracking 0.14em）：规格格标签、表头、dock 展开字。
- **Readout**（600, 2rem 规格格 / 1rem 表格时间列）：只给数字与 IDLE 这类状态词。

**The CJK Is The Display Rule.** 黑标的设计感在中文 700，不在再换一个英文展示体。不要把标题改成 Space Grotesk / Syne / Outfit。

## Layout

窗口最小 1280×800。Overlay 无布局网格。

### World A — 战地控制台

应用壳是固定网格：顶栏 48px。≥1024px 用 `grid-rows-[48px_1fr]` + 左侧 240px Index Rail；<1024px 用 `grid-rows-[48px_auto_1fr]`，Rail 收起为顶部横向 Tab Bar，避免 Tab Bar 吃掉 `1fr` 把主区裁掉。页面内容在 12 列 Work Grid（`AppPage`，gap-3）上排布，常规工具页限宽 `max-w-7xl`，攻略页 `max-w-none` 铺满。

间距三档按职责用，禁止单值复读到所有层级：

- **tight `0.5rem`（`gap-2`）**：组内兄弟。芯片行、按钮簇、表单控件之间。
- **base `0.75rem`（`gap-3`）**：组与组。`AppPage` 栅格、卡片之间、章节之间。
- **roomy `1.25rem`（`px-5 py-5`）**：标准卡片内边距。`card-md` / MacroHeader。工具业务卡默认 `TacticalCard size="sm"` → `px-4 py-4`，是紧凑档不是锚点失效。

行内控件（ControlTile/InlineControl）`p-4`/`p-3`。Rail 导航项 `px-3 py-2`，宏观区（MacroHeader）`gap-4`。

overlay 窗口无布局网格——它们是按 `?mode=` 进入的独立表面，跟随游戏画面，不服从应用壳。

### World B — 夜航黑标

无左侧 240px 轨。顶栏 64px 放产品名、档案、日月切换、全局开关。工具导航是底部居中悬浮 dock（图标 48×48；选中展开出字）。主区：巨型工具名 → 短三色条 → 规格格通栏 → 发丝线表。主区底部留约 7rem 以免被 dock 挡住。

禁止把黑标做成「旧侧栏 + 新配色」。拓扑必须是顶栏 + 底 dock，不是 Index Rail。

## Elevation & Depth

**无阴影。** `--depth: 0`，所有卡片、按钮、输入框 `shadow-none`。深度完全由凹槽关系表达：容器（`base-200`）比页面底（`base-100`）更暗，像开凿进金属面板的槽；槽口由一道 1px 弹壳铜边框收口。tooltip、Dialog、JsonPreBlock 这类「浮出」元素也不加投影，只靠更深的底色（accent/neutral 槽）与边框区分层级。

### Shadow Vocabulary

无。任何 `shadow-*` / `box-shadow` 均不允许。

### Named Rules

**The Zero Shadow Rule.** 阴影恒为 0。需要「浮起」感时，加深背景色阶（base-100 → accent 槽）而不是加投影。

**The Recess, Not Raise Rule.** 战地控制台容器向下沉（更暗），不向上抬。base-200 的 L 值必须小于 base-100。此条不约束黑标选中反转。

### World B — 夜航黑标

同样零投影。深度靠 1px 发丝线与表面色阶（画布 / 表面 / 抬升）。选中 dock 项反转（夜航白底黑字，日间黑底白字），这是唯一允许的「抬起」。背景惊喜是碳纤织纹上的细扫光，用 transform 缓慢平移，不是铺色。`prefers-reduced-motion` 必须关掉扫光与切开动画。

## Shapes

直角为主，圆角只留给「机制件」。valentine 主题：`--radius-field` 与 `--radius-box` 均 0.5rem（输入、卡片、面板微圆），`--radius-selector` 2rem（toggle、badge 这类「开关/印章」件做成胶囊）。Dialog 与按钮同取 field 的 0.5rem。CLAUDE.md 记全局 `--radius: 0`，当前实现以主题 token 的 0.5rem 为准——圆角只服务控件语义，不装饰容器。

overlay 是独立形状语言：应用内边框是铜红，overlay 边框是 `white/15–20`；应用内圆角 0.5rem，overlay 用 0.375rem（rounded-md）+ 1px 黑纱 backdrop-blur。应用内切口的锐利感在 overlay 上让位给「浮在游戏画面上的玻璃片」。

描边与高亮：激活卡片 `ring-2 ring-primary`，favorite 跳转用 1.5s 曳光红描边脉冲，Morse 框选当前步 `border-2 border-primary`。焦点环统一 `outline-primary/50`。

### World B — 夜航黑标

几乎全是 0 圆角。唯一例外：toggle / 圆形图标用满圆 `{rounded.blackmark-full}`。按钮、规格格、dock、表、输入一律直角。dock 图标为自制 SVG（方线帽、1.75 描边），不是 remixicon。

## Components

组件分三套词汇：**战地控制台**走 daisyUI class + Radix headless；**夜航黑标**走演示页 `bm-*` 类（接到生产时保持直角描边语法，不得把 `btn-primary` 红块带进去）；**overlay** 是独立单色读数，不复用前两套。

### Buttons

- **Shape:** 微圆角（0.5rem），无阴影。
- **Primary:** 曳光红底 + 白字，中等高度（2.5rem）水平内边距 1rem。变体映射见 `src/components/ui/button.tsx`：`default→btn-primary`、`outline→btn-outline`、`secondary→btn-secondary`、`ghost→btn-ghost`、`destructive→btn-error`、`link→btn-link`。
- **Hover / Focus:** hover 时曳光红向暗移一档；focus 用 `outline-primary/50` 描边，不改底色。
- **Icon buttons:** `btn-square` 系列（`icon-xs/sm/lg`），图标必须带 `data-icon="inline-start|inline-end"` 语义标记。
- **手感总则（触感强、反馈重）:** 主操作永远是曳光红实心块，destructive 用信号绯实心，不共用「灰底 hover 变红」这种弱反馈。

### Badges

- **Style:** 胶囊（2rem 圆角）。变体：`default→badge-primary`、`secondary→badge-secondary`、`destructive→badge-error`、`outline→badge-outline`、`ghost→badge-ghost`。
- **用法:** 页面编号（01/02/D1/PIN）用 `badge-primary badge-sm`；状态矩阵（StatusMatrix）按 state 映射 success/warning/error/ghost；「已保存」用 secondary（曳光弹黄），「保存中/待保存」用 outline。

### Cards / Containers

- **Corner Style:** 微圆角（0.5rem）。
- **Background:** `base-200`（膛底灰，凹槽）；卡片内的输入井/数据井（ControlTile、DataWell、InlineControl）回落到 `base-100`。
- **Border:** 1px 弹壳铜（`card-border` / `border-base-300`）。
- **Shadow Strategy:** 无，见 Zero Shadow Rule。
- **Internal Padding:** 标准 `px-5 py-5`，紧凑 `card-sm` 为 `px-4 py-4`。
- **Active:** 选中态 `ring-2 ring-primary`，不改底色。

### Inputs / Fields

- **Style:** `input input-sm`，`base-100` 底，1px 铜边框，0.5rem 圆角，高 2rem。
- **Focus:** 边框转曳光红 + `outline-primary/50`，不发光、不加厚。
- **HotkeyField:** 快捷键录制是一个 `outline` 按钮，等宽字体显示键名，录制中显「录制中.../失焦取消」。
- **Switch:** daisyUI toggle，开态曳光红，胶囊形（2rem）。
- **Error / Disabled:** 表单错误用信号绯文本 + ErrorHint（圆 ! 按钮，0 delay tooltip）；disabled 统一 `disabled:opacity-50`。

### Navigation

- **Index Rail（≥1024px）:** 240px 固定列，`base-200` 底，1px 铜右边框。每项：曳光红图标 + 中文名（600）+ `编号 / 英文短名` 元信息（如 `01 / Timer`）。激活项 `btn-active`，不做额外色块。分两段：「通用工具」（01–06）与「三角洲工具」（D1–D2），底部固定设置入口。
- **Top Tab Bar（<1024px）:** 横向滚动 tab，激活项下划 2px 曳光红边。
- **Header:** 48px 顶栏，`base-200`，左准星 logo + 产品名，中间当前工具编号 badge + 名称，右 Profile 切换 + 全局总开关。

### Dialogs / Overlays

- **Dialog:** Radix Portal，遮罩 `bg-base-content/45`（即粉笔白 45% 压暗画面），内容卡 `base-200` + 铜边框 + 0.5rem 圆角，`zoom-in-95/fade` 进场，无阴影。
- **Tooltip:** `neutral`（灰烬中性）底，`text-xs`，0.5rem 圆角，带小箭头，`z-index 9999` 压过一切 overlay。

### Transparent Readout Overlay（signature）

计时器 / 计数器 / 连发器的透明显示窗是这套体系的签名组件，与应用内面板刻意不同文：

- **Surface:** 全屏透明，`bg-black/20` + `border-white/20` + `backdrop-blur-[1px]`，0.375rem 圆角。整窗可设 `fontOpacity`（0.1–1.0）。
- **Type:** JetBrains Mono，600，tracking 0.025em，白字；名称 truncate，状态词（RUNNING/IDLE/FINISHED）等宽小字印在数字旁。
- **Active row:** 曳光红心跳点（`animate-pulse` 圆点）+ `bg-primary/20` + `ring-1 ring-primary/70`。
- **Progress:** 整行底部铺 `white/20` 轨道，填充用信号橙/曳光弹黄系（`bg-warning`），不用曳光红——进度是「消耗」，不是「激活」。
- **铁律:** 无边框、置顶、点击穿透、背景透明。游戏画面永远是底，overlay 不得遮挡准星或抢焦点。

### Operation Warning Overlay（特勤处占用键鼠）

独立小窗（480×220），不是读数仪表。游戏画面仍是底，但这块必须被一眼读完：

- **Surface:** 满幅 `bg-black/80` + `border-white/15` + `backdrop-blur-[1px]`。禁止 daisyUI card、禁止投影。
- **Countdown:** JetBrains Mono 巨数 `text-7xl` tabular-nums；每秒换数带 180ms 轻缩放，顶缘 1px 曳光红导火索 1s 抽尽。
- **Idle:** 当前步骤/账号进度用一行白字，不放大。
- **Emergency:** 底边白 15% 切口 + 粉笔白等宽「紧急停止：热键」。红只留顶缘导火索。无按钮。
- **铁律:** 点击穿透、无边框、置顶。窗口不抢焦点。

### Region Selection Overlay（Morse / 识别框选）

全屏透明拖拽框选。已确认步骤 `border border-white/85 bg-white/10`，当前步骤 `border-2 border-primary bg-primary/12`——曳光红只标记「正在操作」的那一步，其余步骤保持白描边。指示面板用 `bg-background/88` 半透明 + 白边，不遮挡下方游戏画面。

### World B components

### Buttons（黑标）

- **Shape:** 直角，高 48px，水平 32px，1px 墨色描边，透明底。
- **Primary:** hover / focus 填满墨色、字反转到画布色。字距 0.12em，700。
- **Ghost:** 发丝边，hover 边与字升到墨色。
- **Don't** 把战地的曳光红实心块当黑标主按钮。

### Spec cell（黑标）

- 直角，1px 发丝，表面色，内边距 24px。数字 2rem 700（读数改 mono 600）。标签 0.75rem 700 字距 0.14em。

### Dock（黑标签名）

- 底部居中悬浮，发丝边，表面半透明。未选中只出 22px 自制 SVG。选中反转底 + 展开标签 + 顶缘 3px 三色条。竖线分组：收藏 | 计时/计数/连发 | 攻略/识别/息屏 | 特勤/摩斯 | 设置。未选中 hover 出直角发丝提示（Portal，Noto 700 字距 0.14em），选中已出字则不再叠提示。

### Navigation（黑标）

- 无 Index Rail。顶栏 64px 无工具名列表。

## Do's and Don'ts

### Do:

- **Do** 先判断当前是战地、黑标还是 overlay，再选用对应 token 与组件，禁止混用。
- **Do** 战地：容器比页面底更暗（base-200 < base-100），凹槽方向不可反。
- **Do** 战地：默认边框用弹壳铜（base-300），一道 1px。
- **Do** 黑标：直角、发丝线、三色条只做身份、主按钮空心描边。
- **Do** 黑标视觉以 `blackmark-demo.html` 为准，接到生产壳时对照演示，不要发明第三套。
- **Do** 读数、坐标、快捷键、JSON 一律 JetBrains Mono + tabular-nums（两条线路 + overlay）。
- **Do** 状态同时用颜色与文字/图标表达。
- **Do** 战地新页面复用 `app-ui.tsx`。标准骨架：`ToolPageFrame` → `MasterSwitchCard?` → `SyncGroupSection?` → `SyncCardList`。
- **Do** 改战地主题 token 时三套内置主题同改同测，28 key 不变。

### Don't:

- **Don't** 用换 `valentine` 颜色冒充黑标，或把黑标碳纤/dock/巨型标题贴到战地壳上。
- **Don't** 把黑标三色条当按钮填充、进度条或卡片左边装饰。
- **Don't** 把黑标背景做成铺满蓝红渐变或赛博扫描线。扫光必须是细刃。
- **Don't** 给战地加阴影、大面积渐变、柔光、发光边框。
- **Don't** 给 overlay 套黑标或战地铜边；overlay 保持白边玻璃、点击穿透、不抢焦点。
- **Don't** 在正文用等宽字体、在读数用比例字体。
- **Don't** 用 remixicon 替换黑标 dock 的自制 SVG。
- **Don't** 把黑标标题换成 Inter / Space Grotesk 等拉丁展示体而丢掉 Noto Sans SC 700。
- **Don't** 让游戏 overlay 窗口吃黑标 CSS 变量。

