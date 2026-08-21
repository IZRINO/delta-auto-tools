---
version: 1
slug: "src-blackmark-demo-tsx"
primary_target: "src/blackmark-demo.tsx"
related_targets: ["blackmark-demo.html","src/blackmark-demo.css"]
---

# Surface: 黑标演示

## Scope

主窗口第二条视觉线路的收口件。独立入口 `blackmark-demo.html`，不改生产 `App.tsx` 壳。游戏 overlay 不在此表面。

## Visitor mode

Operate。任务扫读优先于表演。

## Audience / job

作者式多账号值机。第一视口必须看见当前工具名、继续/暂停、到期业务。

## Direction

夜航黑标。BMW M 语法翻成操作台：纯黑（夜航）或浅灰（日间）、直角、Noto Sans SC 700/300、4px 三色条只做身份、白描边主按钮、底部居中悬浮图标 dock。

## Memorable moment

选中 dock 项白底（夜航）或黑底（日间）展开出字，顶缘压 3px 三色条。背景是碳纤加一道细展厅扫光，不是铺满蓝红。

## Constraints

- 不写入生产设置；夜航/日间只改演示页。
- 继续/暂停不得被特效或信息架构藏到第二击。
- 自定义 SVG 图标，dock 不用 remixicon。
- 读数 JetBrains Mono + tabular-nums。
- `prefers-reduced-motion` 关掉扫光、标题切开、规格格错开。

## Unresolved

接到设置切换、生产壳替换、全部工具页按黑标拓扑重排。在那之前以本演示为视觉权威。
