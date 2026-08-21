# 关于与更新

关于模块（`src-tauri/src/about/`）展示应用版本、许可证和依赖致谢，并承载集成的 Tauri 自动更新器流程。通过 3 个命令和 1 个进度事件暴露给前端，由 `SettingsDialog` 中的 `AboutPanel` 渲染。

## 用途

- 提供单一 bootstrap payload（`AboutBootstrap`），携带版本、标识符、许可证文本、仓库 URL 和硬编码依赖致谢列表
- 从 Rust 驱动官方 `tauri-plugin-updater` 的检查 -> 下载 -> 安装管线，通过 `about://update-progress` 流式推送进度
- 实现 SemVer 全序比较（`should_offer_update`），使 beta 版本可正确升级到同数值正式版，但正式版不降级到 beta
- 更新器未配置（缺少 pubkey）或无法获取 release JSON 时，优雅降级为「打开 GitHub Release 页面」

## 目录结构

```
src-tauri/src/about/
├── mod.rs          # 命令、UpdateProgress 枚举、SemVer 辅助、依赖列表、错误分类
└── events.rs       # UPDATE_PROGRESS 事件名常量

src/components/app/
├── about-page.tsx     # AboutPanel（版本/更新/日志级别/许可证/致谢）
├── about-types.ts     # TS 类型
├── about-deps.ts      # DEPENDENCIES 常量（前端回退，镜像 Rust 列表）
└── settings-page.tsx  # SettingsDialog：主题/配置/关于 Tab
```

## 关键抽象

| 抽象 | 路径 | 角色 |
|------|------|------|
| `AboutBootstrap` | `src-tauri/src/about/mod.rs` | 一次性 payload：name、version、identifier、license、dependencies 等 |
| `UpdateInfo` | `src-tauri/src/about/mod.rs` | `about_check_for_update` 的结果：`{ available, version?, notes?, pubDate? }` |
| `UpdateProgress` | `src-tauri/src/about/mod.rs` | 标签枚举（`#[serde(tag = "phase")]`），通过事件流式推送。阶段：checking、notAvailable、available、downloading、downloaded、installing、installed、error |
| `should_offer_update` | `src-tauri/src/about/mod.rs` | 纯 SemVer 比较：`version_rank(remote) > version_rank(current)` |
| `version_rank` | `src-tauri/src/about/mod.rs` | `(major, minor, patch, is_stable, pre_release_str)`，正式版高于同数值 beta |

## 工作原理

### 更新检查

`about_check_for_update` 使用 `tauri_plugin_updater::UpdaterExt` 查询配置的 GitHub Releases 端点。它不盲信更新器自身的可用性判断，而是用 `should_offer_update(current, remote)` 在 Rust 侧强制 SemVer 排序。

### SemVer 比较

`version_rank` 将版本拆分为数值元组 `(major, minor, patch)` 加 `is_stable` 布尔值和 pre-release 字符串：

- `0.17.0-beta.5` -> `(0, 17, 0, false, "beta.5")`
- `0.17.0` -> `(0, 17, 0, true, "")`

因 `true > false`，同数值正式版高于其 beta。三种结果：

| 当前 | 远程 | 提供更新？ | 原因 |
|------|------|-----------|------|
| `0.17.0-beta.5` | `0.17.0` | 是 | 正式版 > beta（同数值） |
| `0.17.0-beta.5` | `0.17.1` | 是 | 数值更高 |
| `0.17.0` | `0.17.0-beta.5` | 否 | 正式版不降级到 beta |
| `0.17.0` | `0.17.1` | 是 | 数值更高 |

### 下载与安装

`about_download_and_install` 是流式管线，在每个阶段向 `main` 窗口 emit `UpdateProgress`，再次调用 `should_offer_update`（防御性），然后 `update.download_and_install(progress_cb, on_done_cb)`。成功后 emit `Installed`，前端提示用户通过 `@tauri-apps/plugin-process` 的 `relaunch()` 重启。

### Beta 与正式版端点

Beta 版本不建立独立更新通道，查询与正式版相同的 stable 端点（`/releases/latest/download/latest.json`）。因 GitHub `/releases/latest` 仅解析非 prerelease Release，测试版之间不会互推。关于页在当前版本含 `-` 时展示手装说明；检查更新无更高正式版时文案为「暂无正式版可升」，不写「已是最新」。正式版发布后 `should_offer_update` 返回 true 并下载签名安装包。

### 错误分类

两个 helper 函数将更新器错误翻译为用户可读的中文：
- `classify_updater_error`：pubkey/签名错误 -> 「自动更新未配置签名密钥，请前往 GitHub Release 页面手动下载更新」
- `classify_check_error`：获取/release JSON/404 错误 -> 「暂无可用更新文件...」；网络/超时/DNS 错误 -> 「网络连接失败: ...」

## 集成点

- `src-tauri/src/lib.rs`：3 个命令注册到 `generate_handler![]`
- `src-tauri/tauri.conf.json`：`plugins.updater` 配置 GitHub 端点、`installMode: "passive"`、`pubkey`。pubkey 为空时前端降级为「打开 GitHub Release 页面」
- `src/lib/tauri-events.ts`：`ABOUT_EVENTS.updateProgress` 集中事件名
- [日志系统](../systems/logging.md)：`AboutPanel` 还渲染日志级别单选组，调用 `log_get_level` / `log_set_level`

## 修改入口

- 新增依赖致谢：同时添加到 `built_in_dependencies()` 和 `about-deps.ts` 的 `DEPENDENCIES`，保持同步
- 修改更新端点/pubkey：编辑 `tauri.conf.json` 的 `plugins.updater`，通过 `scripts/setup-update-key.ps1` 重新生成密钥
- 新增 `UpdateProgress` 阶段：扩展枚举和前端 `UpdateProgress` 类型，在 `AboutPanel` 的状态切换中处理
- 修改 SemVer 规则：编辑 `should_offer_update` / `version_rank`，更新测试

## 关键源文件

| 文件 | 用途 |
|------|------|
| `src-tauri/src/about/mod.rs` | 命令、`AboutBootstrap`、`UpdateProgress`、SemVer 辅助、依赖列表、错误分类 |
| `src-tauri/src/about/events.rs` | `UPDATE_PROGRESS` 事件名常量 |
| `src/components/app/about-page.tsx` | `AboutPanel`（版本/更新/日志级别/许可证/致谢 UI） |
| `src/components/app/about-types.ts` | TS 类型 |
| `src/components/app/about-deps.ts` | `DEPENDENCIES` 常量（前端回退） |
| `src/components/app/settings-page.tsx` | `SettingsDialog`，关于 Tab 懒加载 `AboutPanel` |
