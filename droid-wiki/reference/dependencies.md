# 依赖

## Rust 依赖（`src-tauri/Cargo.toml`）

### 核心框架

| Crate | 版本 | 用途 |
|-------|------|------|
| `tauri` | 2.11.5 | 桌面应用框架（`unstable` feature） |
| `tauri-plugin-dialog` | 2 | 文件/消息对话框 |
| `tauri-plugin-opener` | 2.5.4 | 在系统应用中打开 URL/文件 |
| `tauri-plugin-window-state` | 2.4.1 | 持久化窗口状态 |
| `tauri-plugin-updater` | 2 | 通过 GitHub Releases 自动更新 |
| `tauri-plugin-process` | 2 | 进程控制（更新后重启） |

### 原生自动化

| Crate | 版本 | 用途 |
|-------|------|------|
| `willhook` | 0.6.3 | 全局键盘钩子（WH_KEYBOARD_LL） |
| `xcap` | 0.9.8 | 截屏 |
| `enigo` | 0.6.1 | 模拟键盘输入 |
| `rodio` | 0.20 | 音频播放 |
| `image` | 0.25.10 | 图像处理（模板匹配、颜色采样） |
| `crossbeam-channel` | 0.5 | 按键抑制器事件转发通道 |

### 异步与序列化

| Crate | 版本 | 用途 |
|-------|------|------|
| `tokio` | 1.53.1 | 异步运行时（macros、rt-multi-thread、sync、time） |
| `serde` | 1.0.229 | 序列化（derive） |
| `serde_json` | 1.0.151 | JSON 处理 |
| `thiserror` | 2.0.20 | 错误派生宏 |
| `chrono` | 0.4 | 日期/时间（含 serde） |
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
| `react` | 19.2.8 | UI 库 |
| `react-dom` | 19.2.8 | React DOM 渲染器 |
| `@tauri-apps/api` | 2.11.1 | Tauri 前端 API（invoke、events） |
| `@tauri-apps/plugin-process` | ^2.3.1 | 进程控制（重启） |
| `@tauri-apps/plugin-dialog` | ^2.7.2 | 对话框 |
| `@tauri-apps/plugin-opener` | 2.5.4 | 打开外部 URL |

### UI

| 包 | 版本 | 用途 |
|----|------|------|
| `radix-ui` | 1.6.7 | headless 交互组件，保留焦点管理、键盘导航、Portal 与无障碍行为 |
| `daisyui` | ^5.7.21 | 基础组件视觉 class 与主题 token 体系 |
| `@remixicon/react` | ^4.9.0 | 图标库 |
| `class-variance-authority` | ^0.7.1 | 变体样式 |
| `clsx` | ^2.1.1 | class 合并 |
| `tailwind-merge` | 3.6.0 | Tailwind class 去重 |
| `sonner` | ^2.0.8 | Toast 通知 |

### 构建

| 包 | 版本 | 用途 |
|----|------|------|
| `vite` | 8.2.2 | 构建工具 |
| `@vitejs/plugin-react` | 6.1.0 | React 插件 |
| `tailwindcss` | 4.3.3 | CSS 框架（v4，CSS-first） |
| `@tailwindcss/vite` | 4.3.3 | Tailwind Vite 插件 |
| `typescript` | ~6.0.3 | 类型检查 |
| `vitest` | 4.1.11 | 测试运行器 |
| `@vitest/coverage-v8` | 4.1.11 | 测试覆盖率 |

## 依赖说明

- 项目使用 Bun 作为包管理器（`bun.lock`），不使用 npm/pnpm/yarn
- Tailwind v4 为 CSS-first：不存在 `tailwind.config.js`，主题 token 在 `src/App.css` 中
- `devDependencies` 不包含 ESLint 或 Prettier；代码风格由约定和 review 保证
