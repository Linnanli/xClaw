---
name: hybrid-code-retrieval
description: "开发期混合代码检索工作流。用于融合 LSP、code-review-graph、Graphify、semantic_search 与 grep，解决符号定位、引用追踪、影响面分析、跨仓语义探索与否定性结论核验。触发词：混合检索、代码检索、引用追踪、影响面分析、跨仓搜索、找不到定义、这个函数在哪里被调用。"
argument-hint: "描述你的检索目标：符号定位 / 影响面 / 跨仓概念 / 否定性结论"
---

# Hybrid Code Retrieval

## 目标

在当前项目中提供一套"不过度堆叠能力"的开发期检索流程：
- 优先用最短路径得到可靠答案
- 避免同一问题在多个工具上重复试错
- 让"否定性结论"有可审计证据链

## 何时使用

当用户出现以下诉求时使用：
- "这个符号定义在哪里 / 被谁调用"
- "这次改动会影响哪些模块"
- "跨仓库看这个能力在哪实现"
- "确认 X 有没有 Y（缺失/独家/不存在）"
- "语义能找到但代码里定位不准"

## 分层原则（固定顺序）

1. **LSP 层（精确符号）**：`vscode_listCodeUsages` / `vscode_renameSymbol`
2. **结构图谱层（影响面）**：code-review-graph（query/callers/callees/importers）
3. **语义图谱层（跨仓语义）**：Graphify（`query` / `path` / `explain`）
4. **工作区语义层（快速补充）**：`semantic_search`
5. **字面量层（兜底）**：`grep_search`，必要时终端 `grep`

不要跳步并行乱试；按层推进，上一层足够回答时立即停止。

## Quick Path（2-5 分钟）

用于日常开发中的高频、小范围检索，不替代全流程。

1. 先走 **LSP 或 code-review-graph**（按问题类型二选一）：
   - 符号定义/引用问题：LSP
   - 影响面/调用链问题：code-review-graph
2. 若证据不足：补 `semantic_search`（1 轮同义词）
3. 仍不足：`grep_search` / `grep` 做字面量锚定
4. 仅当满足 Graphify 升级条件时，才进入 Graphify

完成标准：拿到至少 2 条一致证据（定义/引用/调用链/字面量任意组合）。

### Quick Path 硬规则

1. **工具调用预算默认 ≤ 3 次**（不含必要的单次重试）。
2. 若超过 3 次，必须显式升级为全流程，并写明升级理由（证据冲突/证据不足/跨仓需求）。
3. 未写升级理由时，禁止继续追加工具调用。

## 决策路由

### A. 精确符号问题（定义/引用/重命名）

触发条件：用户给出明确函数名、类型名、变量名、导入名。

流程：
1. 先用 LSP 工具查 usages/definition。
2. 若 LSP 命中为空：
   - 检查符号拼写、文件语言服务是否可用。
   - 回退到 `grep_search` 定位字面量，再回到 LSP 二次确认。
3. 只有在需要跨模块影响面时，才进入结构图谱层。

### B. 影响面/调用链问题

触发条件："改这个会影响哪里"、"调用链是什么"。

流程：
1. 优先用 code-review-graph 获取 callers/callees/importers。
2. 用 LSP 对关键符号做 spot-check（抽样核验）。
3. 若图谱结果与 LSP 冲突，以 LSP 的当前工作区语义为准，并标注冲突。

### C. 跨仓概念问题（代码 + 文档/设计）

触发条件：跨 `crates/`、`desktop-client/`、参考库的概念检索。

流程：
1. 先用 code-review-graph 定位结构候选。
2. 再用 Graphify 做跨仓语义扩展（query/path/explain）。
3. 结论落地前必须追加字面量确认（grep_search/grep）。

### C2. Graphify 升级条件（单仓问题）

单仓问题默认不使用 Graphify。仅在以下条件满足时升级：
1. LSP + code-review-graph + semantic_search 已执行，但结论冲突或证据不足。
2. 需要代码与非代码资产（文档/说明/设计）联合解释。
3. 用户明确要求语义图谱视角。

不满足以上任一条件时，禁止提前进入 Graphify。

### C2 证据阈值硬规则

对“结论冲突或证据不足”做量化，必须满足以下之一：
1. **冲突阈值**：LSP / code-review-graph / semantic_search 三层中，至少 2 层给出互相矛盾结论。
2. **不足阈值**：执行至前三层后，仍无法产出 ≥ 2 条可核验代码证据（定义/引用/调用链/字面量）。

只有达到上述阈值，单仓问题才允许升级 Graphify。

### D. 否定性结论（X 没有 Y / 缺 Y / 独家）

必须执行三层验证（不可省略）：
1. `semantic_search`（至少 1 组同义查询）
2. `vscode_listCodeUsages`（符号级核验）
3. `grep_search` 或 `grep`（字面量核验）

任一层有反例即撤回否定性结论。

## 执行模板

每次检索输出保持以下结构：

1. **问题类型**：A/B/C/D（路由理由一句话）
2. **已执行层级**：列工具与关键查询
3. **证据**：
   - LSP 证据（definition/reference）
   - 图谱证据（callers/callees/path）
   - 字面量证据（命中关键词/文件）
4. **结论与置信度**：高 / 中 / 低
5. **下一步建议**：仅在证据不足时给出

## 完成判定（Done）

满足以下全部条件才可结束：
- 已按路由执行到"足够回答"的最小层级
- 输出至少 2 条可核验代码证据
- 若包含否定性结论，已完成三层验证
- 工具冲突已显式说明并给出取舍理由

## 反模式

- 同一问题一次性堆满所有工具（无路由）
- 只有语义检索命中就下结论
- 把 Graphify 当作本地符号定义查询的首选
- 单仓问题在未经过前三层前提前使用 Graphify
- 在 LSP 可用时跳过符号级核验
- 未做字面量核验就写"不存在/缺失"

## 参考

- 项目规则：[AGENTS.md](../../../AGENTS.md)
- 结构图谱实践：[skills/code-review-graph-usage/SKILL.md](../../../skills/code-review-graph-usage/SKILL.md)
- Graphify 官方技能路径：`~/.copilot/skills/graphify/SKILL.md`

## 与现有 Skill 的关系

建议写入并保持该引用：`code-review-graph-usage`。

边界约定：
1. 本 skill 负责**检索路由与层级决策**（先用谁、何时升级、何时停止）。
2. `code-review-graph-usage` 负责**code-review-graph 细节实操**（命令参数、坑位、版本兼容、仓库别名）。
3. 进入结构图谱层（B/C 路由）时，若涉及 code-review-graph CLI 细节，优先按 `code-review-graph-usage` 执行，不在本 skill 重复维护同类说明。

不建议在本 skill 复制 `code-review-graph-usage` 的长命令和坑位清单，避免双份文档漂移。

## Graphify 使用策略（硬规则）

1. 进入 Graphify 层时，**优先使用 Graphify skill（`/graphify`）** 作为统一入口。
2. 仅当 skill 不可用或执行失败时，才回退到 Graphify CLI。
3. 无论使用 skill 还是 CLI，均需在输出中记录：触发原因、查询语句、关键证据。

## 示例提示词

- "用混合检索帮我确认 SetupWizard 在 desktop-client 里的调用链。"
- "判断 codex 路线是否已有和这个需求等价的实现，按三层验证给证据。"
- "先用最短路径查 useModelConfig 的真实影响面，不要全工具乱搜。"
