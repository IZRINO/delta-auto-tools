# DESIGN.md — Delta Industrial Brutalist Interface

> 本文件是下一轮 UI 完全重写的视觉源头。不要在原界面上修饰、套皮、微调圆角或替换颜色；按本文件重建界面结构。功能入口与业务能力保持一致，视觉框架从零开始。

## 0. Non-Negotiable Direction

**目标：工业粗粝 / 解密军工档案 / 机械控制台。**

界面必须像一套被拆开的作战设备控制面板：粗黑结构线、压迫感网格、巨型编号、单色告警、密集 telemetry、机械标签、扫描线与纸面噪声。它不是白色 SaaS 后台，不是柔和桌面工具，不是 shadcn 默认卡片堆叠，也不是“在旧 UI 上加一点工业感”。

设计判断标准：截图缩小到 25% 时，仍能看到强烈的黑白结构块、红色警戒线、机械分区和非消费级界面气质。

## 1. Chosen Archetype

采用 **Swiss Industrial Print × Declassified Tactical Control Board**。

- 主基底：浅色新闻纸 / 旧工业说明书。
- 主结构：黑色粗线、硬格、编号、分栏、轴线。
- 数据层：军用 telemetry 小字、坐标、状态码、键位、时间、计数。
- 告警层：单一航空红，只用于当前选择、危险动作、运行态、关键按钮。
- 禁止混入暗色 CRT 作为整体主题。深色只可用于小型数据井、JSON 原始数据、透明游戏叠加窗。

## 2. Required Functional Parity

重写 UI 时只保留功能等价，不保留旧视觉结构。

必须覆盖这些功能表面：

1. 工具导航：Morse、计时/计数、连发器、攻略网站、Delta 账号、游戏数据、工具箱、收藏/未开放工具。
2. Morse：区域选择、识别工作台、结果、历史、自动输入、热键录制。
3. 计时/计数：计时器卡片、计数器卡片、总开关、透明窗口设置、位置校准、排序、手动触发/重置。
4. 连发器：卡片配置、触发键、目标键、间隔、抖动、补齐策略、总开关、透明窗口、排序。
5. 攻略网站：站点切换、自定义站点、刷新档位、当前 URL、内嵌网页区域。
6. Delta：账号列表、登录流程入口、账号选择、游戏数据加载/重试、工具操作入口、原始响应展示。
7. 透明叠加窗：保持游戏可见性、置顶、点击穿透、位置设置功能。

功能一致不等于布局一致。允许重新组织信息层级、导航方式、卡片形态和页面骨架。

## 3. Visual Laws

### 3.1 Geometry

- 90 度直角。默认 `border-radius: 0`。
- 禁止 pill、胶囊按钮、圆润 SaaS 卡片。
- 所有区域必须落在可见网格内。
- 主结构线使用 2px 黑线；内部细分线使用 1px 黑线或灰线。
- 大面板不靠阴影浮起，靠边框、编号、分区标题和填充密度区分层级。

### 3.2 Substrate

主界面像工业印刷物，不像现代玻璃拟态。

- 背景必须有细微纸面噪声或半调纹理。
- 允许使用 `repeating-linear-gradient` 做工程纸网格。
- 禁止柔和渐变、毛玻璃、彩色阴影、外发光。
- 禁止大面积半透明叠层；透明感只用于游戏 overlay，不用于主窗口。

### 3.3 Structural Contrast

每个页面必须同时存在：

- 一个巨大结构元素：页面编号、工具代号、竖排编号、超大标题或占据整列的黑白块。
- 一个高密度数据区：状态矩阵、配置清单、日志、键位表、运行 telemetry。
- 一个红色操作焦点：当前工具、运行中状态、主按钮或选中项。

没有巨大结构元素的页面不合格；只有普通卡片列表的页面不合格。

## 4. Color System

只使用单基底、单强调色。颜色必须粗暴、印刷、功利。

| Role | Color | Usage |
|---|---:|---|
| Paper | `#F1EFE8` | 主背景，旧纸/档案页 |
| Bone | `#DDD8CC` | 次级底板、禁用区、表格隔行 |
| Ink | `#080808` | 主文字、粗边框、结构块 |
| Steel | `#3B3B36` | 次级文字、图标、辅助边框 |
| Ash | `#8A867B` | 元信息、占位、弱标签 |
| Line | `#B9B2A4` | 细网格线、内部分隔 |
| Alert Red | `#E11919` | 唯一通用强调色 |
| Warning Amber | `#A36A00` | 仅语义警告 |
| Valid Green | `#3F6B2A` | 仅语义成功/有效 |
| Data Well | `#141414` | 小型数据井、JSON、叠加窗文本底 |

### Color Rules

- Alert Red 只能占画面 3%–8%。太多会变成游戏 UI 皮肤，太少没有 brutalist 冲击。
- 主按钮可以红底黑字或黑底纸白字 + 红色侧线。
- 普通按钮必须黑白硬边，不得使用柔和填充。
- 状态色只表达状态，不参与品牌与装饰。
- 禁止蓝紫霓虹、橄榄柔和主题、彩虹渐变、玻璃高光。

## 5. Typography

Typography 是主体装饰，不是附属文本。

### 5.1 Macro Type

用于页面标题、编号、工具代号。

- 字体：`Arial Black`, `Impact`, `Bahnschrift Condensed`, `DIN Condensed`, `Arial Narrow`, sans-serif。
- 大小：`clamp(48px, 9vw, 148px)`。
- 行高：0.82–0.95。
- 字距：-0.06em 到 -0.02em。
- 只用 uppercase 英文/数字作为视觉块；中文标题可放在旁边作为小型说明。
- 标题可以被网格线切割、压到边缘、竖排或跨栏。

### 5.2 Micro Type

用于状态、坐标、键位、时间、标签、按钮说明。

- 字体：`JetBrains Mono`, `IBM Plex Mono`, `Consolas`, monospace。
- 大小：10px–13px。
- 字距：0.06em–0.12em。
- 全部 uppercase 英文标签；中文只用于说明与业务文案。
- 数字必须 tabular nums。

### 5.3 Body Type

- 字体：`Bahnschrift`, `Arial Narrow`, `Segoe UI`, sans-serif。
- 默认 14px–15px。
- 不写营销口吻，不写抽象愿景。
- 文案像设备说明：短句、命令式、明确后果。

## 6. Layout System

### 6.1 App Shell

废弃常规“左侧圆润 Sidebar + 右侧卡片内容”的视觉模型。

新 shell 必须是三段式机械界面：

1. **Top Manifest Bar**
   - 横跨全宽。
   - 包含产品代号、当前模块编号、运行状态、时间/版本占位、窗口控制区。
   - 高度紧凑，黑底或纸底黑线均可。

2. **Left Index Rail**
   - 像档案索引或设备槽位，不像普通菜单。
   - 每个工具是编号条目：`01 / MORSE`、`02 / TIMER`、`03 / RAPID`。
   - 当前项必须有粗红条或黑底反白。
   - 收藏不是星星装饰，而是 `PINNED` / `MARKED` 状态标签。

3. **Main Work Grid**
   - 使用 12 列或 16 列 grid。
   - 左上保留巨大模块编号/工具代号。
   - 右侧或底部承载密集操作面板。
   - 页面不能居中成普通内容流；必须贴边、分栏、切割。

### 6.2 Page Composition

每个工具页采用以下骨架，但布局可变：

```text
┌──────────────────────────────────────────────────────────────┐
│ TOP MANIFEST / APP STATUS / MODULE COORDINATES               │
├──────────────┬───────────────────────────────────────────────┤
│ INDEX RAIL   │ MACRO MODULE HEADER                            │
│              ├───────────────┬───────────────┬───────────────┤
│              │ STATUS MATRIX │ PRIMARY OPS   │ SECONDARY OPS │
│              ├───────────────┴───────────────┴───────────────┤
│              │ CONFIG / LOG / DATA GRID                       │
└──────────────┴───────────────────────────────────────────────┘
```

### 6.3 Density Rules

- 空白必须是“结构性空白”，用于衬托巨大编号或分栏，不是松散 padding。
- 表单区域可密集，但 label、值、错误必须对齐成列。
- 列表项必须像装备清单：编号、状态、参数、动作在固定列。
- 移动端可降为单列，但仍保留编号、粗线、黑白块。

## 7. Component Redesign

### 7.1 Buttons

按钮像机械开关，不像网页按钮。

- 直角、2px 黑边。
- Primary：红底黑字，或黑底纸白字 + 红色左边条。
- Secondary：纸底黑字，hover 反相为黑底纸字。
- Destructive：黑底红字或红色斜线标记，不用柔和 destructive 背景。
- Active：整体下移 1px，不使用弹性动画。
- Icon 必须像技术标记，不用圆形图标底。

### 7.2 Navigation Item

导航项是档案索引条。

- 固定高度，左侧编号，右侧英文代号，下方中文名。
- 当前项黑底反白 + 红色竖条。
- 未开放项用斜线纹理或 `LOCKED` 标签。
- hover 只改变边框/反相，不加阴影。

### 7.3 Panels

面板是 `FIELD UNIT`。

- 头部必须有机器标签：`[ UNIT 03 ]`、`CONFIG / FIRE CONTROL`。
- 右上角可以有 `REV`, `SYNC`, `ARMED`, `IDLE` 等状态码。
- 内容区按 grid 分隔。
- 重要面板用粗黑顶部条，不用彩色卡片头。

### 7.4 Status Matrix

替代旧统计卡。

- 小格子矩阵，每格包含 label、value、unit、state。
- 数值巨大或等宽突出。
- 状态靠红条、反相、符号：`●`, `■`, `//`, `>>>`。
- 不用圆形进度、不用柔和 badge 云。

### 7.5 Forms

表单像军工配置表。

- Label 左对齐成固定列。
- 输入框直角、黑边、纸底。
- Focus：2px Alert Red outline。
- 错误：红色粗线 + 简短错误文字。
- 热键录制：输入区反相黑底，显示 `REC >>> KEY INPUT`。

### 7.6 Cards / Config Rows

计时器、计数器、连发器卡片不再像卡片，改成配置行或可展开设备单元。

- 每行左侧大编号：`T-01`, `C-04`, `RF-02`。
- 中间是参数矩阵。
- 右侧是动作列。
- 拖拽排序使用 `GRIP ////` 或黑色条纹，不用可爱拖拽图标。
- 运行中整行反相或红条，不用柔和高亮。

### 7.7 Dialogs

弹窗像检修单，不像居中营销模态。

- 黑色标题条。
- 粗边框，直角。
- 内容分成编号步骤。
- QR 登录弹窗必须有明确状态机：`WAITING`, `SCANNED`, `CONFIRM`, `EXPIRED`, `FAILED`。

### 7.8 Tables / Raw Data

- JSON 与原始响应使用 Data Well：深色底、白字、红色光标线可选。
- 表格使用 1px 黑线网格，不使用 zebra SaaS 风。
- 空值显示 `—` 或 `NULL`，不要留空。

### 7.9 Empty / Error States

空态必须像系统检修页。

- 大编号：`NO DATA / 404 FIELD`。
- 一句话说明。
- 一个明确动作。
- 错误态使用红色斜线/边框，不使用插画。

## 8. Page-Specific UI Concepts

### 8.1 Morse Workbench

视觉概念：信号破译台。

- 巨型标题：`MORSE / DECODER`。
- 区域选择显示为三段坐标槽：`REGION A/B/C`。
- 识别结果像电报码输出：黑底 Data Well + 红色当前输出线。
- 历史记录是窄表格，不是时间线卡片。

### 8.2 Timer / Counter

视觉概念：任务时序板。

- 页面主编号：`SYNC BOARD`。
- Timer 与 Counter 是两个并列系统，不是柔和 Tab 卡片。
- 每个项目是时序配置行，显示 duration / direction / hotkey / state。
- 透明窗口设置像坐标校准面板。

### 8.3 Rapidfire

视觉概念：火控矩阵。

- 页面主编号：`FIRE CONTROL`。
- 总开关必须像 ARM/DISARM 机械开关。
- 卡片行显示 trigger → target、interval、jitter、compensation。
- 运行中用红色 `ARMED` / `FIRING` 状态条。

### 8.4 Strategy Browser

视觉概念：情报终端嵌入作战板。

- 顶部工具条像 URL 贴纸与频道选择器。
- 站点 Tab 是硬边频道按钮：`CH-01`, `CH-02`。
- 当前 URL 用 mono 小字横向压缩显示。
- Web 内容区域必须被粗边框框住，像嵌入式显示器。

### 8.5 Delta Accounts

视觉概念：身份凭据档案柜。

- 每个账号是档案卡：kind、uin/openid、token state、capability。
- 登录入口是编号流程，不是普通按钮组。
- 过期状态用红色 stamped mark。

### 8.6 Game Data

视觉概念：战场资产台账。

- 主数据加载显示为状态矩阵。
- 查询结果优先表格/数据井。
- Loading 是 `FETCHING // IDE GATEWAY` 文本状态，不使用柔和 spinner 作为唯一反馈。

### 8.7 Toolbox

视觉概念：操作命令面板。

- 每个工具是 Command Unit。
- 执行动作必须有确认状态和结果井。
- PC/手机模式切换做成硬件拨档。

## 9. Texture & Degradation

必须加入可控粗粝纹理，否则会退化成普通极简 UI。

允许：

- 纸面噪声：低透明度 SVG noise 或 CSS 纹理。
- 半调点：用于大标题背景或空态，不覆盖正文。
- 斜线警戒纹：用于 locked、disabled、warning 区。
- 扫描线：仅用于 Data Well 和透明叠加窗，不铺满主界面。
- 轻微 misregistration：红色边线可偏移 1px，模拟印刷错位。

禁止：

- 影响文字可读性的噪声。
- 高成本持续动画噪声。
- 真实图片依赖作为核心 UI。
- 随机每次渲染导致布局变化。

## 10. Interaction & Motion

- 默认静态。工业 brutalist 靠结构，不靠动效。
- Hover：反相、边框变粗、红线出现。
- Active：下压 1px。
- Focus：红色硬 outline，不用柔光 ring。
- Loading：文本状态码 + 进度条纹；spinner 只能辅助。
- 新增/触发反馈：一次性红色 flash 或黑白反相，不超过 400ms。

## 11. Overlay Exceptions

透明游戏叠加窗不套主窗口纸面风，但要保持同一工业语汇。

- 背景仍透明或极低透明深色。
- 字体使用 mono，状态条使用红/白/黑。
- 边框直角，允许扫描线。
- 不能破坏置顶、点击穿透、拖动定位、可读性。
- 位置设置窗口可以更像校准靶：十字线、坐标、确认/取消提示。

## 12. Implementation Mandate

下一轮实现时：

1. 先建立新的全局 token、字体、背景纹理、边框系统。
2. 再建立新的 shell：Top Manifest Bar、Left Index Rail、Main Work Grid。
3. 再建立新的基础组件：机械按钮、索引导航项、状态矩阵、配置行、数据井、检修弹窗。
4. 最后逐页迁移功能表面。
5. 不为了复用旧视觉组件而牺牲本文件方向。
6. 不以“功能没变”为理由保留旧页面结构；功能一致即可，视觉和布局必须重做。

## 13. Acceptance Checklist

任一页面完成后必须自检：

- [ ] 是否完全直角，几乎没有圆角？
- [ ] 是否存在粗黑结构线和明确网格？
- [ ] 是否有巨大模块编号/代号？
- [ ] 是否有高密度 telemetry 或状态矩阵？
- [ ] 红色是否只作为关键强调，而不是到处装饰？
- [ ] 是否看起来像工业档案/军工控制板，而不是 SaaS 后台？
- [ ] 是否没有柔和阴影、玻璃、渐变、胶囊、插画？
- [ ] 是否在 25% 缩略图下仍有强烈结构识别？
- [ ] 是否保留原功能入口和操作能力？

## 14. Banned Outcomes

- 在旧 UI 上换色。
- 保留普通 Sidebar + 圆角 Card + Hero 的页面模型。
- 使用“战术白色操作台”“轻量军规仪表感”等旧方向。
- 把 industrial-brutalist 做成普通浅色 dashboard。
- 把 brutalist 做成全黑代码编辑器。
- 用大量 Tailwind raw 色堆视觉。
- 用装饰动画代替结构。
- 新增虚构数据、虚构用户、营销 slogan。
- 为了视觉改变功能语义、命令、持久化、热键状态机或透明窗口能力。
