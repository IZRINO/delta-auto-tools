# Issue tracker: GitHub

本仓库的 Issues 和 PRD 使用 GitHub Issues 管理。所有操作通过 `gh` CLI 完成。

## 约定

- **创建 Issue**：`gh issue create --title "..." --body "..."`。多行正文使用 heredoc。
- **读取 Issue**：`gh issue view <number> --comments`，可用 `jq` 过滤评论和标签。
- **列出 Issue**：
  `gh issue list --state open --json number,title,body,labels,comments --jq '[.[] | {number, title, body, labels: [.labels[].name], comments: [.comments[].body]}]'`
  ，配合 `--label` 和 `--state` 过滤。
- **评论 Issue**：`gh issue comment <number> --body "..."`
- **添加 / 移除标签**：`gh issue edit <number> --add-label "..."` / `--remove-label "..."`
- **关闭 Issue**：`gh issue close <number> --comment "..."`

在仓库 clone 内运行 `gh` 时，它会自动通过 `git remote -v` 推断仓库。

## 当技能要求 "publish to the issue tracker" 时

创建一条 GitHub Issue。

## 当技能要求 "fetch the relevant ticket" 时

运行 `gh issue view <number> --comments`。
