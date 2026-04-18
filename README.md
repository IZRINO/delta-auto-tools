# delta-auto-tools

一个基于 **Tauri 2 + React 19 + TypeScript + Vite + Bun + Rust** 的桌面工具仓库，当前核心功能是 **Morse 识别工作台**。

当前真实运行形态是：
- 主界面：白色桌面工具壳层 + Morse 工作台
- Overlay 模式：通过 `?mode=overlay` 进入透明区域框选层
- 原生能力：由 Tauri command 提供，不经过 HTTP API

## 当前功能

- 配置 3 个识别区域
- 一次进入 overlay，连续完成多个区域框选
- 保存识别设置与热键
- 运行识别并展示结果
- 保存最近结果与历史记录
- 可选自动输入识别结果

## 技术栈

- 前端：React 19、TypeScript、Vite、Tailwind CSS v4、shadcn/ui
- 桌面运行时：Tauri 2
- 原生侧：Rust、tokio、xcap、image、enigo、tauri-plugin-global-shortcut
- 开发工具：Bun、Cargo、PM2

## 常用命令

```bash
bun run dev
bun run build
bun run preview
bun run tauri dev
bun run tauri build
cargo check --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml
```

说明：
- `bun run dev`：仅启动前端 Vite
- `bun run tauri dev`：启动完整桌面应用；涉及 overlay、热键、Tauri invoke/event、原生识别时优先使用
- `bun run build`：前端构建
- `cargo check --manifest-path src-tauri/Cargo.toml`：检查 Rust/Tauri 编译

## 入口与结构

- 前端入口：`index.html` → `src/main.tsx` → `src/App.tsx`
- 原生入口：`src-tauri/src/main.rs` → `src-tauri/src/lib.rs`
- 前端核心业务：`src/components/app/morse-page.tsx`
- 原生核心业务：`src-tauri/src/morse/*`

主要目录：
- `src/`：桌面界面与 overlay 前端交互
- `src-tauri/src/`：Tauri command、状态机、识别与设置持久化
- `docs/CODEMAPS/`：面向 AI/维护者的架构摘要文档
- `.claude/commands/`：PM2 相关快捷命令

## 当前架构要点

- `src/App.tsx` 负责普通桌面模式和 `?mode=overlay` 分支
- `src/components/app/morse-page.tsx` 负责设置、框选、识别结果、历史、热键录制
- `src-tauri/src/morse/mod.rs` 暴露 Tauri commands 并协调状态
- `src-tauri/src/morse/overlay.rs` 负责多步骤区域选择会话
- `src-tauri/src/morse/recognition.rs` 负责截图与 Morse 识别
- `src-tauri/src/morse/settings.rs` 负责 `morse_settings.json` 读写

## 文档与说明

- `CLAUDE.md`：面向 Claude Code 的仓库操作说明
- `AGENTS.md`：仓库事实、修改约束与维护提示
- `docs/CODEMAPS/*.md`：架构、前端、原生层、数据与依赖摘要

## 开发注意事项

- 使用 **Bun**，不要切换到 npm / pnpm / yarn
- 不要把 overlay 改成路由系统；当前约定是 `?mode=overlay`
- 不要把热键输入改回普通文本框；当前应保持录制式交互
- 不要重新引入重灰幕遮挡真实屏幕内容
- 当前仓库没有成型的前端 test/lint 命令，不要在文档中虚构这些脚本
