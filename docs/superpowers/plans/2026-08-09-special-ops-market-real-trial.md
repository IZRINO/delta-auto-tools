# 特勤处交易行真实试运行实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将交易行试运行改为执行一次真实价格分支，达标时完成一次购买确认，不达标时返回，不污染正式购买计数。

**Architecture:** 复用现有 `market_runtime::run_market_trial` 的 `RealSingleAttempt` 分支；只把前端试运行 command 的 `mode` 从 `inspectOnly` 改为 `realSingleAttempt`。runtime 已通过 `persist_official = false` 隔离试运行计数，不写入账号正式状态。

**Tech Stack:** Tauri 2、React 19、TypeScript、Rust、Vitest、Cargo test。

---

### Task 1: 切换交易行试运行模式

**Files:**
- Modify: `src/components/app/special-ops-page.tsx:928`

- [x] **Step 1: 修改 command 参数**

将交易行试运行调用中的：

```ts
mode: "inspectOnly"
```

改为：

```ts
mode: "realSingleAttempt"
```

- [x] **Step 2: 保持现有 runtime 行为**

不要修改 `run_market_trial` 的购买分支。它应继续按以下逻辑运行：

```rust
if mode == MarketTrialMode::RealSingleAttempt {
    match action {
        MarketTrialAction::Buy => {
            driver.click("market.buy", false, cancelled.clone()).await?;
            driver.click("market.confirm", false, cancelled.clone()).await?;
            driver.persist_purchase_click()?;
        }
        MarketTrialAction::Return | MarketTrialAction::OcrFailed => {
            driver.click("market.return", false, cancelled).await?;
        }
    }
}
```

- [x] **Step 3: 验证前端类型与 command 参数**

运行：

```bash
bun run build
```

预期：TypeScript 与 Vite 构建通过。

### Task 2: 验证真实单次试运行分支

**Files:**
- Test: `src-tauri/src/special_ops/market_runtime.rs:486`

- [x] **Step 1: 运行已有真实单次试运行测试**

运行：

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib real_trial_executes_exactly_one_price_branch
```

预期：通过，并确认达标价格只执行一次 `market.buy` 与 `market.confirm`。

- [x] **Step 2: 运行市场相关测试**

运行：

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib market_
```

预期：所有市场、交易行计划、配置测试通过。

- [x] **Step 3: 运行 Rust 质量检查**

运行：

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --lib -- -D warnings
```

预期：全部通过。

### Self-review

- 需求覆盖：真实单次购买、超价返回、OCR 失败返回、试运行计数隔离，均由现有 runtime 分支覆盖。
- 未新增持久化字段、command、UI 配置或购买循环。
- `inspectOnly` 保留为 runtime 枚举兼容值，但前端不再用于交易行试运行。
