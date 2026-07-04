# 依赖

## Rust 依赖（`src-tauri/Cargo.toml`）

### 核心框架

| Crate | 版本 | 用途 |
|-------|------|------|
| `tauri` | 2.11.2 | 桌面应用框架（`unstable` feature） |
| `tauri-plugin-dialog` | 2 | 文件/消息对话框 |
| `tauri-plugin-opener` | 2.5.4 | 在系统应用中打开 URL/文件 |
| `tauri-plugin-window-state` | 2.4.1 | 持久化窗口状态 |
| `tauri-plugin-updater` | 2 | 通过 GitHub Releases 自动更新 |
| `tauri-plugin-process` | 2 | 进程控制（更新后重启） |

### 原生自动化

| Crate | 版本 | 用途 |
|-------|------|------|
| `willhook` | 0.6.3 | 全局键盘钩子（WH_KEYBOARD_LL） |
| `xcap` | 0.9.6 | 截屏 |
| `enigo` | 0.6.1 | 模拟键盘输入 |
| `rodio` | 0.20 | 音频播放 |
| `image` | 0.25.10 | 图像处理（模板匹配、颜色采样） |
| `crossbeam-channel` | 0.5 | 按键抑制器事件转发通道 |
| `windows-sys` | 0.61 | Windows API 绑定（Foundation、UI、Input、ProcessStatus、Threading） |

### 异步与序列化

| Crate | 版本 | 用途 |
|-------|------|------|
| `tokio` | 1.52.3 | 异步运行时（macros、rt-multi-thread、sync、time） |
| `serde` | 1.0.228 | 序列化（derive） |
| `serde_json` | 1.0.150 | JSON 处理 |
| `thiserror` | 2.0.18 | 错误派生宏 |
| `chrono` | 0.4 | 日期/时间（含 serde） |
| `url` | 2.5.8 | URL 解析 |

### 开发

| Crate | 版本 | 用途 |
|-------|------|------|
| `tempfile` | 3.27.0 | 测试用临时目录 |

## 前端依赖（`package.json`）

### 核心

| 包 | 版本 | 用途 |
|----|------|------|
| `react` | 19.2.7 | UI 库 |
| `react-dom` | 19.2.7 | React DOM renderer |
| `@tauri-apps/api` | 2.11.1 | Tauri 前端 API（invoke、events） |
| `@tauri-apps/plugin-updater` | ^2.10.1 | 更新器前端 |
| `@tauri-apps/plugin-process` | ^2.3.1 | 进程控制（重启） |
| `@tauri-apps/plugin-dialog` | ^2.7.1 | 对话框 |
| `@tauri-apps/plugin-opener` | 2.5.4 | 打开外部 URL |

### UI

| 包 | 版本 | 用途 |
|----|------|------|
| `radix-ui` | 1.6.1 | headless 交互组件，保留焦点管理、键盘导航、Portal 与无障碍行为 |
| `daisyui` | ^5.6.10 | 基础组件视觉 class 与主题 token 体系 |
| `@remixicon/react` | ^4.9.0 | 图标库 |
| `chromakit-react` | ^0.1.16 | 颜色输入与颜色面板基础组件 |
| `class-variance-authority` | ^0.7.1 | 变体样式 |
| `clsx` | ^2.1.1 | class 合并 |
| `tailwind-merge` | 3.6.0 | Tailwind class 去重 |
| `sonner` | ^2.0.7 | Toast 通知 |
| `tw-animate-css` | ^1.4.0 | Tailwind 动画工具类 |

### 辅助

| 包 | 版本 | 用途 |
|----|------|------|
| `@fontsource-variable/jetbrains-mono` | ^5.2.8 | JetBrains Mono variable font |
| `culori` | ^4.0.2 | 颜色解析与转换 |

### 构建

| 包 | 版本 | 用途 |
|----|------|------|
| `vite` | 8.1.2 | 构建工具 |
| `@vitejs/plugin-react` | 6.0.3 | React 插件 |
| `tailwindcss` | 4.3.2 | CSS 框架（v4，CSS-first） |
| `@tailwindcss/vite` | 4.3.2 | Tailwind Vite 插件 |
| `typescript` | ~6.0.3 | 类型检查 |
| `vitest` | 4.1.9 | 测试运行器 |
| `@vitest/coverage-v8` | 4.1.9 | 测试覆盖率 |
| `@tauri-apps/cli` | ^2.11.4 | Tauri CLI |
| `@types/react` | 19.2.17 | React 类型 |
| `@types/react-dom` | 19.2.3 | React DOM 类型 |

## 依赖说明

- 项目使用 Bun 作为包管理器（`bun.lock`），不使用 npm/pnpm/yarn
- Tailwind v4 为 CSS-first：不存在 `tailwind.config.js`，主题 token 在 `src/App.css` 中
- `devDependencies` 不包含 ESLint 或 Prettier；代码风格由约定和 review 保证
