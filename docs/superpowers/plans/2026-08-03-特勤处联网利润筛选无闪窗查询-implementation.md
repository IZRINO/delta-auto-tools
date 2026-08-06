# 特勤处联网利润筛选无闪窗查询 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. The user selected Inline Execution; do not create worktrees or dispatch subagents.

**Goal:** 让联网利润筛选在不创建或闪现顶级窗口、不占用用户键鼠的前提下，使用可复用的隐藏 Moligod 子 WebView，并在 round 前后保持严格的“立即 -> 5 分钟 -> 5 分钟 -> 50 分钟”查询 cadence。

**Architecture:** `ProfitQueryControl` 继续只保存进程内资格、generation 与 cadence；进入 active round 时保留下一查询时间，round 结束时恢复等待态而不重置查询组。`MoligodAdapter` 改为 `SpecialOpsState` 持有的单例渲染器：它在主窗口内部创建隐藏、离屏、无 IPC 权限的 child WebView，串行执行查询，每次用新的导航 nonce 与页面脚本读取最终 DOM 利润。前端只把两张长表收进默认折叠区，外层运行状态、错误、保存动作始终显示。

**Tech Stack:** Rust、Tauri 2.11 child `Webview`、Tokio、React 19、TypeScript、Vitest、Cargo test、Bun。

**版本控制边界：** 当前工作区含用户未提交改动。实施期间禁止 `reset`、`revert`、清理工作区或创建 commit；每个任务仅运行定向验证，最终再运行完整门禁。

---

## 文件结构

| 文件 | 责任 |
| --- | --- |
| `src-tauri/src/special_ops/profit/runtime.rs` | 查询 cadence、active round 与暂停/失效之间的内存状态转换。 |
| `src-tauri/src/special_ops/profit/moligod.rs` | 唯一隐藏 child WebView、页面导航、DOM 回传校验、销毁与隔离目录清理。 |
| `src-tauri/src/special_ops/profit/moligod_freshness_guard.js` | document-start 的市场快照网络刷新 guard；不读取利润、不使用 IPC。 |
| `src-tauri/src/special_ops/profit/moligod_scraper.js` | 只读取网页最终“预估净利润”，不重算。 |
| `src-tauri/src/special_ops/mod.rs` | 共享 renderer、scheduler 查询、绑定验证、配置关闭与应用生命周期。 |
| `src/components/app/special-ops-profit-filter.tsx` | 外层状态区和两张默认折叠表。 |
| `src/components/app/special-ops-profit-filter.test.tsx` | UI 折叠与外层状态可见性。 |
| `src/components/app/special-ops-moligod-scraper.test.ts` | DOM 最终利润和刷新语义。 |
| `README.md`、`AGENTS.md`、`droid-wiki/features/special-ops.md` | 无闪窗、缓存与 cadence 规则说明。 |

### Task 1: 锁定 cadence 与 round 状态机

**Files:**
- Modify: `src-tauri/src/special_ops/profit/runtime.rs`
- Test: `src-tauri/src/special_ops/profit/runtime.rs`

- [ ] **Step 1: 写失败测试，证明进入 round 不得清空下一次查询时间。**

~~~rust
#[test]
fn round_freeze_preserves_next_query_and_round_end_does_not_restart_group() {
    let control = ProfitQueryControl::default();
    let lease = control
        .begin_query("2026-08-03", 4, 1_000, vec![rule("rule-a")])
        .unwrap();
    control
        .complete_query(
            &lease,
            2_000,
            HashSet::from(["rule-a".to_string()]),
            "1 个达标".to_string(),
        )
        .unwrap();
    let next = control.snapshot().unwrap().next_query_at_ms;

    control.consume_for_round(lease.generation, vec![]).unwrap();
    assert_eq!(control.snapshot().unwrap().next_query_at_ms, next);

    control.end_active_round("轮次已完成").unwrap();
    let snapshot = control.snapshot().unwrap();
    assert_eq!(snapshot.phase, ProfitRuntimePhase::WaitingNextQuery);
    assert_eq!(snapshot.next_query_at_ms, next);
}
~~~

- [ ] **Step 2: 运行测试，确认旧实现失败。**

Run:

~~~powershell
cargo test --manifest-path src-tauri/Cargo.toml special_ops::profit::runtime::tests::round_freeze_preserves_next_query_and_round_end_does_not_restart_group
~~~

Expected: FAIL；旧 `consume_for_round()` 清空 `next_query_at_ms`，`end_active_round()` 调用 `invalidate()`。

- [ ] **Step 3: 最小化改状态转换。**

在 `sync_window()` 中仅用 `window.active_round` 判断当前 round，禁止历史 `state.phase == ActiveRound` 永久阻塞后续状态转换。`consume_for_round()` 保留 `next_query_at_ms`；`end_active_round()` 清空 active targets 和资格、保留 `group_attempt` 与下一查询时间，并返回 `Result`：

~~~rust
} else if window.active_round {
    state.phase = ProfitRuntimePhase::ActiveRound;
} else if window.now_ms < window.exchange_at_ms {
    state.phase = ProfitRuntimePhase::WaitingExchange;
    state.next_query_at_ms = Some(window.exchange_at_ms);
}

pub(crate) fn end_active_round(&self, reason: &str) -> Result<(), String> {
    let mut state = self.inner.lock()
        .map_err(|_| "利润查询状态已损坏".to_string())?;
    state.active_round_targets.clear();
    state.qualified_rule_ids.clear();
    state.phase = ProfitRuntimePhase::WaitingNextQuery;
    state.last_summary = Some(reason.to_string());
    Ok(())
}
~~~

`rollback_failed_round_start()` 保持 fail-closed：资源启动失败仍撤销 generation、清空资格并暂停。暂停、关闭、配置 revision 变化、截止时间仍使用 `invalidate()`。

- [ ] **Step 4: 补齐边界测试。**

~~~rust
#[test]
fn late_round_end_keeps_expired_query_time_for_one_scheduler_catch_up() {
    // nextQueryAtMs 已过期时仍保留原时间。
}

#[test]
fn cutoff_round_without_query_does_not_create_a_new_query_group() {
    // CutoffBypass round 结束后，后续同步仍为 CutoffBypass。
}
~~~

- [ ] **Step 5: 验证 runtime。**

Run:

~~~powershell
cargo test --manifest-path src-tauri/Cargo.toml special_ops::profit::runtime::tests
~~~

Expected: PASS。

### Task 2: 将 Moligod 顶级窗口改为共享 child WebView

**Files:**
- Modify: `src-tauri/src/special_ops/profit/moligod.rs`
- Create: `src-tauri/src/special_ops/profit/moligod_freshness_guard.js`
- Test: `src-tauri/src/special_ops/profit/moligod.rs`
- Test: `src/components/app/special-ops-moligod-scraper.test.ts`

- [ ] **Step 1: 写失败测试，固定 nonce URL 与 source-level 故障生命周期。**

~~~rust
#[test]
fn query_url_has_unique_nonce_and_stays_on_allowed_origin() {
    let url = moligod_query_url("nonce-7").unwrap();
    assert!(is_allowed_moligod_navigation(&url));
    assert!(url.query_pairs().any(|(key, value)| {
        key == "deltaSpecialOpsQuery" && value == "nonce-7"
    }));
}

#[test]
fn source_level_renderer_failure_requires_session_destruction() {
    assert!(should_destroy_renderer(&Err("Moligod 页面导航失败".to_string())));
    assert!(should_destroy_renderer(&Err("Moligod 查询超时".to_string())));
}
~~~

- [ ] **Step 2: 用持久 session 替换 `WebviewWindowBuilder`。**

保留现有 `MoligodRequestTarget`、title payload schema、`parse_moligod_title()`、精确名称唯一性与整数校验。将 adapter 改为以下状态并让 `fetch()` 先取得 `query_lock`：

~~~rust
const MOLIGOD_RENDERER_LABEL: &str = "special-ops-profit-renderer";

struct MoligodPendingQuery {
    expected: Arc<MoligodExpected>,
    navigation_nonce: String,
    sender: oneshot::Sender<Result<MoligodSnapshot, String>>,
}

struct MoligodWebviewSession {
    webview: tauri::Webview,
    data_path: PathBuf,
    pending: Arc<Mutex<Option<MoligodPendingQuery>>>,
}

pub(crate) struct MoligodAdapter {
    app: AppHandle,
    session: Mutex<Option<MoligodWebviewSession>>,
    query_lock: tokio::sync::Mutex<()>,
}
~~~

查询成功后保留 session。导航、页面、脚本、超时、取消或 title 的 nonce/generation/名称校验失败，清空 pending、关闭 child WebView、后台清理隔离目录；单个目标的 `sourceFailure` 仍只记录审计，不销毁健康 renderer。

- [ ] **Step 3: 通过主窗口添加离屏 child WebView。**

禁止再调用 `WebviewWindowBuilder`。`ensure_session()` 只能从主窗口创建 child，先隐藏再外部导航：

~~~rust
let navigation_pending = Arc::clone(&pending);
let page_pending = Arc::clone(&pending);
let title_pending = Arc::clone(&pending);
let parent = self.app.get_webview_window("main")
    .ok_or_else(|| "主窗口不可用，无法创建 Moligod 隐藏渲染器".to_string())?;
let webview = parent.add_child(
    tauri::WebviewBuilder::new(
        MOLIGOD_RENDERER_LABEL,
        WebviewUrl::App("index.html".into()),
    )
    .initialization_script(build_market_freshness_guard())
    .on_navigation(move |url| {
        let allowed = is_allowed_moligod_navigation(url);
        if !allowed {
            send_pending_error(&navigation_pending, &format!("Moligod 拒绝跨站导航：{url}"));
        }
        allowed
    })
    .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
    .on_download(|_, _| false)
    .on_page_load(move |webview, payload| {
        if payload.event() == tauri::webview::PageLoadEvent::Finished {
            inject_pending_query_script(&webview, payload.url(), &page_pending);
        }
    })
    .on_document_title_changed(move |_, title| {
        dispatch_pending_title_result(&title_pending, &title);
    }),
    tauri::LogicalPosition::new(-10_000.0, -10_000.0),
    tauri::LogicalSize::new(1.0, 1.0),
)?;
webview.set_auto_resize(false)?;
webview.hide()?;
~~~

新增 `inject_pending_query_script(webview: &tauri::Webview, loaded_url: &Url, pending: &PendingSlot)`：它只在页面已完成、URL 仍是当前 `deltaSpecialOpsQuery` nonce、且 pending 未被取消时调用 `build_query_script()` 和 `webview.eval()`；失败时调用 `send_pending_error()`。新增 `dispatch_pending_title_result(pending: &PendingSlot, title: &str)`：它取出当前 expected，调用现有 `validated_title_event()`，仅在结果 nonce 与请求匹配时向 sender 发送一次结果。

现有 `src-tauri/capabilities/special-ops-profit.json` 的 `special-ops-profit-*` label 匹配与空 permissions 必须保持；不添加 IPC、文件、shell、剪贴板、窗口管理或键鼠权限。

- [ ] **Step 4: 分离静态新鲜度 guard 与本次查询脚本。**

每次导航使用 `deltaSpecialOpsQuery=<nonce>`。创建 `moligod_freshness_guard.js`，由 `build_market_freshness_guard()` 以 `include_str!` 注入 document-start：它仅重发同 origin、GET、JSON 且响应 `Cache-Control` 声明 `max-age=300` 的数据请求，第二次请求强制 `cache: "reload"`；静态资源、POST、跨域请求不改。页面加载完成后，对同 nonce 的 pending request 执行 `webview.eval(build_query_script(&expected)?)`。

~~~js
(() => {
    const nativeFetch = window.fetch.bind(window);
    window.fetch = async (input, init) => {
        const response = await nativeFetch(input, init);
        const url = new URL(response.url || String(input), window.location.href);
        const method = (init?.method || (input instanceof Request ? input.method : "GET")).toUpperCase();
        const cacheControl = response.headers.get("cache-control") || "";
        const contentType = response.headers.get("content-type") || "";
        if (url.origin !== window.location.origin || method !== "GET"
            || !/max-age=300/i.test(cacheControl) || !/application\/json/i.test(contentType)) {
            return response;
        }
        return nativeFetch(input, {...init, cache: "reload"});
    };
})();
~~~

~~~rust
fn build_query_script(expected: &MoligodExpected) -> Result<String, String> {
    let config = serde_json::to_string(&MoligodScriptConfig {
        generation: expected.generation,
        nonce: &expected.nonce,
        targets: &expected.targets,
    }).map_err(|error| format!("序列化 Moligod 只读配置失败：{error}"))?;
    Ok(format!(
        "if (window.location.origin === \"https://moligod.com\") {{ window.__DELTA_SPECIAL_OPS_MOLIGOD_CONFIG__ = {config}; {} }}",
        include_str!("moligod_scraper.js")
    ))
}
~~~

`moligod_scraper.js` 继续只读取最终 DOM “预估净利润”。不得新增材料成本、手续费、市场价格或配方重算。

- [ ] **Step 5: 完成取消和销毁。**

~~~rust
pub(crate) fn shutdown(&self) {
    let session = self.session.lock().ok().and_then(|mut slot| slot.take());
    if let Some(session) = session {
        send_pending_error(&session.pending, "Moligod 查询已取消");
        let _ = session.webview.close();
        spawn_data_directory_cleanup(session.data_path);
    }
}
~~~

删除旧每次查询后的 `window.destroy()`。目录锁导致的 `os error 32` 只写 warning，不覆盖已获得结果，也不返回给前端。

- [ ] **Step 6: 增加 DOM 和 Rust 测试。**

在 `special-ops-moligod-scraper.test.ts` raw import `moligod_freshness_guard.js` 并 mock `window.fetch`：断言满足 5 分钟缓存响应的 JSON GET 会二次以 `cache: "reload"` 请求，其他请求只发一次。保留精确名称、分页、负数、重复和 DOM 异常测试。在 Rust 断言：

~~~rust
assert!(!build_market_freshness_guard().contains("__TAURI__"));
assert!(!build_query_script(&expected).unwrap().contains("材料成本"));
assert!(build_query_script(&expected).unwrap()
    .contains("DELTA_SPECIAL_OPS_PROFIT_RESULT:"));
~~~

- [ ] **Step 7: 验证 renderer。**

Run:

~~~powershell
bunx vitest run src/components/app/special-ops-moligod-scraper.test.ts
cargo test --manifest-path src-tauri/Cargo.toml special_ops::profit::moligod::tests
~~~

Expected: PASS。

### Task 3: 接入共享 renderer、scheduler 和 round

**Files:**
- Modify: `src-tauri/src/special_ops/mod.rs`
- Modify: `src-tauri/src/special_ops/profit/runtime.rs`
- Test: `src-tauri/src/special_ops/mod.rs`
- Test: `src-tauri/src/special_ops/round_scheduler.rs`

- [ ] **Step 1: 让 `SpecialOpsState` 持有唯一 adapter。**

~~~rust
pub struct SpecialOpsState {
    settings: Arc<Mutex<SpecialOpsSettings>>,
    login_runtime: Arc<login_runtime::LoginRuntime>,
    profit_runtime: Arc<ProfitQueryControl>,
    moligod_adapter: Arc<MoligodAdapter>,
    round_control: Arc<RoundControl>,
    round_scheduler: Arc<round_scheduler::RoundScheduler>,
}

// initialize()
moligod_adapter: Arc::new(MoligodAdapter::new(app.clone())),
~~~

`execute_profit_query_action()` 与 `special_ops_validate_moligod_binding()` 都必须使用 `state.moligod_adapter`，不再各自 `MoligodAdapter::new()`。`query.rs` 的 fallback 裁决与 `MoligodProfitSource` trait 保持不变。

- [ ] **Step 2: 在应用生命周期和配置关闭时释放 renderer。**

`special_ops_save_profit_settings()` 在成功保存且新配置关闭利润筛选时调用 `state.moligod_adapter.shutdown()`。`shutdown()` 与 `stop_registered()` 在 `profit_runtime.invalidate()` 后调用同一方法。暂停、休眠保护与 revision 变化先使 lease cancellation 为真；配置仍启用时允许保留空闲 renderer，来源失败、筛选关闭与应用退出才销毁。

- [ ] **Step 3: 改 round worker 的结束交接。**

`run_round_worker()` 处理 `end_active_round(reason)` 的错误并记录日志，但禁止再在这里 `invalidate()` 或 `begin_group(now_ms())`：

~~~rust
if let Some(state) = app.try_state::<SpecialOpsState>() {
    if let Err(error) = state.profit_runtime.end_active_round(reason) {
        crate::log_error!("special_ops::profit", "恢复利润查询 cadence 失败", "error" => error);
    }
}
scheduler.wake();
~~~

`consume_for_round()` 冻结的资格仍立即清空，active round 期间 scheduler 不启动新查询；round 结束后 scheduler 使用保留下来的时间，过期则只执行一次 catch-up query。

- [ ] **Step 4: 让审计和 runtime 使用同一个 cadence 计算。**

删除 `mod.rs` 的重复 5/50 分钟常量决策，给 `QueryLease` 增加：

~~~rust
pub(crate) fn next_query_at_ms(&self, completed_at_ms: i64) -> i64 {
    completed_at_ms.saturating_add(if self.attempt >= 3 {
        FIFTY_MINUTES_MS
    } else {
        FIVE_MINUTES_MS
    })
}
~~~

`execute_profit_query_action()` 填写所有 `audit.next_query_at_ms` 与 `complete_query_at_revision()` 前均使用该方法，防止 UI 与 runtime cadence 漂移。

- [ ] **Step 5: 写并运行交接测试。**

~~~rust
#[test]
fn completed_round_keeps_due_query_for_single_catch_up() {
    // 查询完成后 nextQueryAtMs = 302_000；round 在 400_000 结束。
    // poll 返回 QueryProfit，不把时间重置到 400_000。
}

#[test]
fn active_round_never_starts_profit_query() {
    // active_run 为 true 时 poll 不产生 QueryProfit。
}
~~~

Run:

~~~powershell
cargo test --manifest-path src-tauri/Cargo.toml special_ops::profit::query::tests
cargo test --manifest-path src-tauri/Cargo.toml special_ops::round_scheduler::tests
cargo test --manifest-path src-tauri/Cargo.toml special_ops::tests
~~~

Expected: PASS；保持“KKRB 正常缺目标绝不调用 Moligod”“KKRB 整体失败才 fallback”和同一时刻 QueryProfit 优先于 LaunchRound。

### Task 4: 折叠 UI 与同步文档

**Files:**
- Modify: `src/components/app/special-ops-profit-filter.tsx`
- Modify: `src/components/app/special-ops-profit-filter.test.tsx`
- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `droid-wiki/features/special-ops.md`
- Test: `src/lib/security-config.test.ts`

- [ ] **Step 1: 写 UI 失败测试。**

~~~ts
it("默认折叠两张配置表，但外层运行状态和保存入口始终可见", () => {
    expect(profitFilterSource).toContain(
        '<summary className="collapse-title">利润规则（{draft.rules.length}）</summary>',
    );
    expect(profitFilterSource).toContain("业务目标（{boundBindingCount}/{draft.bindings.length}）");
    const firstDetails = profitFilterSource.indexOf('<details className="collapse collapse-arrow">');
    expect(profitFilterSource.indexOf("runtimePhaseLabels[bootstrap.profitRuntime.phase]"))
        .toBeLessThan(firstDetails);
    expect(profitFilterSource.indexOf("保存利润配置")).toBeLessThan(firstDetails);
});
~~~

- [ ] **Step 2: 改 `SpecialOpsProfitFilter` 的层级。**

计算 `boundBindingCount`，将规则表、KKRB 刷新、Moligod 验证、datalist、添加/删除规则移进第一个默认关闭的 `details`；将绑定表移进第二个默认关闭的 `details`。外层保留启用开关、截止时间、当前 phase、下次查询、摘要、错误、冲突、验证反馈和保存按钮。

~~~tsx
const boundBindingCount = useMemo(
    () => draft.bindings.filter((binding) => binding.profitRuleId !== null).length,
    [draft.bindings],
);
~~~

第一个 `details` 的 summary 固定为 `利润规则（{draft.rules.length}）`，内容从当前同级 JSX 原样移动：规则表、`special-ops-kkrb-catalog` datalist、KKRB 名称摘要和“添加利润规则”按钮。第二个 summary 固定为 `业务目标（{boundBindingCount}/{draft.bindings.length}）`，内容只包含当前业务目标绑定表。两个 `details` 都不带 `open` 属性。

折叠状态不得进入 `ProfitConfigurationUpdate`、`SpecialOpsSettings` 或 fingerprint。

- [ ] **Step 3: 更新 capability 断言与三份文档。**

`src/lib/security-config.test.ts` 必须断言：

~~~ts
expect(capability.webviews).toContain("special-ops-profit-*");
expect(capability.permissions).toEqual([]);
expect(capability.remote?.urls).toEqual(["https://moligod.com/*"]);
~~~

文档必须明确：隐藏 renderer 是主窗口 child WebView，不创建顶级窗口；成功复用但每次新 nonce 导航和强制行情刷新；失败/关闭/退出销毁；round 后保留原 `nextQueryAtMs`；两张表默认折叠且外层状态可见。

- [ ] **Step 4: 验证前端和文档。**

Run:

~~~powershell
bunx vitest run src/components/app/special-ops-profit-filter.test.tsx src/components/app/special-ops-profit-utils.test.ts src/lib/security-config.test.ts
bun run build
git diff --check
~~~

Expected: PASS，无 whitespace 错误。

### Task 5: 完整门禁与实机验收

**Files:**
- Verify only: `src-tauri/src/special_ops/profit/runtime.rs`
- Verify only: `src-tauri/src/special_ops/profit/moligod.rs`
- Verify only: `src-tauri/src/special_ops/mod.rs`
- Verify only: `src/components/app/special-ops-profit-filter.tsx`

- [ ] **Step 1: 执行完整质量门禁。**

Run:

~~~powershell
bun run check
git diff --check
codegraph sync
~~~

Expected: 全部 PASS；CodeGraph 无待同步索引。

- [ ] **Step 2: Windows 实机验收。**

1. 令 KKRB 返回整体失败并配置至少一个 Moligod 精确名称。
2. 点击继续，确认没有新顶级窗口、任务栏窗口、焦点切换、键鼠倒计时或 toast。
3. 在主窗口正常操作时完成查询，确认利润审计更新。
4. 再次查询，确认 renderer 被复用、页面用新 nonce 重新导航，旧 DOM 结果不被消费。
5. 在第二次查询时间前启动 qualified round，round 结束后确认 `下次查询` 仍是原时间；若已过期，只执行一次 catch-up query。
6. 关闭筛选、暂停或退出应用，确认迟到结果不写入审计；重新继续时建立一个新的即时查询组。

- [ ] **Step 3: 记录验收，不提交。**

报告每条门禁、实际 WebView2 行为、任何未通过的实机条件。保留当前工作区全部未提交改动。
