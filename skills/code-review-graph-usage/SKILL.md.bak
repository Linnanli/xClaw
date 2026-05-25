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

## Token-Efficiency 规则（来自上游 skill）

上游 `tirth8205/code-review-graph` 的 7 个官方 skill 都在 footer 强调同一组限额，本仓采用：

- **任何 graph 工具调用之前，先调一次 `get_minimal_context(task="<这一轮任务的一句话描述>")`**。
  服务端会按任务返回最小必要节点集，省下来回探查的 token。
- 所有 graph 工具能传 `detail_level` 的，默认填 `"minimal"`；只有 minimal 信息不够再升 `"standard"`。
- **每一轮 review / debug / refactor 任务的预算上限：≤5 次 graph 工具调用、≤800 输出 token**。
  超出时停下来切换策略（grep / 读源文件 / 跨 repo），不要在同一 pattern 上反复打。

## 上游 7 个标准工作流（recipes）

下面把上游 `skills/<name>/SKILL.md` 的步骤序列翻译成本仓可直接套用的脚本。每条都假设你已经
按"标准操作顺序"做完图谱 last_updated 校验。

### Recipe A — build-graph（首次或长时间未刷）

```text
1. list_graph_stats_tool()              # 看 last_updated；若 null → full rebuild
2. build_or_update_graph_tool(full_rebuild=True)   # 仅首次
   或 build_or_update_graph_tool()                 # 增量
3. list_graph_stats_tool()              # 复验：files / nodes / edges / languages
```

何时用：第一次接 repo、做了大重构 / 切大分支、`detect_changes` 报"0 个改动函数"
（即坑位 5）后。其他时间 hooks 自己会跑。

### Recipe B — explore-codebase（陌生区域摸底）

```text
1. get_minimal_context(task="explore <module/feature>")
2. list_graph_stats_tool()              # 总览
3. get_architecture_overview_tool()     # 社区/模块拓扑
4. list_communities_tool() → get_community_tool(<community_id>)
5. semantic_search_nodes_tool(<key term>)            # 嵌入可用时
   或 query_graph_tool(pattern="children_of", target=<file>)  # 嵌入不可用时
6. query_graph_tool(pattern="callers_of|callees_of|imports_of", target=...)
```

何时用：开始研究一个你不熟的子系统(e.g. mcp / sandbox / setup) 之前。
**先广后窄**（stats → arch → community → node）。

### Recipe C — refactor-safely（重命名 / 找 dead code）

```text
1. get_minimal_context(task="refactor <symbol>")
2. refactor_tool(mode="suggest")        # 社区驱动的拆分建议
3. refactor_tool(mode="dead_code")      # 找未引用代码
4. refactor_tool(mode="rename", from=<old>, to=<new>)
                                        # 只 preview，不落盘；返回 refactor_id 和 edit list
5. apply_refactor_tool(refactor_id=...) # 确认 edit list OK 后再应用
6. detect_changes_tool() + get_impact_radius_tool()   # 复验
```

**关键安全闸**：rename 必须先 preview 看完整 edit list 再 apply，不直接落盘；
大重构前用 `get_affected_flows_tool` 确认没碰到关键执行路径。

### Recipe D — debug-issue（追故障）

```text
1. get_minimal_context(task="debug <symptom>")
2. semantic_search_nodes_tool(<error message / fn name>)   # 嵌入可用时
3. query_graph_tool(pattern="callers_of"|"callees_of", target=<可疑函数>)
4. get_flow_tool(<entry point>)         # 完整执行路径
5. detect_changes_tool()                # 最近 commits 有没有改这块
6. get_impact_radius_tool()             # 评估波及面
```

**经验**：新 bug 90% 是最近 commits 引入的，先看 `detect_changes` 再看 flow。

### Recipe E — review-delta（"提交以来"的快速增量评审）

```text
1. build_or_update_graph_tool()         # 先刷
2. get_review_context_tool()            # 自动读 git diff → 返回:
                                        #   - 改动文件
                                        #   - impacted_nodes / impacted_files (blast radius)
                                        #   - 源码片段
                                        #   - 评审提示(测试缺口、宽影响、继承变更)
3. 对每个 impacted_node:
   - 看片段 → 找 bug / style
   - query_graph_tool(pattern="tests_for", target=<func>)   # 测试覆盖
4. 标出无测试的改动 + 高影响 caller
```

**优势**：只发 changed + 2-hop 邻居进上下文,token 比全 repo 评审低 5–10 倍。

### Recipe F — review-changes（带风险评分的全面评审）

```text
1. detect_changes_tool()                # 风险评分（high / medium / low）
2. get_affected_flows_tool()            # 影响的执行路径
3. 对每个 high-risk 函数:
     query_graph_tool(pattern="tests_for", target=<func>)
4. get_impact_radius_tool()
5. 按风险等级汇报:
     - 改了什么 / 为什么要紧
     - 测试覆盖状态
     - 改进建议
     - 合并建议（merge / hold / revert）
```

### Recipe G — review-pr（PR / branch 整体评审,贴进 PR 描述）

```text
1. 确定 PR/branch:  git diff main...<branch>
2. build_or_update_graph_tool(base="main")
3. get_review_context_tool(base="main")          # 返回所有 commits 改动总和
4. get_impact_radius_tool(base="main")           # PR 整体 blast radius
5. 逐文件深挖（高 impact 优先）:
     - 读完整源码（含本 PR 之外的上下文）
     - query_graph_tool(pattern="callers_of", target=<高风险函数>)
     - query_graph_tool(pattern="tests_for", target=<func>)
     - 检查公共 API 是否有破坏性变更
6. 按下面模板出报告:
```

PR 评审输出模板（可直接贴 PR 评论 / "## 工具协助"段）：

```text
## PR Review: <title>

### Summary
<1–3 句概览>

### Risk Assessment
- Overall risk: Low / Medium / High
- Blast radius: X files, Y functions impacted
- Test coverage: N changed functions covered / M total

### File-by-File Review
#### <file_path>
- Changes: <一句话>
- Impact: <谁依赖它>
- Issues: <bug / style / 顾虑>

### Missing Tests
- <function_name> in <file> — 未覆盖

### Recommendations
1. <可执行的具体建议>
2. <可执行的具体建议>
```

**大 PR 建议**：先看 impact 最高（最多 dependent）的文件;用 `semantic_search_nodes`
找可能漏改的相关代码;确认 rename/move 的函数所有 caller 都更新了。

## 选 Recipe 的速查

| 场景 | Recipe |
|---|---|
| 第一次接仓 / 图过期 | A (build-graph) |
| "我要搞懂这块代码" | B (explore-codebase) |
| "我要改名 / 删死代码" | C (refactor-safely) |
| "用户报了个 bug,我要追源头" | D (debug-issue) |
| "刚改完代码自查一下" | E (review-delta) |
| "整段功能合并前最后过一遍" | F (review-changes) |
| "开 PR 之前出评审报告" | G (review-pr) |

## 关联

- 工具来源：`.vscode/mcp.json` 第 41-44 行 stdio server 配置
- 二进制：`/Users/nallylin/.local/bin/code-review-graph`
- venv：`/Users/nallylin/.local/share/code-review-graph-venv/`
- 上游 7 个 skill 原文：`github.com/tirth8205/code-review-graph/tree/main/skills/{build-graph,debug-issue,explore-codebase,refactor-safely,review-changes,review-delta,review-pr}/SKILL.md`
- 推荐配合：`code-review-expert`（语义评审）、`adr-compliance-check`（红线检查）、`code-quality-audit`（工艺审查）
