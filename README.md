# Delta Auto Tools

**Delta Auto Tools** 是一款基于 **Tauri 2 + React + Rust** 的 Windows 桌面工具，面向《三角洲行动》玩家提供本地化辅助能力。

项目聚焦轻量、稳定和桌面端原生体验：前端负责工作台交互，Rust/Tauri 负责快捷键、透明窗口、屏幕识别、本地存储和 Delta 相关接口调用。

## 功能概览

- **摩斯识别工作台**：支持区域框选、快捷键触发识别、识别结果展示、历史记录和自动输入。
- **计时\计数器工作台**：支持多计时器、多计数器、独立总开关、快捷键触发，以及置顶透明显示窗口。
- **连发器工作台**：支持多组连发配置、组合触发键、按住连发、卡片级不追加补齐、卡片级按键最小间距 / 启动抖动策略，以及透明状态窗口。
- **Delta 工具接口**：支持 QQ、微信、QQ 安全中心、Wegame 与先遣服相关登录流程、本地账号管理和游戏数据查询；账号凭据仅在 Rust 侧持有，本地敏感字段使用系统凭据加密保存。
- **攻略网站工作台**：集成 `kkrb.net` 与 `orzice.com` 两类高频更新的外部攻略页面，以贴顶浏览器工具条集中管理内置 / 自定义网址、刷新档位和外部打开入口，并在工具条下方创建 `strategy-content` WebView2 子视图真实导航当前站点，让网页区域占满主应用剩余空间；cookie、JS 跳转、localStorage、同源 API 和人机验证均由目标站点自身处理，不再额外弹出独立攻略浏览器窗口，也不再默认使用 iframe/srcDoc 代理渲染。支持手动刷新、按站点持久化的自动刷新档位和系统浏览器打开兜底；`strategy_fetch_page` / `strategy_open_window` 保留为后端实验 / 兼容入口。

## 技术栈

- 桌面框架：Tauri 2
- 原生能力：Rust
- 前端：React 19、TypeScript、Vite
- 包管理与脚本：Bun
- UI：Tailwind CSS v4、shadcn/ui、Remix Icon；视觉语言为“Delta 战术工业白图纸控制台”（浅色工业战术、硬边网格、单一战术橙强调色）
- 本地存储：SQLite、JSON 配置文件、Windows DPAPI 凭据加密

## 本地开发

```bash
bun install
```

安装前端依赖。

```bash
bun run dev
```

启动 Vite 前端开发服务器。

```bash
bun run tauri dev
```

启动完整桌面开发环境，用于验证快捷键、透明窗口、Tauri commands 和原生能力。

```bash
bun run build
```

执行 TypeScript 检查并构建前端产物。

```bash
bun run test
```

执行前端单元测试。

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

检查 Rust/Tauri 侧编译。

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

执行 Rust 单元测试。

## 项目结构

```text
src/                    # React 前端应用与桌面工作台界面
src/components/app/      # 业务页面与功能组件
src/components/ui/       # shadcn/ui 基础组件
src-tauri/src/           # Rust/Tauri 原生能力与 commands
src-tauri/src/morse/     # 摩斯识别流程
src-tauri/src/timer/     # 计时器与计数器
src-tauri/src/rapidfire/ # 连发器
src-tauri/src/delta/     # Delta 登录、账号与游戏数据接口
docs/                    # 架构决策、开发记录与 UI 设计规范
```
