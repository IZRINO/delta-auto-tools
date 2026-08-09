# 特勤处限时商品检查与交易行购买 Implementation Plan

> 后续变更：区域平均色吸取方案已由 `2026-08-09-special-ops-native-color-picker.md` 替代。现行实现使用原生颜色面板/Hex 输入；旧 `colorSampleRegions` 字段与 `special_ops_sample_limited_supply_color` command 不再保留。

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. User requires Inline Execution; do not dispatch subagents. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在现有特勤处统一 scheduler 中增加每日限时商品识色检查和 02:00-04:00 单目标交易行限价购买，并提供完整校准、试运行、时间轴和恢复能力。

**Architecture:** 新建纯规则模块 `limited_supply.rs`、`market_purchase.rs` 与对应 runtime，避免继续把业务状态机写入 500KB 的 `special_ops/mod.rs`。现有 planner 冻结四类时间任务，runner 保持唯一键鼠 worker，并允许交易行在一次原子流程结束后让出到期制作任务。限时检查配置全局共享；交易行账号独立配置只覆盖商品入口点和设定价格。

**Tech Stack:** Rust、Tokio、Tauri 2、serde、Windows OCR/截图、React 19、TypeScript、daisyUI、Vitest、Bun。

**Execution constraint:** 当前工作区含既有未提交改动。本计划不创建 worktree、不启用子代理、不执行 Git commit；每个任务结束只运行定向测试和 `git diff --check`。

---

## 文件结构

### 新建

- `src-tauri/src/special_ops/limited_supply.rs`：刷新周期、账号结果、九区域聚合与纯规则测试。
- `src-tauri/src/special_ops/limited_supply_runtime.rs`：研发部门导航、页面就绪、双采样与持久化编排。
- `src-tauri/src/special_ops/market_purchase.rs`：价格解析、窗口状态、账号进度与纯规则测试。
- `src-tauri/src/special_ops/market_runtime.rs`：单次商品原子流程、OCR 重试、购买循环与让出结果。
- `src/components/app/special-ops-limited-supply-settings.tsx`：全局限时商品设置、颜色和九区域测试 UI。
- `src/components/app/special-ops-market-settings.tsx`：全局交易行设置、设定价格和试运行 UI。
- `src/components/app/special-ops-limited-market.test.ts`：新前端类型、时间轴和配置归属测试。

### 修改

- `src-tauri/src/special_ops/mod.rs`：serde 模型、normalize、校准目标、冻结配置、生产 driver、commands、时间轴投影。
- `src-tauri/src/special_ops/round_planner.rs`：四类任务冻结与业务优先级。
- `src-tauri/src/special_ops/round_account.rs`：制作、子弹、限时、交易行执行顺序。
- `src-tauri/src/special_ops/round_runner.rs`：局部业务失败、队尾补偿、交易行让出制作与动态重排。
- `src-tauri/src/special_ops/round_scheduler.rs`：新任务唤醒时间测试。
- `src-tauri/src/special_ops/game_navigation.rs`：新任务目的地映射测试，复用现有 Lobby/StationGrid 导航。
- `src-tauri/src/special_ops/windows_ocr.rs`：保留原始 OCR token，供交易行删除非 ASCII 数字后解析价格；原账号扫描接口行为不变。
- `src-tauri/src/recognition/mod.rs`：向 crate 内重导出识色类型。
- `src-tauri/src/recognition/watcher/mod.rs`：向 crate 内重导出 `match_color_probes`。
- `src-tauri/src/lib.rs`：注册新 Tauri commands，不启用局部 app ACL。
- `src-tauri/src/profile/types.rs`、`src-tauri/src/profile/apply.rs`、`src-tauri/src/profile/mod.rs`：Profile round-trip 与旧快照兼容测试。
- `src/components/app/special-ops-types.ts`：camelCase 对应类型与 command 包装。
- `src/components/app/special-ops-utils.ts`：新任务标签、结果与时间轴辅助函数。
- `src/components/app/special-ops-page.tsx`：组合新设置组件、账号覆盖、试运行和提醒确认。
- `src/components/app/special-ops-operation-overlay.tsx`：新 run kind 的中文运行提示。
- `src/components/app/special-ops-page.test.tsx`、`src/components/app/special-ops-utils.test.ts`、`src/components/app/special-ops-operation-overlay.test.ts`：UI 回归测试。
- `README.md`、`AGENTS.md`、`droid-wiki/features/special-ops.md`、`droid-wiki/systems/profile-system.md`：行为、commands、持久化与备份文档。

---

### Task 1：建立纯业务模型与时间规则

**Files:**
- Create: `src-tauri/src/special_ops/limited_supply.rs`
- Create: `src-tauri/src/special_ops/market_purchase.rs`
- Modify: `src-tauri/src/special_ops/mod.rs:1-22`

- [ ] **Step 1：先写限时周期失败测试**

```rust
#[test]
fn noon_cycle_expires_at_twenty_hundred() {
    let cycle = LimitedSupplyCycle::for_day_and_minute("2026-08-08", 12 * 60).unwrap();
    assert_eq!(cycle.id, "2026-08-08T12:00");
    assert!(cycle.contains_minute(19 * 60 + 59));
    assert!(!cycle.contains_minute(20 * 60));
}

#[test]
fn evening_cycle_survives_midnight_until_noon() {
    let cycle = LimitedSupplyCycle::for_day_and_minute("2026-08-08", 23 * 60).unwrap();
    assert_eq!(cycle.id, "2026-08-08T20:00");
}
```

- [ ] **Step 2：运行测试并确认因类型不存在而失败**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml limited_supply::tests -- --nocapture
```

Expected: FAIL，`LimitedSupplyCycle` 或模块尚未定义。

- [ ] **Step 3：实现最小限时模型**

```rust
pub(crate) const LIMITED_SUPPLY_TIMES: [u16; 2] = [12 * 60, 20 * 60];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LimitedSupplyAccountState {
    pub cycle_id: Option<String>,
    pub outcome: LimitedSupplyOutcome,
    pub checked_at_ms: Option<i64>,
    pub matched_region: Option<u8>,
    pub matched_color: Option<[u8; 3]>,
    pub acknowledged: bool,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum LimitedSupplyOutcome {
    #[default]
    Pending,
    NoHighValue,
    HighValue,
    Failed,
}
```

实现 `LimitedSupplyCycle::for_day_and_minute`、周期 ID、当前周期失效判断，不读取系统时间。

- [ ] **Step 4：先写交易行解析和边界失败测试**

```rust
#[test]
fn price_parser_keeps_only_ascii_digits() {
    assert_eq!(parse_market_price("价格 12,345 库存6"), Some(123_456));
    assert_eq!(parse_market_price("无价格"), None);
    assert_eq!(parse_market_price("0"), None);
}

#[test]
fn equal_price_is_buyable() {
    assert_eq!(price_decision(10_000, 10_000), PriceDecision::Buy);
    assert_eq!(price_decision(10_001, 10_000), PriceDecision::Return);
}
```

- [ ] **Step 5：实现最小交易行模型并验证**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MarketBusinessConfig {
    pub product_point: Option<CalibrationRect>,
    pub max_price: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct MarketAccountState {
    pub day: Option<String>,
    pub completed_count: u32,
    pub status: MarketTaskStatus,
    pub last_error: Option<String>,
}

pub(crate) fn parse_market_price(text: &str) -> Option<u64> {
    let digits = text.chars().filter(char::is_ascii_digit).collect::<String>();
    digits.parse::<u64>().ok().filter(|value| *value > 0)
}
```

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml limited_supply::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml market_purchase::tests -- --nocapture
git diff --check
```

Expected: 新增纯规则测试全部 PASS。

---

### Task 2：接入 serde 设置、normalize、Profile 与校准目标

**Files:**
- Modify: `src-tauri/src/special_ops/mod.rs:310-574`
- Modify: `src-tauri/src/special_ops/mod.rs:687-820`
- Modify: `src-tauri/src/special_ops/mod.rs:2231-2660`
- Modify: `src-tauri/src/profile/types.rs`
- Modify: `src-tauri/src/profile/apply.rs`
- Modify: `src-tauri/src/profile/mod.rs`

- [ ] **Step 1：写旧配置反序列化失败测试**

```rust
#[test]
fn legacy_settings_gain_disabled_new_features_without_losing_accounts() {
    let value = serde_json::json!({
        "enabled": true,
        "paused": true,
        "dailyExchangeTime": "08:00",
        "emergencyHotkey": "Ctrl+Shift+F12",
        "defaultBusinessConfig": BusinessConfig::default(),
        "accounts": []
    });
    let normalized = normalize_settings(serde_json::from_value(value).unwrap()).unwrap();
    assert!(!normalized.limited_supply.enabled);
    assert!(!normalized.market_purchase.enabled);
}
```

- [ ] **Step 2：运行并确认缺字段或类型导致失败**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml legacy_settings_gain_disabled_new_features_without_losing_accounts -- --exact --nocapture
```

Expected: FAIL，新字段尚不存在。

- [ ] **Step 3：扩展设置结构**

```rust
pub struct BusinessConfig {
    pub stations: Vec<StationBusinessConfig>,
    #[serde(default)]
    pub recipe_points: Vec<AccountRecipePoint>,
    pub ammo_targets: Vec<AmmoBusinessTarget>,
    #[serde(default)]
    pub market: MarketBusinessConfig,
}

pub struct AccountPlan {
    // existing fields
    #[serde(default)]
    pub limited_supply: LimitedSupplyAccountState,
    #[serde(default)]
    pub market: MarketAccountState,
}

pub struct SpecialOpsSettings {
    // existing fields
    #[serde(default)]
    pub limited_supply: LimitedSupplySettings,
    #[serde(default)]
    pub market_purchase: MarketPurchaseSettings,
}
```

默认关闭两个新功能；页面就绪默认 10 秒且 normalize 限制在 5-60 秒；交易行固定等待默认 3000ms 且限制在 0-60000ms；研发部门等待默认复用现有军需处后续点击等待；价格和购买次数必须为正数。旧配置的安全默认值为价格 1、购买 1 次，避免用户首次启用后产生大批量操作。

- [ ] **Step 4：新增全局校准目标与账号业务点路由测试**

```rust
#[test]
fn default_calibration_contains_limited_and_market_targets() {
    let keys = default_calibration_targets().into_iter().map(|item| item.key).collect::<HashSet<_>>();
    for key in [
        "limited.research", "limited.ready", "limited.color.1", "limited.color.9",
        "market.entry", "market.product", "market.price", "market.return", "market.buy", "market.confirm",
    ] {
        assert!(keys.contains(key), "缺少 {key}");
    }
}

#[test]
fn account_market_selection_only_updates_product_point() {
    // 提交 business.market.product + accountId 后，只写 independentBusinessConfig.market.productPoint。
}
```

新增目标：

```rust
("limited.research", "研发部门点击点", ClickPoint)
("limited.ready", "研发部门页面就绪识别区域", RecognitionRegion)
("limited.color.1"..="limited.color.9", "限时商品识色区域", InputRegion)
("market.entry", "交易行入口点击点", ClickPoint)
("market.product", "默认商品入口点击点", ClickPoint)
("market.price", "价格 OCR 区域", RecognitionRegion/Ocr)
("market.return", "高价返回点击点", ClickPoint)
("market.buy", "达标购买点击点", ClickPoint)
("market.confirm", "最终确认购买点击点", ClickPoint)
```

- [ ] **Step 5：验证 normalize、Profile round-trip 与校准测试**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml special_ops::tests::legacy_settings_gain_disabled_new_features_without_losing_accounts -- --exact
cargo test --manifest-path src-tauri/Cargo.toml profile:: -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml default_calibration_contains_limited_and_market_targets -- --exact --nocapture
cargo test --manifest-path src-tauri/Cargo.toml account_market_selection_only_updates_product_point -- --exact --nocapture
git diff --check
```

Expected: 旧配置、Profile 与校准路由测试 PASS；参考图仍只保存路径。

---

### Task 3：投影 24 小时时间轴并冻结四类任务

**Files:**
- Modify: `src-tauri/src/special_ops/mod.rs:860-910`
- Modify: `src-tauri/src/special_ops/mod.rs:8640-9030`
- Modify: `src-tauri/src/special_ops/round_planner.rs`
- Modify: `src-tauri/src/special_ops/round_scheduler.rs`

- [ ] **Step 1：写限时与交易行投影失败测试**

```rust
#[test]
fn schedule_projects_current_limited_cycle_and_market_window() {
    let settings = settings_with_new_features_enabled();
    let snapshot = build_schedule(&settings, shanghai_ms("2026-08-08 02:30"));
    assert!(snapshot.timeline_tasks.iter().any(|task| task.kind == TimelineTaskKind::MarketPurchase));
    assert!(snapshot.timeline_tasks.iter().any(|task| task.kind == TimelineTaskKind::LimitedSupplyCheck));
}

#[test]
fn old_limited_cycle_and_market_after_four_are_not_due() {
    // 20:00 后不保留 12:00 待检查；04:00 后不保留当天交易行待执行。
}
```

- [ ] **Step 2：运行并确认新 kind 尚不存在**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml schedule_projects_current_limited_cycle_and_market_window -- --exact --nocapture
cargo test --manifest-path src-tauri/Cargo.toml old_limited_cycle_and_market_after_four_are_not_due -- --exact --nocapture
```

Expected: FAIL，`TimelineTaskKind` 缺少新 variant。

- [ ] **Step 3：扩展时间轴类型**

```rust
pub enum TimelineTaskKind {
    Craft,
    Ammo,
    LimitedSupplyCheck,
    MarketPurchase,
}

pub struct TimelineTask {
    // existing fields
    #[serde(default)]
    pub limited_cycle_id: Option<String>,
    #[serde(default)]
    pub market_completed_count: Option<u32>,
    #[serde(default)]
    pub market_target_count: Option<u32>,
    #[serde(default)]
    pub business_state: Option<TimelineBusinessState>,
}
```

`DueAccount` 增加 `limited_supply_due: bool`、`market_purchase_due: bool`。非 `Ready` 账号仍投影，只有 `Ready` 账号进入 due/frozen plan。

- [ ] **Step 4：扩展 `AccountRoundTask` 并验证排序**

```rust
pub(crate) struct AccountRoundTask {
    // existing fields
    pub limited_supply_cycle_id: Option<String>,
    pub market_purchase_day: Option<String>,
}
```

planner 按 `(scheduledAt, accountOrder, businessOrder, id)` 冻结；同一时间桶内 `Craft=0`、`Ammo=100`、`Limited=200`、`Market=300`。未来制作 lookahead 保持原行为。

新增测试：

```rust
#[test]
fn same_account_same_time_orders_craft_ammo_limited_market() { /* 断言四类顺序 */ }

#[test]
fn non_ready_new_tasks_project_but_do_not_freeze() { /* timeline 有，round plan 无 */ }
```

- [ ] **Step 5：验证 planner 与 scheduler**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml round_planner::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml round_scheduler::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml special_ops::tests::schedule_ -- --nocapture
git diff --check
```

Expected: 时间轴、due、next wake 和 planner 测试 PASS。

---

### Task 4：复用识色算法并实现限时商品 runtime

**Files:**
- Modify: `src-tauri/src/recognition/mod.rs`
- Modify: `src-tauri/src/recognition/watcher/mod.rs`
- Create: `src-tauri/src/special_ops/limited_supply_runtime.rs`
- Modify: `src-tauri/src/special_ops/mod.rs`

- [ ] **Step 1：最小化公开现有识色接口**

```rust
// recognition/mod.rs
pub(crate) use self::types::{ColorMatchMethod, ColorMatchMode, ColorProbe, ColorTarget};

// recognition/watcher/mod.rs
pub(crate) use matching::match_color_probes;
```

不复制 RGB 距离或 AnyPixel 算法，不新增依赖。

- [ ] **Step 2：写限时状态机失败测试**

```rust
#[tokio::test]
async fn high_value_requires_same_region_in_two_valid_samples() {
    let driver = FakeLimitedDriver::samples(vec![sample_hit(2), sample_hit(2)]);
    let result = run_limited_supply(&driver, config(), cancel()).await;
    assert_eq!(result, LimitedRunStop::Completed(LimitedSupplyOutcome::HighValue));
    assert_eq!(driver.persisted_region(), Some(2));
}

#[tokio::test]
async fn inconsistent_samples_resample_without_persisting() { /* hit/miss -> hit/hit */ }

#[tokio::test]
async fn first_ready_timeout_requests_one_deferred_retry() { /* RetryableTimeout */ }
```

- [ ] **Step 3：运行并确认 runtime 不存在**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml limited_supply_runtime::tests -- --nocapture
```

Expected: FAIL，模块或 `run_limited_supply` 尚未定义。

- [ ] **Step 4：实现 driver 与状态机**

```rust
#[allow(async_fn_in_trait)]
pub(crate) trait LimitedSupplyDriver: Send + Sync {
    async fn wait_and_click(&self, key: &str, cancelled: Arc<AtomicBool>) -> Result<(), LimitedRunError>;
    async fn delay(&self, duration: Duration, cancelled: Arc<AtomicBool>) -> Result<(), LimitedRunError>;
    async fn wait_ready(&self, timeout: Duration, cancelled: Arc<AtomicBool>) -> Result<(), LimitedRunError>;
    async fn sample_colors(&self, cancelled: Arc<AtomicBool>) -> Result<LimitedColorSample, LimitedRunError>;
    fn persist_result(&self, result: &LimitedSupplyCheckResult) -> Result<(), LimitedRunError>;
}
```

状态机固定执行 `ammo.department -> ammo.supply -> limited.research -> limited.ready -> 双采样`。截图失败不生成 miss；不一致样本在超时内重采样。首次 ready 超时返回可队尾重试结果，第二次由 runner 转为当前周期失败。

- [ ] **Step 5：实现生产截图适配并验证**

生产 driver 使用现有 `template_observer`、`recognition::watcher::capture_region`、`match_color_probes`、全局输入锁和 mouse parking。每轮颜色样本先捕获九个区域，再统一聚合；不弹通知。

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml limited_supply_runtime::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml recognition::watcher::tests -- --nocapture
cargo check --manifest-path src-tauri/Cargo.toml
git diff --check
```

Expected: runtime、复用算法和编译检查 PASS。

---

### Task 5：实现交易行单次原子流程与购买循环

**Files:**
- Create: `src-tauri/src/special_ops/market_runtime.rs`
- Modify: `src-tauri/src/special_ops/mod.rs`
- Modify: `src-tauri/src/special_ops/windows_ocr.rs`

- [ ] **Step 1：写价格分支与重试失败测试**

```rust
#[tokio::test]
async fn high_price_returns_without_incrementing() {
    let driver = FakeMarketDriver::ocr(["12,001"]);
    let result = run_market_atomic(&driver, config_with_price(12_000), cancel()).await;
    assert_eq!(result, MarketAtomicResult::Returned);
    assert_eq!(driver.actions(), ["click:market.product", "click:market.return"]);
}

#[tokio::test]
async fn equal_price_buys_and_persists_after_final_click() {
    let driver = FakeMarketDriver::ocr(["12,000"]);
    let result = run_market_atomic(&driver, config_with_price(12_000), cancel()).await;
    assert_eq!(result, MarketAtomicResult::Purchased);
    assert_eq!(driver.actions(), ["click:market.product", "click:market.buy", "click:market.confirm", "persist:1"]);
}

#[tokio::test]
async fn three_failed_pages_end_current_account() { /* 3 reads x 3 pages，最后 LocalFailure */ }

#[test]
fn comma_separated_ocr_token_reaches_market_parser() {
    // Windows OCR 返回 "12,345" 时不得被 numeric_words 提前丢弃。
    assert_eq!(parse_market_price("12,345"), Some(12_345));
}
```

- [ ] **Step 2：运行并确认失败**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml market_runtime::tests -- --nocapture
```

Expected: FAIL，交易行 runtime 尚未定义。

- [ ] **Step 3：实现原子流程接口**

```rust
#[allow(async_fn_in_trait)]
pub(crate) trait MarketDriver: Send + Sync {
    async fn click(&self, key: &str, countdown: bool, cancelled: Arc<AtomicBool>) -> Result<(), MarketRunError>;
    async fn click_point(&self, point: &RegionRect, countdown: bool, cancelled: Arc<AtomicBool>) -> Result<(), MarketRunError>;
    async fn read_price(&self, cancelled: Arc<AtomicBool>) -> Result<String, MarketRunError>;
    async fn delay(&self, duration: Duration, cancelled: Arc<AtomicBool>) -> Result<(), MarketRunError>;
    fn persist_purchase_click(&self) -> Result<u32, MarketRunError>;
}
```

抽出 `recognize_words` 返回原始 Windows OCR token；既有 `recognize_numeric_words` 继续包装并过滤纯数字，保证已记住账号扫描行为不变。交易行把原始 token 文本按阅读顺序拼接，再由 `parse_market_price` 删除全部非 ASCII 数字。单页 OCR 读取三次、间隔 500ms；三次失败点击 A 返回。连续三页失败后返回 `MarketRunStop::PriceRecognitionFailed`。最终购买点击先完成输入，再持久化计数。

- [ ] **Step 4：实现循环退出条件**

```rust
pub(crate) enum MarketRunStop {
    Completed,
    YieldedForCraft,
    PauseRequested,
    WindowClosed,
    PriceRecognitionFailed,
    EmergencyStopped,
    SystemFailure { step: String, message: String },
}
```

每个原子流程完成后按顺序检查：紧急停止、暂停请求、04:00、购买次数、`next_craft_at_ms <= now_ms`。仅 `Purchased` 增加次数；`Returned` 继续循环。

- [ ] **Step 5：验证 runtime**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml market_runtime::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml market_purchase::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml windows_ocr::tests -- --nocapture
cargo check --manifest-path src-tauri/Cargo.toml
git diff --check
```

Expected: 高价、等价、低价、OCR 九次失败、暂停、04:00 和让出测试 PASS。

---

### Task 6：接入 round，支持局部失败、队尾补偿和交易行让出制作

**Files:**
- Modify: `src-tauri/src/special_ops/round_account.rs`
- Modify: `src-tauri/src/special_ops/round_runner.rs`
- Modify: `src-tauri/src/special_ops/round_planner.rs`
- Modify: `src-tauri/src/special_ops/mod.rs:4914-6358`

- [ ] **Step 1：写 `round_account` 顺序失败测试**

```rust
#[tokio::test]
async fn account_runs_craft_ammo_limited_then_market() {
    let driver = FakeDriver::success();
    run_task_in_session(&driver, &all_business_task(), cancel()).await.unwrap();
    assert_eq!(driver.actions(), ["craft", "ammo", "limited", "market"]);
}
```

- [ ] **Step 2：扩展 session driver 与成功结果**

```rust
pub(crate) trait AccountSessionDriver {
    // existing methods
    async fn limited_supply(&self, task: &AccountRoundTask, cancelled: Arc<AtomicBool>) -> Result<LimitedSessionResult, AccountRunError>;
    async fn market(&self, task: &AccountRoundTask, next_craft_at_ms: Option<i64>, cancelled: Arc<AtomicBool>) -> Result<MarketSessionResult, AccountRunError>;
}

pub(crate) struct AccountRunSuccess {
    pub processed_stations: usize,
    pub market_pending: bool,
    pub market_yielded: bool,
}
```

- [ ] **Step 3：写 runner 动态重排失败测试**

```rust
#[tokio::test]
async fn yielded_market_runs_due_craft_before_resuming_market() {
    // market@02:00 -> craft@02:10 -> resumed market
    assert_eq!(actions, ["market:yield", "close", "craft", "close", "market:resume"]);
}

#[tokio::test]
async fn limited_timeout_moves_only_limited_task_to_tail_once() { /* 第二次局部失败，不删账号其他任务 */ }

#[tokio::test]
async fn four_oclock_discards_all_remaining_market_tasks() { /* 当前原子完成后清队列 */ }
```

- [ ] **Step 4：把 runner 队列改为持有任务 clone**

将 `VecDeque<(usize, &AccountRoundTask, u8)>` 改为拥有 `QueuedRoundTask { original_index, task, navigation_retries, business_retries }`。市场返回 `market_pending=true` 时：

1. 若因制作到期让出，把市场任务重新插入所有已到期制作任务之后。
2. 若当前未到制作时间，继续同账号市场循环，不等待未来制作。
3. 04:00 后删除所有 `market_purchase_day.is_some()` 任务。
4. 限时 ready 首次超时只把当前限时任务移到队尾；第二次持久化局部失败，不删除账号其他任务。

交易行 runtime 每个原子流程边界使用冻结时间轴中全局最早的未来制作时间，不只查看当前账号。因制作到期让出时必须关闭当前游戏会话并重新登录/导航到确定页面；不得在交易行页面直接调用制作 runtime。制作完成且仍早于 04:00 时，再按已保存次数恢复交易行。

- [ ] **Step 5：实现 ProductionRoundDriver 冻结与持久化**

`FrozenRoundAccount` 按 `(accountId, scheduledAt)` 保存：

```rust
pub limited_supply: Option<FrozenLimitedSupplyRun>,
pub market: Option<FrozenMarketRun>,
```

局部业务结果使用 revision 临界区逐项持久化；不得写 `AccountFailure` 或修改 `AccountStatus`。公共登录/导航失败继续走现有账号规则，系统错误继续全面暂停。

- [ ] **Step 6：验证 round 回归**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml round_account::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml round_runner::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml round_planner::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml special_ops::tests::round_ -- --nocapture
git diff --check
```

Expected: 现有会话保持测试和新业务重排测试全部 PASS。

---

### Task 7：新增试运行、提醒确认 commands 与运行窗口状态

**Files:**
- Modify: `src-tauri/src/special_ops/mod.rs:7226-7700`
- Modify: `src-tauri/src/special_ops/login_runtime.rs`
- Modify: `src-tauri/src/lib.rs:280-300`
- Modify: `src/components/app/special-ops-operation-overlay.tsx`
- Modify: `src/components/app/special-ops-operation-overlay.test.ts`

- [ ] **Step 1：写 command contract 失败测试**

```rust
#[test]
fn new_commands_are_registered() {
    let source = include_str!("../src/lib.rs");
    for command in [
        "special_ops_start_limited_supply_trial",
        "special_ops_start_market_trial",
        "special_ops_acknowledge_limited_supply",
        "special_ops_test_limited_supply_colors",
    ] {
        assert!(source.contains(command));
    }
}
```

- [ ] **Step 2：增加 run kind 与 command 参数**

```rust
pub enum LoginRunKind {
    Login, Navigation, Craft, Ammo, LimitedSupply, Market, Round,
}

pub enum MarketTrialMode {
    InspectOnly,
    RealSingleAttempt,
}
```

Commands：

```rust
special_ops_start_limited_supply_trial(account_id, settings_revision)
special_ops_start_market_trial(account_id, mode, settings_revision)
special_ops_acknowledge_limited_supply(account_id, cycle_id, settings_revision)
special_ops_test_limited_supply_colors(environment_id, region_index, settings_revision)
```

- [ ] **Step 3：实现无副作用试运行策略**

限时试运行 driver 的 `persist_result` 只返回 UI 结果，不写 settings。交易行 `InspectOnly` 在 OCR 决策后停止且不点击 A/B；`RealSingleAttempt` 执行一条分支，但 `persist_purchase_click` 使用 no-op 计数器，不写正式次数。

- [ ] **Step 4：注册 command，禁止新增局部 ACL**

只修改 `src-tauri/src/lib.rs` 的 `generate_handler![]`。不得创建 `src-tauri/permissions/*.toml`，不得向 capability 添加无 namespace app permission。

- [ ] **Step 5：更新运行窗口并验证**

新增中文文案：

```ts
limitedSupply: "正在检查限时商品"
market: "正在执行交易行购买"
```

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml special_ops_async_command_contract -- --nocapture
bunx vitest run src/components/app/special-ops-operation-overlay.test.ts
cargo check --manifest-path src-tauri/Cargo.toml
git diff --check
```

Expected: commands、run kind、overlay 测试 PASS。

---

### Task 8：扩展前端类型、纯辅助函数与账号配置覆盖

**Files:**
- Modify: `src/components/app/special-ops-types.ts`
- Modify: `src/components/app/special-ops-utils.ts`
- Modify: `src/components/app/special-ops-utils.test.ts`
- Create: `src/components/app/special-ops-limited-market.test.ts`

- [ ] **Step 1：写 TypeScript 失败测试**

```ts
test("账号独立设置只覆盖交易行商品点与价格", () => {
  const result = enableIndependentSettings(account, defaults);
  expect(result.independentBusinessConfig?.market).toEqual(defaults.market);
  expect(Object.keys(result.independentBusinessConfig!.market).sort()).toEqual(["maxPrice", "productPoint"]);
});

test("时间轴识别新任务标签", () => {
  expect(timelineTaskLabel(task("limitedSupplyCheck"))).toBe("限时商品检查");
  expect(timelineTaskLabel(task("marketPurchase"))).toBe("交易行购买");
});
```

- [ ] **Step 2：运行并确认类型/函数缺失**

Run:

```powershell
bunx vitest run src/components/app/special-ops-limited-market.test.ts src/components/app/special-ops-utils.test.ts
```

Expected: FAIL，新类型或 helper 不存在。

- [ ] **Step 3：增加 camelCase 类型**

```ts
export type LimitedSupplyOutcome = "pending" | "noHighValue" | "highValue" | "failed";
export type MarketTaskStatus = "pending" | "running" | "completed" | "priceRecognitionFailed" | "windowClosed";
export type MarketBusinessConfig = {productPoint: CalibrationRect | null; maxPrice: number};
export type TimelineTaskKind = "craft" | "ammo" | "limitedSupplyCheck" | "marketPurchase";
export type LoginRunKind = "login" | "navigation" | "craft" | "ammo" | "limitedSupply" | "market" | "round";
```

同步 Rust 新字段，不使用 `any` 或类型断言绕过。

- [ ] **Step 4：实现 helper 并验证**

实现新任务 label、业务状态文案、市场进度文案和高价值提醒判断。保持 `groupTimelineTasks` 只按 `scheduledAtMs` 视觉分组，不改变调度。

Run:

```powershell
bunx vitest run src/components/app/special-ops-limited-market.test.ts src/components/app/special-ops-utils.test.ts
bun run build
git diff --check
```

Expected: 新前端纯逻辑测试和 TypeScript build PASS。

---

### Task 9：实现限时与交易行设置、校准和价格 UI

**Files:**
- Create: `src/components/app/special-ops-limited-supply-settings.tsx`
- Create: `src/components/app/special-ops-market-settings.tsx`
- Modify: `src/components/app/special-ops-page.tsx:650-710`
- Modify: `src/components/app/special-ops-page.tsx:1125-1325`
- Modify: `src/components/app/special-ops-page.test.tsx`

- [ ] **Step 1：写 UI 失败测试**

```tsx
test("限时商品只显示全局配置，不出现在账号独立设置", () => {
  render(<SpecialOpsPage/>);
  expect(screen.getByText("限时商品检查")).toBeTruthy();
  expect(screen.queryByText("账号限时商品设置")).toBeNull();
});

test("账号独立交易行只显示商品入口和设定价格", () => {
  render(<SpecialOpsPage/>);
  expect(screen.getByLabelText("独立设定价格")).toBeTruthy();
  expect(screen.queryByLabelText("独立购买次数")).toBeNull();
});
```

- [ ] **Step 2：运行并确认 UI 不存在**

Run:

```powershell
bunx vitest run src/components/app/special-ops-page.test.tsx
```

Expected: FAIL，设置组件和字段尚未渲染。

- [ ] **Step 3：实现两个 daisyUI 设置组件**

实施时先调用项目 `daisyui`、`daisyui-usage`、`daisyui-colors` skills。组件使用现有 `Button`、`Switch`、`DraftInput`、`collapse`、`fieldset`，不新增旧战术风格 CSS。

`SpecialOpsLimitedSupplySettings` 显示：总开关、研发等待、就绪超时、两种颜色/容差、九区域状态、单区测试和全部测试。

`SpecialOpsMarketSettings` 显示：总开关、固定窗口、入口等待、全局设定价格、购买次数、备注、全局商品点击点和试运行按钮。

- [ ] **Step 4：接入 calibration overlay**

复用 `beginCalibration`：

```ts
beginCalibration(environment, "limited.color.1")
beginCalibration(environment, "market.product")
beginCalibration(environment, "business.market.product", account.id)
```

账号独立设置只调用 `updateIndependentBusiness(account, {market: {productPoint, maxPrice}})`；不得暴露限时参数、购买次数、OCR 区域、A/B 点。

- [ ] **Step 5：验证 UI**

Run:

```powershell
bunx vitest run src/components/app/special-ops-page.test.tsx src/components/app/special-ops-limited-market.test.ts
bun run build
git diff --check
```

Expected: 设置归属、设定价格、校准按钮和 build PASS。

---

### Task 10：实现时间轴提醒、试运行结果与人工清除

**Files:**
- Modify: `src/components/app/special-ops-page.tsx:269-445`
- Modify: `src/components/app/special-ops-page.tsx:850-945`
- Modify: `src/components/app/special-ops-page.test.tsx`
- Modify: `src/components/app/special-ops-operation-overlay.tsx`

- [ ] **Step 1：写时间轴失败测试**

```tsx
test("高价值提醒不改变账号状态且可人工清除", async () => {
  renderTimeline(highValueTask({accountStatus: "ready"}));
  expect(screen.getByText("发现高价值物品")).toBeTruthy();
  expect(screen.getByText("就绪")).toBeTruthy();
  await user.click(screen.getByRole("button", {name: "已检查"}));
  expect(invoke).toHaveBeenCalledWith("special_ops_acknowledge_limited_supply", expect.anything());
});

test("交易行任务显示购买进度", () => {
  renderTimeline(marketTask({completed: 4, target: 10}));
  expect(screen.getByText("已执行 4/10 次")).toBeTruthy();
});
```

- [ ] **Step 2：实现任务行分支**

限时任务显示周期、检查失败或高价值信息；只有 `highValue && !acknowledged` 显示“已检查”。无高价值不保留待处理任务行。交易行显示 02:00-04:00、进度、价格识别异常；04:00 后后端不再投影。

- [ ] **Step 3：接入四个前端 command**

```ts
invoke("special_ops_start_limited_supply_trial", ...)
invoke("special_ops_start_market_trial", {mode: "inspectOnly", ...})
invoke("special_ops_start_market_trial", {mode: "realSingleAttempt", ...})
invoke("special_ops_acknowledge_limited_supply", ...)
```

试运行结果显示九区命中详情、OCR 原文、解析值、上限与判断；提交失败必须在对应区域显示错误，禁止静默返回。

- [ ] **Step 4：验证时间轴和试运行 UI**

Run:

```powershell
bunx vitest run src/components/app/special-ops-page.test.tsx src/components/app/special-ops-operation-overlay.test.ts
bun run build
git diff --check
```

Expected: 高价值清除、交易行进度、试运行命令和运行提示 PASS。

---

### Task 11：补齐文档、契约测试与完整质量门禁

**Files:**
- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `droid-wiki/features/special-ops.md`
- Modify: `droid-wiki/systems/profile-system.md`
- Modify: `src-tauri/tests/special_ops_async_command_contract.rs`

- [ ] **Step 1：更新使用与架构文档**

文档必须写明：

- 固定 12:00/20:00 周期和 02:00-04:00 窗口。
- 限时商品全局配置；交易行账号只覆盖商品点和价格。
- AnyPixel 双采样、研发 ready 模板、OCR 只保留数字。
- 交易行原子边界、让出制作、04:00 全局结束。
- 试运行不写正式次数。
- 新 Tauri commands、serde 字段、Profile 备份只保存图片路径。

- [ ] **Step 2：运行定向后端测试**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml limited_supply::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml limited_supply_runtime::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml market_purchase::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml market_runtime::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml round_planner::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml round_runner::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml round_account::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml round_scheduler::tests -- --nocapture
```

Expected: 全部 PASS，0 failed。

- [ ] **Step 3：运行定向前端测试**

Run:

```powershell
bunx vitest run src/components/app/special-ops-limited-market.test.ts src/components/app/special-ops-page.test.tsx src/components/app/special-ops-utils.test.ts src/components/app/special-ops-operation-overlay.test.ts
```

Expected: 全部 PASS，0 failed。

- [ ] **Step 4：运行完整门禁**

Run:

```powershell
bun run check
```

Expected: TypeScript、515+ 前端测试、coverage、Rust fmt、Clippy `-D warnings`、884+ Rust 测试和集成测试全部通过；真实桌面 smoke tests保持 ignored。

- [ ] **Step 5：刷新 CodeGraph 并检查工作区**

Run:

```powershell
codegraph sync
git diff --check
git status --short --branch
```

Expected: CodeGraph 可用时同步成功；`git diff --check` 无错误。若 CLI 仍不在 PATH，明确记录未同步，不伪造成功。保持所有改动未提交，等待用户桌面实测。

---

## 实机验收顺序

1. 单独测试 `limited.ready` 与九个识色区域。
2. 运行限时商品试运行，验证动画结束后才识色，正式周期状态不变化。
3. 运行交易行安全试运行，核对 OCR 原文、解析值和 `<=` 判断。
4. 运行真实试买一次，确认不增加正式次数。
5. 在 02:00-04:00 验证正式次数逐次保存与暂停恢复。
6. 构造交易行运行中制作到期，验证当前原子流程结束后切制作，再恢复交易行。
7. 验证 04:00 到点后不启动新原子流程，所有账号交易行任务消失。
8. 验证 12:00/20:00 高价值提醒、人工“已检查”和下周期自动失效。
