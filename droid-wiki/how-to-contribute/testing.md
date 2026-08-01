# 测试

## 前端测试（Vitest）

前端测试与源文件同目录放在 `src/` 下，使用 Vitest。运行全部测试用 `bun run test`，带覆盖率用 `bun run test:coverage`。coverage 统计 `src/**/*.{ts,tsx}`，全局阈值为 lines 25.49%、statements 25.67%、functions 22.31%、branches 25.76%；`autosave-queue.ts`、`tauri-listener.ts`、`recognition-card-reducer.ts` 的 lines 阈值为 90%。

运行单个文件：

```bash
bunx vitest run src/components/app/morse-utils.test.ts
```

### 测试覆盖

| 测试文件 | 范围 |
|----------|------|
| `src/components/app/morse-utils.test.ts` | Morse 序列化、格式化、热键解析 |
| `src/components/app/timer-utils.test.ts` | 计时器 settings 转换、进度计算、倒计时格式化 |
| `src/components/app/counter-utils.test.ts` | 计数器 settings 转换 |
| `src/components/app/favorites-utils.test.ts` | 收藏 ID 读写、卡片过滤 |
| `src/components/app/rapidfire-types.test.ts` | 连发器类型常量 |
| `src/components/app/recognition-utils.test.ts` | 音频颜色转换、探针表单解析 |
| `src/components/app/strategy-utils.test.ts` | 攻略站点常量、刷新档位 |
| `src/components/app/theme-utils.test.ts` | 主题 token 合并、应用、导入、hex 规范化 |
| `src/components/app/profile-utils.test.ts` | 配置时间戳格式、名称验证、快照助手 |
| `src/components/app/about-deps.test.ts` | 依赖列表基础 |
| `src/hooks/use-autosave.test.ts` | Autosave 防抖逻辑 |
| `src/hooks/use-bootstrap-form-logic.test.ts` | Bootstrap/form 双状态脏检测 |
| `src/hooks/use-hotkey-recorder.test.ts` | 热键录制交互逻辑 |
| `src/lib/logging.test.ts` | TraceId 生成、setTraceId/clearTraceId、日志序列化 |

### 模式

前端测试聚焦 `*-utils.ts` 中的纯逻辑函数和 hook 行为。不直接测试 Tauri command 调用；测试环境中 `useNativeShell()` 返回 false，测试断言 `invoke` 未被调用或 hook 优雅降级。

## Rust 测试（cargo test）

Rust 测试是各模块内的内联 `#[cfg(test)] mod tests` 块。运行：

```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

运行单个测试：

```bash
cargo test --manifest-path src-tauri/Cargo.toml <test_name>
```

## 统一质量门禁

Windows 本地与 GitHub Actions 共用 `bun run check`。脚本按依赖顺序执行 TypeScript、Vitest、coverage、Rust fmt、Clippy `-D warnings`、Rust tests，任一步失败立即退出。Rust tests 通过 `--test-threads=1` 串行执行，因为 Windows 原生进程/窗口测试共享系统句柄，避免测试间竞态导致测试进程崩溃。CI 定义位于 `.github/workflows/ci.yml`。

### 测试覆盖

- `hotkey_types.rs` / `hotkeys.rs`：热键解析、冲突检测（Strict vs AllowHold）、hold 匹配、组合修饰键、计时器+连发器同键共触
- `morse/decoder.rs`：数字 0-9 摩斯解码、未知模式报错
- `morse/recognition.rs`：DPI/多显示器区域坐标转换
- `morse/mod.rs`：历史记录上限
- `morse/types.rs`：Settings 默认值
- `timer/types.rs` / `settings.rs`：计时器默认值、settings 读写往返
- `timer/mod.rs`：透明窗口尺寸计算、settings 验证
- `recognition/types.rs`：识别触发卡片反序列化默认值、旧字段迁移、识色探针往返
- `theme/apply.rs`：`merge_theme_tokens`（覆盖、追加、顺序、去重）、`find_theme`
- `theme/builtins.rs`：3 套 daisyUI 内置主题、唯一 ID、token key 一致性
- `theme/mod.rs`：`build_bootstrap`、`theme_import`（合法/非法 key 拒绝）、导出
- `profile/types.rs` / `settings.rs` / `mod.rs`：快照空、默认值、camelCase、往返、profile_id 唯一性
- `logging/format.rs` / `writer.rs` / `mod.rs`：格式字段顺序、轮转、清理（tempdir）、级别过滤、session_id、TraceContext
- `sync_tool.rs`：`normalize_sync_settings`（默认条目插入、孤儿分组重分配、重复 ID 拒绝）、`apply_position_event`（移动/提交/取消）、`ToolLifecycleRegistry`

## 添加测试

- 新增纯逻辑函数应有对应 `*.test.ts` 文件
- 新增 Rust 类型应有 serde 往返测试和默认值测试
- 热键冲突规则由 `hotkeys.rs` 中的测试守护；修改冲突策略时更新这些测试
