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

### 异步与序列化

| Crate | 版本 | 用途 |
|-------|------|------|
| `tokio` | 1.52.3 | 异步运行时（macros、rt-multi-thread、sync、time） |
| `serde` | 1.0.228 | 序列化（derive） |
| `serde_json` | 1.0.150 | JSON 处理 |
| `thiserror` | 2.0.18 | 错误派生宏 |
| `chrono` | 0.4 | 日期/时间（含 serde） |
| `regex` | 1.12.4 | 正则匹配 |
| `url` | 2.5.8 | URL 解析 |

### 系统

| Crate | 版本 | 用途 |
|-------|------|------|
| `windows-sys` | 0.61 | Windows API 绑定（Foundation、UI、System） |

### 开发

| Crate | 版本 | 用途 |
|-------|------|------|
| `tempfile` | 3.27.0 | 测试用临时目录 |

## 前端依赖（`package.json`）

### 核心

| 包 | 版本 | 用途 |
|----|------|------|
| `react` | 19.2.7 | UI 库 |
| `react-dom` | 19.2.7 | React DOM 渲染器 |
| `@tauri-apps/api` | 2.11.0 | Tauri 前端 API（invoke、events） |
| `@tauri-apps/plugin-updater` | ^2.10.1 | 更新器前端 |
| `@tauri-apps/plugin-process` | ^2.3.1 | 进程控制（重启） |
| `@tauri-apps/plugin-dialog` | ^2.0.0 | 对话框 |
| `@tauri-apps/plugin-opener` | 2.5.4 | 打开外部 URL |

### UI

| 包 | 版本 | 用途 |
|----|------|------|
| `radix-ui` | 1.5.0 | shadcn/ui 基础组件 |
| `@base-ui/react` | 1.5.0 | 额外基础组件 |
| `shadcn` | 4.11.0 | 组件系统 |
| `@remixicon/react` | ^4.9.0 | 图标库 |
| `class-variance-authority` | ^0.7.1 | 变体样式 |
| `clsx` | ^2.1.1 | class 合并 |
| `tailwind-merge` | 3.6.0 | Tailwind class 去重 |
| `react-colorful` | ^5.7.0 | 颜色选择器（批准的例外） |
| `sonner` | ^2.0.7 | Toast 通知 |
| `vaul` | ^1.1.2 | Drawer 组件 |

### 构建

| 包 | 版本 | 用途 |
|----|------|------|
| `vite` | 7.3.5 | 构建工具 |
| `@vitejs/plugin-react` | 4.7.0 | React 插件 |
| `tailwindcss` | 4.3.1 | CSS 框架（v4，CSS-first） |
| `@tailwindcss/vite` | 4.3.1 | Tailwind Vite 插件 |
| `typescript` | ~5.8.3 | 类型检查 |
| `vitest` | 3.2.6 | 测试运行器 |
| `@vitest/coverage-v8` | 3.2.6 | 测试覆盖率 |

### 其他

| 包 | 版本 | 用途 |
|----|------|------|
| `cmdk` | ^1.1.1 | 命令面板 |
| `date-fns` | 4.4.0 | 日期工具 |
| `recharts` | 3.8.1 | 图表 |
| `react-resizable-panels` | 4.11.2 | 可调面板 |
| `input-otp` | ^1.4.2 | OTP 输入 |
| `react-day-picker` | ^9.14.0 | 日期选择器 |
| `embla-carousel-react` | ^8.6.0 | 轮播 |

## 依赖说明

- 项目使用 Bun 作为包管理器（`bun.lock`），不使用 npm/pnpm/yarn
- `react-colorful`（约 3KB）是唯一批准的第三方颜色选择器；不使用 shadcn 官方 color-picker
- Tailwind v4 为 CSS-first：不存在 `tailwind.config.js`，主题 token 在 `src/App.css` 中
- `devDependencies` 不包含 ESLint 或 Prettier；代码风格由约定和 review 保证
