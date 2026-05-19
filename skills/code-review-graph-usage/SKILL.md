---
name: code-review-graph-usage
description: "x-claw 仓库已挂载 code-review-graph MCP（6 个 repo 跨仓代码知识图）。本 skill 给出在重构/迁移工作中正确使用它的判定流程、常见坑位、以及什么时候回退到 grep。"
---

# code-review-graph 使用规范（x-claw 专版）

## 这个工具是干什么的

把代码用 Tree-sitter 解析成"节点+边"的图谱，存进 SQLite：

- **节点**：File / Class（含 enum/struct）/ Function / Test
- **边**：CALLS（函数调用）、CONTAINS（文件包含）、IMPORTS_FROM（模块依赖）、INHERITS、REFERENCES、TESTED_BY

适合回答："改了这个函数会影响哪些地方？"、"这个 trait 被谁实现？"、"两个 repo 哪些代码长得像？"

不适合直接回答："这个枚举被哪些文件用了？"（详见下文坑位 1）。

## 本仓已注册的 6 个 repo

| alias | 路径 | 用途 |
|---|---|---|
| `ironclaw` | `desktop-client/ironclaw/` | 桌面端主体（默认 repo） |
| `xclaw-core` | `crates/` | 所有 `dasclaw_*` 共享 crate |
| `ironclaw-main` | `ironclaw-main/` | 上游对照，迁移取舍证据来源 |
| `codex-cli` | `codex-cli-main/` | OpenAI codex 上游，verbatim port 的对照 |
| `claw-code` | `claw-code/` | claw-code 实现参考 |
| `claude-code` | `claude-code-main/` | Anthropic claude-code 实现参考 |

**默认 repo 是 ironclaw**（由 `.vscode/mcp.json` 里 `--repo` 指定）。绝大多数 MCP 工具调用都只看默认 repo。

## 什么时候应该用它（重构工作流）

每个有"搬迁"或"修改公共类型"性质的 PR，开工前先用图谱做：

1. **找调用方**：要搬一个 `pub fn` 之前，先 `query_graph_tool(pattern=callers_of, target=<函数名>)`
2. **估爆炸半径**：改动落盘后跑 `get_impact_radius_tool()` 看 blast radius
3. **找上游对照**：要决定"该不该合并某个分裂"时，用 `cross_repo_search_tool` 对照 `ironclaw-main` / `codex-cli` 看上游怎么处理
4. **审查上下文**：开 PR 前可用 `get_review_context_tool` 拉源码片段附在 PR 描述里

把这些产出贴进 PR 描述的"## 工具协助"段（PR #642 是范例）。

## 标准操作顺序（每次新会话第一次用之前）

```text
# 1. 看图谱状态
mcp_code-review-g_list_graph_stats_tool()
  → 看 last_updated 是不是今天；不是就刷新

# 2. 刷新默认 repo（ironclaw）
mcp_code-review-g_build_or_update_graph_tool()
  → 跑增量更新

# 3. 想刷新其他 repo（MCP 工具不接 repo 参数）—— 走 CLI
cd <repo 路径>
/Users/nallylin/.local/bin/code-review-graph update

# 4. 看当前 git 改动的影响范围
mcp_code-review-g_get_impact_radius_tool()
mcp_code-review-g_detect_changes_tool()
```

## 选用模式的判定表

| 想知道什么 | 用哪个工具/pattern | 备注 |
|---|---|---|
| 谁调用了某个**函数** | `query_graph_tool(pattern=callers_of, target=<函数 qualified_name 或简名>)` | 简名会得到 ambiguous + 候选列表，挑一个再查 |
| 某个**函数**调用了谁 | `query_graph_tool(pattern=callees_of, target=...)` | 同上 |
| 谁导入了某个**文件/模块** | `query_graph_tool(pattern=importers_of, target=<文件简名如 tools/tool.rs>)` | **绝对路径会返回 0；用简名拿 ambiguous + 候选** |
| 某个**类/枚举**被哪些地方用 | **fallback 到 `grep_search`** | 见下方坑位 1 |
| 某个 trait 被谁实现 | `query_graph_tool(pattern=inheritors_of, target=...)` | 仅 INHERITS 边 |
| 某个执行流被哪些代码触发 | `get_flow_tool` / `list_flows_tool` | |
| 跨 repo 找相似代码 | `cross_repo_search_tool` | 不限默认 repo |
| 改动后的代码风险评估 | `get_impact_radius_tool()` + `detect_changes_tool()` | 不接参数，自动读 git diff |
| PR 评审证据包 | `get_review_context_tool` | 输出可直接贴 PR |

## 常见坑位（实战发现）

### 坑位 1：`callers_of <Class>` 永远返回 0

**原因**：图里的 `CALLS` 边只指向 **Function**，不指向 Class/Enum/Struct。

**对策**：要查"谁用了某个枚举类型"，**回退到 `grep_search`**，搜 `use <模块路径>::<类型名>` 或 `<类型名>::`。这是 PR #642 工具协助表里采用的方法。

### 坑位 2：`importers_of` 用绝对路径返回 0

**对策**：传短路径（如 `tools/tool.rs`），拿到 ambiguous 候选列表后人肉挑 File 节点。

### 坑位 3：MCP 工具不接 `repo` / `alias` / `repo_path` 参数

报错样式：`unexpected_keyword_argument`。

**对策**：跨 repo 操作走 CLI（`/Users/nallylin/.local/bin/code-review-graph register|build|update`）；跨 repo 查询用 `cross_repo_search_tool`。

### 坑位 4：`semantic_search_nodes_tool` 找不到东西

**原因**：`list_graph_stats` 里 `Embeddings: 0 nodes embedded`，向量没生成。

**修复尝试**：`/Users/nallylin/.local/share/code-review-graph-venv/bin/pip install sentence-transformers`。**本仓当前装失败（依赖冲突）**，所以这条路暂时不能走，改用关键字版 `query_graph_tool` 即可。

### 坑位 5：图谱过期

**症状**：`detect_changes_tool` 返回"7 个改动文件，0 个改动函数"——节点没在图里。

**修复**：调 `build_or_update_graph_tool()` 增量刷一遍。本仓上次因 `tools/tool.rs` 改动后忘刷，就出现这个症状。

### 坑位 6：默认 repo 之外的 repo "看起来不存在"

例如：在默认 repo=`ironclaw` 时查 `crates/dasclaw_tool/` 里的符号，会找不到。

**对策**：那些节点在 `xclaw-core` repo 里，要用 `cross_repo_search_tool` 跨过去。

## 输出规范

在 PR 描述里贴"## 工具协助"段时，至少包含：

- 用了哪些 pattern 和 target
- 关键查询返回了什么（数量、是否 ambiguous、关键候选）
- 哪些查询触发了坑位，回退到了什么方法
- `get_impact_radius` 的 blast radius 数字

让 reviewer 不用重新跑一遍分析。

## 自检清单（开始一个新的搬迁 PR 之前）

- [ ] `list_graph_stats_tool` 看默认 repo 的 last_updated 是不是当天
- [ ] 必要时 `build_or_update_graph_tool` 刷新
- [ ] 跨 repo 任务先用 CLI 把相关 repo `update` 一遍
- [ ] 想查类型用方时，直接用 grep，不浪费时间在 `callers_of` 上
- [ ] PR 描述里准备好"## 工具协助"段

## 关联

- 工具来源：`.vscode/mcp.json` 第 41-44 行 stdio server 配置
- 二进制：`/Users/nallylin/.local/bin/code-review-graph`
- venv：`/Users/nallylin/.local/share/code-review-graph-venv/`
- 推荐配合：`code-review-expert`（语义评审）、`adr-compliance-check`（红线检查）、`code-quality-audit`（工艺审查）
