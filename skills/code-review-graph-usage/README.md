# code-review-graph-usage

x-claw 仓库已挂 6 个 repo 的代码知识图（code-review-graph MCP）。
本 skill 给出**在 dasclaw 重构/迁移任务中正确使用图谱的判定流程**，
踩过的坑都写在 `SKILL.md` 里。

## 何时读它

- 开一个会搬迁公共类型 / 拆 crate / 跨 repo 对照上游的 PR 之前
- 看到 `code-review-graph` 工具调用报错或返回 0 结果时（先来对一下坑位章节，再决定是回退到 grep 还是修工具）
- 写 PR "## 工具协助" 段时

## 关键产出

每个搬迁 PR 的描述必须贴"## 工具协助"段。本 skill 第"输出规范"节列出最小内容。
