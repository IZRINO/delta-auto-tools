# 连发器双主键触发键 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 连发器触发键支持 `A+B` 这类双主键，按住语义与 `Shift+A` 相同；同时按住 Shift+A+B 时 `Shift+A` 与 `A+B` 两张卡片都开火。

**Architecture:** `HotkeyBinding` 增加 `extra_primary`。`parse` 仍拒双主键；`parse_allowing_chord` 给连发器 hold 注册与触发键归一化。Hold 匹配改为按下集合子集重算。录入无修饰键时第一主键抬起才存单键，第二主键按下即存和弦。

**Tech Stack:** Rust / Tauri 2, React 19, TypeScript, Vitest, cargo test

## Global Constraints

- 最多两个主键；目标键仍单键
- 不改摩斯/计时器/计数器/识别录入 UI
- 无新 command、无新持久化字段
- 中文 UI/注释/commit；serde camelCase 不变
- 编码前 ponytail：最短 diff，不引入 N 键和弦
- 不自动 git commit（用户未要求）

Spec: `docs/superpowers/specs/2026-09-19-rapidfire-chord-trigger-design.md`

---

### Task 1: HotkeyBinding 双主键解析

**Files:**
- Modify: `src-tauri/src/hotkey_types.rs`

**Interfaces:**
- Produces: `HotkeyBinding.extra_primary: Option<PrimaryKey>`
- Produces: `HotkeyBinding::parse_allowing_chord(raw: &str) -> Result<Self, String>`
- Produces: `hotkey_to_string_allowing_chord(raw: &str) -> Result<String, String>`
- Produces: `hotkey_primary_labels(raw: &str) -> Result<Vec<String>, String>`
- `parse` 行为不变

- [ ] **Step 1: 写失败测试**

在 `hotkey_types.rs` tests 追加：

```rust
#[test]
fn parse_still_rejects_two_primaries() {
    let error = HotkeyBinding::parse("A+B").expect_err("should reject");
    assert!(error.contains("多个主键"));
}

#[test]
fn parse_allowing_chord_normalizes_two_primaries() {
    let binding = HotkeyBinding::parse_allowing_chord("B+A").unwrap();
    assert_eq!(binding.primary, PrimaryKey::Letter('A'));
    assert_eq!(binding.extra_primary, Some(PrimaryKey::Letter('B')));
    assert!(binding.modifiers.is_empty());
    assert_eq!(hotkey_to_string_allowing_chord("b+a").unwrap(), "A+B");
}

#[test]
fn parse_allowing_chord_keeps_shift_combo() {
    let binding = HotkeyBinding::parse_allowing_chord("Shift+A").unwrap();
    assert_eq!(binding.primary, PrimaryKey::Letter('A'));
    assert_eq!(binding.extra_primary, None);
    assert!(binding.modifiers.contains(&ModifierKey::Shift));
}

#[test]
fn parse_allowing_chord_rejects_three_primaries() {
    let error = HotkeyBinding::parse_allowing_chord("A+B+C").expect_err("should reject");
    assert!(error.contains("最多两个主键"));
}

#[test]
fn parse_allowing_chord_encodes_plus_as_second_primary() {
    assert_eq!(hotkey_to_string_allowing_chord("A++").unwrap(), "A++");
    let binding = HotkeyBinding::parse_allowing_chord("A++").unwrap();
    assert_eq!(binding.primary, PrimaryKey::Letter('A'));
    assert_eq!(binding.extra_primary, Some(PrimaryKey::Named(NamedKey::Plus)));
}
```

- [ ] **Step 2: 跑测试确认失败**

```
cargo test --manifest-path src-tauri/Cargo.toml parse_allowing_chord_normalizes_two_primaries -- --nocapture
```

Expected: compile fail，`parse_allowing_chord` 不存在

- [ ] **Step 3: 最小实现**

- `HotkeyBinding` 加 `extra_primary: Option<PrimaryKey>`，`parse` 设 `None`
- `parse_with(raw, allow_chord)`：第二个非修饰段写入 extra；第三段报「组合键最多两个主键」
- 两主键按 `primary_to_string` 排序，`NamedKey::Plus` 永远排最后（保证 `A++` 而不是 `++A`）
- 空 segment 且 `ends_with('+')` 且 peek 为空时，即使已有 primary 也当成 `"+"`（让 `A++` 能解析）
- `binding_to_string` 追加 extra
- `hotkey_primary_labels`：chord 用 `parse_allowing_chord`，返回 1 或 2 个标签；`hotkey_primary_label` 保持 `parse`

- [ ] **Step 4: 跑测试确认通过**

```
cargo test --manifest-path src-tauri/Cargo.toml --lib hotkey_types
```

---

### Task 2: Hold 按下集合重算 + 主例

**Files:**
- Modify: `src-tauri/src/hotkeys.rs`

**Interfaces:**
- Consumes: `extra_primary`, `parse_allowing_chord`
- `replace_hold_scope` / mixed hold 解析改 `parse_allowing_chord`
- `hold_actions_for_event` 改为 pressed_primaries + pressed_modifiers + active_bindings 重算

- [ ] **Step 1: 写失败测试（主例）**

保留现有 `Shift+-` / 裸 `1`+`Shift+1` 测试，改调用点适配新 state。新增：

```rust
fn hold_shift_a_and_a_b_both_fire_when_all_held() {
    // 注册 Shift+A 与 A+B
    // Down Shift → 空
    // Down A → Down Shift+A
    // Down B → Down A+B
    // Up Shift → Up Shift+A
    // Up B → Up A+B
    // Up A → 空
}
```

另测 `replace_hold_scope("A+B")` 成功。

- [ ] **Step 2: 跑测试确认失败**

`A+B` 注册失败或 B 按下不 Down 第二张卡。

- [ ] **Step 3: 最小实现**

每次 Down/Up 更新按下集合，desired = 主键全在集合内且 modifiers ⊆ effective。Down = desired−active，Up = active−desired。Alt 作主键时仍从 effective 修饰里去掉 Alt。注入事件忽略。key repeat（主键已在集合）不重复 Down。

- [ ] **Step 4: 跑测试**

```
cargo test --manifest-path src-tauri/Cargo.toml --lib hotkeys
```

现有 hold 回归必须绿。

---

### Task 3: 抑制双主键 + 连发器 normalize

**Files:**
- Modify: `src-tauri/src/key_suppressor.rs`
- Modify: `src-tauri/src/hotkeys.rs` (`suppress_key` / `unsuppress_key`)
- Modify: `src-tauri/src/rapidfire/mod.rs`
- Modify: `src-tauri/src/rapidfire/keys.rs`（`trigger_primary_label` 改走 labels）

- [ ] **Step 1: 测试**

```rust
// key_suppressor: hotkey_primaries_to_vks("A+B") 两个 VK
// normalize_card("b+a") → trigger_key "A+B"
// normalize_card("A+B+C") 失败含「最多两个主键」
```

- [ ] **Step 2: 确认失败**
- [ ] **Step 3: 实现**

`normalize_trigger_key` → `hotkey_to_string_allowing_chord`。两个主键都 `parse_target_key`。`suppress_key("A+B")` 抑制两个 VK。

- [ ] **Step 4: 跑测试**

```
cargo test --manifest-path src-tauri/Cargo.toml --lib rapidfire
cargo test --manifest-path src-tauri/Cargo.toml --lib key_suppressor
```

---

### Task 4: 前端归一化 / 校验

**Files:**
- Modify: `src/components/app/rapidfire-types.ts`
- Modify: `src/components/app/rapidfire-types.test.ts`

- [ ] **Step 1: 测试**

```ts
form.cards[0].triggerKey = "b+a"
expect(parseRapidfireSettingsForm(form).cards[0].triggerKey).toBe("A+B")

form.cards[0].triggerKey = "A+B+C"
expect(() => parseRapidfireSettingsForm(form)).toThrow("最多两个主键")
```

- [ ] **Step 2: 确认失败**（当前抛「只能包含一个主键」）
- [ ] **Step 3: `normalizeRapidfireHotkey` / `validateRapidfireHotkeyPrimary` 允第二主键，排序规则与 Rust 一致（Plus 最后）
- [ ] **Step 4:** `bunx vitest run src/components/app/rapidfire-types.test.ts`

---

### Task 5: 触发键录入状态机

**Files:**
- Modify: `src/components/app/rapidfire-types.ts`（纯函数 `reduceRapidfireTriggerChord`）
- Modify: `src/hooks/use-hotkey-recorder.ts`（可选 `formatKeyUp`）
- Modify: `src/components/app/rapidfire-page.tsx`
- Test: `src/components/app/rapidfire-types.test.ts`, `src/hooks/use-hotkey-recorder.test.ts`

录入规则：

1. 修饰键单独 down → 不提交
2. 已有 Ctrl/Alt/Shift/Win + 第一主键 down → 立即提交（`Shift+A`）
3. 无修饰 + 第一主键 down → pending
4. 第二主键 down → 提交 `A+B`
5. 第一主键 up 且无第二键 → 提交单键
6. 目标键路径不变

- [ ] **Step 1: 纯函数测试先红**
- [ ] **Step 2: 实现 reduce + hook keyup + KeyRecorderButton onKeyUp**
- [ ] **Step 3:** `bunx vitest run src/components/app/rapidfire-types.test.ts src/hooks/use-hotkey-recorder.test.ts`

文案：录制中提示「可再按第二键组成 A+B，松手保存单键」。

---

### Task 6: Wiki

**Files:**
- Modify: `droid-wiki/features/rapidfire.md`
- Modify: `droid-wiki/systems/hotkeys.md`

触发键：单键、`Shift+A`、或双主键 `A+B`。子集匹配：Shift+A+B 同时打到 `Shift+A` 与 `A+B`。`extra_primary`、`parse` vs `parse_allowing_chord`、hold 按按下集合重算。

---

### Task 7: 门禁

```
cargo test --manifest-path src-tauri/Cargo.toml --lib hotkey_types
cargo test --manifest-path src-tauri/Cargo.toml --lib hotkeys
cargo test --manifest-path src-tauri/Cargo.toml --lib rapidfire
bunx vitest run src/components/app/rapidfire-types.test.ts src/hooks/use-hotkey-recorder.test.ts
```

全绿后再视情况 `cargo test --manifest-path src-tauri/Cargo.toml`。
