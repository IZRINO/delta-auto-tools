# Delta Auto Tools

这是一个基于 **Tauri 2 + React 19 + TypeScript + Vite + Bun + Rust** 的桌面工具仓库，当前提供 **摩斯密码解析工作台**。

## 当前功能

- 配置 3 个识别区域
- 使用 overlay 连续完成多区域框选
- 通过快捷键触发识别
- 在工作台顶部进行设置与测试验证
- 展示识别结果与区域级细节
- 保存设置与历史记录
- 支持热键流程下的自动输入识别结果

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

## 目录概览

- `src/App.tsx`：桌面壳层与 `?mode=overlay` 分支
- `src/components/app/morse-page.tsx`：主工作台容器
- `src/components/app/morse-overlay.tsx`：overlay 框选界面
- `src/components/app/morse-panels.tsx`：控制台、结果、区域、历史等展示块
- `src/components/app/morse-utils.ts`：纯逻辑工具函数
- `src/components/app/morse-types.ts`：前端内部共享类型与常量
- `src-tauri/src/morse/mod.rs`：Tauri command 与主状态入口
- `src-tauri/src/morse/overlay.rs`：区域框选状态机
- `src-tauri/src/morse/settings.rs`：设置持久化
- `src-tauri/src/morse/recognition.rs`：识别流程

## 关键约束

- 保持白色桌面工具风格
- 不引入路由替代 `?mode=overlay`
- overlay 必须保持透明背景
- 一次进入 overlay 后应支持连续完成多个框选
- 热键录制保持前端录制、Rust 保存与注册的职责划分
