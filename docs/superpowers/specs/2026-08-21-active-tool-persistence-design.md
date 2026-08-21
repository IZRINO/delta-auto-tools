# 工具页记忆

日期：2026-08-21  
状态：设计已确认，进入实施  
批次：1.0 Beta 未决三项之应用壳（另两份：规格格换皮、Beta 更新文案）

## 1. 目标

主窗口刷新或重启后仍打开上次工具页。现实现把 `activeTool` 放在 `AppShell` 的 `useState`，默认摩斯，无持久化。主题、总开关、收藏已用 `delta-auto-tools:` 前缀的 localStorage。

## 2. 非目标

- 不加 React Router，不把工具页写进 URL。`?mode=` 继续专供 overlay。
- 不持久化卡片滚动位置、折叠分组、识别当前编辑卡、摩斯 ChannelTabs。
- 不按 Profile 分工具页。切 Profile 只重载配置，留在当前工具。
- overlay / display / position 窗口不读写此键（它们 early-return，也不得在 mount 时把主窗口记下的工具盖掉）。

## 3. 方案选择

1. **localStorage（采用）**：与收藏/主题同模式，WebView 刷新和进程重启都活。无后端 command。
2. 查询参数 `?tool=`。和 `?mode=` 抢解析，overlay 窗口会带错参数或被主窗口污染。
3. Rust 设置文件。为 UI 选页加 command 和 revision，过重。

## 4. 设计

### 4.1 键与值

- 键：`delta-auto-tools:active-tool:v1`
- 值：纯字符串，合法集合与 `App.tsx` 的 `ToolId` 同步：

```text
timer | counter | rapidfire | strategy | recognition | privacyScreen | specialOps | morse | favorites
```

### 4.2 解析

抽纯函数到 `src/components/app/active-tool.ts`，作为合法 id 的唯一来源：

- `ACTIVE_TOOL_STORAGE_KEY`
- `ACTIVE_TOOL_IDS`（上表元组）
- `type ToolId = (typeof ACTIVE_TOOL_IDS)[number]`
- `parseActiveTool(raw: string | null): ToolId`：命中集合原样返回；`null`、空串、未知值 → `"morse"`（保持现默认）。

`App.tsx` 的 `ToolId` 改为从这个模块导入。侧栏 `tools` / `deltaTools` 的 `id` 必须是该联合成员；新增工具页先改 `ACTIVE_TOOL_IDS`。

读写包 `try/catch`。隐私模式或配额满时降级为内存态，不抛到 UI。

### 4.3 读写时机

- `AppShell`：`useState<ToolId>(() => parseActiveTool(localStorage.getItem(key)))`。SSR/无 `window` 时直接 `"morse"`。
- 所有会改当前工具的入口走同一个 `selectTool(id)`：写入 state + localStorage。覆盖：
  - 左侧 Index Rail
  - 顶栏 `TopTabBar`
  - `handleFavoritesNavigate`（从收藏跳到 timer/counter/rapidfire）
- overlay 分支在 `selectTool` 之前 return，且 **禁止** mount 时 `setItem`。只读初始化也不会写；不要加「把默认值回写存储」的 effect，避免主窗口在摩斯、overlay 同时启动时互相覆盖（共享 origin 的 localStorage）。

### 4.4 非法值

存储被手改成 `"delta"` 或旧 id：解析回摩斯，**不**把摩斯写回（等用户下一次主动切换再写）。避免用默认值污染用户尚未打开主窗口时的键。

## 5. 测试

`active-tool.ts` 单测：

- 九个合法 id 原样返回
- `null` / `""` / `"nope"` / `"Timer"` → `morse`
- 不测 `AppShell` 整页

## 6. 文档

`droid-wiki/overview/architecture.md` 或 `patterns-and-conventions.md` 补一句：主窗口当前工具页存在 `delta-auto-tools:active-tool:v1`。不改 README。
