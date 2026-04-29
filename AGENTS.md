# Agent Rules

## 语言规则

**所有回答必须用中文。**

## 沟通规则

1. **不要假设用户清楚自己想要什么。** 当动机或目标不清晰时，停下来讨论，而不是猜测着往前冲。做错了再改的成本远高于多问一句。
2. **目标清晰但路径不是最短的，直接说并建议更好的办法。** 用户可能因为惯性选择了次优方案，AI 有责任指出更短的路径——但最终决定权在用户。

### 反馈原则

1. **开发完成后调用 MCP 工具**：
   - 每次代码开发完成后，必须调用 `mcp-feedback-enhanced` 工具。
   - 每次给开发者解释方案或者让开发选择方案，必须调用 `mcp-feedback-enhanced` 工具。
   - 工具调用后，Agent 必须等待用户反馈，收到明确指示后再继续执行后续任务。

### 会话轮换原则

**核心标准：不要用“1 个 PR”判断会话是否该结束；以“闭环 milestone”判断。**

#### 什么算一个闭环 milestone

满足以下 3 条中的至少 2 条，即视为完成一个阶段，可建议新开会话：
- 一个明确决策已定稿（如 ADR 从 draft 变 accepted，或关键方案被用户确认）
- 一段实现已落地并完成验证（build / test / lint / smoke 至少一类可执行验证通过）
- 产出已沉淀为可交接状态（PR、ADR、handoff、研究笔记、完成报告）

#### 何时必须提醒用户考虑新会话

- 同一会话内已经完成 1 个以上闭环 milestone，且准备进入下一个阶段
- 已完成 2 个以上非平凡 PR 或 stacked PR 切片，准备继续下一个子波次
- 已经写过 handoff、conversation summary，或已经明显依赖摘要态上下文推进工作
- 任务模式即将切换：例如从“实现/调试”切到“架构对账/研究/文档”，或反过来
- 出现 token budget、context compaction、summary 接管之类信号

#### 提醒动作（固定流程）

1. 先把当前状态写入 `/memories/session/<topic>-handoff.md`
2. 调用 `resolve_memory_file_uri` 拿到该文件的**完整 file:// URI**
3. 把完整 URI 同步登记到本文件下方的 [Session Handoff Index](#session-handoff-index) 表格中
4. 明确说明"当前已完成的闭环 milestone 是什么"
5. 给用户的下一会话开局 prompt 中**必须包含完整 URI**（不是 `/memories/session/...` 短路径），因为新会话的 agent 不一定能解析短路径

#### Session Handoff Index

> 当前活跃的 handoff 文件清单。完成下一会话或废弃后，把对应行移到本表下方的"已归档"区。
>
> 每条目格式：`日期 | 主题 | 完整 file:// URI | 下一会话开局 prompt 摘要`

| 日期 | 主题 | 完整 URI | 开局 prompt |
|---|---|---|---|
| 2026-04-26 | W3-A 契合度评估方案（Step 1 待做） | `file:///Users/nallylin/Library/Application%20Support/Code/User/workspaceStorage/185633b60e9bb925751de80a50ffce63/GitHub.copilot-chat/memory-tool/memories/MTg0ZDhhNTctOWQ0Zi00N2I1LTlmODEtMTFjMTdmZTgyY2Jl/w3a-compatibility-evaluation-handoff.md` | 读该 URI 后做 Step 1：整理 `docs/plans/architecture-refactor/adr-112-input-checklist.md` |
| 2026-04-28 | W3-A Step 3 ADR 主体撰写（Step 1 已闭环） | `file:///Users/nallylin/Library/Application%20Support/Code/User/workspaceStorage/185633b60e9bb925751de80a50ffce63/GitHub.copilot-chat/memory-tool/memories/YTkxYmVkNDItYzI3MS00MWIyLTgxMmYtZjg4OGMyZTc5NjEz/w3a-step3-adr-handoff.md` | 读该 URI 后做 Step 3：撰写 `docs/plans/architecture-refactor/adr-112-compatibility-evaluation.md`（按 9 章节骨架 + 14×3 评分卡 + Phase 0 10 行 + 16 条偏离声明 + 4 KPI + CI 三档） |
| 2026-04-28 | Phase 0 P0-1 Prompt 装配三连收口（W3-A Step 1+3 双闭环） | `file:///Users/nallylin/Library/Application%20Support/Code/User/workspaceStorage/185633b60e9bb925751de80a50ffce63/GitHub.copilot-chat/memory-tool/memories/YTkxYmVkNDItYzI3MS00MWIyLTgxMmYtZjg4OGMyZTc5NjEz/phase0-p01-prompt-handoff.md` | 读该 URI 后做 P0-1：TDD 红测 → 合并 ironclaw 3 builder 为 LayeredPromptBuilder → 删 IRONCLAW_PROMPT_LAYERING env → 统一 PROMPT_CACHE_BOUNDARY 常量 → nextest+build 全绿 → 三 skill 自审 → 开 PR |
| 2026-04-29 | P0-1 完成报告（PR #39 已开，CI 主要项 pass，闭环） | `file:///Users/nallylin/Library/Application%20Support/Code/User/workspaceStorage/185633b60e9bb925751de80a50ffce63/GitHub.copilot-chat/memory-tool/memories/YzU4NDZiZjctNzUxMy00NDBkLTliNWYtNTI4MTFiOTA3YmRk/p01-prompt-unification-completion.md` | P0-1 已闭环（PR #39）。下一会话可直接进入 P0-2 Tool bootstrap_tools()，读 ADR-112 §5 + input-checklist 即可，无需读本 handoff |
| 2026-04-30 | P0-2 PR #3 — bootstrap_tools() 实现 + register_*_tools 私有化（PR #36/39/40/41/42 已合并 to xClaw） | `file:///Users/nallylin/Library/Application%20Support/Code/User/workspaceStorage/185633b60e9bb925751de80a50ffce63/GitHub.copilot-chat/memory-tool/memories/YzU4NDZiZjctNzUxMy00NDBkLTliNWYtNTI4MTFiOTA3YmRk/p02-pr3-bootstrap-impl-handoff.md` | 读该 URI + p02-bootstrap-tools-design.md §3.2 → TDD 红测（5 个 req_p02_pr3_*）→ 替换 9 marker trait 为具体类型 → 实现 bootstrap_tools 按 mode + 字段分发 → 私有化 12 register → cargo nextest + build 全绿 → 三 skill 自审 → 开 PR base=xClaw |

##### 已归档

（暂无）

#### 注意

- **单个 PR 默认不是阶段边界**；PR 可能只是为了 reviewability 被拆小
- stacked PR 以“该组 PR 是否完成一个闭环 milestone”来判断，而不是以最前面那个 PR 是否已开来判断
- 如果只是一个很小的 typo / 文档修订 / lint 修复，不要机械地建议新会话
- 会话轮换属于**提醒机制，不是强制中断机制**：完成提醒动作后，若用户明确要求继续在当前会话推进，Agent 应继续执行任务。

### Skills 强制使用规范

每个开发回合都要主动核对是否触发 skill，不要靠"想起来"才用。以下三个 skill 是默认触发，**不是可选**：

| Skill | 触发时机 | 缺省后果 |
|-------|---------|---------|
| `code-quality-audit` | **写完一段非平凡实现后**（新函数/新模块/重构 > 50 LOC）、commit/push 前、用户说"审查/检查质量" | 会把补丁式代码、过长函数、unwrap/clone 滥用、重复造轮子的隐患合并进主线 |
| `code-simplifier` | **`code-quality-audit` 之后**，对刚改完的代码做收敛（消嵌套/去重复/改命名） | 留下啰嗦/低可读代码，后续重构成本飙升 |
| `code-review-expert` | **PR 自审前**（push 之前）、PR 合并前、用户说"review/审查这次改动" | 漏掉 SOLID 违规、安全风险、依赖耦合等高阶问题 |

**执行顺序（默认管线）**：
1. 实现完成 → `cargo build` 0 错误 0 警告
2. `code-quality-audit` 自查（必须）
3. `code-simplifier` 收敛（必须）
4. `cargo fmt` + `python3.12 scripts/check_no_panics.py`
5. commit 前 `code-review-expert` 自审（必须）
6. push + 开 PR

**不需要执行 skill 的场景**：
- 单字符/单行 typo 修复
- 仅文档/注释修改
- 仅 fmt/lint 自动修复
- 用户明确说"跳过 review，先跑起来"

**遗忘检测**：每次准备 push 前，先问自己"刚才有没有跑过 code-quality-audit 和 code-review-expert？"，如果没有，回到对应步骤补做。

### GitHub PR 工作流

#### 默认原则

- **先完成局部闭环，再开 PR**：实现、验证、必要的自审都完成后再提交，不要把“未验证草稿”直接推成 PR
- **PR 要小而完整**：优先拆成可 review 的小 PR，但每个 PR 都必须有明确边界、验证结果和 merge 顺序
- **能 stacked 就 stacked，不要把无关改动塞进同一个 PR**

#### 标准流程

1. 完成当前切片实现
2. 运行最小充分验证：先窄测试，再必要的 build / workspace build
3. 按 Skills 管线完成 `code-quality-audit` → `code-simplifier` → `code-review-expert`（适用时）
4. commit，commit message 说明当前切片的真实边界
5. push 分支
6. 开 PR，并在 PR 描述中写清：
    - 这个 PR 做了什么
    - 明确没做什么
    - 验证命令和结果
    - 风险 / 后续步骤
7. 如果是 stacked PR，必须额外写清：
    - base 分支不是 `xClaw` 而是上一个 feature branch
    - merge 顺序
    - “请先 merge #X，再看本 PR”
8. 开完 PR 后，用 `gh pr checks` 或等价方式至少看一轮状态，并把结果同步给用户
9. 若当前闭环 milestone 已完成，按“会话轮换原则”判断是否该建议新会话

#### PR 描述最低要求

- 背景 / 目标
- 改动范围
- 非目标（What’s NOT in this PR）
- 验证
- 后续 PR / 下一步

#### 禁止事项

- ❌ 还没跑基本验证就开 PR
- ❌ stacked PR 不写 base / merge 顺序
- ❌ PR 标题和 commit / 实际改动边界不一致
- ❌ 一个 PR 混入多个互不相干的主题
- ❌ 明明已经完成闭环 milestone，却继续在同一会话无限追加新阶段

### 分析工具使用规范（Round 17 建立 / Round 18 实证）

**重大架构决策/能力盘点/跨项目对比前，必须按三级顺序使用工具**：

| 级别 | 工具 | 使用场景 |
|------|------|---------|
| Level 1 语义层 | `semantic_search` | 概念搜索（跨命名等价实现），**必须先用** |
| Level 2 符号层 | `vscode_listCodeUsages` | LSP 引用/定义/实现图，核验判断 |
| Level 3 字面量层 | `rg` / `grep` | 已知确切词后再用 |

**反模式（禁止）**：
- ❌ 跳过 Level 1，直接 `rg <英文词>` 找概念 — 漏掉异名等价实现
- ❌ 判定"独家/缺失"时只看单一 repo — 必须三方交叉验证
- ❌ 未读解构文档（如 `decode-claude-code-main/`）就给架构结论
- ❌ 写架构对账文档时没有先按本规范核验能力表每一格 — 会把主观猜测当结论

**执行流程（最低标准）**：
1. 先用 `semantic_search` 搜概念（≥ 2 种语义表达）
2. 若有符号级疑问，用 `vscode_listCodeUsages` 打引用图
3. 最后才用 `rg` 定位确切位置
4. 对于"X 没有 Y"这类否定性结论，**必须明确给出 Level 1 + Level 3 双证据**才能落笔

**触发判断表**（什么场景必须做三层验证）：

| 场景 | 是否必须做三层验证 |
|------|---------------------|
| 否定性结论："X 没有 Y" / "X 是独家的" / "缺失 Y" | ✅ 必须 |
| 新增模块 / 新建 crate / 新建文件 | ✅ 必须（先 semantic_search 是否已有等价） |
| 跨项目对账（codex / claw-code / ironclaw 能力对比） | ✅ 必须 |
| 架构对账文档（如 14-md） | ✅ 必须（Round 17/18 实证错误率 4/19） |
| 已知模块内 bug 修复 | ❌ 不需要 |
| 仅文档 / 格式化 / lint 修改 | ❌ 不需要 |
| 已确定模块内 feature 扩展（不涉及"是否复用"） | ⚠️ 可选 |
| commit message 写"已检查 X 是否已有，结论：…" | ✅ 推荐（过程透明度） |

**教训来源**：
- Round 1-15 多次误判（"codex 没 forkSubagent"、"ironclaw Prompt Cache 独家"）的根因均为字面量搜索陷阱。
- **Round 18 实证**：14 文档 Round 17 版本列出的 4 项 P0/P1 "缺口"（`<system-reminder>` 标签 / CYBER_RISK_INSTRUCTION 文本 / 多层 CLAUDE.md 加载 / 压缩阈值），经 `semantic_search` 验证全部是**伪缺口** —— claw-code `runtime/src/prompt.rs:480` 与 `prompt.rs:197`、codex `openai_models.rs:306` 早已实现。4/19 的文档错误率直接证明：**不做 semantic_search 就动笔写对账文档是不合格的**。

---


## 代码图谱与打包工具（2026-04-29 启用）

### Repomix 核心模块打包

预先生成的 LLM 友好打包（位于 `docs/repomix-out/`，已 gitignore）：

| 文件 | 模块 | 文件数 | tokens |
|---|---|---|---|
| `codex-rs-core.md` | codex-cli-main 内核 (core/tools/protocol/mcp-server/exec/sandboxing/...) | 602 | 947K |
| `claw-code-core.md` | claw-code (rust/+src/) | 70 | 274K |
| `ironclaw-main-core.md` | ironclaw-main (src/+crates/) | 522 | 1.78M |
| `desktop-ironclaw-core.md` | desktop-client/ironclaw (src/+crates/) | 385 | 1.11M |

重新生成命令模板（注意根 `.gitignore` 含 `/ironclaw-main/`，需 `--no-gitignore`）：
```bash
repomix --compress --style markdown --no-gitignore \
  --include "src/**/*.rs,crates/**/*.rs" --ignore "**/target/**" \
  -o /Users/nallylin/Documents/code/x-claw/docs/repomix-out/<name>.md
```

MCP 接入：`.vscode/mcp.json` 已添加 `repomix` server (`npx -y repomix --mcp`)。

**何时使用 Repomix**（适合「全景概览 / 一次性灌入 LLM」）：
- 接手陌生模块前先读一遍打包文件，比逐个 `read_file` 快
- 跨 crate 重构前先核对模块边界与公共 API
- 让外部 LLM（无 IDE 工具链）也能理解代码：把 `.md` 直接拷给它
- 写 ADR / 设计文档前盘点既有实现
- ⚠️ **不适合**：单个符号查询、增量改动追踪、行级精确定位 → 改用 `grep_search` 或 code-review-graph

### code-review-graph 增量图谱

CLI: `/Users/nallylin/.local/bin/code-review-graph`（多 repo registry 已注册 4 个）：

| alias | path | nodes | edges | files |
|---|---|---|---|---|
| `ironclaw` | `desktop-client/ironclaw` | 20519 | 196690 | 1116 |
| `codex-cli` | `codex-cli-main` | 34382 | 336693 | 2273 |
| `claw-code` | `claw-code` | 4522 | 37559 | 156 |
| `ironclaw-main` | `ironclaw-main` | 23674 | 220750 | 846 |

常用命令：
- `code-review-graph repos` — 列出已注册 repo
- `code-review-graph build --repo <path>` — 全量重建
- `code-review-graph update --repo <path>` — 增量更新
- `code-review-graph watch --repo <path>` — 自动增量（**仅 desktop-client/ironclaw 启用**）
- `code-review-graph detect-changes --base HEAD~N [--brief]` — impact radius 报告
- `code-review-graph status` — 图谱统计

MCP 接入：`.vscode/mcp.json` 中 `code-review-graph` server 已指向 `desktop-client/ironclaw`，提供 `mcp_code-review-g_*` 工具集（impact_radius / affected_flows / review_context / semantic_search_nodes / community / minimal_context 等）。

注意事项：
- `register` 要求路径下有 `.git` 或 `.code-review-graph`；首次对子目录用 `build` 自动创建后再 register
- `watch` 是后台守护进程，改文件即触发增量；不要并发对同一 repo 跑 build/update（共享 SQLite）
- `detect-changes` 风险评分: ≥0.7 高风险，需重点 review；untested 列表标记缺测试

**何时使用 code-review-graph**（适合「精确定位 / 影响面分析」）：
- PR review 前评估改动影响半径：`detect-changes --base origin/xClaw` → 看 impact_radius 与 untested
- 重构前查调用方/被调用方：`mcp_code-review-g_impact_radius` / `affected_flows`
- 找语义相近的实现避免重复造轮子：`mcp_code-review-g_semantic_search_nodes`
- 给 LLM 提供「最小可读上下文」而非整文件：`mcp_code-review-g_minimal_context`
- 探索代码社群结构 / 模块耦合：`mcp_code-review-g_community`
- ⚠️ **不适合**：纯文档变更（图谱不索引）、模块全貌讲解 → 改用 Repomix 或 `read_file`

### 工具选型速查

| 任务 | 首选工具 |
|---|---|
| 「这个 crate 大概做什么 / 给我一份概览」 | Repomix `*-core.md` |
| 「改了 X，会波及哪些测试 / 调用方」 | code-review-graph `detect-changes` / `impact_radius` |
| 「找一个名字叫 XXX 的函数」 | `grep_search` / `file_search` |
| 「找一个『大概是这意思』的实现」 | code-review-graph `semantic_search_nodes` |
| 「贴给外部 LLM 让它出方案」 | Repomix 打包 |
| 「PR 评审清单 / 风险打分」 | code-review-graph `detect-changes --brief` |

## Rust 开发规范

### 开发流程（TDD）

新增 Rust 函数/模块/结构体/API 端点时（不含最小 bug 修复）：
1. 编写测试用例（Red）
2. 实现功能代码（Green）
3. 重构优化（Refactor）
4. `cargo build` 完整编译验证（不只是 `cargo test`）
5. 0 编译错误，0 编译警告

### 测试运行器：默认使用 nextest

**项目默认用 `cargo nextest run` 替代 `cargo test`**，进程级并行更快，无 fixture 串扰风险。

```bash
# 推荐
cargo nextest run -p ironclaw --lib sandbox::os_executor
cargo nextest run -p desktop-client --lib engine_startup_tests

# 仅当 nextest 未安装时回退
cargo test -p ironclaw
```

未装请：`cargo install cargo-nextest --locked`。CI 使用何种命令以仓库 workflow 为准（不强制改 CI）。

### 构建缓存清理：默认使用 cargo sweep --time 3

**磁盘满时优先用 `cargo sweep --time 3`，禁止用 `cargo clean` 或 `cargo sweep --time 0` 一次清干净**。一次性清掉全部产物会导致下次 build 需要从零重编全部依赖（10+ 分钟），sccache 也帮不上忙（增量缓存丢了）。

```bash
# 推荐：清掉 3 天没访问过的产物，保留近期增量缓存
cargo sweep --time 3

# 自动定时清理（cron）
0 3 * * * cd ~/Documents/code/x-claw && cargo sweep --time 3

# 禁止：一次清干净
# cargo clean              ← 不要用
# cargo sweep --time 0     ← 不要用
```

未装请：`cargo install cargo-sweep --locked`。

详细工作流（含 sccache、`cargo build -p desktop-client --lib` 增量编译）见 [desktop-client/README.md](desktop-client/README.md)。

### 外部库使用

- **先搜索项目中该库的现有用法**（`rg "libsql::" src/`），参考项目代码而非外部文档
- **优先使用成熟社区库**，避免重复造轮子（如 `config-rs`、`reqwest`、`serde`）
- 检查 `Cargo.toml` 中的依赖版本，注意 breaking changes
- 分阶段实现，先验证核心功能编译通过

### 编码标准参考

详细的 Rust 编码规范、错误处理模式、测试代码示例和质量门禁脚本用法见 `.kiro/steering/rust-coding-standards.md`（手动引用）。

### 数据库访问层迁移策略（Admin Backend）

Admin Backend 正在从 `tokio-postgres + deadpool-postgres` 逐步迁移到 **SQLx**。

**规则：**
- **新模块**必须使用 `sqlx`，通过 `state.sqlx_pool` 访问数据库
- **存量模块**维持 `tokio-postgres`，按需迁移，不强制一次性重写
- 两套连接池在 `AppState` 中并存：`db_pool`（legacy）和 `sqlx_pool`（新）

**SQLx 使用模式：**
```rust
// 1. 定义强类型结果结构体
#[derive(sqlx::FromRow, serde::Serialize)]
struct MyRow { field: String, count: i64 }

// 2. 用 query_as 绑定参数（$1 占位符）
let rows = sqlx::query_as::<_, MyRow>("SELECT field, COUNT(*) AS count FROM t WHERE id = $1")
    .bind(id)
    .fetch_all(&state.sqlx_pool)
    .await
    .map_err(|e| Error::Database(e.to_string()))?;
```

**DATE_TRUNC / INTERVAL 的处理：**
这两类 SQL 片段无法通过 `$1` 参数化（pg 协议限制）。必须用枚举白名单替代字符串，
参考 `admin-backend/src/handlers/reports.rs` 中的 `Period` 枚举模式。

---

## 测试规则

### 核心原则

> **测试的目标不是追求高覆盖率，而是确保系统在所有情况下都是安全的。**

1. **测试失败路径和成功路径同等重要** — 每个功能都要有失败路径测试
2. **安全功能必须"故障安全"（Fail-Safe）** — 失败时拒绝操作，而非允许操作（Fail-Open）
3. **真实环境测试不可或缺** — 模拟测试只能验证逻辑，无法验证集成
4. **契约测试验证业务目标可达成** — 不只是"能解析"，而是"够用"
5. **安全审计测试验证无泄露** — 日志、错误信息、网络响应中不得包含原始敏感数据
6. **`cargo test` 通过 ≠ `cargo build` 通过** — 测试可能绕过有问题的代码
7. **测试不应固化错误设计** — 测试断言必须从业务目标出发，而非从实现假设出发

### PICT 测试设计（新增）

当一个功能包含**多参数、多角色、多状态、多环境或多失败模式组合**时，补测试应优先使用 `pict-test-designer` 设计测试矩阵，而不是只凭直觉补 happy path。

规则：
- **先用 `pict-test-designer`，再决定是否需要真实 PICT 生成** — `pict-test-designer` 负责参数拆解、取值分组、约束整理、预期结果设计；真实 PICT 负责机械生成可复现的 pairwise 组合。
- **仅用 `pict-test-designer` 即可的场景** — 参数规模小到中等、组合仍可人工审查、当前目标是补测试设计或生成测试代码，而不是输出可审计的组合覆盖证明。
- **必须再跑真实 PICT 的场景** — 参数较多且组合明显爆炸、需要证明 2-way 覆盖完整、需要把组合结果沉淀为 CI/评审可复现资产、或属于审批/权限/配额/策略/安全等高风险组合逻辑。
- **agent 内的“设计 -> 真实生成 -> 映射 -> 落代码” workflow 统一使用 `pairwise-test-generator` skill** — 不要把这套多步流程直接展开复制到 `AGENTS.md`。
- **真实 PICT 生成统一走 `scripts/pict_generate.py`** — 不要在 agent workflow 里手动切换 `pict`、`pypict`、Docker。默认先走本机 `pict`；若本机不可用且允许容器，再用 `python3 scripts/pict_generate.py <model-file> --backend docker --docker-build-if-missing --format json`。
- **不要假设存在可直接拉取的公开 `pict` 镜像** — 当前 Docker 路径是通过上游 `microsoft/pict` 仓库里的 `Containerfile` 在本地构建镜像，不是依赖 Docker Hub 现成官方镜像。
- **PICT 只用于测试设计，不替代现有测试分层** — 生成出的组合必须映射到单元、失败路径、契约、安全审计、集成、E2E 测试中。
- **不要新建一套平行的“大而全测试”** — 优先扩充现有测试文件和测试族。
- **小而直接的校验函数不必强行 PICT 化** — 简单校验继续用聚焦型单元测试。
- **涉及真实边界的问题不得因使用 PICT 而降级** — SSE 事件格式、Tauri 状态注入、迁移完整性、策略验签等仍必须保留真实环境或跨层验证。
- **安全相关场景必须显式覆盖失败路径组合** — 尤其是 DLP、审批、权限、策略同步和敏感信息处理。

### PICT 模型文件放置

- **与单个方案/重构/测试设计强绑定的 `.pict` 模型** 放在对应的 `docs/plans/` 下，与同名矩阵或方案文档相邻，便于一起评审和迭代。
- **可复用、长期维护、跨模块共享的 `.pict` 模型** 不应长期堆在 `docs/plans/`；当模型从“某次方案资产”演变为“项目公共测试资产”时，应迁移到单独的共享目录（如后续新增 `docs/pict-models/` 或测试资产目录）。
- **当前 `owner-id-model1.pict` / `owner-id-model2.pict` 放在 `docs/plans/` 是合适的**，因为它们直接服务于 `owner-id-pict-test-matrix.md` 这份具体方案，不是通用公共模型。

推荐流程：
1. 用 `pict-test-designer` 输出参数、取值、约束、预期结果
2. 判断是否满足“必须再跑真实 PICT”的条件
3. 若需要，使用真实 PICT 生成 pairwise 组合结果
4. 将组合结果映射到现有测试族，补 Rust/前端测试代码
5. 仍需完成失败路径、契约、真实环境和安全审计验证，不能只停在组合测试

### 测试维度矩阵

| 测试维度 | 正常路径 | 错误路径 | 降级逻辑 | 真实环境 | 契约测试 | 安全审计 |
|---------|---------|---------|---------|---------|---------|---------|
| 单元测试 | ✅ 必须 | ✅ 必须 | ⚠️ 如有 | N/A | N/A | ✅ 必须 |
| 集成测试 | ✅ 必须 | ✅ 必须 | ⚠️ 如有 | ✅ 推荐 | ✅ 必须 | ✅ 必须 |
| E2E 测试 | ✅ 必须 | ✅ 推荐 | ⚠️ 如有 | ✅ 必须 | N/A | ✅ 推荐 |

### 覆盖率目标

| 维度 | 功能模块 | 安全模块 |
|------|---------|---------|
| 单元测试 | >90% | 100% |
| 集成测试 | >80% | 100% |
| 失败路径 | >80% | 100% |
| 契约测试 | >90% | 100% |
| 安全审计 | — | 100% |

### 覆盖率验证

```bash
# 全量覆盖率报告（需要 cargo-llvm-cov）
./scripts/coverage.sh

# 按模块过滤
./scripts/coverage.sh safety

# 输出 lcov 格式（CI 集成用）
COV_FORMAT=lcov ./scripts/coverage.sh

# 包含集成测试
COV_ALL_TARGETS=1 ./scripts/coverage.sh
```

安装：`cargo install cargo-llvm-cov`

### 测试文件组织

```
tests/
├── {module}_unit_tests.rs           # 单元测试
├── {module}_failure_tests.rs        # 失败路径测试
├── {module}_integration_tests.rs    # 集成测试
├── {module}_contract_tests.rs       # 契约测试
├── {module}_security_audit_tests.rs # 安全审计测试
├── {module}_reliability_tests.rs    # 可靠性测试
├── {module}_regression_tests.rs     # 回归测试
└── integration_smoke_tests.rs       # 编译+迁移+路由冒烟（全局唯一）
```

### 测试命名规范

- 需求测试: `req_{module}_{id}_{description}`
- 安全测试: `test_security_{attack_type}`
- 失败路径: `test_failure_{scenario}`
- 契约测试: `test_contract_{interface}_{case}`
- 审计测试: `test_audit_{security_concern}`
- 可靠性: `test_{failure_scenario}_recovery`
- 回归测试: `test_{feature}_backward_compatibility`

### 历史教训摘要

以下是项目中真实发生过的测试盲区，新功能开发时必须警惕：

| 盲区 | 教训 | 防护措施 |
|------|------|---------|
| DLP 失败路径缺失 | 测试只覆盖成功路径，降级逻辑允许原始消息发送 | 每个功能必须有失败路径测试 |
| model-configs 404 | `cargo test` 通过但 `cargo build` 失败，迁移文件存在但未执行 | `integration_smoke_tests.rs` 三层防护 |
| client-models 缺字段 | 测试主动断言了错误行为，把错误设计固化为"规范" | 契约测试从业务目标出发 |
| SSE 事件名不匹配 | 测试用模拟数据，未验证真实后端事件格式 | 真实环境集成测试 + 参考已有实现 |
| Tauri state not managed | 单元测试绕过 Tauri 状态注入，运行时 panic | 启动时序测试 + 冒烟测试 |
| tokio-postgres JSONB 类型 | `row.get::<_, String>()` 对 JSONB 列运行时 panic，编译期不报错；`tokio::time::interval` 首次 tick 立即触发，启动即崩溃 | 写 `row.get` 前核对迁移文件中列的实际类型；启动时立即执行的后台任务必须有冒烟测试覆盖 |

详细案例分析、代码示例和 E2E 测试模式见 `docs/testing-guide.md`。

---

## Admin Backend 冒烟测试（integration_smoke_tests.rs）

**强制要求**：每次新增迁移文件时，必须同步在 `integration_smoke_tests.rs` 的 `required` 列表中追加对应条目。

三层防护：
1. **编译检查** — 调用 `create_router()` 强制编译所有 handler
2. **迁移完整性** — 查询 `information_schema` 验证每个迁移对应的表存在
3. **HTTP 路由冒烟** — 用 `tower::ServiceExt::oneshot` 验证路由不返回 404

---

## Tauri 命令注册契约测试

所有 Tauri 命令通过 `all_tauri_commands!()` 宏集中管理（`desktop-client/src/lib.rs`），`main.rs` 和测试共用同一份注册表。

新增命令流程：
1. 在 `src/ipc.rs` 或 `src/commands.rs` 实现命令
2. 在 `all_tauri_commands!()` 宏中添加
3. 在 `tests/tauri_command_contract_tests.rs` 的 `FRONTEND_INVOKED_COMMANDS` 中添加
4. 运行 `cargo test --test tauri_command_contract_tests`
5. 前端添加对应 `invoke()` 调用

> 普通单元测试直接调用 Rust 函数，绕过 IPC 路由，无法捕获 "Command not found" 错误。契约测试是唯一能在 CI 阶段验证 IPC 路由完整性的方法。

---

## 启动时序测试（Desktop Client）

涉及 Tauri 状态管理的改动，必须运行：
```bash
cargo test -p desktop-client --lib engine_startup_tests
```

覆盖维度：启动时序（状态转换）、失败路径（未就绪时友好错误）、并发安全（多线程初始化）、冒烟测试（模拟真实启动流程）。

> 单元测试通过 ≠ 运行时安全。涉及异步状态注入的场景，必须用时序测试和冒烟测试覆盖。

参考：`desktop-client/src/engine_startup_tests.rs`

---

## 统一检查清单

### 功能开发

- [ ] 已检查主项目/共享 crate 是否有可复用能力
- [ ] 已遵循 TDD 流程（测试优先）
- [ ] 单元测试覆盖正常路径 + 错误路径
- [ ] 已编写集成测试和契约测试
- [ ] `cargo build` 编译通过，0 错误 0 警告
- [ ] 所有测试通过
- [ ] **W3-A Phase 0 进行中**：若本 PR 属于 P0-1 ~ P0-10 任务族，必须按 [ADR-112 §5.4 评估嵌入节奏](docs/plans/architecture-refactor/adr-112-compatibility-evaluation.md#54-评估嵌入节奏b1--b2--b3) 自带 H1 unit + H2 integration 测试（W3-A 收尾后此项删除）

### 安全功能（额外）

- [ ] 失败路径测试覆盖率 100%
- [ ] 安全审计测试：敏感信息不泄露（日志、错误、网络）
- [ ] 降级逻辑采用 Fail-Safe 设计
- [ ] 契约测试验证业务目标可达成


### GitHub PR

- [ ] 当前切片已完成最小充分验证后再开 PR
- [ ] 已按需执行 `code-quality-audit`、`code-simplifier`、`code-review-expert`
- [ ] PR 描述已写清背景、范围、非目标、验证、后续步骤
- [ ] 如果是 stacked PR，已写清 base、merge 顺序、先看哪个 PR
- [ ] 已至少运行一轮 `gh pr checks` 或等价检查并同步结果
- [ ] 如果当前已完成闭环 milestone，已按会话轮换原则写 handoff 并提醒用户考虑新会话

---

## 参考文档索引

- `docs/testing-guide.md` — 测试代码示例、历史教训详细分析、E2E 测试模式
- `docs/architecture-guide.md` — 架构详细说明、复用流程图、代码示例
- `desktop-client/DLP_LESSONS_SUMMARY.md` — DLP 测试盲区教训
- `desktop-client/DLP_TESTING_LESSONS_LEARNED.md` — 详细经验教训
- `desktop-client/DLP_SUPPLEMENTARY_TEST_PLAN.md` — 补充测试计划
- `desktop-client/src/engine_startup_tests.rs` — 启动时序测试
- `admin-backend/tests/integration_smoke_tests.rs` — 冒烟测试
- `desktop-client/tests/tauri_command_contract_tests.rs` — Tauri 命令契约测试
- `admin-backend/ui/cypress/` — Cypress E2E 测试
- `.kiro/steering/rust-coding-standards.md` — Rust 编码标准、错误处理模式、测试代码示例、质量门禁脚本
