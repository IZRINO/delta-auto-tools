# Delta Auto Tools 日志系统设计 Spec

> 版本：v1.0  
> 日期：2026-06-16

---

## 1. 目标

为 Delta Auto Tools 添加分级日志系统，要求：

- Rust 后端与 React 前端日志写入**同一个文件**，格式完全统一
- 日志文件存放在**软件安装目录**的 `logs/` 子文件夹
- 支持五个级别：`ERROR / WARN / INFO / DEBUG / TRACE`
- 混合格式：前半段人类可读 + `|` 后半段 JSON 结构化
- 支持跨前后端链路追踪（`trace_id`）和运行实例标识（`session_id`）
- 按天轮转，自动清理过期文件
- 通过配置文件控制级别过滤

---

## 2. 日志格式

### 2.1 完整格式定义

```
{timestamp} | {level} | {origin} | {location} | {trace} | {session} | {message} | {json_payload}
```

### 2.2 各字段规范

| 字段 | 宽度/格式 | 说明 | Rust 来源 | 前端来源 |
|------|----------|------|-----------|---------|
| `timestamp` | `yyyy-MM-dd HH:mm:ss.SSS +ZZZZ` | 带毫秒+时区 | `chrono::Local::now()` | 通过 Tauri Command 由 Rust 生成（前端不自己生成时间） |
| `level` | 左对齐 5 字符 `ERROR / WARN  / INFO  / DEBUG / TRACE` | 日志级别 | 传入参数 | 传入参数 |
| `origin` | `[RUST]·{module}` 或 `[FE]·{component}` | 区分前后端+模块路径 | `module_path!()` 截断 | 调用方显式传入（如 `timer-page`） |
| `location` | `{file}:{line}` 或 `{Component}:{hook}` | 代码定位 | `file!():line!()` | 调用方显式传入（如 `TimerPage:autosave`） |
| `trace` | `trace:{hex4}` | 4位hex链路追踪ID | 从 ThreadLocal 获取（需要显式设置） | 前端生成并通过 `log_write_frontend` 传入 |
| `session` | `sess:{alnum6}` | 6位本次运行实例ID | Rust 启动时生成 `LazyLock` | 通过 `log_get_session_id` Tauri Command 获取 |
| `message` | 自由文本 | 人类可读消息 | 传入参数 | 传入参数 |
| `json_payload` | `{...}` JSON 对象 | 结构化附加数据 | `serde_json::Value` | `Record<string, unknown>` 序列化后传入 |

### 2.3 格式示例

```
2025-06-16 14:32:01.234 +0800 | INFO  | [RUST]·morse::mod       | mod.rs:142       | trace:a7f3 | sess:8k2m9p | 识别完成，结果: 1234 | {"msg":"识别完成，结果: 1234","ctx":{"result":"1234","regions":3,"binary_threshold":120},"duration_ms":234,"thread_id":12345}
2025-06-16 14:32:01.456 +0800 | WARN  | [FE]·timer-page         | TimerPage:autosave | trace:a7f3 | sess:8k2m9p | 热键解析失败 | {"msg":"热键解析失败","ctx":{"hotkey":"Ctrl+ invalid","error":"Unrecognized key"},"window_label":"main"}
2025-06-16 14:32:02.112 +0800 | ERROR | [RUST]·delta::commands  | commands.rs:88   | trace:--   | sess:8k2m9p | 游戏数据查询失败: 请求超时 | {"msg":"游戏数据查询失败: 请求超时","ctx":{"endpoint":"get_player","account_id":42},"duration_ms":5000,"thread_id":12348}
```

无 trace 时填 `trace:--`。

### 2.4 json_payload 内部字段约定

| 字段 | 类型 | 必选 | 说明 |
|------|------|------|------|
| `msg` | string | 是 | 与 message 字段内容相同，方便脚本提取 |
| `ctx` | object | 否 | 业务上下文（识别结果、热键值、计时器配置等） |
| `duration_ms` | number | 否 | 操作耗时毫秒数 |
| `memory_kb` | number | 否 | 进程内存占用（Rust 后端 DEBUG 级别自动附加） |
| `thread_id` | number | 否 | OS 线程 ID（Rust 后端自动附加） |
| `window_label` | string | 否 | Tauri 窗口标签 |
| `account_id` | number | 否 | 关联的 Delta 账号 ID |
| `card_id` | string | 否 | 关联的工具卡片 ID |

---

## 3. 文件存放与轮转

### 3.1 路径策略

```
主路径:  {current_exe().parent()}/logs/
回退路径: {app_local_data_dir()}/logs/    (即 %LocalAppData%\org.izrino.delta-auto-tools\logs\)
```

初始化逻辑：

1. 尝试 `current_exe().parent()?.join("logs")`
2. 尝试 `fs::create_dir_all()`
3. 尝试在目录内创建测试文件写入
4. 上述任一步失败 → fallback 到 `app_local_data_dir().join("logs")`
5. 在首条日志中记录实际使用路径：`INFO | [RUST]·logging | 日志目录: {实际路径}`

### 3.2 文件命名

```
delta-{yyyyMMdd}.log
```

示例：`delta-20250616.log`

### 3.3 轮转机制

- 每次 `log_write()` 调用前检查当前日期
- 跨天时关闭旧 `BufWriter`，打开新日期文件
- 日期判断使用 `chrono::Local::now().format("%Y%m%d")`

### 3.4 清理策略

- 启动时扫描 `logs/` 目录，删除修改时间超过 **30 天** 的 `.log` 文件
- 目录总大小超过 **100 MB** 时，按修改时间从旧到新删除直到低于阈值
- 清理逻辑仅在初始化时执行一次，不在运行中反复扫描

---

## 4. Rust 后端实现

### 4.1 新增模块结构

```
src-tauri/src/
├── logging/
│   ├── mod.rs         # 公共接口 + Tauri Command + session_id 管理
│   ├── format.rs      # 格式化：拼接混合格式字符串
│   ├── writer.rs      # 文件写入、BufWriter、按天轮转、清理
│   └── macros.rs      # log_error! / log_warn! / log_info! / log_debug! / log_trace! 宏
```

### 4.2 核心类型

```rust
// logging/mod.rs

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    /// 数值越大越详细，用于级别过滤
    pub fn value(&self) -> u8 {
        match self {
            Self::Error => 0,
            Self::Warn  => 1,
            Self::Info  => 2,
            Self::Debug => 3,
            Self::Trace => 4,
        }
    }
}

/// 前端通过 Tauri Command 传入的日志请求
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendLogRequest {
    pub level: LogLevel,
    pub source: String,       // e.g. "timer-page"
    pub location: String,     // e.g. "TimerPage:autosave"
    pub trace_id: String,     // e.g. "a7f3" 或 "--"
    pub message: String,
    pub payload: Option<serde_json::Value>,  // JSON 附加数据
}
```

### 4.3 全局状态

```rust
// logging/writer.rs

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Mutex;

struct LogWriterInner {
    writer: BufWriter<File>,
    current_date: String,        // "20250616"
    log_dir: PathBuf,
}

pub struct LogWriter {
    inner: Mutex<Option<LogWriterInner>>,
    global_level: Mutex<LogLevel>,
}

impl LogWriter {
    pub fn new(log_dir: PathBuf) -> Self { ... }
    pub fn write(&self, level: LogLevel, formatted_line: &str) { ... }
    pub fn set_level(&self, level: LogLevel) { ... }
    pub fn should_log(&self, level: LogLevel) -> bool { ... }
    fn ensure_writer(&self, date_str: &str) -> Result<(), String> { ... }
    fn rotate_if_needed(&self) { ... }
}
```

### 4.4 格式化

```rust
// logging/format.rs

pub fn format_log_line(
    timestamp: &chrono::DateTime<chrono::Local>,
    level: LogLevel,
    origin: &str,      // "[RUST]·morse::mod"
    location: &str,    // "mod.rs:142"
    trace_id: &str,    // "a7f3" 或 "--"
    session_id: &str,  // "8k2m9p"
    message: &str,
    payload: Option<&serde_json::Value>,
) -> String {
    // 前半段: 2025-06-16 14:32:01.234 +0800 | INFO  | [RUST]·morse::mod | mod.rs:142 | trace:a7f3 | sess:8k2m9p | 识别完成，结果: 1234
    // 后半段: | {"msg":"识别完成，结果: 1234","ctx":{...},"duration_ms":234}
}
```

- timestamp 格式：`%Y-%m-%d %H:%M:%S%.3f %z` → `2025-06-16 14:32:01.234 +0800`
- level 左对齐 5 字符填充空格
- origin 最长 24 字符，超出截断尾部
- location 最长 20 字符，超出截断尾部

### 4.5 宏定义

```rust
// logging/macros.rs

#[macro_export]
macro_rules! log_error {
    ($source:expr, $msg:expr $(, $key:expr => $val:expr)*) => {
        $crate::logging::log_write(
            $crate::logging::LogLevel::Error,
            $source,
            concat!(file!(), ":", line!()),
            $msg,
            Some(serde_json::json!({ $($key: $val),* }))
        )
    };
    ($source:expr, $msg:expr) => {
        $crate::logging::log_write(
            $crate::logging::LogLevel::Error,
            $source,
            concat!(file!(), ":", line!()),
            $msg,
            None
        )
    };
}

// log_warn! / log_info! / log_debug! / log_trace! 同理
```

调用示例：

```rust
log_info!("morse::mod", "识别完成", "result" => "1234", "regions" => 3);
log_error!("delta::commands", "请求超时", "endpoint" => "get_player", "duration_ms" => 5000);
log_warn!("timer::mod", "热键冲突");
```

宏内部自动：

- 生成 `origin` 为 `[RUST]·{source}`
- 生成 `location` 为 `{file}:{line}`
- 注入 `thread_id`（从 `std::thread::current().id()`）
- 注入 `trace_id`（从 `TraceContext::current()`）
- DEBUG 级别自动注入 `memory_kb`

### 4.6 trace_id 管理

```rust
// logging/mod.rs (TraceContext 部分)

use std::cell::RefCell;

thread_local! {
    static CURRENT_TRACE_ID: RefCell<String> = RefCell::new("--".to_string());
}

pub struct TraceContext;

impl TraceContext {
    /// 设置当前线程的 trace_id
    pub fn set(trace_id: &str) {
        CURRENT_TRACE_ID.with(|v| *v.borrow_mut() = trace_id.to_string());
    }

    /// 获取当前线程的 trace_id
    pub fn current() -> String {
        CURRENT_TRACE_ID.with(|v| v.borrow().clone())
    }

    /// 清除当前线程的 trace_id（恢复为 "--"）
    pub fn clear() {
        CURRENT_TRACE_ID.with(|v| *v.borrow_mut() = "--".to_string());
    }
}
```

当前 Rust `TraceContext` 只在后端显式调用 `TraceContext::set()` 时生效。前端发起的 Tauri command 由 `invokeLogged()` 记录 traceId；后端 command 内部日志不会自动继承前端 traceId。若某条后端链路需要跨前后端强关联，应给该 command 增加显式 trace 参数或集中入口包装。

### 4.7 session_id 生成

```rust
// logging/mod.rs

use std::sync::LazyLock;

static SESSION_ID: LazyLock<String> = LazyLock::new(|| {
    // 6 位小写字母数字随机短码
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let chars: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut id = String::with_capacity(6);
    let mut remaining = seed;
    for _ in 0..6 {
        let idx = (remaining % 36) as usize;
        id.push(chars[idx] as char);
        remaining /= 36;
    }
    id
});

pub fn session_id() -> &'static str {
    &SESSION_ID
}
```

### 4.8 Tauri Command

```rust
// logging/mod.rs

#[tauri::command]
pub fn log_write_frontend(request: FrontendLogRequest) -> Result<(), String> {
    let timestamp = chrono::Local::now();
    let origin = format!("[FE]·{}", request.source);
    let trace_tag = if request.trace_id.is_empty() {
        "--".to_string()
    } else {
        request.trace_id
    };
    let session = session_id().to_string();

    // 构建 payload：合并 msg 字段
    let mut payload = request.payload.unwrap_or_default();
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("msg".to_string(), serde_json::Value::String(request.message.clone()));
    } else {
        payload = serde_json::json!({"msg": request.message});
    }

    let line = format::format_log_line(
        &timestamp, request.level, &origin, &request.location,
        &trace_tag, &session, &request.message, Some(&payload),
    );

    logging::writer().write(request.level, &line);
    Ok(())
}

#[tauri::command]
pub fn log_get_session_id() -> String {
    session_id().to_string()
}
```

### 4.9 初始化与注册

```rust
// lib.rs 变更

mod logging;  // 新增

// setup 回调中：
let log_writer = logging::init_logger(app.handle())?;  // 创建目录、清理旧文件、打开文件
app.manage(log_writer);

// generate_handler![] 新增：
logging::log_write_frontend,
logging::log_get_session_id,
```

### 4.10 关闭时 flush

```rust
// lib.rs on_window_event CloseRequested 中增加：
let log_writer = app.state::<logging::LogWriter>();
logging::shutdown(&log_writer);  // flush BufWriter
```

---

## 5. 前端实现

### 5.1 新增文件

```
src/lib/
├── logging.ts         # 前端日志接口
```

### 5.2 核心接口

```typescript
// src/lib/logging.ts

import { invoke as rawInvoke } from "@tauri-apps/api/core";

export type FrontendLogLevel = "error" | "warn" | "info" | "debug" | "trace";

let _sessionId: string | null = null;
let _traceId: string = "--";

/** 初始化：从 Rust 获取 session_id */
export async function initLogging(): Promise<void> {
  if (!checkNativeShell()) return;
  try {
    _sessionId = await rawInvoke<string>("log_get_session_id");
  } catch {
    _sessionId = null;
  }
}

/** 生成 4 位 hex trace_id */
export function generateTraceId(): string {
  const arr = new Uint8Array(2);
  crypto.getRandomValues(arr);
  return Array.from(arr, b => b.toString(16).padStart(2, "0")).join("");
}

/** 设置当前追踪 ID（在操作入口调用） */
export function setTraceId(id: string): void {
  _traceId = id;
}

/** 清除当前追踪 ID */
export function clearTraceId(): void {
  _traceId = "--";
}

/** 获取当前追踪 ID */
export function currentTraceId(): string {
  return _traceId;
}

/** 写入日志到 Rust 后端 */
export async function logFrontend(
  level: FrontendLogLevel,
  source: string,
  location: string,
  message: string,
  payload?: Record<string, unknown>,
): Promise<void> {
  if (!checkNativeShell()) return;

  try {
    await rawInvoke("log_write_frontend", {
      request: {
        level,
        source,
        location,
        traceId: _traceId,
        message,
        payload: payload ?? null,
      },
    });
  } catch {
    // 日志写入失败不应阻塞业务，静默忽略
  }
}

/** 便捷方法 */
export const log = {
  error: (source: string, location: string, message: string, payload?: Record<string, unknown>) =>
    logFrontend("error", source, location, message, payload),
  warn: (source: string, location: string, message: string, payload?: Record<string, unknown>) =>
    logFrontend("warn", source, location, message, payload),
  info: (source: string, location: string, message: string, payload?: Record<string, unknown>) =>
    logFrontend("info", source, location, message, payload),
  debug: (source: string, location: string, message: string, payload?: Record<string, unknown>) =>
    logFrontend("debug", source, location, message, payload),
  trace: (source: string, location: string, message: string, payload?: Record<string, unknown>) =>
    logFrontend("trace", source, location, message, payload),
};

function checkNativeShell(): boolean {
  if (typeof window === "undefined") return false;
  const w = window as Window & { __TAURI_INTERNALS__?: unknown };
  return Boolean(w.__TAURI_INTERNALS__);
}
```

### 5.3 Tauri command 包装

`src/lib/logging.ts` 额外提供 `invokeLogged()`，生产代码通过 `invokeLogged as invoke` 调用 Tauri command：

- `*_get_*` / `*_read_*` 归类为读取型 command，开始/完成日志写 `DEBUG`
- 其他 command 归类为用户操作，开始/完成日志写 `INFO`
- 任意 command 失败写 `ERROR`，记录原始错误并继续向调用方抛出
- payload 会截断深层/长文本，并屏蔽 `token`、`ticket`、`cookie`、`secret`、`password`、`authorization` 等字段

native shell 检测同时接受 `__TAURI_INTERNALS__` 与 `__TAURI__`，必须与 `useNativeShell()` 的判定保持一致。

该包装器覆盖前端发起的用户操作、配置保存、位置设置、测试播放、更新检查和系统 command 错误。

### 5.4 console 劫持（可选）

在 `main.tsx` 初始化日志后，覆盖全局 console 方法使其同时写入日志文件：

```typescript
// main.tsx 中 initLogging() 之后：

const origConsole = {
  log: console.log,
  warn: console.warn,
  error: console.error,
};

console.log = (...args) => {
  origConsole.log(...args);
  log.info("console", "auto", args.map(String).join(" "));
};

console.warn = (...args) => {
  origConsole.warn(...args);
  log.warn("console", "auto", args.map(String).join(" "));
};

console.error = (...args) => {
  origConsole.error(...args);
  log.error("console", "auto", args.map(String).join(" "));
};
```

- 此行为仅在生产环境（`import.meta.env.PROD === true`）生效
- 开发环境保留原始 console 行为，避免 Vite HMR 热更新时大量日志污染

### 5.5 trace_id 使用模式

前端在用户操作入口生成 trace_id，贯穿整个操作链路：

```typescript
// morse-page.tsx 示例
async function handleRunRecognition() {
  const tid = generateTraceId();
  setTraceId(tid);
  try {
    log.info("morse-page", "MorsePage:invoke", "调用 morse_run_recognition");
    await invoke("morse_run_recognition", { ... });
  } finally {
    clearTraceId();
  }
}
```

当前前端操作链路通过 `invokeLogged()` 记录 traceId；后端内部链路可在需要时显式设置 `TraceContext`。

---

## 6. 级别控制

### 6.1 配置文件

路径：`{app_config_dir()}/log_settings.json`

```json
{
  "globalLevel": "info",
  "moduleLevels": {
    "morse": "debug",
    "delta": "warn",
    "frontend": "info"
  }
}
```

### 6.2 类型

```rust
#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LogSettings {
    pub global_level: LogLevel,
    #[serde(default)]
    pub module_levels: HashMap<String, LogLevel>,
}

impl Default for LogSettings {
    fn default() -> Self {
        Self {
            global_level: LogLevel::Info,
            module_levels: HashMap::new(),
        }
    }
}
```

### 6.3 过滤逻辑

```rust
pub fn should_log(&self, level: LogLevel, origin: &str) -> bool {
    // 从 origin 提取模块名，如 "[RUST]·morse::mod" → "morse"
    // 从 "[FE]·timer-page" → "timer"
    let module = extract_module(origin);
    let threshold = self.module_levels
        .get(module)
        .copied()
        .unwrap_or(self.global_level);
    level.value() <= threshold.value()
}
```

过滤在格式化之前执行，低于级别的日志直接丢弃，不产生格式化和写入开销。

### 6.4 运行时更新

提供 Tauri Command：

```rust
#[tauri::command]
pub fn log_set_level(settings: LogSettings) -> Result<(), String> {
    // 保存到 log_settings.json + 更新内存中的过滤阈值
}
```

---

## 7. 新增 Cargo 依赖

```toml
# Cargo.toml [dependencies] 新增
chrono = { version = "0.4", features = ["serde"] }
```

仅此一个新增依赖。项目已依赖 `serde`、`serde_json`、`tokio`（含 `sync` feature），不需要额外引入。

---

## 8. 需要修改的现有文件清单

| 文件 | 修改内容 |
|------|---------|
| `src-tauri/Cargo.toml` | 新增 `chrono` 依赖 |
| `src-tauri/src/lib.rs` | 新增 `mod logging;`、setup 中初始化 logger、manage LogWriter、close 时 flush、generate_handler![] 新增两个命令 |
| `src/main.tsx` | import + 调用 `initLogging()`、生产环境 console 劫持 |

---

## 9. 新增文件清单

| 文件 | 说明 |
|------|------|
| `src-tauri/src/logging/mod.rs` | 公共接口、LogLevel、FrontendLogRequest、TraceContext、session_id、Tauri Commands |
| `src-tauri/src/logging/format.rs` | 混合格式拼接逻辑 |
| `src-tauri/src/logging/writer.rs` | LogWriter（BufWriter + Mutex + 按天轮转 + 清理 + 级别过滤） |
| `src-tauri/src/logging/macros.rs` | log_error! / log_warn! / log_info! / log_debug! / log_trace! 宏 |
| `src/lib/logging.ts` | 前端日志接口（initLogging、logFrontend、invokeLogged、generateTraceId、setTraceId/clearTraceId、便捷 log 对象） |

---

## 10. 后续渐进接入计划

日志系统基础设施搭建完成后，后续可逐步在各模块添加日志调用：

| 优先级 | 模块 | 关键日志点 |
|--------|------|-----------|
| P0 | morse | 识别流程开始/完成/失败、overlay 框选开始/提交/取消、热键录制开始/结束 |
| P0 | rapidfire | 连发 session 开始/停止/补齐、热键冲突拒绝、总开关切换 |
| P0 | command 覆盖 | 前端 `invokeLogged()` 已覆盖所有生产 Tauri command 开始/完成/失败；后端按模块继续补业务上下文 |
| P1 | timer | 计时器触发/完成/重复触发忽略、总开关切换 |
| P1 | counter | 计数器触发/重置/调整 |
| P1 | hotkeys | scope 注册/解绑/冲突检测、键盘钩子安装失败 |
| P2 | strategy | WebView 创建/销毁/重建、自动刷新触发 |
| P2 | about | 更新检查/下载/安装进度 |
| P2 | global_state | 总开关切换 |

跨前后端 trace 自动注入仍是后续增强项；当前不要假设所有后端 command 日志都带前端 traceId。

---

## 11. 测试要求

### 11.1 Rust 测试

| 测试文件 | 覆盖内容 |
|---------|---------|
| `logging/format.rs` | 格式化输出符合 Spec 字段顺序、宽度、截断规则 |
| `logging/writer.rs` | 按天轮转（mock 日期）、清理策略（tempdir + 旧文件）、级别过滤 |
| `logging/mod.rs` | session_id 生成一致性、TraceContext set/get/clear、FrontendLogRequest 反序列化 |

### 11.2 前端测试

| 测试文件 | 覆盖内容 |
|---------|---------|
| `logging.test.ts` | `generateTraceId` 格式/唯一性、`setTraceId`/`clearTraceId` 状态切换、`logFrontend` 参数序列化正确性（mock invoke）、`invokeLogged` 成功/失败日志、非 native shell 时不调用 invoke |
