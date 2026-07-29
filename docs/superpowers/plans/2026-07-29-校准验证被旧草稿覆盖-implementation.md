# 校准验证被旧草稿覆盖修复 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 阻止无未保存修改的 `flushSettings()` 把测试前旧草稿回写到后端，保留刚通过的制作校准验证签名。

**Architecture:** 保留现有 autosave queue、revision 和 Tauri commands。只在前端 `flushSettings()` 的 clean 分支返回当前 bootstrap；pending debounce 与 in-flight save 分支保持原行为。

**Tech Stack:** React 19、TypeScript、Vitest、Bun

**Execution constraint:** Inline Execution；不调用子代理，不创建 worktree。生产文件含历史修改，不整文件 stage，不创建混入历史改动的 commit。

---

### Task 1: clean flush 不再重复保存旧草稿

**Files:**
- Modify: `src/components/app/special-ops-page.test.tsx`
- Modify: `src/components/app/special-ops-page.tsx:283-299`

- [ ] **Step 1: 写失败回归测试**

在现有 source contract 测试中加入：

```ts
it("没有未保存修改时 flush 不重复保存旧草稿", () => {
    expect(pageSource).toContain("if (!settingsDirtyRef.current) return bootstrapRef.current;");
});
```

- [ ] **Step 2: 验证 RED**

Run:

```powershell
bunx vitest run src/components/app/special-ops-page.test.tsx -t "没有未保存修改时 flush 不重复保存旧草稿"
```

Expected: FAIL；当前 `flushSettings()` clean 分支仍调用 `enqueueSave`。

- [ ] **Step 3: 写最小实现**

在 `flushSettings()` 入口增加 clean fast path：

```ts
const flushSettings = async () => {
    if (!settingsDirtyRef.current) return bootstrapRef.current;
    // 原 pending timer / in-flight save / fallback save 分支保持不变
};
```

该判断必须位于清理 pending timer 前。`save()` 会在创建 pending request 时同步设置 `settingsDirtyRef.current = true`，因此真实编辑仍进入原保存路径。

- [ ] **Step 4: 验证 GREEN 与相关前端行为**

Run:

```powershell
bunx vitest run src/components/app/special-ops-page.test.tsx src/components/app/special-ops-utils.test.ts
bun run build
```

Expected: 全部 PASS。

- [ ] **Step 5: 更新索引并跑统一门禁**

Run:

```powershell
codegraph sync
git diff --check
bun run check
```

Expected: CodeGraph 同步成功；diff 无空白错误；TypeScript、Vitest、coverage、Rust fmt、Clippy 与 Rust tests 全部 PASS。

- [ ] **Step 6: 交付实机复测**

用户操作：测试 `craft.inProgress.pharmacy` 通过后立即启动制药台试运行。Expected：不再出现“尚未测试或验证失效”；后端保存的 `verifiedSignature` 保持非空。
