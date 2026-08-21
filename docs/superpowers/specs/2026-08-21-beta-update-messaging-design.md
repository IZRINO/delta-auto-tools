# Beta 检查更新文案

日期：2026-08-21  
状态：设计已确认，进入实施  
批次：1.0 Beta 未决三项之发布说明（另两份：规格格换皮、工具页记忆）

## 1. 目标

Beta 安装包检查更新时，用户不再把「已是最新」理解成系统坏了。当前 Beta 无 `.sig`、无 `latest.json`，端点仍是 GitHub `/releases/latest/download/latest.json`（只解析非 prerelease）。这是既有发布策略，不是漏做。

本 spec 只把策略说清楚。不建立 Beta 自动更新通道。

## 2. 非目标

- 不为 Beta 生成 `.sig` / `latest.json` / `latest-beta.json`。
- 不改 `src-tauri/tauri.beta.conf.json` 的 `createUpdaterArtifacts: false`。
- 不改 `tauri.conf.json` 的 updater `endpoints`。
- 不改 `should_offer_update` / `version_rank`。
- 不让正式版看到 Beta，不让 Beta 自动升到下一份 Beta。

若以后要 Beta→Beta，另开 spec：签名构建、稳定 URL 的 `latest-beta.json`、按当前是否预发布切换端点。本轮不做。

## 3. 方案选择

1. **关于页分流文案（采用）**：当前版本含 `-` 视为测试版，检查区先说明「测试包互不推送，正式版发布后可升」。检查按钮仍走 stable 端点。
2. 签名 Beta 通道。每次 Beta 要私钥，和现有「无签名快推」相反。
3. 检查更新在 Beta 上直接禁用。用户无法在正式版发布后从应用内升上去。

采用 1。与 `droid-wiki/features/about.md`、`deployment.md` 已写策略一致，只补 UI。

## 4. 设计

### 4.1 判定

前端用关于页 bootstrap 的 `version` 字符串。含 `"-"` 即测试版（覆盖 `1.0.0-beta.1`、`0.20.1-beta.1`）。不要在 Rust 加新字段。

### 4.2 文案

测试版时，`FieldUnit header="更新状态"` 内、按钮行上方固定说明（中文，可换行）：

> 当前是测试版。测试包之间不会自动更新，请从 GitHub Release 手动下载。正式版发布后，可在此检查并升级。

正式版不显示这段。

### 4.3 检查结果

- 测试版点「检查更新」：行为与现在相同（查 stable `latest.json`）。
  - 无更高正式版：`notAvailable`。说明句保留，避免只剩「已是最新」造成误解。可将状态矩阵/短句改为「暂无正式版可升」，不要写「已是最新」。
  - 有更高正式版（同数值正式版或更高数值）：照常 `available`，可下载安装。
- 「打开 GitHub Release」始终可用。
- 浏览器预览仍显示「更新功能仅在桌面端可用」。

不改 `about_check_for_update` 的返回结构。只改 `about-page.tsx` 对 `version` + `progress.phase` 的展示。

## 5. 测试

- 纯函数或页面级：`version` 含 `-` 时说明句出现；不含时不出现。
- `0.20.1-beta.1` 在 `notAvailable` 时不出现「已是最新」字面。
- 不测真实 GitHub 网络。

## 6. 文档

`droid-wiki/features/about.md` 的「Beta 与正式版端点」段补一句：测试版关于页展示手装说明，检查更新仍走 stable，无正式版时文案为「暂无正式版可升」。`deployment.md` 策略段不改（本来就无 Beta 通道）。
