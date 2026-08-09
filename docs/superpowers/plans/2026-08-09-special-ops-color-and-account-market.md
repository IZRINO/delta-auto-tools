# 特勤处识色反馈与账号级交易行配置 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 复用 Recognition 识色能力，修复限时商品取色/测试无反馈，并把交易行购买业务配置迁入默认账号配置和账号独立配置，同时保持限时商品全局配置。

**Architecture:** 保留 `LimitedSupplySettings` 作为全局限时商品配置；扩展 `BusinessConfig.market` 保存交易行账号业务配置，scheduler 在冻结任务时解析账号有效配置。颜色取样与识色测试共用 Recognition watcher 的截图、平均 RGB、距离和双采样实现，特勤处 command 只负责权限、持久化和业务结果映射。

**Tech Stack:** Rust、Tauri 2、serde、image、React 19、TypeScript、Vitest、Cargo test、Bun。

---

## 实施状态

- [x] Recognition 颜色采样 helper 与限时商品取色/双采样反馈
- [x] 限时商品颜色配置移入通用设置，9 个校准区域不重复显示吸取按钮
- [x] 交易行业务配置迁入默认/账号独立 `BusinessConfig.market`
- [x] 旧 `marketPurchase` 字段一次性迁移并覆盖独立配置兼容场景
- [x] scheduler、时间轴、试运行入口与人工检查按钮接入
- [x] 前端 518 项、Rust 921 项单测及 4 项 async command contract 通过

## 文件责任地图

- `src-tauri/src/recognition/watcher/matching.rs`：共享颜色采样、距离和命中算法。
- `src-tauri/src/recognition/watcher/mod.rs`：共享 helper re-export 与单元测试入口。
- `src-tauri/src/special_ops/limited_supply_runtime.rs`：限时商品业务双采样结果，不再重复实现颜色基础算法。
- `src-tauri/src/special_ops/limited_supply.rs`：全局限时商品设置与运行态类型。
- `src-tauri/src/special_ops/market_purchase.rs`：交易行业务配置、运行态和价格判定。
- `src-tauri/src/special_ops/mod.rs`：`BusinessConfig`、normalize、freeze、scheduler、Tauri command 和后端测试。
- `src-tauri/src/lib.rs`：Tauri command 注册。
- `src-tauri/tests/special_ops_async_command_contract.rs`：异步 command 注册契约。
- `src/components/app/special-ops-types.ts`：前端 bootstrap 类型。
- `src/components/app/special-ops-page.tsx`：通用设置、默认账号配置、账号独立配置、校准测试反馈。
- `src/components/app/special-ops-page.test.tsx`：前端交互与字段可见性测试。
- `droid-wiki/features/special-ops.md`、`README.md`、`AGENTS.md`：行为和持久化结构同步。

当前计划按用户要求只修改工作区，不创建提交、不切换分支、不创建 worktree。

## Task 1: 抽取 Recognition 共享颜色采样接口

**Files:**
- Modify: `src-tauri/src/recognition/watcher/matching.rs`
- Modify: `src-tauri/src/recognition/watcher/mod.rs`
- Test: `src-tauri/src/recognition/watcher/mod.rs`

- [ ] **Step 1: 写共享 helper 的失败测试**

在 `watcher` 测试模块增加以下行为测试：

```rust
#[test]
fn sample_region_color_uses_average_rgb() {
    let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        2,
        1,
        image::Rgba([10, 20, 30, 255]),
    ));
    assert_eq!(sample_region_color(&image), [10, 20, 30]);
}

#[test]
fn color_match_summary_reports_distance_and_match() {
    let summary = compare_sampled_color([100, 110, 120], [100, 110, 120], 0);
    assert!(summary.matched);
    assert_eq!(summary.distance, 0.0);
}
```

- [ ] **Step 2: 运行测试并确认当前缺失**

运行：

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib recognition::watcher::tests::sample_region_color_uses_average_rgb
```

预期：FAIL，helper 或返回类型尚不存在。

- [ ] **Step 3: 实现最小共享接口**

在 `matching.rs` 增加只依赖 `image::DynamicImage` 的公共 crate 内 helper：

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ColorMatchSummary {
    pub sampled_color: [u8; 3],
    pub target_color: [u8; 3],
    pub distance: f32,
    pub tolerance: u8,
    pub matched: bool,
}

pub(crate) fn sample_region_color(image: &image::DynamicImage) -> [u8; 3] {
    average_region_rgb(image)
}

pub(crate) fn compare_sampled_color(
    sampled_color: [u8; 3],
    target_color: [u8; 3],
    tolerance: u8,
) -> ColorMatchSummary {
    let distance = color_distance(sampled_color, target_color);
    ColorMatchSummary {
        sampled_color,
        target_color,
        distance,
        tolerance,
        matched: distance <= f32::from(tolerance),
    }
}
```

`color_distance` 使用当前 Recognition 实现，不复制另一套公式。通过 `watcher/mod.rs` re-export 供 `special_ops` 使用。

- [ ] **Step 4: 运行共享测试**

运行：

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib recognition::watcher::tests
```

预期：现有 watcher 测试和新增测试全部通过。

## Task 2: 修复限时商品取色与识色测试反馈

**Files:**
- Modify: `src-tauri/src/special_ops/limited_supply_runtime.rs`
- Modify: `src-tauri/src/special_ops/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/tests/special_ops_async_command_contract.rs`
- Test: `src-tauri/src/special_ops/limited_supply_runtime.rs`
- Test: `src-tauri/src/special_ops/mod.rs`

- [ ] **Step 1: 写取色和测试结果的失败测试**

覆盖以下契约：

```rust
#[test]
fn limited_color_test_keeps_unmatched_result_as_successful_probe() {
    let result = LimitedSupplyColorProbeResult {
        sampled_color: [0, 0, 0],
        target_color: [255, 255, 255],
        distance: 441.6,
        tolerance: 30,
        matched: false,
    };
    assert!(!result.matched);
}
```

增加 command 层测试要求：区域未配置、截图失败、游戏上下文缺失必须返回错误；有效但未命中必须返回结果而非 `Err`。

- [ ] **Step 2: 运行失败测试**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib limited_supply -- --nocapture
```

预期：新增类型或测试契约尚未满足。

- [ ] **Step 3: 改造限时商品 runtime**

`limited_supply_runtime.rs` 只保留区域级双采样业务编排，调用 watcher 的 `sample_region_color` 和 `compare_sampled_color`。结果结构至少包含：

```rust
pub(crate) struct LimitedColorProbeResult {
    pub sampled_color: [u8; 3],
    pub target_color: [u8; 3],
    pub distance: f32,
    pub tolerance: u8,
    pub matched: bool,
}
```

`special_ops_test_limited_supply_colors` 返回每个区域两次样本和汇总；未命中是 `matched=false` 的正常结果，不更新周期状态。

- [ ] **Step 4: 修正取色 command 的反馈链路**

`special_ops_sample_limited_supply_color`：

1. 校验 `regionIndex` 为 1–9、`colorIndex` 为 0 或 1。
2. 校验当前校准区域已配置。
3. 聚焦游戏、移动鼠标到独占停放点、截图。
4. 调用共享 `sample_region_color`。
5. 只在持久化成功后返回 RGB；任一步失败返回明确错误并保持旧颜色。

command 注册继续位于 `src-tauri/src/lib.rs` 的 `generate_handler![]`，并在 async command contract 中保留注册断言。

- [ ] **Step 5: 运行后端测试**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib limited_supply -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml --lib special_ops::tests::default_calibration_contains_limited_and_market_targets -- --exact
cargo test --manifest-path src-tauri/Cargo.toml --test special_ops_async_command_contract
```

预期：全部通过。

## Task 3: 将颜色操作移到通用设置并恢复可见反馈

**Files:**
- Modify: `src/components/app/special-ops-page.tsx`
- Modify: `src/components/app/special-ops-page.test.tsx`
- Modify: `src/components/app/special-ops-types.ts`

- [ ] **Step 1: 写前端失败测试**

增加行为断言：

```tsx
it("颜色吸取成功后显示 RGB，识色未命中显示测试结果", async () => {
  // mock invoke 返回 sampledColor 与 matched=false
  // 点击通用设置中的吸取按钮和校准行测试按钮
  // 断言页面出现 R,G,B、未命中、距离等文字
});
```

增加布局断言：9 个 `limited.color.N` 校准行不再渲染吸取按钮；通用设置颜色 1/2 行渲染吸取按钮。

- [ ] **Step 2: 运行前端失败测试**

```powershell
bunx vitest run src/components/app/special-ops-page.test.tsx
```

预期：新断言失败。

- [ ] **Step 3: 调整页面状态与 command 结果显示**

保留单独的 `samplingLimitedColor` 和 `testingTargetKey` 状态，但统一错误展示到对应通用设置组或校准行；不要在 `catch` 中只复位鼠标/状态。

颜色操作 UI：

- 颜色 1/2 与 RGB 字段同组显示“吸取 1”“吸取 2”。
- 进行中显示“吸取中”，按钮禁用。
- 成功更新 RGB 草稿并显示结果。
- 失败显示错误，不覆盖原值。

识色测试 UI：

- 显示两次采样结果。
- 显示每次距离、匹配颜色和最终命中/未命中。
- 未命中不调用保存。

- [ ] **Step 4: 运行前端测试**

```powershell
bunx vitest run src/components/app/special-ops-page.test.tsx
```

预期：相关测试通过。

## Task 4: 扩展交易行账号业务配置与旧配置迁移

**Files:**
- Modify: `src-tauri/src/special_ops/market_purchase.rs`
- Modify: `src-tauri/src/special_ops/mod.rs`
- Modify: `src/components/app/special-ops-types.ts`
- Test: `src-tauri/src/special_ops/market_purchase.rs`
- Test: `src-tauri/src/special_ops/mod.rs`

- [ ] **Step 1: 写配置迁移和继承失败测试**

覆盖：

1. 旧 `marketPurchase.enabled/purchaseCount/itemNote` 迁移到 `defaultBusinessConfig.market`。
2. 已存在新字段时旧字段不覆盖新值。
3. migration 重复执行结果不变化。
4. 账号独立配置关闭时使用默认 market。
5. 账号独立配置开启后只覆盖该账号 market。

- [ ] **Step 2: 运行迁移测试确认失败**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib special_ops::tests::legacy_settings_gain_disabled_limited_and_market_features -- --exact
```

预期：新字段和迁移断言尚未满足。

- [ ] **Step 3: 扩展 `MarketBusinessConfig`**

在 `src-tauri/src/special_ops/market_purchase.rs` 增加带 serde 默认值的字段：

```rust
pub struct MarketBusinessConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_purchase_count")]
    pub purchase_count: u32,
    #[serde(default)]
    pub item_note: String,
    #[serde(default)]
    pub product_point: Option<CalibrationRect>,
    #[serde(default = "default_max_price")]
    pub max_price: u64,
}
```

`MarketPurchaseSettings` 保留 `entry_delay_ms` 作为全局通用参数；旧字段保留反序列化兼容，但 normalize 后运行时只读取 `BusinessConfig.market`。

- [ ] **Step 4: 实现 normalize 迁移**

在 `normalize_settings` 中：

- 新 market 字段存在时保持新值。
- 缺失新字段时从旧 `settings.market_purchase` 填充默认账号配置。
- 将购买次数规范化为至少 1。
- 将价格规范化为合法非负值，默认值保持现有兼容行为。
- 对所有账号的 `independent_business_config.market` 应用字段默认值，但不复制运行态 `MarketAccountState`。

- [ ] **Step 5: 运行 Rust 配置测试**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib special_ops -- --nocapture
```

预期：新增迁移、继承、独立覆盖测试通过。

## Task 5: 让 scheduler 和 runtime 使用账号有效交易行配置

**Files:**
- Modify: `src-tauri/src/special_ops/mod.rs`
- Modify: `src-tauri/src/special_ops/market_runtime.rs`
- Modify: `src-tauri/src/special_ops/round_planner.rs`
- Modify: `src-tauri/src/special_ops/round_runner.rs`
- Test: `src-tauri/src/special_ops/market_runtime.rs`
- Test: `src-tauri/src/special_ops/mod.rs`

- [ ] **Step 1: 写有效配置解析测试**

```rust
#[test]
fn effective_market_business_config_uses_account_override_only_when_enabled() {
    // independentSettingsEnabled=false -> default market
    // true -> account.independentBusinessConfig.market
}
```

增加 scheduler 测试：默认账号关闭交易行时不创建购买任务；独立账号开启交易行时只创建该账号任务；两个账号可拥有不同次数和价格。

- [ ] **Step 2: 实现单一解析函数**

在 `special_ops/mod.rs` 增加：

```rust
fn effective_market_business_config(
    settings: &SpecialOpsSettings,
    account: &AccountPlan,
) -> MarketBusinessConfig {
    if account.independent_settings_enabled {
        account
            .independent_business_config
            .as_ref()
            .map(|config| config.market.clone())
            .unwrap_or_else(|| settings.default_business_config.market.clone())
    } else {
        settings.default_business_config.market.clone()
    }
}
```

所有 scheduler、freeze、trial 入口统一使用该函数，不在多个调用点重复判断继承关系。

- [ ] **Step 3: 修改任务规划与冻结**

交易行任务创建条件改为有效 `market.enabled`；任务目标次数和备注取有效 `market.purchase_count/item_note`；冻结配置携带有效 `market` 与全局 `entry_delay_ms`。

runtime 继续使用校准环境中的 `market.entry`、`market.price`、`market.return`、`market.buy`、`market.confirm`，其中 `market.price` 仅负责 OCR，最终确认购买使用独立点击点，不读取账号坐标。

- [ ] **Step 4: 运行 runtime 和 scheduler 测试**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib market_runtime -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml --lib special_ops::tests::schedule_projects_current_limited_cycle_and_market_window -- --exact
```

预期：现有流程和新增账号差异测试全部通过。

## Task 6: 重排前端交易行配置界面

**Files:**
- Modify: `src/components/app/special-ops-page.tsx`
- Modify: `src/components/app/special-ops-types.ts`
- Modify: `src/components/app/special-ops-page.test.tsx`

- [ ] **Step 1: 写 UI 失败测试**

验证：

- 默认账号配置显示交易行开关、购买次数、备注、最高价格、商品入口点击点。
- 账号独立设置关闭时隐藏账号级交易行字段。
- 账号独立设置开启后显示独立字段。
- 限时商品字段不出现在账号配置。

- [ ] **Step 2: 运行测试确认失败**

```powershell
bunx vitest run src/components/app/special-ops-page.test.tsx
```

- [ ] **Step 3: 修改 bootstrap 类型和更新函数**

让 `BusinessConfig.market` 类型包含 `enabled`、`purchaseCount`、`itemNote`、`maxPrice`、`productPoint`；`marketPurchase` 前端类型只保留全局入口等待参数和兼容字段读取。

新增两个更新入口：

```ts
updateDefaultMarketBusiness(patch: Partial<BusinessConfig["market"]>)
updateIndependentMarketBusiness(accountId: string, patch: Partial<BusinessConfig["market"]>)
```

保存仍通过现有 `save`/revision 队列，不新增 Tauri command。

- [ ] **Step 4: 移动交易行面板**

删除全局交易行购买业务面板中的启用、次数、备注、最高价格、商品入口点击点输入；将这些字段放入默认账号配置和账号独立设置区域。保留全局入口等待时间在通用设置。

交易行入口点击点仍调用校准 overlay，提交上下文使用默认或账号 business context；校准环境 target 本体仍只有一份。

限时商品颜色组放入通用设置，与颜色 1/2 RGB 同组；校准区域行删除吸取按钮。

- [ ] **Step 5: 运行前端测试和 build**

```powershell
bunx vitest run src/components/app/special-ops-page.test.tsx
bun run build
```

预期：测试和生产构建通过。

## Task 7: 文档、契约测试与全量验证

**Files:**
- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `droid-wiki/features/special-ops.md`
- Modify: `droid-wiki/systems/profile-system.md`
- Modify: `src-tauri/tests/special_ops_async_command_contract.rs`

- [ ] **Step 1: 更新文档**

同步以下事实：

- 限时商品全局配置与颜色吸取位置。
- 识色测试返回命中/未命中明细。
- 交易行业务配置归属 `BusinessConfig.market`，账号按继承/独立配置解析。
- 显示环境校准 target 不复制到账号。
- 旧配置迁移规则。

- [ ] **Step 2: 扩展 command contract**

确认 `special_ops_sample_limited_supply_color`、`special_ops_test_limited_supply_colors` 注册且命令签名保持 camelCase。

- [ ] **Step 3: 运行定向验证**

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo test --manifest-path src-tauri/Cargo.toml --lib limited_supply -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml --lib market_runtime -- --nocapture
bunx vitest run src/components/app/special-ops-page.test.tsx
git diff --check
```

- [ ] **Step 4: 运行全量质量门禁**

```powershell
bun run check
```

预期：TypeScript、516+ 前端测试、coverage、Rust fmt、Clippy、Rust library/integration tests 全部通过。若 Tauri 开发版运行中导致 `os error 5`，先关闭开发版再重跑，不把锁文件错误当作代码失败。

## 计划自检

- 设计文档中的四项目标均映射到 Task 2、3、4、6。
- 限时商品不进入账号配置由 Task 4、6、7 显式验证。
- 颜色复用由 Task 1、2 覆盖；UI 无反馈由 Task 3 覆盖。
- 交易行配置迁移、继承、scheduler/runtime 使用由 Task 4、5 覆盖。
- 未使用 TODO、TBD 或未定义函数名。
