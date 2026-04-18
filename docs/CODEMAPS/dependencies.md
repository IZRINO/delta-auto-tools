<!-- Generated: 2026-04-19 | Files scanned: 79 | Token estimate: ~760 -->

# 依赖 Codemap

## 前端依赖
- `react`, `react-dom`
  - 主界面渲染
- `@tauri-apps/api`
  - `invoke` / `listen`
- `@remixicon/react`
  - 图标
- `@tailwindcss/vite`, `tailwindcss`
  - Tailwind v4 样式系统
- `shadcn`
  - UI 生成与组件约定
- `@base-ui/react`, `radix-ui`
  - 基础 UI 交互能力
- `clsx`, `tailwind-merge`, `class-variance-authority`
  - class 组合
- `sonner`, `vaul`, `cmdk`, `react-day-picker`, `recharts`
  - 已安装 UI 生态，当前业务只部分使用

## 原生依赖
- `tauri`
  - 桌面运行时
- `tauri-plugin-opener`
  - 打开外部资源
- `tauri-plugin-global-shortcut`
  - 全局热键
- `serde`, `serde_json`
  - 序列化 / 反序列化
- `tokio`
  - async runtime / oneshot
- `xcap`
  - 屏幕截图
- `image`
  - 图像处理与阈值化
- `enigo`
  - 自动输入
- `tauri-build`
  - Rust build dependency

## 外部系统/设备交互
- 屏幕截图：本机显示器
- 全局热键：操作系统快捷键注册
- 自动输入：操作系统键盘注入
- 本地配置目录：Tauri app config dir

## 开发工具链
- Bun：脚本执行
- Vite：前端 dev/build
- Cargo：Rust 构建
- PM2：开发时多进程编排

## PM2 服务
- `delta-auto-tools-1420`
  - `bun run dev`
- `delta-auto-tools-tauri`
  - 等待 1420 后运行 `bun run tauri dev --no-dev-server`

## 路径与样式约定
- 别名：`@/components`, `@/components/ui`, `@/lib`, `@/hooks`
- Tailwind CSS 入口：`src/App.css`
- shadcn 配置：`components.json`

## 当前没有的依赖面
- 无数据库驱动
- 无 HTTP server / router
- 无认证服务
- 无第三方云 API
