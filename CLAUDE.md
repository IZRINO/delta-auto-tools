# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概览

这是一个 **Tauri 2 + React 19 + TypeScript + Vite + Bun + Rust** 的桌面工具仓库，当前核心功能是“摩斯密码解析”工作台。

应用分成两部分：
- **前端桌面界面**：`src/`
- **Tauri 原生能力与识别逻辑**：`src-tauri/`

当前不是通用 Web 应用，也没有路由系统；主界面和 overlay 都由 `src/App.tsx` 基于查询参数切换。

## 常用命令

### 开发

```bash
bun run dev
```
仅启动前端 Vite 开发服务器。

```bash
bun run tauri dev
```
启动完整桌面应用开发环境。**涉及界面交互、overlay、热键、Tauri invoke/event、原生识别链路时，优先使用这个命令验证。**

### 构建

```bash
bun run tauri build
```
执行桌面应用构建；Tauri 会先跑前端构建。

```bash
bun run build
```
执行前端 TypeScript 检查与 Vite 构建。

### 测试

```bash
bun run test
```
执行前端 Vitest 单元测试，当前重点覆盖 `src/components/app/morse-utils.ts` 中的纯逻辑。

```bash
bun run test:coverage
```
输出前端覆盖率摘要。

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```
执行 Rust 单元测试，当前重点覆盖 overlay、settings、types 与历史裁剪逻辑。

### Rust 侧检查

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```
检查 Tauri/Rust 侧是否可编译。

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
```
格式化 Rust 代码。

## 运行与端口约定

Tauri 配置在 `src-tauri/tauri.conf.json`：
- `beforeDevCommand`: `bun run dev`
- `devUrl`: `http://localhost:1420`
- `beforeBuildCommand`: `bun run build`
- `frontendDist`: `../dist`

默认开发端口：
- Vite: `1420`
- 当设置 `TAURI_DEV_HOST` 时，HMR 使用 `1421`

不要随意改这些端口假设；PM2 和 Tauri dev 流程都依赖它们。

## 代码结构

### 前端入口链路

```text
index.html
  -> src/main.tsx
  -> src/App.tsx
  -> src/components/app/morse-page.tsx
```

关键点：
- `src/main.tsx` 在根部提供 `TooltipProvider`，依赖 tooltip 的 UI 组件必须通过这个入口渲染。
- `src/App.tsx` 负责两种模式：
  - 正常桌面壳层
  - `?mode=overlay` 直接进入区域框选层
- **不要引入路由来实现 overlay**；当前约定就是查询参数分支。

### 前端主功能文件

- `src/App.tsx`
  - 桌面壳层
  - 侧边栏与主标题
  - 判断 `mode=overlay`
- `src/components/app/morse-page.tsx`
  - 主工作台容器
  - 设置加载/保存
  - 区域框选入口
  - 热键录制状态
  - 识别结果与历史记录
  - Tauri `invoke` / `listen` 对接
- `src/components/app/morse-overlay.tsx`
  - overlay 前端交互
  - 拖拽框选 UI
  - 多步骤状态提示
- `src/components/app/morse-panels.tsx`
  - 控制台、结果、区域、历史等展示块
- `src/components/app/morse-utils.ts`
  - 纯函数工具与格式化逻辑
- `src/components/app/morse-types.ts`
  - 页面内部共享类型与常量
- `src/App.css`
  - Tailwind v4 导入
  - shadcn 主题 token
  - 桌面壳层样式
  - overlay 模式透明背景控制
- `src/components/ui/*`
  - shadcn/ui 风格基础组件

### 原生入口链路

```text
src-tauri/src/main.rs
  -> src-tauri/src/lib.rs
  -> src-tauri/src/morse/mod.rs
  -> src-tauri/src/morse/*
```

关键点：
- `src-tauri/src/lib.rs` 注册 Tauri plugin、state 和 command。
- `src-tauri/src/morse/mod.rs` 是摩斯功能模块入口，负责：
  - 初始化设置与热键
  - 管理全局状态
  - 暴露 Tauri commands
  - 触发识别流程
  - 写入历史记录

### Rust morse 模块职责

- `src-tauri/src/morse/types.rs`
  - 前后端共享数据结构：设置、结果、历史、区域进度
- `src-tauri/src/morse/settings.rs`
  - 设置持久化到 `morse_settings.json`
  - 使用 `app.path().app_config_dir()` 下的配置目录
  - 包含可测试的文件读写与序列化辅助逻辑
- `src-tauri/src/morse/overlay.rs`
  - 多步骤区域选择状态机
  - 创建全屏透明 overlay 窗口
  - 管理一次会话中 1~3 个区域的 staged 结果
- `src-tauri/src/morse/recognition.rs`
  - 截图、灰度化、阈值二值化、连通域检测、摩斯符号识别
  - 使用 `xcap` 读取屏幕区域，`image` 做图像处理
- `src-tauri/src/morse/decoder.rs`
  - 摩斯结果转数字
- `src-tauri/src/morse/input.rs`
  - 使用 `enigo` 自动输入识别结果

## 当前核心业务流

### 1. 启动

- `morse::initialize()` 读取本地设置
- 注册当前热键
- 将 `MorseState` 注入 Tauri state

### 2. 前端加载

`morse-page.tsx` 启动后会调用：

```text
morse_get_bootstrap
```

拿到：
- 当前设置
- 最近一次识别结果
- 历史记录

### 3. 区域框选

前端调用：

```text
morse_begin_region_selection(slots)
```

Rust 侧会：
- 检查当前是否已有识别任务或框选任务
- 创建透明全屏 overlay 窗口
- 将待完成的 slot 会话放进 `pending_selection`

前端 overlay 在同一轮会话中依次提交多个框：

```text
morse_overlay_submit_selection(slot, rect)
```

实现细节：
- `overlay.rs` 里使用 `staged_regions` 暂存本轮会话结果
- **只有最后一步完成时才真正保存 settings**
- 中途取消不会污染已保存配置

取消时调用：

```text
morse_overlay_cancel_selection(slot)
```

### 4. 识别

前端调用：

```text
morse_run_recognition(autoType?)
```

Rust 侧会：
- 检查 3 个区域是否已配置
- 对每个区域截图并识别
- 聚合 3 位结果
- 可选自动输入
- 发出事件：`morse://run-finished`

前端会监听该事件更新结果与历史。

### 5. 热键保存

前端热键 UI 只是**录制快捷键字符串**，真正的解绑/注册逻辑在 Rust：

- `morse_save_settings()` 会比较新旧热键
- 若热键有变化：
  1. 先解绑旧热键
  2. 再注册新热键
  3. 如果保存失败或注册失败，则尝试回滚旧热键

**不要在前端重复实现热键注册逻辑。**

## UI / 交互约束

这些是当前仓库已经形成的约定，改动时要保持：

- 保持 **白色桌面工具风格**，不要改回营销页或 dashboard 模板感。
- overlay 入口必须继续支持：

```text
?mode=overlay
```

- overlay 模式必须保持透明背景，不能重新引入整页白底或重度灰幕遮挡真实屏幕。
- 区域选择体验应保持“**一次进入 overlay，连续完成多个框选**”，不要退回旧的“三次独立 session”。
- 热键输入应保持“录制式交互”，不是普通文本输入框。

## 组件与样式约定

- 使用 Bun，不要切换到 npm / pnpm / yarn。
- 使用仓库已有别名：
  - `@/components`
  - `@/components/ui`
  - `@/lib`
  - `@/hooks`
- Tailwind v4 是 **CSS-first** 配置，主题 token 在 `src/App.css`，不是 `tailwind.config.*`。
- 现有 UI 基础组件已经在 `src/components/ui/*`，优先复用。

## PM2 服务

| Port | Name | Type |
|------|------|------|
| 1420 | delta-auto-tools-1420 | vite |
| app | delta-auto-tools-tauri | tauri |

`delta-auto-tools-tauri` 会先等待 `1420` 端口就绪，再启动 Tauri dev。

相关文件：
- `ecosystem.config.cjs`
- `scripts/wait-for-port.cjs`
- `.claude/commands/pm2-*.md`
- `.claude/scripts/pm2-*.ps1`

## 变更时优先相信什么

如果文档与代码不一致，优先以这些文件为准：
1. `src-tauri/tauri.conf.json`
2. `package.json`
3. `src-tauri/src/lib.rs`
4. `src/` 与 `src-tauri/src/` 的实际代码

`README.md` 和 `AGENTS.md` 可能滞后于当前实现，修改较大功能后记得一并同步。

## 当前已知注意点

- 当前仓库里没有完善的前端测试体系与脚本；本轮新增的是 Vitest 单测与覆盖率脚本。
- `src/components/app/morse-page.tsx` 已按容器职责整理；新增展示块与工具模块时应保持职责清晰。
- 热键录制当前基于浏览器键盘事件格式化字符串；如果后续修复特殊组合键兼容性，优先保持与 `tauri_plugin_global_shortcut` 的格式一致。
- `src/App.css` 同时承载主题 token 和桌面壳层样式，改 overlay 背景时不要破坏正常主界面背景。
