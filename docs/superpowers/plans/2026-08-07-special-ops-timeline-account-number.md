# 特勤处任务时间轴账号序号 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在未来 24 小时任务时间轴的 QQ 账号前显示按账号配置顺序生成的 `01/02/03` 序号。

**Architecture:** 前端从 `bootstrap.settings.accounts` 一次生成 `accountId -> 两位序号` 映射，任务行只读取映射。保持 Rust schema、调度和持久化不变。

**Tech Stack:** React 19、TypeScript、Vitest、Vite、Bun

---

### Task 1: 账号序号格式与时间轴接入

**Files:**
- Modify: `src/components/app/special-ops-page.tsx`
- Test: `src/components/app/special-ops-page.test.tsx`

- [ ] **Step 1: 写失败测试**

在页面源码测试中断言存在基于 `bootstrap.settings.accounts` 的 `Map`，并断言任务标题使用 `padStart(2, "0")` 生成的序号。

- [ ] **Step 2: 运行测试确认失败**

Run: `bunx vitest run src/components/app/special-ops-page.test.tsx --reporter=dot`

Expected: 新增断言失败，原因是账号序号映射尚未实现。

- [ ] **Step 3: 写最小实现**

在 `SpecialOpsTimeline` 中按账号数组顺序建立映射：

```tsx
const accountNumbers = new Map(
    bootstrap.settings.accounts.map((account, index) => [
        account.id,
        String(index + 1).padStart(2, "0"),
    ]),
);
```

任务标题改为：

```tsx
{accountNumbers.get(task.accountId) ?? "--"} 账号 {task.qqAccount || task.accountId}
```

- [ ] **Step 4: 运行定向测试与构建**

Run: `bunx vitest run src/components/app/special-ops-page.test.tsx --reporter=dot`

Expected: PASS。

Run: `bun run build`

Expected: TypeScript 与 Vite build 均通过。

- [ ] **Step 5: 检查差异并同步 CodeGraph**

Run: `git diff --check`

Expected: 无空白错误。

Run: `codegraph sync`

Expected: 索引同步成功。
