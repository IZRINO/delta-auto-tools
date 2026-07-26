# WeGame 管理员权限启动设计

日期：2026-07-26

状态：已批准；2026-07-26 按真实 exe manifest 验证修正构建实现

范围：特勤处登录试运行及后续自动切号流程

## 问题

WeGame `wegame.exe` 的 Windows manifest 使用 `requireAdministrator`。Delta Auto Tools 当前以普通权限运行，并通过 `std::process::Command::spawn` 直接启动 WeGame。该 API 不负责弹出 UAC 提权提示，因此 `StartWeGame` 步骤在进程创建阶段失败。

当前失败结果还存在两个诊断缺陷：

- `run_step` 丢弃底层启动错误，只保存“步骤执行失败”。
- 持久化失败信息已包含步骤名，前端又追加步骤名，最终显示为 `StartWeGame：StartWeGame：步骤执行失败`。

## 决策

Delta Auto Tools Windows 可执行文件统一声明 `requireAdministrator`。用户每次启动软件时确认一次 UAC；随后 WeGame 由同权限进程直接启动，不在每次切号时重复请求提权。

继续使用现有 `Command::spawn`、进程精确结束逻辑和登录状态机。无需引入提权 helper、IPC 或新依赖。

## 构建实现

`tauri-build` 仍生成图标与版本资源，但不再向共享 `resource.lib` 写入默认 application manifest。仓库保存两份显式 manifest：

- `src-tauri/windows/app.manifest`：Common Controls v6 + `requireAdministrator`；
- `src-tauri/windows/common-controls.manifest`：不声明 UAC，只提供共享 definition identity 与 Common Controls v6。

`src-tauri/build.rs` 通过全 target `rustc-link-arg` 注入 Common Controls 基础 manifest，使 library unit-test harness 也获得有效 activation context；tests 保留 linker 默认的 `asInvoker`。主程序再通过 `rustc-link-arg-bin` 关闭默认 UAC 片段并注入 `app.manifest`。`Cargo.toml` 继续关闭不含测试的 `src/main.rs` test harness。无需新增依赖。非 Windows target 不新增运行时行为。

不能只在 `tauri-build` 默认 manifest 之外追加 `/MANIFESTUAC`：默认 manifest 已作为资源嵌入，fresh build 的真实 exe 仍可能只保留 Common Controls、丢失 `requestedExecutionLevel`。最终结果必须以 `mt.exe` 读取真实产物为准。

开发版必须从管理员终端运行 `bun run tauri dev`。正式安装版或 debug exe 由 Windows 在启动时显示一次 UAC。

## 错误处理

`StartWeGame` 失败时记录安全的 Windows 错误类别和错误码，供后续定位。日志及持久化结果禁止包含：

- QQ 账号；
- 密码；
- 账号对象序列化内容；
- 用户目录或可执行文件完整路径。

`LoginFlowResult::Paused` 继续分别保存 `failedStep` 与失败信息。失败信息不再重复嵌入步骤名，前端仍由 `failedStep` 负责显示步骤。

其他登录步骤继续使用现有截图、参考图、窗口及双采样识别结果，不改变暂停、紧急停止或账号状态规则。

## UI 行为

- 软件启动：Windows 显示一次 UAC；拒绝后软件不启动。
- 自动化运行期间：不再为每次 WeGame 重启显示 UAC。
- 失败提示：显示一次步骤名，例如 `StartWeGame：启动程序失败（Windows 错误 740）`，禁止重复前缀。

本改动不新增设置项或权限开关。管理员权限是 Windows 版运行前提，避免出现“配置允许关闭、但关闭后自动化必然失败”的无效状态。

## 验证

按 TDD 增加最小回归覆盖：

1. Rust 测试 exe 保持普通权限并可由 `cargo test` 直接运行。
2. 登录步骤失败结果不重复步骤名。
3. 失败信息不泄露账号、密码、可执行文件路径或模拟 driver 的原始敏感错误。
4. Rust 单元测试、前端相关测试、`cargo check` 与前端 build 通过。
5. 构建后读取实际 `delta-auto-tools.exe` manifest，确认包含 `requireAdministrator` 与 Common Controls v6。

人工验收：

1. 从非管理员桌面启动软件，只出现一次 UAC。
2. 登录试运行强制结束旧 WeGame 后可重新启动 WeGame。
3. 同一软件会话再次切号时不出现第二次 UAC。
4. WeGame 启动失败时，UI 不再显示重复步骤名。

## 文档同步

同步更新 README、AGENTS 与对应 droid-wiki 开发说明：

- Windows 版启动需要管理员权限；
- `bun run tauri dev` 需在管理员终端执行；
- UAC 只在 Delta Auto Tools 启动时出现一次。

## 非目标

- 不开发独立提权 helper。
- 不改 WeGame 安装或兼容性设置。
- 不绕过 Windows UAC。
- 不改登录步骤顺序、校准目标或账号业务配置。
- 不自动点击或模拟确认 UAC。
