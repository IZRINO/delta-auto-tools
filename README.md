# Delta Auto Tools

**Delta Auto Tools** 是一款基于 **Tauri 2 + React + Rust** 的 Windows 桌面工具，面向《三角洲行动》玩家提供本地化辅助能力。

项目聚焦轻量、稳定和桌面端原生体验：前端负责工作台交互，Rust/Tauri 负责快捷键、透明窗口、屏幕识别、本地存储和 Delta 相关接口调用。

## 功能概览

- **摩斯识别工作台**：支持区域框选、快捷键触发识别、识别结果展示、历史记录和自动输入。
- **计时\计数器工作台**：支持多计时器、多计数器、独立总开关、快捷键触发，以及置顶透明显示窗口。
- **连发器工作台**：支持多组连发配置、组合触发键、按住连发、卡片级不追加补齐和透明状态窗口。
- **Delta 工具接口**：支持 QQ、微信、QQ 安全中心、Wegame 与先遣服相关登录流程、本地账号管理和游戏数据查询；账号凭据仅在 Rust 侧持有，本地敏感字段使用系统凭据加密保存。
- **攻略网站工作台**：集成 `kkrb.net` 与 `orzice.com` 两类高频更新的外部攻略页面，按 Tab 切换站点，每个 Tab 全屏展示。该面板由 Rust 端 `strategy_fetch_page` 命令代理拉取（完整 Chrome 135 浏览器头，避开 WebView UA 引发的人机验证；自动嗅探 `document.cookie + location.href` JS 重定向并跟随），前端再用 `<iframe srcDoc>` 渲染并自动注入 `<base href>`。**对于纯客户端人机验证**（如 kkrb cdn-shield / CC check：检测 `navigator.webdriver` / `HeadlessChrome` UA / 零 viewport / `window._phantom` / `performance.navigation`），代理层嗅探 `<title>CC check</title>` / `/cdn-shield/` / "安全验证" + "点击确认您是真人" / `verification-card` 后，把 `challenge` 字段返回给前端；前端把"应用内打开"按钮（`strategy_open_in_view` Tauri 命令）升到主操作位，由 Tauri 在主进程下新建 `WebviewWindow(WebviewUrl::External(...))` 子窗口（top-level navigation，不受 X-Frame-Options / CSP frame-ancestors 限制），由真正的 WebView2 Chromium 跑过验证；同一 host 复用窗口。支持**新增 / 删除自定义攻略网站**（localStorage 持久化，user_xxx 命名空间），按站点独立设置自动刷新间隔（30 秒 / 1 分钟 / 2 分钟 / 5 分钟 / 10 分钟 / 关闭）、立即刷新、应用内打开、浏览器打开、最近拉取时间显示，代理拉取失败时通过 Alert 提示并降级到应用内 / 外部打开。

## 技术栈

- 桌面框架：Tauri 2
- 原生能力：Rust
- 前端：React 19、TypeScript、Vite
- 包管理与脚本：Bun
- UI：Tailwind CSS v4、shadcn/ui、Remix Icon
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
docs/                    # 架构决策与开发记录
```
