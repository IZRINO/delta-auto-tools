# 配置

## 设置文件

所有设置为 JSON 文件，存储在 Tauri app config 目录（`%APPDATA%/org.izrino.delta-auto-tools/`）。每个工具有自己的文件。Rust 侧公共 `settings::save_settings` 先写同目录临时文件，再替换目标文件；Windows 使用 `MoveFileExW` 覆盖写入，降低进程异常退出导致配置文件半写的概率。

| 文件 | 工具 | 内容 |
|------|------|------|
| `morse_settings.json` | Morse | 热键、区域、自动输入、自动点击链设置 |
| `timer_settings.json` | 计时器 | `timerEnabled`、timers 数组（duration、direction、hotkey）、display 设置 |
| `counter_settings.json` | 计数器 | `counterEnabled`、counters 数组（startValue、hotkey）、display 设置 |
| `rapidfire_settings.json` | 连发器 | `rapidfireEnabled`、cards 数组（trigger、target、interval、jitter、spacing、no-append）、compensation delay |
| `recognition_settings.json` | 识别触发 | `recognitionEnabled`、cards 数组（trigger mode、activation、effects、cooldown、probes） |
| `theme_settings.json` | 主题 | `activeThemeId`、custom themes、token overrides |
| `profile_settings.json` | 配置 | `profiles` 数组、`activeProfileId` |
| `counter_state.json` | 计数器（运行态） | 累积计数器值（独立于配置） |
| `log_settings.json` | 日志 | 全局日志级别、按模块覆盖 |

## Tauri 配置

`src-tauri/tauri.conf.json` 包含：

- `productName`：`delta-auto-tools`
- `identifier`：`org.izrino.delta-auto-tools`
- 窗口：1280x800，最小 1280x800
- Bundle target：`nsis`
- Updater：GitHub Releases 端点，`installMode: "passive"`，`pubkey`（公开签名密钥）
- CSP：null（无内容安全策略限制）
- `createUpdaterArtifacts: true`（构建时生成 .sig 文件）

## Capabilities

`src-tauri/capabilities/default.json` 定义前端可调用的 Tauri 命令。新增命令必须添加到此处，否则 `invoke()` 会失败。

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
- 文件读取失败仍返回错误；该路径通常代表权限、磁盘或路径问题，不会被默认配置掩盖。
