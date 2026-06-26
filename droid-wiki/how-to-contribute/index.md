# 如何贡献

## 工作认领

Issues 使用 GitHub Issues 跟踪，通过 `gh` CLI 读写。项目使用五级分流标签：`needs-triage`、`needs-info`、`ready-for-agent`、`ready-for-human`、`wontfix`。标签定义见 `docs/agents/triage-labels.md`。

认领工作时，查找标记为 `ready-for-agent` 或 `ready-for-human` 的 issue。开始前完整阅读 issue，并评论说明你正在认领。

## PR 流程

1. 从 `master`（默认分支）创建分支
2. 提交聚焦的 commit，commit message 使用中文（遵循项目约定）
3. 请求 review 前运行 `bun run test`、`cargo check --manifest-path src-tauri/Cargo.toml`、`cargo test --manifest-path src-tauri/Cargo.toml`
4. 向 `master` 发起 PR，概述改了什么以及为什么
5. 处理 review 反馈，合并批准前不要 squash

## 完成定义

- 所有测试通过（Vitest + cargo test）
- `cargo check` 和 `bun run build`（tsc + vite build）无错误
- 新增 Tauri command 时，同时注册到 `src-tauri/src/lib.rs` 的 `generate_handler![]` 和 `src-tauri/capabilities/default.json`
- 修改 settings 结构时，serde 仍使用 `#[serde(rename_all = "camelCase")]`，前端类型匹配
- UI 改动遵循工业粗粝设计系统（无圆角卡片、无柔和阴影、Amber 仅作强调色）

参见：
- [开发流程](development-workflow.md)
- [测试](testing.md)
- [调试](debugging.md)
- [模式与约定](patterns-and-conventions.md)
- [工具链](tooling.md)
