# Domain Docs

工程技能在探索代码库时，应如何消费本仓库的领域文档。

## 开始探索前，先阅读这些文件

- **`CONTEXT.md`** 位于仓库根目录；或
- **`CONTEXT-MAP.md`** 位于仓库根目录（如果存在）——它指向每个上下文对应的 `CONTEXT.md`。请阅读与主题相关的所有上下文。
- **`docs/adr/`** — 阅读与你即将工作区域相关的 ADR。在多上下文仓库中，还需检查 `src/<context>/docs/adr/` 中的上下文级决策。

如果以上文件不存在，**静默继续**。不要标记其缺失，也不要建议预先创建。生产者技能（`/grill-with-docs`）会在术语或决策实际被确定时惰性创建它们。

## 文件结构

单上下文仓库（大多数仓库）：

```
/
├── CONTEXT.md
├── docs/adr/
│   ├── 0001-event-sourced-orders.md
│   └── 0002-postgres-for-write-model.md
└── src/
```

多上下文仓库（根目录存在 `CONTEXT-MAP.md`）：

```
/
├── CONTEXT-MAP.md
├── docs/adr/                          ← 系统级决策
└── src/
    ├── ordering/
    │   ├── CONTEXT.md
    │   └── docs/adr/                  ← 上下文级决策
    └── billing/
        ├── CONTEXT.md
        └── docs/adr/
```

## 使用词汇表的术语

当你的输出命名一个领域概念（如 issue 标题、重构提案、假设、测试名称）时，使用 `CONTEXT.md` 中定义的术语。不要偏离词汇表明确避免的同义词。

如果你需要的概念尚未在词汇表中，这是一个信号——要么你在发明项目不使用的语言（请重新考虑），要么存在真实缺口（请记录给 `/grill-with-docs`）。

## 标记 ADR 冲突

如果你的输出与现有 ADR 矛盾，请显式指出，而不是静默覆盖：

> _与 ADR-0007（event-sourced orders）矛盾——但值得重新讨论，因为……_
