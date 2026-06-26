# 部署与发布

Delta Auto Tools 是 Windows 桌面应用，通过 GitHub Releases 以 NSIS 安装包分发。无服务器部署、无 Docker、无自动构建的 CI/CD 管线。Release 在本地构建后手动上传。

## 构建

### 前置条件

Tauri 签名密钥对必须存在。运行 `scripts/setup-update-key.ps1` 一次生成。私钥保存到 `$HOME/.tauri/delta-auto-tools.key`（不入库），公钥写入 `tauri.conf.json` 的 `plugins.updater.pubkey`。

### 签名正式版构建

```bash
# 设置签名密钥（内容，非路径）
$env:TAURI_SIGNING_PRIVATE_KEY = "<私钥内容>"
# 可选：$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "<密码>"

bun run tauri build
```

或使用一键脚本：

```bash
scripts/build-release.ps1
```

产物：
- `src-tauri/target/release/bundle/nsis/delta-auto-tools_<version>_x64-setup.exe`
- `src-tauri/target/release/bundle/nsis/delta-auto-tools_<version>_x64-setup.exe.sig`

### Beta 构建（无签名）

Beta 版本不需要签名：

```bash
bun run tauri build
```

仅产出 `.exe`（无 `.sig`）。

### latest.json

签名构建后运行 `scripts/generate-latest-json.ps1`，从 `.sig` 文件生成 `latest.json`。这是 Tauri 更新器运行时拉取的清单文件。

## 版本编号

版本遵循 SemVer。Beta 版本使用 `<major>.<minor>.<patch>-beta.<N>`（如 `0.17.0-beta.1`）。更新器做 SemVer 全序比较：同数值正式版 > beta，更高数值 > 更低数值，正式版不降级到 beta。

三个版本源必须同步：`package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`。

## 发布流程

1. 更新三个文件中的版本号
2. 构建（正式版签名，Beta 版无签名）
3. 提交，subject 为 `发布 v<version>`，正文包含 `变更：` 段列出实际变更
4. 打 Tag：`git tag -a v<version> -m "发布 v<version>"` 并推送
5. 创建 GitHub Release 并上传资产：
   - **正式版**：3 个资产 - `.exe`、`.sig`、`latest.json`
   - **Beta**：1 个资产 - 仅 `.exe`，加 `--prerelease` 标记
6. 验证：`gh release view v<version> --json tagName,isDraft,isPrerelease,assets`

## 自动更新机制

应用检查 `https://github.com/IZRINO/delta-auto-tools/releases/latest/download/latest.json` 获取更新。GitHub 的 `/releases/latest` 端点仅解析非 prerelease Release，因此 beta 用户不会被推送其他 beta；他们在下一个正式版发布时更新。

Beta 构建使用与正式版相同的 stable 端点。因 `0.17.0-beta.5 < 0.17.0`（SemVer），beta 用户在 `0.17.0` 发布时会收到更新提示。

## 网络与代理

如 `git push` 或 `gh release` 因连接错误失败，设置本地代理环境变量：

```bash
$env:HTTP_PROXY = "http://127.0.0.1:7897"
$env:HTTPS_PROXY = "http://127.0.0.1:7897"
```

`set` 命令中值后面不要留尾随空格（Windows cmd 会将其带入变量）。
