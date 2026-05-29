# Delta Auto Tools

这是一个基于 **Tauri 2 + React 19 + TypeScript + Vite + Bun + Rust** 的桌面工具仓库，当前提供 **摩斯密码解析工作台**、**计时\计数器工作台**、**连发器工作台** 与 **Delta API 工具**。

## 当前功能

### 摩斯密码解析

- 配置 3 个识别区域
- 使用 overlay 连续完成多区域框选
- 通过快捷键触发识别
- 在工作台顶部进行设置与测试验证
- 展示识别结果与区域级细节
- 保存设置与历史记录
- 支持热键流程下的自动输入识别结果

### 计时\计数器

- 在当前工具菜单下作为独立功能使用
- 支持多个计时器卡片，每个卡片可编辑名称、计时秒数、正/反计时方向和快捷键
- 计时器卡片支持通过拖动排序，透明窗口按相同顺序显示
- 相同快捷键会同时触发多个计时器；运行中的计时器会忽略重复触发，直到结束后才能再次触发
- 计时器透明窗口共享一个可调宽度位置，每行文本有随剩余时间减少的进度背景
- 支持多个计数器卡片，每个卡片可编辑名称、起始数和快捷键
- 计数器拥有独立透明窗口，按快捷键累加，重置按钮会恢复到设置的起始数
- 计时器和计数器各有独立总开关；关闭某一类功能后只隐藏对应透明窗口并解绑对应快捷键，配置仍持久化保留
- 透明窗口字体透明度可调；计时结束后保持终值并高亮斜体显示
- 位置设置窗口支持拖动位置，按 Enter 保存，按 Esc 退出修改

### 连发器

- 支持多个连发器卡片，每个卡片可编辑名称、触发键、目标键、连发间隔和目标键按下抖动；触发键支持单键或 Ctrl/Alt/Shift/Win 组合键（如 `Shift+-`）
- 相同触发键可绑定多张卡片，按下时同时启动独立连发会话
- 松开触发键后如果触发次数为奇数，会按全局补齐延迟范围等待后补发一次
- 全局按键最小间距可调，用于控制多个连发会话之间共享的目标键触发节流
- 连发器透明窗口支持显示/隐藏、位置设置和宽度调整

### Delta API 工具

- 账号管理支持 QQ、微信、QQ安全中心、Wegame QQ、Wegame 微信与先遣服扫码登录
- 登录流程通过 Tauri commands 直接调用原生 Rust 服务，成功后持久化到本地 SQLite
- 工具箱按账号能力展示 Wegame 运营、QQ安全中心查询与先遣服测试列表
- 游戏数据页通过已登录 QQ/微信账号查询玩家、战绩、资产、对局等数据

## 常用命令

### Graphify 安装

```bash
uv tool install graphifyy
```

使用 `uv` 管理工具时，可通过上面的命令安装 `graphifyy`。

```bash
pip install graphifyy
```

如果当前环境使用的是 `pip`，也可以直接通过该命令安装。

```bash
graphify install --platform opencode
```

安装完成后，使用该命令安装 OpenCode 相关插件。

```bash
bun run dev
```
启动前端 Vite 开发服务器。

```bash
bun run tauri dev
```
启动完整桌面开发环境，适合验证 overlay、热键、事件与原生识别流程。

```bash
bun run build
```
执行前端 TypeScript 检查与 Vite 生产构建。

```bash
bun run test
```
执行前端 Vitest 单元测试。

```bash
bun run test:coverage
```
输出前端覆盖率摘要。

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```
检查 Rust/Tauri 侧编译。

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```
执行 Rust 单元测试。

## Gitee 协作流程

- 仓库地址：`https://gitee.com/IZRINO/delta-auto-tools.git`。
- Issues 回复应说明处理结论、影响范围、验证方式和需要确认的功能点。
- 回复 Issue 后默认保持开放，等待提报者或维护者确认功能符合预期；不要回复后直接关闭。
- 只有收到明确确认、重复问题已合并追踪，或维护者明确判定无需继续处理时，才关闭 Issue。
- 修复已提交但尚未确认时，在 Issue 中标明提交/版本与验证入口，并保持待确认状态。

## 目录概览

- `src/App.tsx`：桌面壳层、当前工具菜单与 `?mode=overlay` / `?mode=timer-display` / `?mode=timer-position` / `?mode=counter-display` / `?mode=counter-position` 分支
- `src/components/app/morse-page.tsx`：摩斯密码解析主工作台容器
- `src/components/app/morse-overlay.tsx`：摩斯区域框选界面
- `src/components/app/morse-panels.tsx`：摩斯控制台、结果、区域、历史等展示块
- `src/components/app/morse-utils.ts`：摩斯纯逻辑工具函数
- `src/components/app/morse-types.ts`：摩斯前端内部共享类型与常量
- `src/components/app/timer-page.tsx`：计时\计数器工作台、透明窗口与位置设置界面
- `src/components/app/timer-utils.ts`：计时\计数器纯逻辑工具函数
- `src/components/app/timer-types.ts`：计时\计数器前端内部共享类型与常量
- `src/components/app/rapidfire-page.tsx`：连发器工作台、透明窗口与位置设置界面
- `src/components/app/rapidfire-types.ts`：连发器前端内部共享类型、常量与表单转换函数
- `src/components/app/delta-login-dialog.tsx`：Delta 账号扫码登录 Dialog
- `src/components/app/delta-login-utils.ts`：Delta 登录 invoke 参数与响应提取工具
- `src/components/app/delta-types.ts`：Delta 前端账号、能力、登录流程类型与常量
- `src-tauri/src/morse/mod.rs`：摩斯 Tauri command 与主状态入口
- `src-tauri/src/morse/overlay.rs`：摩斯区域框选状态机
- `src-tauri/src/morse/settings.rs`：摩斯设置持久化
- `src-tauri/src/morse/recognition.rs`：摩斯识别流程
- `src-tauri/src/timer/mod.rs`：计时器 Tauri command、状态、窗口和运行态编排
- `src-tauri/src/hotkeys.rs`：共享热键监听，供摩斯与计时器注册各自快捷键
- `src-tauri/src/timer/settings.rs`：计时器设置持久化
- `src-tauri/src/timer/types.rs`：计时器 Rust DTO
- `src-tauri/src/rapidfire/mod.rs`：连发器 Tauri command、状态、窗口和运行态编排
- `src-tauri/src/rapidfire/settings.rs`：连发器设置持久化
- `src-tauri/src/rapidfire/types.rs`：连发器 Rust DTO
- `src-tauri/src/delta/commands.rs`：Delta Tauri commands 与账号持久化编排
- `src-tauri/src/delta/services/`：QQ、微信、QQ安全中心、Wegame、先遣服与游戏数据服务
- `src-tauri/src/delta/storage/repo.rs`：Delta 账号 SQLite 存储

## 关键约束

- 保持白色桌面工具风格
- 不引入路由替代 `?mode=overlay`、`?mode=timer-display`、`?mode=timer-position`、`?mode=counter-display`、`?mode=counter-position`
- overlay 必须保持透明背景
- 一次进入 overlay 后应支持连续完成多个框选
- 热键录制保持前端录制、Rust 保存与注册的职责划分
- 计时器和计数器透明窗口保持无边框、透明、置顶、点击穿透，避免挡游戏
- 连发器透明窗口保持无边框、透明、置顶、点击穿透，避免挡游戏
