# 工具链

## 构建工具

### Vite

前端使用 Vite 8 + `@vitejs/plugin-react`。开发服务器运行在端口 1420（strictPort，被占用则失败而非递增）。配置在 `vite.config.ts`。Tailwind v4 通过 `@tailwindcss/vite` 集成。

### TypeScript

TypeScript 6，strict 模式。`bun run build` 执行 `tsc && vite build`。路径别名：`@/components`、`@/components/ui`、`@/lib`、`@/hooks`（在 tsconfig 和 vite 中配置）。

### Tailwind CSS v4

CSS-first 配置，不存在 `tailwind.config.js`。主题 token 在 `src/App.css` 的 `@theme inline` 中定义。全局 `--radius: 0` 保证 90 度直角。

### Bun

Bun 是包管理器和脚本运行器。不要使用 npm/pnpm/yarn。`bun install` 读取 `bun.lock`。

`bun run check` 由 Bun 启动，但其中 Vitest 与 V8 coverage 固定使用 Node.js 24 runtime；Windows CI 通过 `actions/setup-node` 显式安装该版本。Node.js 不参与依赖安装。

### Tauri 2

Tauri CLI 通过 `bun run tauri` 可用。配置在 `src-tauri/tauri.conf.json`。权限按 main、overlay、remote Strategy WebView 拆分到 `src-tauri/capabilities/default.json`、`overlays.json`、`strategy.json`。

## Rust 工具链

### Cargo

`src-tauri/Cargo.toml` 是 manifest。crate 名为 `delta-auto-tools`，库名为 `delta_auto_tools_lib`。编译检查用 `cargo check --manifest-path src-tauri/Cargo.toml`。测试用 `cargo test --manifest-path src-tauri/Cargo.toml`。

### 关键 Rust 依赖

- `willhook`：全局键盘钩子（WH_KEYBOARD_LL）
- `xcap`：截屏
- `enigo`：键盘输入模拟
- `rodio`：音频播放
- `image`：图像处理（模板匹配、颜色采样）
- `tauri` 2 及插件：dialog、opener、window-state、updater、process

## PM2

`ecosystem.config.cjs` 将 Vite 和 Tauri 拆为两个 PM2 进程。Tauri 进程通过 `scripts/wait-for-port.cjs` 等待端口 1420 可用后启动。适合开发时在后台同时运行两者。

## 发布脚本

| 脚本 | 用途 |
|------|------|
| `scripts/build-release.ps1` | 一键签名构建：设置 TAURI_SIGNING_PRIVATE_KEY，执行 tauri build，生成 .sig |
| `scripts/check.ps1` | Windows 全量质量门禁；本地与 GitHub Actions 共用 |
| `scripts/generate-latest-json.ps1` | 从 .sig 文件生成 `latest.json`（Tauri 更新器清单） |
| `scripts/setup-update-key.ps1` | 生成 Tauri 签名密钥对（首次设置） |
| `scripts/wait-for-port.cjs` | 等待端口 1420 可用（PM2 使用） |

## UI 组件

项目保留 Radix headless 组件的焦点管理、键盘导航、Portal 与无障碍行为，视觉层使用 daisyUI class。基础组件在 `src/components/ui/`。图标用 remixicon。新增组件时优先复用现有包装，不再使用外部组件生成 CLI。

## Codegraph

仓库有 `.codegraph/` 目录，表示 codegraph MCP 服务器可用于开发期间的符号搜索和依赖探索。
