# Issue #83 Optional Retrigger Policy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 `always` 常驻识图/识色增加默认关闭的“目标消失后再触发”开关，并要求连续 2 次未命中后才重新武装。

**Architecture:** 在 `RecognitionCard` 持久化 `retriggerAfterDisappear` 布尔字段，前端表单完整 round-trip；后端扩展现有 `MatchGate`，用同一状态机实现 cooldown 周期触发和消失后重触发两种策略。RegionWatch / ColorWatch 只产生 `Matched`、`NotMatched`、`CaptureFailed` observation，不复制策略判断。

**Tech Stack:** React 19、TypeScript、Vitest、Tauri 2、Rust、serde、Tokio、Bun

---

## 文件结构

- `src/components/app/recognition-types.ts`：前端持久化类型、表单类型和新卡片默认值。
- `src/components/app/recognition-utils.ts`：settings/form 双向转换，旧配置缺省值兼容。
- `src/components/app/recognition-utils.test.ts`：字段默认值与 round-trip 回归测试。
- `src/components/app/recognition-page.tsx`：`always` 开关和页面级 `cardToForm` 转换。
- `src-tauri/src/recognition/types.rs`：Rust serde 字段与旧 JSON 默认值。
- `src-tauri/src/recognition/watcher/manager.rs`：重复触发 gate、watcher 接线和状态机测试。
- `src-tauri/src/recognition/mod.rs`：测试用 `RecognitionCard` struct literal 补字段。
- `droid-wiki/features/recognition.md`：默认行为、开关和重新武装规则。

## Task 1：前端字段与 round-trip

**Files:**
- Modify: `src/components/app/recognition-types.ts:83-194`
- Modify: `src/components/app/recognition-utils.ts:173-215`
- Modify: `src/components/app/recognition-utils.ts:320-343`
- Test: `src/components/app/recognition-utils.test.ts:13-50`
- Test: `src/components/app/recognition-utils.test.ts:52-107`

- [ ] **Step 1: 在测试表单 helper 增加默认关闭字段**

在 `draftCard` 的 `watchPollIntervalMs` 后加入：

```typescript
retriggerAfterDisappear: false,
```

- [ ] **Step 2: 写旧配置默认关闭和 true round-trip 失败测试**

在 `describe("settingsToForm")` 中加入：

```typescript
it("旧卡片缺少消失后重触发字段时默认关闭", () => {
    const legacyCard: typeof DEFAULT_RECOGNITION_CARD & {retriggerAfterDisappear?: boolean} = {
        ...DEFAULT_RECOGNITION_CARD,
        id: "legacy",
        name: "旧卡片",
        enabled: false,
    };
    delete legacyCard.retriggerAfterDisappear;

    const form = settingsToForm({recognitionEnabled: true, cards: [legacyCard]});

    expect(form.cards[0].retriggerAfterDisappear).toBe(false);
});

it("消失后重触发字段在 settings 和 form 间保持 true", () => {
    const form = settingsToForm({
        recognitionEnabled: true,
        cards: [{
            ...DEFAULT_RECOGNITION_CARD,
            id: "edge",
            name: "边沿触发",
            enabled: false,
            triggerMode: "regionWatch",
            retriggerAfterDisappear: true,
        }],
    });

    expect(form.cards[0].retriggerAfterDisappear).toBe(true);
    expect(parseSettingsForm(form).cards[0].retriggerAfterDisappear).toBe(true);
});
```

- [ ] **Step 3: 运行定向测试并确认失败**

```powershell
bunx vitest run src/components/app/recognition-utils.test.ts
```

Expected: FAIL，字段不存在或结果为 `undefined`。

- [ ] **Step 4: 增加前端类型与默认值**

在 `RecognitionCard` 增加可选字段，允许表示旧配置：

```typescript
retriggerAfterDisappear?: boolean;
```

在 `RecognitionCardForm` 增加稳定字段：

```typescript
retriggerAfterDisappear: boolean;
```

在 `DEFAULT_RECOGNITION_CARD` 增加：

```typescript
retriggerAfterDisappear: false,
```

同时在 `recognition-utils.test.ts` 中不经过 `draftCard` 的 inline `RecognitionCardForm`（当前 `getRecognitionCardFormErrors combo 文件不足标记 audioFiles 错误` 用例）加入：

```typescript
retriggerAfterDisappear: false,
```

- [ ] **Step 5: 更新 settings/form 双向转换**

`recognition-utils.ts` 的 `cardToForm` 返回值加入：

```typescript
retriggerAfterDisappear: card.retriggerAfterDisappear ?? false,
```

`parseCardForm` 返回值加入：

```typescript
retriggerAfterDisappear: form.retriggerAfterDisappear ?? false,
```

- [ ] **Step 6: 运行定向测试并确认通过**

```powershell
bunx vitest run src/components/app/recognition-utils.test.ts
```

Expected: `recognition-utils.test.ts` 全部 PASS。

- [ ] **Step 7: 提交前端数据链路**

```powershell
git add src/components/app/recognition-types.ts src/components/app/recognition-utils.ts src/components/app/recognition-utils.test.ts
git commit -m "feat(recognition): 持久化消失后重触发选项"
```

## Task 2：Rust serde 兼容

**Files:**
- Modify: `src-tauri/src/recognition/types.rs:79-141`
- Test: `src-tauri/src/recognition/types.rs:540-590`
- Modify: `src-tauri/src/recognition/watcher/manager.rs:870-930`
- Modify: `src-tauri/src/recognition/mod.rs:1148-1195`
- Modify: `src-tauri/src/recognition/mod.rs:1682-1765`

- [ ] **Step 1: 写旧 JSON 缺字段与 camelCase 序列化测试**

在 `types.rs` 测试模块加入：

```rust
#[test]
fn recognition_card_retrigger_defaults_false_and_serializes_camel_case() {
    let mut card: RecognitionCard = serde_json::from_value(serde_json::json!({
        "id": "legacy",
        "name": "旧卡片"
    }))
    .expect("旧卡片应可反序列化");

    assert!(!card.retrigger_after_disappear);

    card.retrigger_after_disappear = true;
    let value = serde_json::to_value(card).expect("卡片应可序列化");
    assert_eq!(value["retriggerAfterDisappear"], true);
}
```

- [ ] **Step 2: 运行测试并确认失败**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml recognition_card_retrigger_defaults_false_and_serializes_camel_case
```

Expected: FAIL，`RecognitionCard` 尚无 `retrigger_after_disappear`。

- [ ] **Step 3: 增加 serde 字段**

在 `watch_poll_interval_ms` 后加入：

```rust
/// 常驻识别是否锁定当前命中，等待目标连续两次未命中后再触发。
#[serde(default)]
pub retrigger_after_disappear: bool,
```

- [ ] **Step 4: 补齐显式 struct literals**

运行：

```powershell
rg -n "RecognitionCard \{" src-tauri/src
```

对每个显式 literal 加入：

```rust
retrigger_after_disappear: false,
```

只有专门验证开启策略的测试 literal 使用 `true`。

- [ ] **Step 5: 运行 serde 测试和 cargo check**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml recognition_card_retrigger_defaults_false_and_serializes_camel_case
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: 定向测试 PASS；`cargo check` exit 0，无遗漏 literal。

- [ ] **Step 6: 提交 Rust 数据模型**

```powershell
git add src-tauri/src/recognition/types.rs src-tauri/src/recognition/watcher/manager.rs src-tauri/src/recognition/mod.rs
git commit -m "feat(recognition): 增加重复触发策略字段"
```

## Task 3：用 TDD 重写 MatchGate 两种策略

**Files:**
- Modify: `src-tauri/src/recognition/watcher/manager.rs:220-257`
- Test: `src-tauri/src/recognition/watcher/manager.rs:795-853`

- [ ] **Step 1: 用明确策略测试替换旧 gate 测试**

```rust
#[test]
fn cooldown_repeat_triggers_again_after_cooldown_while_still_matched() {
    let start = Instant::now();
    let mut gate = MatchGate::new(false);

    assert!(gate.observe(MatchObservation::Matched, 1000, start));
    assert!(!gate.observe(
        MatchObservation::Matched,
        1000,
        start + Duration::from_millis(999),
    ));
    assert!(gate.observe(
        MatchObservation::Matched,
        1000,
        start + Duration::from_millis(1000),
    ));
}

#[test]
fn after_disappear_requires_two_consecutive_misses() {
    let start = Instant::now();
    let mut gate = MatchGate::new(true);

    assert!(gate.observe(MatchObservation::Matched, 0, start));
    gate.observe(MatchObservation::NotMatched, 0, start + Duration::from_secs(1));
    assert!(!gate.observe(
        MatchObservation::Matched,
        0,
        start + Duration::from_secs(2),
    ));
    gate.observe(MatchObservation::NotMatched, 0, start + Duration::from_secs(3));
    gate.observe(MatchObservation::NotMatched, 0, start + Duration::from_secs(4));
    assert!(gate.observe(
        MatchObservation::Matched,
        0,
        start + Duration::from_secs(5),
    ));
}

#[test]
fn after_disappear_capture_failure_does_not_count_as_miss() {
    let start = Instant::now();
    let mut gate = MatchGate::new(true);

    assert!(gate.observe(MatchObservation::Matched, 0, start));
    gate.observe(MatchObservation::NotMatched, 0, start + Duration::from_secs(1));
    gate.observe(MatchObservation::CaptureFailed, 0, start + Duration::from_secs(2));
    assert!(!gate.observe(
        MatchObservation::Matched,
        0,
        start + Duration::from_secs(3),
    ));
}

#[test]
fn after_disappear_consumes_rising_edge_during_cooldown() {
    let start = Instant::now();
    let mut gate = MatchGate::new(true);

    assert!(gate.observe(MatchObservation::Matched, 5000, start));
    gate.observe(MatchObservation::NotMatched, 5000, start + Duration::from_secs(1));
    gate.observe(MatchObservation::NotMatched, 5000, start + Duration::from_secs(2));
    assert!(!gate.observe(
        MatchObservation::Matched,
        5000,
        start + Duration::from_secs(3),
    ));
    assert!(!gate.observe(
        MatchObservation::Matched,
        5000,
        start + Duration::from_secs(6),
    ));
}
```

- [ ] **Step 2: 运行 gate 测试并确认失败**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml match_gate
cargo test --manifest-path src-tauri/Cargo.toml cooldown_repeat_triggers_again_after_cooldown_while_still_matched
cargo test --manifest-path src-tauri/Cargo.toml after_disappear
```

Expected: FAIL，`MatchGate::new` 不存在，旧 gate 不支持两种策略。

- [ ] **Step 3: 实现最小双策略 gate**

```rust
const REARM_MISS_COUNT: u8 = 2;

#[derive(Debug)]
struct MatchGate {
    retrigger_after_disappear: bool,
    was_matched: bool,
    consecutive_misses: u8,
    last_triggered: Option<Instant>,
}

impl MatchGate {
    fn new(retrigger_after_disappear: bool) -> Self {
        Self {
            retrigger_after_disappear,
            was_matched: false,
            consecutive_misses: 0,
            last_triggered: None,
        }
    }

    fn observe(&mut self, observation: MatchObservation, cooldown_ms: u32, now: Instant) -> bool {
        match observation {
            MatchObservation::CaptureFailed => false,
            MatchObservation::NotMatched => {
                if self.retrigger_after_disappear && self.was_matched {
                    self.consecutive_misses = self.consecutive_misses.saturating_add(1);
                    if self.consecutive_misses >= REARM_MISS_COUNT {
                        self.was_matched = false;
                        self.consecutive_misses = 0;
                    }
                }
                false
            }
            MatchObservation::Matched => {
                self.consecutive_misses = 0;
                if self.retrigger_after_disappear && self.was_matched {
                    return false;
                }
                if self.retrigger_after_disappear {
                    self.was_matched = true;
                }
                let ready = self
                    .last_triggered
                    .map(|last| {
                        now.duration_since(last) >= Duration::from_millis(cooldown_ms as u64)
                    })
                    .unwrap_or(true);
                if ready {
                    self.last_triggered = Some(now);
                }
                ready
            }
        }
    }
}
```

- [ ] **Step 4: 运行 gate 测试并确认通过**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml match_gate
cargo test --manifest-path src-tauri/Cargo.toml cooldown_repeat_triggers_again_after_cooldown_while_still_matched
cargo test --manifest-path src-tauri/Cargo.toml after_disappear
```

Expected: 所有新 gate 测试 PASS。

- [ ] **Step 5: 提交状态机**

```powershell
git add src-tauri/src/recognition/watcher/manager.rs
git commit -m "fix(recognition): 稳定识别重复触发门控"
```

## Task 4：接入 RegionWatch 与 ColorWatch

**Files:**
- Modify: `src-tauri/src/recognition/watcher/manager.rs:29-138`
- Modify: `src-tauri/src/recognition/watcher/manager.rs:539-725`

- [ ] **Step 1: 从卡片读取策略并传给两个 watcher**

在 `restart_watchers` 卡片局部变量中加入：

```rust
let retrigger_after_disappear = card.retrigger_after_disappear;
```

RegionWatch 和 ColorWatch 调用均在 `cooldown_ms` 后传入：

```rust
retrigger_after_disappear,
```

- [ ] **Step 2: 扩展两个 watcher 签名并初始化 gate**

两个函数均增加参数：

```rust
cooldown_ms: u32,
retrigger_after_disappear: bool,
```

把两处：

```rust
let mut match_gate = MatchGate::default();
```

改为：

```rust
let mut match_gate = MatchGate::new(retrigger_after_disappear);
```

保留每个 tick 都截图、匹配并调用 `observe` 的数据流；不得在 cooldown 检查前跳过采集。

- [ ] **Step 3: 运行 watcher 测试与 cargo check**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml recognition::watcher::manager::tests
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: manager 测试全部 PASS；`cargo check` exit 0。

- [ ] **Step 4: 提交 runtime 接线**

```powershell
git add src-tauri/src/recognition/watcher/manager.rs
git commit -m "feat(recognition): 接入可选重触发策略"
```

## Task 5：增加 UI 开关并补页面转换

**Files:**
- Modify: `src/components/app/recognition-page.tsx:1140-1203`
- Modify: `src/components/app/recognition-page.tsx:1824-1872`

- [ ] **Step 1: 更新页面级 cardToForm**

在页面底部重复转换函数中加入：

```typescript
retriggerAfterDisappear: card.retriggerAfterDisappear ?? false,
```

该函数用于 `cardToForm(createEmptyRecognitionCard())`，不能只改共享转换。

- [ ] **Step 2: 在持续识别 activation 下增加 Switch**

在“激活方式” Select 后加入：

```tsx
{(card.activationMode ?? "always") === "always" && (
    <Field>
        <FieldLabel>重复触发</FieldLabel>
        <FieldContent>
            <label className="flex items-center gap-2 border border-base-300 bg-base-100 px-2 py-2 text-xs font-medium">
                <Switch
                    checked={card.retriggerAfterDisappear ?? false}
                    onCheckedChange={(checked) => onUpdate({retriggerAfterDisappear: checked})}
                />
                目标消失后再触发
            </label>
        </FieldContent>
    </Field>
)}
```

该位置已在 `!isHotkey` 分支，只覆盖 RegionWatch / ColorWatch；非 `always` 不显示。

- [ ] **Step 3: 运行前端定向测试与 production build**

```powershell
bunx vitest run src/components/app/recognition-utils.test.ts
bun run build
```

Expected: 定向测试 PASS；`tsc && vite build` exit 0。

- [ ] **Step 4: 提交 UI**

```powershell
git add src/components/app/recognition-page.tsx
git commit -m "feat(recognition): 增加目标消失重触发开关"
```

## Task 6：同步 wiki 并完整验证

**Files:**
- Modify: `droid-wiki/features/recognition.md`

- [ ] **Step 1: 更新当前行为说明**

将强制上升沿说明替换为：

```markdown
- `always` 常驻 RegionWatch / ColorWatch 按 `watchPollIntervalMs` 持续检查。“目标消失后再触发”默认关闭：持续命中时按 `cooldownMs` 周期触发；开启后同一命中区间只触发一次，连续 2 次未命中后重新武装。截图失败不计入未命中，`cooldownMs` 继续限制实际触发间隔。
```

- [ ] **Step 2: 运行格式与 diff 检查**

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
git diff --check
```

Expected: `git diff --check` 无输出。若全仓 fmt 仅报告已知基线 `src-tauri/src/hotkey_types.rs`，记录基线并运行：

```powershell
rustfmt --edition 2021 --check src-tauri/src/recognition/types.rs src-tauri/src/recognition/watcher/manager.rs src-tauri/src/recognition/mod.rs
```

Expected: 本轮 Rust 文件 exit 0。

- [ ] **Step 3: 运行完整前端验证**

```powershell
bun run test
bun run build
```

Expected: 所有 Vitest 测试 PASS；production build exit 0。

- [ ] **Step 4: 运行完整 Rust 验证**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: 所有 Rust 测试 PASS；`cargo check` exit 0。

- [ ] **Step 5: 同步 CodeGraph**

```powershell
codegraph sync
git status --short
```

Expected: CodeGraph 同步成功；Git 只包含 wiki 未提交改动。

- [ ] **Step 6: 提交文档**

```powershell
git add droid-wiki/features/recognition.md
git commit -m "docs(recognition): 说明重复触发策略"
```

- [ ] **Step 7: 最终核验**

```powershell
git status --short
git log -7 --oneline --decorate
```

Expected: 工作区干净；日志包含本计划五个实现/文档 commit。不要关闭 #83；待发布验证版本并收到用户确认后再关闭。
