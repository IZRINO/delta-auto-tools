# 配置

## 设置文件

所有设置为 JSON 文件，存储在 Tauri app config 目录（`%APPDATA%/org.izrino.delta-auto-tools/`）。每个工具有自己的文件。Rust 侧公共 `settings::save_settings` 先写同目录临时文件，再替换目标文件；Windows 使用 `MoveFileExW` 覆盖写入，降低进程异常退出导致配置文件半写的概率。

| 文件 | 工具 | 内容 |
|------|------|------|
| `morse_settings.json` | Morse | 热键、区域、自动输入、自动点击链设置 |
| `timer_settings.json` | 计时器 | `timerEnabled`、timers 数组（duration、direction、hotkey）、display 设置 |
| `counter_settings.json` | 计数器 | `counterEnabled`、counters 数组（startValue、hotkey）、display 设置 |
| `rapidfire_settings.json` | 连发器 | `rapidfireEnabled`、cards 数组（trigger、target、interval、jitter、spacing、no-append）、compensation delay |
| `recognition_settings.json` | 识别触发 | `recognitionEnabled`、cards 数组（trigger mode、`hotkeyRepeatMode`、activation、effects、cooldown、probes） |
| `theme_settings.json` | 主题 | `activeThemeId`、custom themes、token overrides |
| `profile_settings.json` | 配置 | `profiles` 数组、`activeProfileId` |
| `counter_state.json` | 计数器（运行态） | 累积计数器值（独立于配置） |
| `log_settings.json` | 日志 | 全局日志级别、按模块覆盖 |

Recognition Hotkey 卡片的 `hotkeyRepeatMode` 可取 `once` 或 `whileHeld`，缺失时按 `once` 读取。`whileHeld` 要求 `cooldownMs >= 10`；`once` 继续允许 `cooldownMs = 0`。RegionWatch / ColorWatch 保存时会把该字段归一为 `once`。

## Tauri 配置

`src-tauri/tauri.conf.json` 包含：

- `productName`：`delta-auto-tools`
- `identifier`：`org.izrino.delta-auto-tools`
- 窗口：1280x800，最小 1280x800
- Bundle target：`nsis`
- Updater：GitHub Releases 端点，`installMode: "passive"`，`pubkey`（公开签名密钥）
- 生产 CSP：默认仅允许 `'self'`；`connect-src` 额外允许 Tauri `ipc:` / `http://ipc.localhost`；图片额外允许 `asset:` / `http://asset.localhost` / `data:` / `blob:`；仅 style 允许现有 inline 样式
- 开发 CSP：在生产规则上额外放行 `ws://localhost:1420` / `ws://localhost:1421` 供 Vite HMR
- `createUpdaterArtifacts: true`（构建时生成 .sig 文件）

`src-tauri/tauri.beta.conf.json` 是 beta 构建覆盖配置，仅将 `bundle.createUpdaterArtifacts` 设为 `false`。beta 发布命令使用 `bun run tauri build --config src-tauri/tauri.beta.conf.json`，避免无签名环境下因 updater artifact 签名失败。

## Capabilities

Capability 按窗口信任边界拆分：

| 文件 | 匹配窗口/WebView | 权限边界 |
|------|------------------|----------|
| `default.json` | `main` | event listen/unlisten；Strategy 子 WebView 创建与窗口几何/可见性/销毁；dialog open/save；HTTP(S) open-url；restart |
| `overlays.json` | Morse/Timer/Counter/Rapidfire/Recognition 的 display/position/selection 窗口 | event listen/unlisten 和最小窗口几何/显示/隐藏/销毁权限；无 dialog/opener/process/updater |
| `strategy.json` | remote `strategy-content` WebView | `permissions: []`，远程页不得访问 IPC |

不使用 `core:default`、`opener:default`、`updater:default`、`process:default` 权限集。新增前端 Tauri API 调用时，必须根据实际调用窗口将单个 `allow-*` permission 加到对应 capability。

## 环境变量

| 变量 | 用途 |
|------|------|
| `TAURI_SIGNING_PRIVATE_KEY` | 签名正式版构建的私钥内容（生成 .sig 必需） |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 签名密钥的可选密码 |
| `TAURI_SIGNING_PRIVATE_KEY_PATH` | 替代方案：密钥文件路径而非内容 |
| `HTTP_PROXY` / `HTTPS_PROXY` | GitHub 慢时的本地代理（用于 git push / gh release） |

## 主题 CSS token

定义在 `src/App.css` 的 `@theme inline` 和 `:root` 中：

| Token | 颜色 | 角色 |
|-------|------|------|
| `--carbon` | `#0C0C0B` | 主背景 |
| `--slate` | `#171715` | 次级面板 |
| `--iron` | `#232320` | 卡片表面 |
| `--chalk` | `#D8D4CC` | 主文字、边框 |
| `--zinc` | `#807C74` | 次级文字 |
| `--dust` | `#545250` | 元信息 |
| `--seam` | `#2A2926` | 网格线 |
| `--amber` | `#E8A000` | 唯一强调色 |
| `--rust` | `#C85400` | 警告/危险 |
| `--moss` | `#3F8A30` | 成功/有效 |
| `--void` | `#050504` | 数据井、JSON 显示 |
| `--alert-red` | `#E11919` | 当前选择、危险（亮色主题变体） |

全局 `--radius: 0` 强制 90 度直角。

## PM2

`ecosystem.config.cjs` 定义两个进程：`delta-auto-tools-vite`（Vite 开发服务器）和 `delta-auto-tools-tauri`（Tauri 开发）。Tauri 进程通过 `scripts/wait-for-port.cjs` 等待端口 1420。
## 配置恢复策略

- 通用 `load_settings` 遇到损坏 JSON 时，会把原文件重命名为 `<file>.corrupt-<timestamp>`，并返回默认配置，避免单个配置文件阻断 Tauri setup。
- 工具启动阶段遇到语义非法配置（例如字段组合无法 normalize）时，会把对应工具配置重命名为 `<file>.invalid-<timestamp>`，然后用默认配置重试初始化。Recognition 会同时检查当前 `recognition_settings.json` 和旧版迁移文件 `audio_settings.json`。
- 文件读取失败仍返回错误；该路径通常代表权限、磁盘或路径问题，不会被默认配置掩盖。
