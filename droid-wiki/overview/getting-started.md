# 快速开始

## 前置条件

- Windows 10 或 11（应用使用 Windows 专有的 willhook 键盘钩子和 xcap 截屏，其他平台不支持原生功能）
- [Bun](https://bun.sh/) 用于前端包管理和脚本
- [Rust](https://rustup.rs/) 工具链（stable）
- [Tauri 2 前置条件](https://v2.tauri.app/start/prerequisites/)：Windows 上需要 WebView2 运行时和 MSVC 构建工具

## 安装

```bash
bun install
```

安装 `package.json` 中的所有前端依赖。

## 运行

### 仅前端开发服务器

```bash
bun run dev
```

启动 Vite 于 `http://localhost:1420`（strictPort）。适合在浏览器中做 UI 开发，但所有 Tauri 命令会被禁用（应用检测到缺少 `__TAURI_INTERNALS__` 后显示占位提示）。

### 完整桌面开发

```bash
bun run tauri dev
```

先启动 Vite，再启动 Tauri 窗口。这是原生功能（热键、截屏、透明窗口）实际可用的模式。

### PM2 编排

仓库有 `ecosystem.config.cjs`，将 Vite 和 Tauri 拆为两个 PM2 进程。Tauri 进程会等待端口 1420 可用后启动。

## 构建

### 前端构建

```bash
bun run build
```

执行 `tsc && vite build`，产物在 `dist/`。

### 桌面构建（NSIS 安装包）

```bash
bun run tauri build
```

产物为 `src-tauri/target/release/bundle/nsis/delta-auto-tools_<version>_x64-setup.exe`。

签名构建（自动更新器必需）：构建前设置 `TAURI_SIGNING_PRIVATE_KEY` 环境变量为私钥内容，或使用 `scripts/build-release.ps1` 一键签名构建。签名构建后运行 `scripts/generate-latest-json.ps1` 生成 `latest.json`（Tauri 更新器清单文件）。

## 测试

### 前端测试

```bash
bun run test              # 全部 Vitest 测试
bun run test:coverage     # 带覆盖率（目前仅覆盖 morse-utils.ts）
```

运行单个文件：

```bash
bunx vitest run src/components/app/morse-utils.test.ts
```

### Rust 测试

```bash
cargo check --manifest-path src-tauri/Cargo.toml    # 编译检查
cargo test --manifest-path src-tauri/Cargo.toml     # 单元测试
```

运行单个测试：

```bash
cargo test --manifest-path src-tauri/Cargo.toml <test_name>
```

## 版本号同步

更新版本号时必须同步修改三个文件：`package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`。如 `Cargo.lock` 中本包版本随解析更新，也应一并提交。
