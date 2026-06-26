# 开发流程

## 分支与编码循环

1. `git checkout master; git pull` 从最新代码开始
2. 创建 feature 分支：`git checkout -b <topic>`
3. 编码改动，遵循 [模式与约定](patterns-and-conventions.md)
4. 本地测试：
   - `bun run test` 前端 Vitest 测试
   - `cargo check --manifest-path src-tauri/Cargo.toml` Rust 编译检查
   - `cargo test --manifest-path src-tauri/Cargo.toml` Rust 单元测试
5. UI 开发用 `bun run dev` 获取浏览器预览（原生命令禁用）。完整集成测试用 `bun run tauri dev`。

## Tauri command 检查清单

新增或修改 Tauri command 时，三处都要更新：

1. 在模块中定义 `#[tauri::command]` 函数
2. 在 `src-tauri/src/lib.rs` 的 `generate_handler![]` 中注册（按模块注释分组）
3. 在 `src-tauri/capabilities/default.json` 中添加权限

遗漏任何一处都会导致前端 `invoke()` 运行时失败。

## Settings 变更

修改工具 settings 结构时：

1. 添加字段时使用 `#[serde(default = "fn")]` 保证向后兼容反序列化
2. 结构体使用 `#[serde(rename_all = "camelCase")]`（所有面向前端的结构体必须）
3. 更新对应 `*-types.ts` 中的前端类型
4. 更新 `*-utils.ts` 中的 `settingsToForm()` / `parseSettingsForm()` 转换
5. 如字段需要触发热键重注册或 watcher 重启，在模块的 `save_settings` 中处理

## Commit 约定

- Commit message 使用中文（遵循 AGENTS.md）
- 版本发布时 subject 为 `发布 v<version>`，正文必须包含 `变更：` 段，列出实际变更项。完整发布流程见 [部署与发布](../deployment.md)。

## 版本号同步

更新版本号时必须同步修改三个文件：`package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`。如 `Cargo.lock` 中本包版本随解析更新，也应一并提交。
