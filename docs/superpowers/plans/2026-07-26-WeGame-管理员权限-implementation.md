# WeGame 管理员权限启动 Implementation Plan

> **执行方式：** Inline Execution；不调用子代理。步骤使用 checkbox (`- [ ]`) 跟踪。

**Goal:** 让 Delta Auto Tools 启动时统一请求管理员权限，使特勤处登录流程可直接强退并重启 WeGame，同时保留安全、无重复前缀的失败诊断。

**Architecture:** 禁止 `tauri-build` 把默认 application manifest 写入共享资源。全 target 合并 Common Controls 基础 manifest，tests 由 linker 默认写入 `asInvoker`；主程序单独关闭默认 UAC 并合并 `requireAdministrator` manifest。不引入 helper 或新依赖。登录流程继续使用当前状态机；原生启动层输出标准 Windows 错误码，production driver 将其转为结构化观察结果，通用步骤执行器仍丢弃未经分类的原始错误。

**Tech Stack:** Rust 2021、Tauri 2、`tauri-build`、Cargo tests、Bun/Vitest、Windows application manifest

---

## 文件结构

- Modify: `src-tauri/Cargo.toml` — 关闭不含测试的主程序 test harness。
- Modify: `src-tauri/build.rs` — 为全 target 注入基础 manifest，并为主程序追加管理员 manifest。
- Create: `src-tauri/windows/app.manifest`、`src-tauri/windows/common-controls.manifest` — 分离主程序权限与共享 Common Controls activation context。
- Create: `src-tauri/tests/windows_manifest_contract.rs` — 锁定源 manifest 与实际 test exe 权限契约。
- Modify: `src-tauri/src/special_ops/desktop_runtime.rs` — 标准化原生启动错误码。
- Modify: `src-tauri/src/special_ops/login_runtime.rs` — 将启动错误转为安全观察结果并写安全日志。
- Modify: `src-tauri/src/special_ops/login_flow.rs` — 去掉失败消息中的重复步骤名并格式化启动失败。
- Modify: `README.md`、`AGENTS.md`、`droid-wiki/overview/getting-started.md`、`droid-wiki/how-to-contribute/development-workflow.md` — 同步管理员权限运行要求。

### Task 1: 只给主程序嵌入管理员权限

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/build.rs`
- Create: `src-tauri/windows/app.manifest`
- Create: `src-tauri/windows/common-controls.manifest`
- Create: `src-tauri/tests/windows_manifest_contract.rs`

- [x] **Step 1: RED：增加主程序与 tests manifest 契约测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test windows_manifest_contract`

Expected: FAIL，提示缺少 `windows/app.manifest`。

- [x] **Step 2: 分离主程序权限与共享基础 manifest**

`tauri_build::WindowsAttributes::new_without_app_manifest()` 保留图标和版本资源，但不写共享 application manifest。全 target `rustc-link-arg` 注入 `common-controls.manifest`；tests 使用 linker 默认 `asInvoker`；`rustc-link-arg-bin` 为主程序注入 `app.manifest`。

- [x] **Step 3: 禁止生成空的提权 test harness**

`Cargo.toml` 增加：

```toml
[[bin]]
name = "delta-auto-tools"
path = "src/main.rs"
test = false
```

原因：`rustc-link-arg-bin` 同样作用于 bin test harness；该 harness 没有测试，却会让普通终端执行 `cargo test` 时触发 Windows 740。

- [x] **Step 4: 验证测试保持普通权限**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test windows_manifest_contract`

Expected: PASS；使用 `mt.exe` 读取 integration test exe，确认包含 `asInvoker` 与 Common Controls v6。

- [x] **Step 5: 构建并检查真实主程序 manifest**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`

Expected: PASS；实际 `delta-auto-tools.exe` manifest 同时包含 `requireAdministrator` 与 `Microsoft.Windows.Common-Controls`。

- [x] **Step 6: 提交**

```bash
git add src-tauri/build.rs src-tauri/windows src-tauri/tests/windows_manifest_contract.rs docs/superpowers
git commit -m "fix(special-ops): 修正 Windows 权限清单"
```

### Task 2: 保留安全启动诊断并删除重复步骤名

**Files:**
- Modify: `src-tauri/src/special_ops/desktop_runtime.rs`
- Modify: `src-tauri/src/special_ops/login_runtime.rs`
- Modify: `src-tauri/src/special_ops/login_flow.rs`

- [ ] **Step 1: 写失败测试**

在 `desktop_runtime.rs` 测试中要求标准错误：

```rust
assert_eq!(
    format_win32_error("启动程序失败", std::io::Error::from_raw_os_error(740)),
    "启动程序失败（Windows 错误 740）"
);
```

在 `login_runtime.rs` 测试中要求只提取标准错误码：

```rust
assert_eq!(
    launch_observation("启动程序失败（Windows 错误 740）"),
    LoginObservation::LaunchFailed {
        windows_error_code: Some(740),
    }
);
assert_eq!(
    launch_observation("RAW_DRIVER_SECRET|C:\\private\\wegame.exe"),
    LoginObservation::LaunchFailed {
        windows_error_code: None,
    }
);
```

在 `login_flow.rs` 测试中要求：

```rust
assert_eq!(
    format_observation(
        "步骤执行失败",
        LoginObservation::LaunchFailed {
            windows_error_code: Some(740),
        },
    ),
    "启动程序失败（Windows 错误 740）"
);
```

并将已有通用失败断言改为 `last_observation == "步骤执行失败"`，双采样超时断言改为 `步骤超时；最后识别结果：...`。

- [ ] **Step 2: 运行测试并确认 RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml special_ops::`

Expected: FAIL，原因分别为旧错误格式、缺少 `LaunchFailed`、失败消息仍包含步骤名。

- [ ] **Step 3: 标准化 Windows 错误**

`desktop_runtime.rs`：

```rust
fn format_win32_error(action: &str, error: std::io::Error) -> String {
    error
        .raw_os_error()
        .map_or_else(|| action.to_string(), |code| format!("{action}（Windows 错误 {code}）"))
}
```

`WindowsDesktopRuntime::launch` 的 spawn 错误改用：

```rust
.map_err(|error| format_win32_error("启动程序失败", error))
```

- [ ] **Step 4: 增加安全启动观察结果**

`login_flow.rs` 的 `LoginObservation` 增加：

```rust
LaunchFailed { windows_error_code: Option<i32> },
```

`login_runtime.rs` 增加：

```rust
fn launch_observation(error: &str) -> LoginObservation {
    let windows_error_code = error
        .strip_prefix("启动程序失败（Windows 错误 ")
        .and_then(|value| value.strip_suffix('）'))
        .and_then(|value| value.parse().ok());
    LoginObservation::LaunchFailed { windows_error_code }
}
```

`ProductionLoginDriver::launch` 在返回错误前设置 observation，并只记录错误码：

```rust
let result = tokio::task::spawn_blocking(move || WindowsDesktopRuntime.launch(&executable))
    .await
    .map_err(|error| format!("程序启动任务失败: {error}"))?;
if let Err(error) = &result {
    let observation = launch_observation(error);
    let windows_error_code = match observation {
        LoginObservation::LaunchFailed { windows_error_code } => windows_error_code,
        _ => None,
    };
    self.set_observation(observation);
    crate::log_error!(
        "special_ops::login",
        "WeGame 启动失败",
        "windows_error_code" => windows_error_code
    );
}
result
```

- [ ] **Step 5: 删除失败消息中的重复步骤名**

`login_flow.rs` 保留 `failed_step`，但将 formatter 改为：

```rust
fn format_observation(kind: &str, observation: LoginObservation) -> String {
    match observation {
        LoginObservation::None => kind.to_string(),
        LoginObservation::LaunchFailed { windows_error_code: Some(code) } => {
            format!("启动程序失败（Windows 错误 {code}）")
        }
        LoginObservation::LaunchFailed { windows_error_code: None } => {
            "启动程序失败".to_string()
        }
        LoginObservation::TemplateSamples { samples } => format!(
            "{kind}；最后识别结果：双采样相似度 {:.2}% / {:.2}%",
            samples[0] * 100.0,
            samples[1] * 100.0
        ),
        LoginObservation::CaptureFailed => format!("{kind}；最后识别结果：截图失败"),
        LoginObservation::ReferenceImageFailed => {
            format!("{kind}；最后识别结果：参考图读取失败")
        }
        LoginObservation::WindowNotFound => format!("{kind}；最后识别结果：未找到游戏窗口"),
    }
}
```

`paused` 调用改为 `format_observation(kind, driver.last_observation())`。未经结构化分类的 driver 原始错误继续由 `run_step` 丢弃。

- [ ] **Step 6: 运行相关测试并确认 GREEN**

Run: `cargo test --manifest-path src-tauri/Cargo.toml special_ops::`

Expected: 所有 `special_ops` Rust 测试 PASS，敏感信息断言继续通过。

- [ ] **Step 7: 提交**

```bash
git add src-tauri/src/special_ops/desktop_runtime.rs src-tauri/src/special_ops/login_runtime.rs src-tauri/src/special_ops/login_flow.rs
git commit -m "fix(special-ops): 保留安全启动错误诊断"
```

### Task 3: 同步管理员权限文档

**Files:**
- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `droid-wiki/overview/getting-started.md`
- Modify: `droid-wiki/how-to-contribute/development-workflow.md`

- [ ] **Step 1: 更新开发命令说明**

四份文档统一写明：

```text
Windows 桌面版使用管理员权限运行。执行 `bun run tauri dev` 前必须先打开管理员 PowerShell；软件启动时只显示一次 UAC，后续 WeGame 切号不重复提权。
```

仅 `bun run dev` 的浏览器 UI 开发不要求管理员权限。

- [ ] **Step 2: 检查文档一致性**

Run: `rg -n "管理员权限|管理员 PowerShell|bun run tauri dev" README.md AGENTS.md droid-wiki/overview/getting-started.md droid-wiki/how-to-contribute/development-workflow.md`

Expected: 四份文档均包含管理员终端要求，不存在“普通终端可完整启动桌面版”的冲突说明。

- [ ] **Step 3: 提交**

```bash
git add README.md AGENTS.md droid-wiki/overview/getting-started.md droid-wiki/how-to-contribute/development-workflow.md
git commit -m "docs: 说明 Windows 管理员权限要求"
```

### Task 4: 全量验证并合并回 master

**Files:**
- Verify: all modified files

- [x] **Step 1: 更新 CodeGraph 索引**

Run: `codegraph sync`

Expected: 索引同步成功。

- [x] **Step 2: 运行质量门禁**

Run: `bun run check`

Expected: TypeScript、Vitest、coverage、Rust fmt、Clippy `-D warnings`、Rust tests 全部 PASS。

- [x] **Step 3: 构建前端与 Windows exe**

Run: `bun run build`

Expected: PASS。

Run: `cargo build --manifest-path src-tauri/Cargo.toml`

Expected: PASS，生成 `src-tauri/target/debug/delta-auto-tools.exe`。

- [x] **Step 4: 检查实际 exe manifest**

使用 Windows SDK `mt.exe` 提取或读取 exe manifest，确认同时包含：

```text
requestedExecutionLevel level="requireAdministrator" uiAccess="false"
Microsoft.Windows.Common-Controls
```

- [x] **Step 5: 核对工作区与提交**

Run: `git status --short --branch`

Expected: 除进入任务前已存在且内容 hash 未变化的 `src-tauri/Cargo.toml` 行尾状态外，无未提交任务改动。

- [ ] **Step 6: 合并回 master**

在 `C:\delta-auto-tools` 检查 `master` 无未提交改动，再执行：

```bash
git merge --no-ff codex/special-ops-login -m "merge: 完成特勤处登录切号与 WeGame 提权"
```

合并后在 `master` 再运行 `bun run check`。通过后删除已合并 worktree 与临时分支；若 `src-tauri/Cargo.toml` 行尾状态仍属于进入任务前状态，先验证 blob hash，禁止覆盖用户内容。
