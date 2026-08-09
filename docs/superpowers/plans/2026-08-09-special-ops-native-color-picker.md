# 特勤处限时商品原生颜色选择 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. User requires Inline Execution: do not dispatch subagents, create worktrees, or commit Git changes.

**Goal:** 用 Recognition 识别触发同款原生颜色控件替换限时商品区域平均色吸取，并删除失效的区域绑定 UI、说明折叠和后端取色 command。

**Architecture:** `LimitedSupplySettings.colors` 与 `colorTolerances` 继续作为全局配置真相；前端负责 hex 与 RGB 转换并通过现有 settings autosave 保存。9 个 `limited.color.N` 继续只负责正式 `AnyPixel` 识别和试运行测试，旧 `colorSampleRegions` 与取色 command 整条删除。

**Tech Stack:** React 19、TypeScript、daisyUI、Vitest、Tauri 2、Rust、serde、Bun

---

## 文件结构

- `src/components/app/special-ops-utils.ts`：新增限时商品 RGB/hex 纯转换函数。
- `src/components/app/special-ops-utils.test.ts`：转换与无效输入测试。
- `src/components/app/special-ops-page.tsx`：颜色控件、9 区测试、旧取色 UI/state/invoke 清理。
- `src/components/app/special-ops-page.test.tsx`：颜色控件与删除项回归测试。
- `src/components/app/special-ops-types.ts`：删除 `colorSampleRegions` 与取色结果类型。
- `src-tauri/src/special_ops/limited_supply.rs`：删除持久化字段，增加旧 JSON 兼容测试。
- `src-tauri/src/special_ops/mod.rs`：删除 normalize 校验、结果结构和取色 command。
- `src-tauri/src/lib.rs`：删除 command 注册。
- `src-tauri/tests/special_ops_async_command_contract.rs`：确认旧 command 不再注册。
- `README.md`、`AGENTS.md`、`droid-wiki/features/special-ops.md`：同步当前颜色选择方式与 command 列表。
- `docs/superpowers/plans/2026-08-09-special-ops-color-and-account-market.md`：标记旧区域取色方案已被本计划替代。

### Task 1: 增加 RGB/hex 纯转换函数

**Files:**
- Modify: `src/components/app/special-ops-utils.test.ts`
- Modify: `src/components/app/special-ops-utils.ts`

- [x] **Step 1: 写失败测试**

在 `special-ops-utils.test.ts` 的 import 中加入 `limitedColorToHex`、`parseLimitedColorHex`，增加：

```ts
describe("限时商品颜色转换", () => {
    it("把 RGB 转成六位小写 hex", () => {
        expect(limitedColorToHex([0, 15, 255])).toBe("#000fff");
        expect(limitedColorToHex([255, 255, 255])).toBe("#ffffff");
    });

    it("只接受六位 hex 并返回 RGB", () => {
        expect(parseLimitedColorHex("#00Ff80")).toEqual([0, 255, 128]);
        expect(parseLimitedColorHex("00ff80")).toEqual([0, 255, 128]);
        expect(parseLimitedColorHex("#fff")).toBeNull();
        expect(parseLimitedColorHex("#gg0000")).toBeNull();
    });
});
```

- [x] **Step 2: 验证测试因函数不存在失败**

Run:

```powershell
bunx vitest run src/components/app/special-ops-utils.test.ts
```

Expected: FAIL，提示 `limitedColorToHex` 或 `parseLimitedColorHex` 未导出。

- [x] **Step 3: 实现最小转换函数**

在 `special-ops-utils.ts` 增加：

```ts
export function limitedColorToHex(color: [number, number, number]): string {
    return `#${color.map((channel) => Math.max(0, Math.min(255, Math.trunc(channel))).toString(16).padStart(2, "0")).join("")}`;
}

export function parseLimitedColorHex(value: string): [number, number, number] | null {
    const normalized = value.trim().replace(/^#/, "");
    if (!/^[0-9a-fA-F]{6}$/.test(normalized)) return null;
    return [0, 2, 4].map((offset) => Number.parseInt(normalized.slice(offset, offset + 2), 16)) as [number, number, number];
}
```

- [x] **Step 4: 验证转换测试通过**

Run:

```powershell
bunx vitest run src/components/app/special-ops-utils.test.ts
```

Expected: PASS。

### Task 2: 用原生颜色控件替换区域吸取 UI

**Files:**
- Modify: `src/components/app/special-ops-page.test.tsx`
- Modify: `src/components/app/special-ops-page.tsx`
- Modify: `src/components/app/special-ops-types.ts`

- [x] **Step 1: 写页面失败测试**

把现有“限时商品取色放在通用设置”测试改为：

```ts
it("限时商品使用原生颜色面板且不再绑定取色区域", () => {
    const html = renderToStaticMarkup(createElement(SpecialOpsPage));
    expect((html.match(/type="color"/g) ?? []).length).toBe(2);
    expect(pageSource).toContain("limitedColorToHex");
    expect(pageSource).toContain("parseLimitedColorHex");
    expect(pageSource).not.toContain("colorSampleRegions");
    expect(pageSource).not.toContain("samplingLimitedColor");
    expect(pageSource).not.toContain("sampleLimitedColor");
    expect(pageSource).not.toContain("special_ops_sample_limited_supply_color");
    expect(pageSource).not.toContain("取色区域");
    expect(pageSource).not.toContain("限时商品识色区域校准");
});
```

- [x] **Step 2: 验证页面测试因旧 UI 仍存在失败**

Run:

```powershell
bunx vitest run src/components/app/special-ops-page.test.tsx
```

Expected: FAIL，至少命中 `type="color"` 数量不足或旧 `colorSampleRegions` 仍存在。

- [x] **Step 3: 删除旧前端类型与采样状态**

在 `special-ops-types.ts`：

```ts
export type LimitedSupplySettings = {
    enabled: boolean;
    researchDelayMs: number;
    readyTimeoutMs: number;
    colors: [[number, number, number], [number, number, number]];
    colorTolerances: [number, number];
};
```

删除 `LimitedSupplyColorSampleResult`。在 `special-ops-page.tsx` 删除对应 import、`samplingLimitedColor` state、`sampleLimitedColor` 函数及空 bootstrap 中的 `colorSampleRegions`。

- [x] **Step 4: 增加颜色更新函数**

在 `SpecialOpsPage` 的限时商品配置附近增加：

```ts
const updateLimitedColor = (colorIndex: number, color: [number, number, number]) => {
    const colors = [...limitedSupply.colors] as [[number, number, number], [number, number, number]];
    colors[colorIndex] = color;
    updateLimitedSupply({colors});
};

const commitLimitedColorHex = (colorIndex: number, value: string) => {
    const color = parseLimitedColorHex(value);
    if (!color) {
        setError("颜色必须使用 #RRGGBB 格式");
        return;
    }
    setError(null);
    updateLimitedColor(colorIndex, color);
};
```

- [x] **Step 5: 渲染 Recognition 同款原生颜色控件**

用下列结构替换每个颜色卡片中的三个 RGB 输入、区域下拉框和吸取按钮：

```tsx
<div className="mt-2 flex items-end gap-2">
    <label className="form-control gap-1">
        <span className="label-text text-xs">目标颜色</span>
        <input
            type="color"
            value={limitedColorToHex(color)}
            onChange={(event) => {
                const next = parseLimitedColorHex(event.target.value);
                if (next) updateLimitedColor(colorIndex, next);
            }}
            className="h-9 w-12 cursor-pointer border border-base-300 bg-transparent p-0"
            aria-label={`颜色 ${colorIndex + 1}`}
        />
    </label>
    <label className="form-control min-w-0 flex-1 gap-1">
        <span className="label-text text-xs">Hex</span>
        <DraftInput
            className="font-mono"
            value={limitedColorToHex(color)}
            onCommit={(value) => commitLimitedColorHex(colorIndex, value)}
        />
    </label>
    <label className="form-control w-28 gap-1">
        <span className="label-text text-xs">容差</span>
        <DraftInput
            type="number"
            min={0}
            max={255}
            value={String(limitedSupply.colorTolerances[colorIndex])}
            onCommit={(value) => {
                const tolerances = [...limitedSupply.colorTolerances] as [number, number];
                tolerances[colorIndex] = Math.max(0, Math.min(255, Math.trunc(Number(value) || 0)));
                updateLimitedSupply({colorTolerances: tolerances});
            }}
        />
    </label>
</div>
```

删除“限时商品识色区域校准”整个 `<details>`。

- [x] **Step 6: 把识色测试改为覆盖全部 9 区**

将 `testLimitedColors` 中区域来源改为：

```ts
const results: LimitedSupplyColorTestResult[] = [];
for (let regionIndex = 1; regionIndex <= 9; regionIndex += 1) {
    results.push(await invoke<LimitedSupplyColorTestResult>(
        "special_ops_test_limited_supply_colors",
        {environmentId: activeEnvironment.id, regionIndex, settingsRevision: saved.settingsRevision},
    ));
}
```

保留 `limitedColorFeedback`，它只显示 9 区双采样测试结果，不再显示取色结果。

- [x] **Step 7: 验证页面和转换测试通过**

Run:

```powershell
bunx vitest run src/components/app/special-ops-page.test.tsx src/components/app/special-ops-utils.test.ts
```

Expected: PASS。

### Task 3: 删除旧 Rust 字段与取色 command

**Files:**
- Modify: `src-tauri/src/special_ops/limited_supply.rs`
- Modify: `src-tauri/src/special_ops/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/tests/special_ops_async_command_contract.rs`

- [x] **Step 1: 写旧 JSON 兼容与 command 删除失败测试**

在 `limited_supply.rs` 测试模块增加：

```rust
#[test]
fn legacy_color_sample_regions_are_ignored_and_not_reserialized() {
    let mut value = serde_json::to_value(super::LimitedSupplySettings::default()).unwrap();
    value["colorSampleRegions"] = serde_json::json!([1, 9]);

    let settings: super::LimitedSupplySettings = serde_json::from_value(value).unwrap();
    let serialized = serde_json::to_value(settings).unwrap();

    assert!(serialized.get("colorSampleRegions").is_none());
}
```

把 command contract 拆成保留项与删除项：

```rust
#[test]
fn limited_market_commands_are_registered_without_legacy_color_sampler() {
    let source = include_str!("../src/lib.rs");
    for command in [
        "special_ops_start_limited_supply_trial",
        "special_ops_start_market_trial",
        "special_ops_acknowledge_limited_supply",
        "special_ops_test_limited_supply_colors",
    ] {
        assert!(source.contains(command), "缺少 Tauri command 注册：{command}");
    }
    assert!(!source.contains("special_ops_sample_limited_supply_color"));
}
```

- [x] **Step 2: 验证 command contract 因旧注册仍存在失败**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml limited_market_commands_are_registered_without_legacy_color_sampler
```

Expected: FAIL，旧 command 仍在 `src-tauri/src/lib.rs`。

- [x] **Step 3: 删除持久化字段和 normalize 校验**

从 `limited_supply.rs` 删除：

```rust
fn default_color_sample_regions() -> [u8; 2] { ... }
pub color_sample_regions: [u8; 2],
```

并从 `Default` 删除字段初始化。从 `special_ops/mod.rs::normalize_settings` 删除“限时商品取色区域必须是 1–9”校验块。

- [x] **Step 4: 删除取色结果和 command**

从 `special_ops/mod.rs` 删除：

- `LimitedSupplyColorSampleResult`。
- 完整 `special_ops_sample_limited_supply_color` 函数。

从 `src-tauri/src/lib.rs` 的 `generate_handler![]` 删除：

```rust
special_ops::special_ops_sample_limited_supply_color,
```

更新 contract 测试列表，不新增 ACL permission。

- [x] **Step 5: 验证 Rust 定向测试**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml legacy_color_sample_regions_are_ignored_and_not_reserialized
cargo test --manifest-path src-tauri/Cargo.toml limited_market_commands_are_registered_without_legacy_color_sampler
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: 全部 PASS。

### Task 4: 同步文档并执行完整门禁

**Files:**
- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `droid-wiki/features/special-ops.md`
- Modify: `docs/superpowers/plans/2026-08-09-special-ops-color-and-account-market.md`

- [x] **Step 1: 更新用户文档**

文档统一写明：

```text
限时商品颜色使用原生颜色面板，可用系统吸管或手动选色；9 个校准区域只用于 AnyPixel 识别。区域平均色吸取 command 已删除，不保存截图。
```

从 `AGENTS.md` 和 wiki 删除 `special_ops_sample_limited_supply_color`。在旧计划顶部增加：

```markdown
> 后续变更：区域平均色吸取方案已由 `2026-08-09-special-ops-native-color-picker.md` 替代；旧取色字段与 command 不再保留。
```

- [x] **Step 2: 扫描废弃符号**

Run:

```powershell
rg -n "colorSampleRegions|color_sample_regions|LimitedSupplyColorSampleResult|special_ops_sample_limited_supply_color|samplingLimitedColor|sampleLimitedColor|取色区域|限时商品识色区域校准" src src-tauri README.md AGENTS.md droid-wiki
```

Expected: 无结果。设计和历史计划中的说明不计入该扫描范围。

- [x] **Step 3: 运行前端构建与测试**

Run:

```powershell
bun run build
bun run test
```

Expected: PASS。

- [x] **Step 4: 运行完整门禁**

Run:

```powershell
bun run check
cargo check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Expected: 全部 exit code 0。

- [x] **Step 5: 刷新索引并复核工作区**

Run:

```powershell
codegraph sync
git diff --check
git status --short --branch
```

Expected: CodeGraph 同步成功；`git diff --check` 无空白错误；仅保留本轮及用户既有未提交改动，不创建 commit。
