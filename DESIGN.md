---
name: 三角洲行动工具
description: 暗底红边的战地控制台，为无人值守自动化与局内实时读数而生
colors:
  tracer-red: "oklch(54% 0.21 25)"
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
rounded:
  none: "0rem"
  field: "0.5rem"
  box: "0.5rem"
  selector: "2rem"
spacing:
  hair: "0.125rem"
  tight: "0.5rem"
  base: "0.75rem"
  card: "1rem"
  roomy: "1.25rem"
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
---
<!-- IMPECCABLE_BODY_HEAD -->
# Design System: 三角洲行动工具

<!-- 本文档面向生成新页面的 AI agent，覆盖「应用内界面」与「overlay 窗口」两类表面。 -->

## Overview

**Creative North Star: "The Field Console（战地控制台）"**

应用内界面是一整块制式装备面板：深色枪管金属底，内容开凿成比背景更暗的凹槽，用一道 1px 弹壳铜红切口勾出边界。语气冷静、紧密、可信——数字和状态自己说话，界面不表演紧张感。这不是 SaaS 仪表盘，没有圆角柔光卡片，也没有插画装饰；一切信息以扫读为先，红色只出现在真正需要注意力的地方。

overlay 是另一块战场。游戏画面才是底，面板退为游戏画面上方一层单声道仪表板：黑纱玻璃、JetBrains Mono 等宽读数、白字，RUNNING / IDLE 状态词直接印在数字旁。它不追求与应用内的红边面板同文同形，追求被一眼读完且不遮挡准星。

**Key Characteristics:**

- 双表面制式分离：应用内 = 红边制式面板；overlay = 游戏上的单色读数仪表。
- 凹槽式深度，零阴影；深度由明暗差与 1px 切口边框表达。
- 强调色单一——曳光红；黄色只留给 overlay 进度条与次级功能，不做第二个主色。
- 读数用 JetBrains Mono + tabular-nums；界面字用 Segoe UI Variable。
- 三套运行时主题（olive-amber / valentine / arctic-blue）全部覆盖同一份 28-token 集合，改动 token 必须整套改。

## Colors

默认主题为 `valentine`（黑红）。下面的描述与值均以 valentine 为准；三套主题在同一 token key 下换值，不新增 key。

### Primary

- **曳光红 Tracer Red**（`oklch(54% 0.21 25)`）：唯一强调色。用于主按钮、失败/紧急、占用键鼠、favorite 高亮描边。配白色文字 `oklch(100% 0 281.288)`，对比 ≥4.5:1。不铺边框、不铺导航图标。

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

**The One Voice Rule.** 曳光红是唯一主色，任何一屏的曳光红面积不超过约 10%。它的稀缺性就是它的意义——主按钮、当前激活项、状态灯心跳。大面积铺红即降级为噪音。

**The Copper Seam Rule.** 默认边框色就是弹壳铜（`base-300`）。不要另发明灰边框，也不要把边框做成曳光红；一道 1px 铜缝就是全部边界语言。

**The Yellow Is Not A Second Accent Rule.** 曳光弹黄只出现在 overlay 进度条、次级 badge 和「已保存」提示。不得用它做主按钮、链接或大面积背景；它和曳光红同框时，红永远压黄。

## Typography

**Display Font:** Segoe UI Variable Display（回退 Segoe UI → system-ui）
**Body Font:** Segoe UI Variable Text（回退 Segoe UI → system-ui）
**Readout/Mono Font:** JetBrains Mono Variable（回退 Cascadia Mono → Consolas）

**Character:** 界面用 Windows 原生可变字体，冷静、合规、不抢戏；一切「读数」——倒计时数字、计数、坐标、快捷键名、JSON、token——切到 JetBrains Mono 且开 tabular-nums，保证逐帧刷新时位宽不跳。正文字重固定在 400 / 600 两档，标题 600，不做 300/700/800 的花样。

### Hierarchy

- **Display**（600, 3rem, lh 1, tabular-nums）：MacroNumber 巨数，单页最多一处，用于真正需要「一眼读数」的量（如剩余时间、总计数）。
- **Headline**（600, 2.25rem, lh 1.25）：PageHero / MacroHeader 的页面标题 `text-4xl`，每页一个。
- **Title**（600, 1rem, lh 1.5）：CardTitle / SectionHeader / DialogTitle `text-base`。
- **Body**（400, 0.875rem, lh 1.625）：正文 `text-sm`，行宽限 `max-w-[64ch]`，长文本一律 truncate 防溢出。桌面密集工具正当例外，不要「修正」到 1rem。
- **Label**（400, 0.75rem, lh 1.5）：标签、元信息、按钮文字 `text-xs`，次级内容用 `text-base-content/60`。
- **Caption**（400, 0.6875rem, lh 1.5）：密集元信息唯一小级 `text-caption`。时间轴行、档案次级、收藏格子标注。禁止再散落任意 px/rem。
- **Readout**（600, 1rem, tracking 0.025em, tabular-nums）：overlay 读数、快捷键录制框、ConfigRow 数值列、DataWell/JsonPreBlock 全文。暗底补偿只加在 readout 的 `letterSpacing: 0.025em`，界面字不引入第三档字重。

### Named Rules

**The Two Weight Rule.** 全系统只用 400 与 600。标题与正文靠字号和色阶（`/60`、`/70`）分层，不靠字重堆叠。

**The Readout Is Mono Rule.** 凡是用户要「读数」的地方——时间、计数、坐标、快捷键、状态词——必须是等宽字体并开 tabular-nums。比例字体的数字逐帧跳动即视为 bug。

## Layout

应用壳是固定网格：顶栏 48px（`grid-rows-[48px_1fr]`），≥1024px 时左侧 240px Index Rail，内容区独立滚动；<1024px 时 Rail 收起为顶部横向 Tab Bar。窗口最小 1280×800。页面内容在 12 列 Work Grid（`AppPage`，gap-3）上排布，常规工具页限宽 `max-w-7xl`，攻略页 `max-w-none` 铺满。

间距三档按职责用，禁止单值复读到所有层级：

- **tight `0.5rem`（`gap-2`）**：组内兄弟。芯片行、按钮簇、表单控件之间。
- **base `0.75rem`（`gap-3`）**：组与组。`AppPage` 栅格、卡片之间、章节之间。
- **roomy `1.25rem`（`px-5 py-5`）**：标准卡片内边距。`card-md` / MacroHeader。工具业务卡默认 `TacticalCard size="sm"` → `px-4 py-4`，是紧凑档不是锚点失效。

行内控件（ControlTile/InlineControl）`p-4`/`p-3`。Rail 导航项 `px-3 py-2`，宏观区（MacroHeader）`gap-4`。

overlay 窗口无布局网格——它们是按 `?mode=` 进入的独立表面，跟随游戏画面，不服从应用壳。

## Elevation & Depth

**无阴影。** `--depth: 0`，所有卡片、按钮、输入框 `shadow-none`。深度完全由凹槽关系表达：容器（`base-200`）比页面底（`base-100`）更暗，像开凿进金属面板的槽；槽口由一道 1px 弹壳铜边框收口。tooltip、Dialog、JsonPreBlock 这类「浮出」元素也不加投影，只靠更深的底色（accent/neutral 槽）与边框区分层级。

### Shadow Vocabulary

无。任何 `shadow-*` / `box-shadow` 均不允许。

### Named Rules

**The Zero Shadow Rule.** 阴影恒为 0。需要「浮起」感时，加深背景色阶（base-100 → accent 槽）而不是加投影。

**The Recess, Not Raise Rule.** 容器向下沉（更暗），不向上抬（更亮或投影）。base-200 的 L 值（18.8%）必须小于 base-100（21.5%），这个方向不能反。

## Shapes

直角为主，圆角只留给「机制件」。valentine 主题：`--radius-field` 与 `--radius-box` 均 0.5rem（输入、卡片、面板微圆），`--radius-selector` 2rem（toggle、badge 这类「开关/印章」件做成胶囊）。Dialog 与按钮同取 field 的 0.5rem。CLAUDE.md 记全局 `--radius: 0`，当前实现以主题 token 的 0.5rem 为准——圆角只服务控件语义，不装饰容器。

overlay 是独立形状语言：应用内边框是铜红，overlay 边框是 `white/15–20`；应用内圆角 0.5rem，overlay 用 0.375rem（rounded-md）+ 1px 黑纱 backdrop-blur。应用内切口的锐利感在 overlay 上让位给「浮在游戏画面上的玻璃片」。

描边与高亮：激活卡片 `ring-2 ring-primary`，favorite 跳转用 1.5s 曳光红描边脉冲，Morse 框选当前步 `border-2 border-primary`。焦点环统一 `outline-primary/50`。

## Components

组件分两套词汇：**应用内**走 daisyUI class + Radix headless 行为；**overlay** 是独立的单色读数系统，不复用应用内组件样式。

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

### Region Selection Overlay（Morse / 识别框选）

全屏透明拖拽框选。已确认步骤 `border border-white/85 bg-white/10`，当前步骤 `border-2 border-primary bg-primary/12`——曳光红只标记「正在操作」的那一步，其余步骤保持白描边。指示面板用 `bg-background/88` 半透明 + 白边，不遮挡下方游戏画面。

## Do's and Don'ts

### Do:

- **Do** 容器一律比页面底更暗（base-200 < base-100），凹槽方向不可反。
- **Do** 默认边框就用弹壳铜（base-300），一道 1px 就是全部边界。
- **Do** 读数、坐标、快捷键、JSON 一律 JetBrains Mono + tabular-nums。
- **Do** 状态同时用颜色与文字/图标表达（令牌状态 = 色点 + 剩余天数，不是只有色点）。
- **Do** 新页面复用 `app-ui.tsx` 共享件（AppPage/MacroHeader/ConfigRow/DataWell/EmptyState/MacroNumber），三个以上页面同构时先扩共享件。
- **Do** 改主题 token 时三套内置主题同改同测，token key 集合保持 28 个一致。

### Don't:

- **Don't** 加任何阴影、渐变、柔光、发光边框——零阴影是硬约束。
- **Don't** 发明第二强调色；曳光弹黄不是主色，信号橙是状态色不是装饰色。
- **Don't** 用曳光红铺大面积背景或整行填充；它只做切口、激活、心跳。
- **Don't** 新增旧桌面/战术风自定义 CSS 类，不得回流 shadcn 默认视觉或 SaaS 圆角卡片+插画风格，不得用赛博朋克霓虹描边/扫描线套路。
- **Don't** 让 overlay 继承应用内铜红边框或 0.5rem 圆角；overlay 是白边玻璃片，遵守点击穿透与背景透明铁律。
- **Don't** 在正文用等宽字体、在读数用比例字体——两者各归各位。

