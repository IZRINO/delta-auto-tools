# 工具页规格格与黑标原语

日期：2026-08-21  
状态：设计已确认，进入实施  
批次：1.0 Beta 未决三项之界面换皮（另两份：工具页记忆、Beta 更新文案）

## 1. 目标

计时器、计数器、连发器、识别、摩斯五页的工作台（可重复卡片 + 同页总开关、分组、连发器全局设定）改为同一套规格格，不再只套黑标标题壳、内部继续用战地字段组。

先升级共享原语，再按页换皮。摩斯校准页不是冻结核。行为、命令、autosave、热键、拖拽、收藏、透明窗入口一律不改。

## 2. 非目标

- 不改收藏页（仍用 `TacticalCard`）。
- 不改特勤处、攻略、息屏、设置对话框的内容结构。关于页/主题面板若使用 `FieldUnit`，只接受标题条视觉变化，不重排内容。
- 不改任何 overlay / display / position 窗口。
- 不删除 `TacticalCard` / `SectionHeader` / `ControlTile` / `InlineControl` 导出（收藏页仍依赖）。
- 不新增路由、不改 Tauri command、不改 reducer / 持久化 schema。
- 不上签名 Beta 通道（见第三份 spec）。

## 3. 方案选择

三种做法：

1. **就地升级原语（采用）**：改 `FieldUnit` 标题条、改 `ConfigRow` 可放控件、新增非 `btn` 折页条。五页都吃这套。关于/主题的 `FieldUnit` 标题跟着变黑标。
2. 平行新组件（`StampUnit` / `SpecControlRow`），旧原语留作遗产。两套现场单元会漂。
3. 只改 class，结构仍是 `TacticalCard` + `Field` + `ControlTile`。没有规格格，折页 DOM 仍是 `btn`。

采用 1。用户已确认：先改原语，换皮范围含同页总开关/分组/连发器全局设定，后续决策按推荐。

## 4. 共享原语

全部放 `src/components/app/app-ui.tsx`，daisyUI token + Tailwind，禁止新战术 CSS 类。

### 4.1 FieldUnit 黑标标题

现标题是细边加粗字，不是黑标。改为标题槽本身即黑标壳：

- 容器：`card card-border bg-base-200 shadow-none`（保持）。
- 标题条：`flex items-center justify-between gap-2 border-b-2 border-base-content bg-base-200 px-3 py-2`。
- 标题文字：`font-mono text-xs font-semibold text-base-content`。`header` 仍是 `ReactNode`（字符串或名称输入）。
- 新增可选 `headerActions?: ReactNode`，贴在标题条右侧（开关、收藏、删除、拖拽、分组、序号徽章）。
- 新增可选 `description?: ReactNode`，放在标题文字下方，`text-caption text-base-content/60`，截断。
- 新增 `padBody?: boolean`，默认 `true`（`p-3`，关于/主题/摩斯混排区不破）。规格格卡片传 `false`，本体 `p-0`，让 `ConfigRow` 通栏。
- `footer`：`border-t-2 border-base-content px-3 py-2`，放动作簇（计数器 ±1/重置、识别测试、摩斯一次框选）。

黑标是 `border-b-2 border-base-content` 描边壳，不是 `bg-base-content` 反相填充。Valentine 下 `base-content` 是浅色，反相会变成白条，和现有卡片标题条不一致。现有识别卡片标题已是 `border-b-2 border-base-content`，以此为视觉锚。

### 4.2 ConfigRow 可编辑规格行

现实现把值列 `truncate`，只适合只读读数。改为可放控件：

- 栅格保持四列：`grid-cols-[max-content_minmax(0,1fr)_max-content_max-content]`，`items-center`，`border-b border-base-300 px-3 py-2`。
- 标签：`text-xs text-base-content/60`，不截断。
- 值列：`min-w-0 flex justify-end`，**去掉 `truncate`**。控件在值列内右对齐；输入/热键按钮 `w-full max-w-full`；拨档 `w-full`。
- 单位列、状态点保持。无单位时仍占空列，避免行与行错位。
- 规格行一律 `border-b`。`footer` 用 `border-t-2`，交接允许双线，不为最后一行加 `last:` 特例。
- `state` 仍可选。编辑行默认 `idle`。不要为每个输入编造 valid。
- 允许 `className`。不把规格格拆成两列。

规格格是**单列堆叠**，不要把计时器现有 `sm:grid-cols-2` 字段组原样搬进规格行。宽屏也不拆成两列规格格：标签对齐是规格格的可读性。

不进规格行的内容：

- 动作按钮（框选、±1、重置、测试、选图、删图）。
- 参考图预览、识色探针列表这类块状编辑器。这些放 `FieldUnit` 体内独立区块，或折页内部，仍用通栏边框行，不套 `Field`/`ControlTile`。

### 4.3 StampFold 折页条

新组件。原生 `<button type="button">`，**禁止**走 `Button`，**禁止** `btn` / `btn-ghost` / `btn-outline`。

```text
class: w-full flex items-center justify-between gap-2
       border border-base-content bg-transparent
       px-3 py-2 font-mono text-xs font-semibold
       text-left
```

- 左侧：标题。右侧：可选 `trailing`（徽章、计数）+ 箭头。
- 箭头随 Radix `data-state=open` 旋转。
- 与 `CollapsibleTrigger asChild` 配合。
- 折页展开区只放 `ConfigRow` 或通栏行，不要再套 `ControlTile`。

替换点：

- 摩斯「点击区域配置」
- 连发器卡片「高级校准面板」
- `SyncGroupSection`「显示参数」
- 删除无调用方的 `DisplaySettingsInline`

帮助圆钮仍是 daisyUI `btn-circle`，本 spec 不动。

### 4.4 HotkeyField

增加 `labeled?: boolean`，默认 `true`（单独使用仍带 `Field`/`FieldLabel`）。放进 `ConfigRow` 时 `labeled={false}`，标签由规格行提供，避免双标签。

## 5. 页面映射

五页工作台凡 `TacticalCard` + `SectionHeader` + `ControlTile`/`Field` 组，改为 `FieldUnit` + `ConfigRow` / `StampFold` / `footer`。`ChannelTabs` 保留。

### 5.1 计时器（样板页，先做）

- 总开关：`FieldUnit header="总开关"`，一行 `ConfigRow` 启用开关。
- 分组：`SyncGroupSection` 每组一个 `FieldUnit`。标题条：组名输入 + 启用 + 删 + 位置。`StampFold`「显示参数」→ 透明度、窗口宽度两行规格格。
- 卡片：`padBody={false}`。标题条：名称输入、`description` 运行读数、分组选择、拖拽、收藏、启用、删除、序号。本体：每段秒数、方向、触发模式、多段数、运行中忽略、快捷键。全部 `ConfigRow`。

### 5.2 计数器

同构。`footer`：−1 / +1 / 重置为起始数（仍用 `Button`）。

### 5.3 连发器

页级三块战术卡片（全局设定、通道分组、其他）改 `FieldUnit` + 规格格。卡片同构；「高级校准」改 `StampFold`，展开后触发抖动/间距等为规格行。运行中状态条可留在标题条下，不进规格行。

### 5.4 识别

总开关改 `FieldUnit`。分组不是折页条：每组一个 `FieldUnit`，标题条含折叠按钮、组名、启用、空组删除；`group.collapsed` 为真时不渲染卡片。`RecognitionCardEditor` 外壳改 `FieldUnit`。触发源、热键、区域阈值、冷却等为规格行。参考图列表、音频列表、识色探针保持块状编辑，不塞进四列规格行。reducer、memo `compare`、测试播放逻辑不改。

### 5.5 摩斯

`FieldUnit` 标题自动变成黑标。校准页已有规格行，去掉值列截断后输入框不再被裁。窗位：每个窗位一行规格格（标签=窗位名，值=坐标或「未配置」+「框选/重选」按钮，状态点锁定/待锁定）。`footer` 只放「一次框选三段」。点击区域改 `StampFold`。报码/历史保持 `DataWell` 类信息块。

## 6. 数据流与错误

无新状态。失败仍显示在页级 `FieldError` / `alert`，不因换皮改写入路径。`runStateClass` 继续加在 `FieldUnit` 的 `className` 上（运行中描边）。

## 7. 测试

- `ConfigRow`：值列 class 不含 `truncate`；可渲染按钮子节点。
- `StampFold`：静态 markup 不含 `btn` 类；含 `border-base-content`。
- `FieldUnit`：有 `header` 时标题条含 `border-b-2`。
- 识别卡片 editor 的 memo 测试随 markup 更新 class 断言，不改行为断言。
- 连发器 ChannelTabs 契约测试不动。
- 不写视觉回归截图。

## 8. 实施顺序

1. 原语（FieldUnit / ConfigRow / StampFold / HotkeyField `labeled`）+ 单测。
2. 计时器整页（含 `SyncGroupSection`）验收。
3. 计数器。
4. 连发器。
5. 识别。
6. 摩斯收尾。
7. 删 `DisplaySettingsInline`。

一步一页。禁止五页同时改。

## 9. 文档

- `DESIGN.md`：写明 `FieldUnit` 黑标条、`ConfigRow` 可编辑、`StampFold` 禁止 `btn`。
- `droid-wiki/how-to-contribute/patterns-and-conventions.md`：新页面优先 `FieldUnit`+`ConfigRow`，不新建战术卡片。
- 不改各工具 feature wiki 的行为段（行为未变）。
