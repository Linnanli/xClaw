---
name: code-review-graph-usage
description: "x-claw 使用 code-review-graph CLI（v2.3.2）来跨 6 个 repo 做代码知识图查询。本 skill 以 CLI 为主线、MCP 为补充，给出每个子命令的实测用法、典型输出、坑位修复。语义搜索（sentence-transformers）已在 2026-05 修好。"
---

# code-review-graph CLI 使用规范（x-claw 专版）

> 优先级：**CLI 用于构建/更新/跨 repo/CI**，**MCP 用于查询/语义搜索/refactor 交互**。
> CLI 没有 `query-graph`、`semantic-search`、`refactor` 子命令——那些只在 MCP 里。

## 环境（已实测可用）

| 项 | 路径 / 版本 |
|---|---|
| 二进制 | `/Users/nallylin/.local/bin/code-review-graph` |
| venv | `/Users/nallylin/.local/share/code-review-graph-venv/` |
| Python | 3.12.13（x86_64 macOS；**3.13 上 torch 无 wheel**） |
| 包版本 | code-review-graph 2.3.2 / torch 2.2.2 / sentence-transformers 5.5.1 / transformers 4.57.6 / numpy 1.26.4 / **rich 13.9.4** / **igraph 1.0.0** |
| 嵌入模型 | `all-MiniLM-L6-v2`（384 维，CPU，本地推理） |

**踩坑记录（已修）**：
- `transformers>=5.0` 有坏注解（`tuple[..., list[nn.Module]]`），会在 `import sentence_transformers` 时 `NameError`。锁 `transformers<5`。
- `numpy>=2.0` 与 torch 2.2 ABI 不兼容（`_ARRAY_API not found`）。锁 `numpy<2`。
- Python 3.13 + Intel macOS 没 torch wheel，venv 必须用 3.12。
- **`rich>=14`（含 15.0.0）移除了 `from rich import traceback` 入口**，工具内部 `import rich.traceback` 直接报 `ModuleNotFoundError: No module named 'rich.traceback'`，导致 `get_knowledge_gaps_tool` / `get_suggested_questions_tool` / `get_hub_nodes_tool` 等高层 MCP 工具全部不可用。锁 `rich<14`。
- **`igraph` 未安装时社区检测降级成 file-based**（按目录硬切，cohesion ~0.1，整个 `crates/` 会被合并成 1 个超大社区）。装 `igraph` 后走 Leiden 算法，实测从 10 个目录式 → **1761 个**真正基于耦合的社区。必装。

如果以后要重建 venv：

```bash
python3.12 -m venv /Users/nallylin/.local/share/code-review-graph-venv
/Users/nallylin/.local/share/code-review-graph-venv/bin/pip install \
  'code-review-graph==2.3.2' 'torch==2.2.*' \
  'sentence-transformers' 'transformers<5' 'numpy<2' \
  'rich>=13,<14' 'igraph'
```

## 8 个已注册 repo（`code-review-graph repos`）

```
  /Users/nallylin/Documents/code/x-claw/desktop-client/ironclaw  (ironclaw)         ← MCP 默认 repo
  /Users/nallylin/Documents/code/x-claw/desktop-client/src       (desktop-client-app)  Tauri Rust backend
  /Users/nallylin/Documents/code/x-claw/desktop-client/src-ui    (desktop-client-ui)   React TS/TSX 前端
  /Users/nallylin/Documents/code/x-claw/crates                   (xclaw-core)
  /Users/nallylin/Documents/code/x-claw/ironclaw-main            (ironclaw-main)
  /Users/nallylin/Documents/code/x-claw/codex-cli-main           (codex-cli)
  /Users/nallylin/Documents/code/x-claw/claw-code                (claw-code)
  /Users/nallylin/Documents/code/x-claw/claude-code-main         (claude-code)
```

`xclaw-core` 是所有 `dasclaw_*` crate 的图（截至 2026-05：6871 节点 / 45882 边 / 333 文件 / 6538 嵌入）。

`desktop-client-app` / `desktop-client-ui` 是 Tauri 桌面客户端两份分库（Rust backend + React 前端拆开），
避免跨语言合库拖低 Leiden 聊类质量。`ironclaw` 独立保留。

## CLI 子命令清单（全部实测）

`code-review-graph -h` 一共 13 个子命令：

| 子命令 | 干什么 | 典型场景 |
|---|---|---|
| `install` / `init` | 把 MCP server 注册到 Claude / Cursor / VS Code | 装机一次 |
| `register <path>` | 把一个仓加进 multi-repo registry | 接新 repo |
| `unregister <alias>` | 从 registry 摘除 | 清理 |
| `repos` | 列出已注册 repo | 排错时先看 |
| `build` | 全量重建图（重新解析所有文件） | 切大分支、首次 |
| `update` | 增量更新（仅改动文件） | 日常 |
| `postprocess` | 重跑后处理（flow / community / FTS） | 单跑某个分析 |
| `watch` | 监视改动自动 update | 长时间会话 |
| `status` | 显示图统计 + 当前 branch | 排错时第二步看 |
| `visualize` | 生成可交互 HTML 图（或 graphml/cypher/obsidian/svg） | 出图给人看 |
| `wiki` | 按 community 生成 markdown wiki | 出文档 |
| `eval` | 跑评估基准（token_efficiency / impact_accuracy 等） | 调优 |
| `detect-changes` | 分析 git diff 的影响 | PR 前自查 |
| `serve` | 启动 MCP server（stdio） | 给 Copilot 用 |

下面是每条的实测命令（在 x-claw 工作区跑过、能复现）。

### `repos` — 列出已注册 repo

```bash
$ code-review-graph repos
  /Users/nallylin/Documents/code/x-claw/desktop-client/ironclaw  (ironclaw)
  /Users/nallylin/Documents/code/x-claw/crates                   (xclaw-core)
  ...
```

无 flag，无输入。出问题先跑这条确认 registry 还在。

### `status` — 查图谱健康

```bash
$ code-review-graph status --repo /Users/nallylin/Documents/code/x-claw/crates
Nodes: 6871
Edges: 45882
Files: 333
Languages: rust, powershell
Last updated: 2026-05-25T14:47:40
Built on branch: feat/dasclaw-cli-mcp-wiring
Built at commit: d9090af3ad87
WARNING: Graph was built on 'feat/dasclaw-cli-mcp-wiring' but you are now on 'xClaw'.
         Run 'code-review-graph build' to rebuild.
```

**warning 是分支切换提示，不是错误**。日常 `update` 即可；切了大分支才 `build`。

### `build` / `update` / `postprocess` — 维护图

```bash
# 全量重建（含 postprocess）
code-review-graph build --repo <path>

# 仅签名+FTS，跳过 flow/community（快）
code-review-graph build --repo <path> --skip-flows

# 增量更新（最常用）
code-review-graph update --repo <path>

# 已有图，只跑 postprocess
code-review-graph postprocess --repo <path> [--no-flows | --no-communities | --no-fts]
```

实测 `update` 在 xclaw-core 上 3 个文件改动耗时 < 5s。

### `detect-changes` — git diff 影响分析

```bash
$ code-review-graph detect-changes --base origin/xClaw --brief \
    --repo /Users/nallylin/Documents/code/x-claw/crates
Analyzed 1 changed file(s):
  - 0 changed function(s)/class(es)
  - 0 affected flow(s)
  - 0 test gap(s)
  - Overall risk score: 0.00
```

- `--base` 默认 `HEAD~1`，要对 PR 评估用 `origin/<base-branch>`。
- `--brief` 出一行 summary；去掉得到逐文件 / 逐函数细表。
- 若回报 "0 changed function(s)" 但你确实改了函数 → 图过期，先 `update`。

### `visualize` — 出可交互图

```bash
# 生成 HTML 到 .crg/visualization.html 并本地开服
code-review-graph visualize --repo <path> --serve   # → http://localhost:8765

# 导出给 Obsidian
code-review-graph visualize --repo <path> --format obsidian
```

`--mode` 取 `auto|full|community|file`；大 repo 默认走 community 模式避免渲染卡死。

### `wiki` — 自动生成模块文档

```bash
code-review-graph wiki --repo <path>          # 仅刷新变更的 page
code-review-graph wiki --repo <path> --force  # 全量重生
```

输出落到 `<repo>/.crg/wiki/`。可以直接放到 PR 描述里展示模块结构。

### `eval` — 跑评估基准

```bash
code-review-graph eval --all --repo xclaw-core --report \
    --output-dir /tmp/crg-eval
```

5 个 benchmark：`token_efficiency`、`impact_accuracy`、`flow_completeness`、
`search_quality`、`build_performance`。**单跑十几分钟到一小时不等**，不要在 PR
默认 CI 里跑。

### `register` / `unregister` — 多仓注册

```bash
code-review-graph register --root <path> --alias <name>
code-review-graph unregister <alias>
```

注册后立刻跑一次 `build --repo <path>`。

### `watch` — 守护进程式增量

```bash
code-review-graph watch --repo <path>
```

后台跑，文件保存自动 `update`。开发会话期间挂着省手动 update 的功夫。

### `serve` — 给 Copilot/Claude 用的 MCP server

```bash
code-review-graph serve --repo /Users/nallylin/Documents/code/x-claw/desktop-client/ironclaw
```

stdio transport。`.vscode/mcp.json` 已配置，Copilot 启动时自动拉起。手动跑用来 debug。

### `install` / `init` — 给客户端写配置

把 MCP server entry 注入到 `~/.cursor/`、`~/.claude/` 等。**装机一次**就够了，
本仓已经装过，不要再跑。

## 语义搜索（CLI 没有，走 MCP 或 Python API）

CLI 没有 `semantic-search` 子命令。要做语义搜索：

**方式 A（推荐）—— MCP**：

```text
mcp_code-review-g_semantic_search_nodes_tool(query="LLM provider responder trait", limit=5)
```

需要先 `mcp_code-review-g_embed_graph_tool()` 把节点向量化（耗时 ~3-5 分钟 / 7k 节点 / CPU）。
本仓 xclaw-core 已经 embedded 6538 节点，可直接查。

**方式 B —— Python API（脚本场景）**：

```python
from code_review_graph.tools.docs import embed_graph
from code_review_graph.tools.query import semantic_search_nodes

# 一次性：建嵌入
embed_graph(repo_root="/Users/nallylin/Documents/code/x-claw/crates")

# 查询
result = semantic_search_nodes(
    query="LLM provider responder trait",
    limit=5,
    repo_root="/Users/nallylin/Documents/code/x-claw/crates",
    detail_level="minimal",
)
```

返回结构（截断）：

```json
{
  "results": [
    {"name": "ProviderKind", "kind": "Class",
     "file_path": ".../dasclaw_llm_provider/src/providers/mod.rs",
     "score": 0.0156},
    ...
  ]
}
```

`score` 是 RRF 融合分（FTS + 嵌入），rank 1 ≈ 1/61 ≈ 0.0164，按相对排序看而不是绝对阈值。

## CLI vs MCP —— 决策表

| 任务 | CLI | MCP |
|---|---|---|
| 全量 / 增量构图 | **是**（脚本化、CI 用） | 有 `build_or_update_graph_tool` 但锁默认 repo |
| 跨 repo 维护 | **是**（`--repo` 指定）| MCP 工具不接 repo 参数（坑位 3）|
| git diff 影响分析 | **是**（CI 调）| 有 `detect_changes_tool` 但只看默认 repo |
| 出图 / 出 wiki | **是** | 无 |
| 跑 benchmark | **是** | 无 |
| 模式查询（callers_of 等） | 无 | **是**（`query_graph_tool`）|
| 语义搜索 | 无 | **是**（`semantic_search_nodes_tool`）|
| Refactor 预演 | 无 | **是**（`refactor_tool`）|
| Review context / impact radius | 无 | **是** |
| 给 Copilot 用 | 不直接（用 `serve` 起 server）| **是**（默认通道）|

## 标准工作流（每次会话开始时）

```bash
# 1. 看 registry 还在
code-review-graph repos

# 2. 看当前 git 改动落在哪个 repo
# 默认 repo 是 ironclaw；若改动在 crates/、用 xclaw-core；若在 ironclaw-main/、对应 alias
git status -s | head

# 3. 刷该 repo
code-review-graph update --repo <对应 repo 的绝对路径>

# 4. 看图健康
code-review-graph status --repo <path>

# 5. PR 之前自查
code-review-graph detect-changes --base origin/xClaw --brief --repo <path>
```

## 坑位（实战）

### 坑位 1：MCP 工具不接 `repo` 参数

`mcp_code-review-g_query_graph_tool(..., repo_root=...)` 会报
`unexpected_keyword_argument`。MCP 锁默认 repo（ironclaw）。

**对策**：跨 repo 走 CLI，或用 `cross_repo_search_tool`（这个收 repo list）。

### 坑位 2：`callers_of <Class>` 永远返回 0

图里的 `CALLS` 边只接 Function。查"谁用了某个 enum/struct" → **回退 `grep_search`**。

### 坑位 3：`importers_of` 用绝对路径返回 0

传短路径（`tools/tool.rs`），拿到 ambiguous 候选后人肉挑 File 节点。

### 坑位 4：图过期，`detect-changes` 报 "0 改动函数"

跑 `code-review-graph update --repo <path>` 刷掉。

### 坑位 5：venv 装新依赖前先看 Python 版本

`.../code-review-graph-venv/bin/python -V` 必须是 3.12。装新包先 `pip install --dry-run`
看是否会触发 torch / transformers 升级；升级会重新触发坑位（见环境章节）。

### 坑位 6：`build_on_branch` warning ≠ 错误

切了分支会显示 "Graph was built on 'A' but you are now on 'B'"。日常 `update` 足够，
**只有在跨大功能分支（比如从 feat/* 切回 main）才需要 `build` 全量重建**。

### 坑位 7：高层 MCP 工具报 `No module named 'rich.traceback'`

症状：`get_knowledge_gaps_tool` / `get_suggested_questions_tool` / `get_hub_nodes_tool` 等
带 `rich` traceback 装饰的工具调用一次全报错。

原因：venv 里 `rich==15.0.0`（或任何 14+）已移除 `rich.traceback` 子模块。

修复：

```bash
/Users/nallylin/.local/share/code-review-graph-venv/bin/pip install 'rich>=13,<14'
# 然后 reload VS Code 或重启 MCP server 让进程重新加载 rich
```

### 坑位 8：社区粒度异常粗（按目录切）

症状：`list_communities_tool` 返回的社区只有 10 个左右，`description` 全是
`Directory-based community: <dir>`，单个社区动辄 1 万+ 节点，cohesion < 0.15。

原因：`igraph` 未安装，build/postprocess 日志会有一行
`INFO: igraph not available, using file-based community detection`。

修复：

```bash
/Users/nallylin/.local/share/code-review-graph-venv/bin/pip install igraph
code-review-graph postprocess --repo <path>
```

实测 x-claw 根 DB（25862 节点）跑 Leiden ~30s，输出 **1761 个**真社区，cohesion 显著上升。
**接新 repo 或重建 venv 后务必同时装 igraph**。

### 坑位 9：每个子目录都有 `.code-review-graph/graph.db` —— 是分库

实测 7 份独立 DB（不是想象中的全局一份）：

```
./.code-review-graph                        364M   (xClaw root，混语言)
./crates/.code-review-graph                  75M   (xclaw-core)
./desktop-client/ironclaw/.code-review-graph 176M  (ironclaw)
./ironclaw-main/.code-review-graph          272M
./codex-cli-main/.code-review-graph         464M
./claw-code/.code-review-graph               49M
./claude-code-main/.code-review-graph          0   (空，没建过)
```

**关键**：`build` / `update` 不带 `--repo` 时会 auto-detect 到 `.git` 根，写入根 DB，**不会写子库**。
要写子库必须显式 `--repo <绝对路径>`，例如：

```bash
code-review-graph build --repo /Users/nallylin/Documents/code/x-claw/crates
```

MCP 默认 repo 是 `ironclaw`（即 `desktop-client/ironclaw/.code-review-graph`），不是根 DB。
查 dasclaw_* crate 时要走 `xclaw-core` 别名或 CLI 显式 `--repo crates/`。

### 坑位 10：`register` 必须在 build 之后才能跑

症状：`code-review-graph register <path> --alias <name>` 报
`ERROR: Path does not look like a repository (no .git or .code-review-graph)`。

原因：`register` 权衡的是「路径里已有 .git 或 .code-review-graph」。子目录（没独立 .git）
首次接入时两者都没有。

修复：先 `code-review-graph build --repo <绝对路径>`（会创建 `.code-review-graph/`），再 `register`。
顺序不能反。

## Token 预算（保留上游纪律）

- 每轮 review/debug/refactor 任务 **≤5 次 graph 工具调用、≤800 输出 token**。
- MCP 工具能传 `detail_level` 的，**默认 `"minimal"`**。
- 先 `get_minimal_context(task="...")` 再开始查图。

## 选 Recipe 速查（不变，原 7 个 Recipe 见 SKILL.md.bak）

| 场景 | Recipe | CLI 起步命令 |
|---|---|---|
| 第一次接仓 / 图过期 | A build-graph | `code-review-graph build --repo <path>` |
| "我要搞懂这块代码" | B explore | `code-review-graph status` + MCP `get_architecture_overview_tool` |
| "我要改名 / 删死代码" | C refactor | MCP `refactor_tool` |
| "用户报了 bug 追源头" | D debug | MCP `query_graph_tool` + `get_flow_tool` |
| "刚改完代码自查" | E review-delta | `code-review-graph detect-changes` + MCP `get_review_context_tool` |
| "整段功能合并前过一遍" | F review-changes | 同 E + `get_impact_radius_tool` |
| "PR 之前出评审报告" | G review-pr | `update` → MCP `get_review_context_tool(base="origin/xClaw")` |

## 关联

- 老版本（MCP-first，含 7 个 Recipe 完整步骤）：`SKILL.md.bak`
- 上游 README：https://github.com/tirth8205/code-review-graph
- `.vscode/mcp.json` 第 41-44 行：MCP server 配置
- 二进制：`/Users/nallylin/.local/bin/code-review-graph`
- venv：`/Users/nallylin/.local/share/code-review-graph-venv/`
