# ADR-156：F4.6 builtin 工具下沉落点决策

- 状态：Proposed
- 提案人：Coding Agent（基于 ADR-152 §3 F4.6 留空条款 + ADR-153 无头 agent 框架定位 + 40/41 文档现状盘点）
- 关联：ADR-152（agent and capability fusion）§3 阶段 F4 / §11.9 / ADR-153（headless agent framework）§4.2 / ADR-129（verbatim port mandate）§1.3 / ADR-155（F4.4 orchestrator 落点决策，模板与对账方法参考）
- 时间：2026-05-22

---

## 1. 背景

ADR-152 §3 阶段 F4 条款写到：

> **F4.6+**：`tools/builtin` 分批搬（21.5K LoC、28 工具，建议 6-8 刀按职能分组）。

也就是说 F4.6 在 ADR-152 落笔时同样是一个**显式留空的占位**，要求后续单独 ADR 决定怎么分组、落到哪个 crate、按什么节奏切 PR。本 ADR 就是来填这个坑的。

补充背景：

- F4.4（ADR-155）已完成，orchestrator 决策为"留桌面"
- F4.5（ADR-152-f45 + 附录）已完成，`dasclaw_wasm_tools` 已建并切流量
- 40-tool-ecosystem-inventory.md 已盘点四方来源，确认 builtin 是 desktop 独有的最大未下沉资产
- 41-target-architecture-drift-analysis.md 已识别 31 蓝图 v2.4 在 F4.6 上完全留白

## 2. builtin 现状到底是什么

读完 `desktop-client/ironclaw/src/tools/builtin/` 全部 35 个 `.rs` 文件、共 21505 行后的真实清单：

### 2.1 工具数量与体量

| 维度 | 数值 |
|------|------|
| 总文件数 | 35 个 `.rs` |
| 总行数 | 21505 LOC |
| 工具数 | 约 50 个（按 `pub use Xxx, Yyy, Zzz` 展开后） |
| ADR-152 §3 原估 | 28 工具 / 21.5K LOC |
| 实际差异 | 工具数被低估约 1.8 倍，LOC 估计精准 |

### 2.2 按文件体量排序的工具

| 文件 | 行数 | 工具职能 |
|------|------|---------|
| `routine.rs` | 2639 | EventEmit / RoutineCreate / RoutineDelete / RoutineFire / RoutineHistory / RoutineList / RoutineUpdate |
| `job.rs` | 2359 | CancelJob / CreateJob / JobEvents / JobPrompt / JobStatus / ListJobs |
| `shell.rs` | 1666 | Shell（含 `classify_command_risk`）|
| `http.rs` | 1513 | Http |
| `skill_tools.rs` | 1322 | SkillInstall / SkillList / SkillRemove / SkillSearch |
| `message.rs` | 1075 | Message |
| `memory.rs` | 974 | MemoryRead / MemorySearch / MemoryTree / MemoryWrite |
| `file.rs` | 956 | ApplyPatch / ListDir / ReadFile / WriteFile |
| `extension_tools.rs` | 828 | ExtensionInfo / ToolActivate / ToolAuth / ToolInstall / ToolList / ToolRemove / ToolSearch / ToolUpgrade |
| `path_utils.rs` | 737 | path 校验工具集（被多个工具复用）|
| `time.rs` | 594 | Time |
| `web_search.rs` | 552 | WebSearch |
| `restart.rs` | 482 | Restart |
| `code_edit.rs` | 438 | CodeEdit |
| `grep_search.rs` | 405 | GrepSearch |
| `sub_agent.rs` | 400 | SubAgent |
| `web_fetch.rs` | 366 | WebFetch |
| `glob_search.rs` | 331 | GlobSearch |
| `image_edit.rs` | 329 | ImageEdit |
| `tool_info.rs` | 297 | ToolInfo |
| `json.rs` | 288 | Json |
| `plan_mode.rs` | 276 | PlanMode |
| `image_analyze.rs` | 257 | ImageAnalyze |
| `image_gen.rs` | 247 | ImageGenerate |
| `secrets_tools.rs` | 218 | SecretDelete / SecretList |
| `git/` 子目录 | 1185 | GitBranch / GitCommit / GitDiff / GitLog / GitPush / GitStaleCheck / GitStatus（7 个工具）|
| `file_guard.rs` | 184 | 内部辅助 |
| `session_fork.rs` | 173 | SessionFork |
| `html_converter.rs` | 128 | 内部辅助（`convert_html_to_markdown`）|
| `lsp/mod.rs` | 10 | LspQueryTool（已是薄壳，真正实现在 dasclaw_lsp）|
| `echo.rs` | 48 | Echo（demo）|
| `mod.rs` | 87 | re-export 入口 |

### 2.3 反向依赖矩阵

对全部 35 个文件做 `use crate::` 扫描，按反向依赖目标分类：

| 反向依赖目标 | 命中次数 | 命中文件 |
|--------------|----------|----------|
| `tools::tool::` | 30 个文件 | 几乎所有（已对应 `dasclaw_tool`）|
| `tools::builtin::path_utils::` | 5 个文件 | code_edit / file / file_guard / glob_search / grep_search / image_analyze / image_edit |
| `tools::wasm::` | 1 个文件 | http.rs（已对应 `dasclaw_wasm_tools`）|
| `tools::registry::` | 1 个文件 | tool_info.rs |
| `sandbox::` | 1 个文件 | shell.rs |
| `safety::` | 1 个文件 | http.rs |
| `workspace::` | 2 个文件 | file.rs / memory.rs |
| `channels::` | 2 个文件 | job.rs / message.rs |
| `db::` | 2 个文件 | job.rs / routine.rs |
| `bootstrap::dasclaw_base_dir` | 2 个文件 | job.rs / message.rs |
| `extensions::` | 2 个文件 | extension_tools.rs / message.rs |
| `orchestrator::` | 2 个文件 | job.rs |
| `agent::routine*::` | 2 个文件 | routine.rs |
| `skills::` | 2 个文件 | skill_tools.rs |
| `history::` | 1 个文件 | job.rs |

**对已下沉 crates 的依赖**（健康信号）：

- `dasclaw_runtime::secrets::SecretsStore` × 3
- `dasclaw_runtime::context::ContextManager` × 1
- `dasclaw_runtime::{JobState, JobContextCore}` × 2
- `dasclaw_bash_validation::{PermissionMode, ...}` × 2
- `ironclaw_common::*` × 1（job.rs）

### 2.4 按反向依赖耦合度的工具分组（关键洞察）

按 §2.3 矩阵，把 35 个文件按"反向依赖深度"切成 5 个 tier：

| Tier | 文件数 | LOC | 反向依赖特征 |
|------|--------|-----|--------------|
| **T0 几乎无 desktop 依赖** | 12 | ~3400 | 只 use `tools::tool::`（已是 `dasclaw_tool`）+ 自包含 |
| **T1 仅依赖 builtin 内** | 7 | ~3400 | 只 use `tools::tool::` + `tools::builtin::path_utils::` |
| **T2 依赖已下沉模块** | 3 | ~3200 | 依赖 sandbox / safety / wasm_tools（均已 crates 化）|
| **T3 依赖 desktop 服务皮** | 5 | ~3600 | 依赖 workspace / channels / db / extensions（desktop 独占）|
| **T4 强依赖桌面后端** | 3 | ~7900 | job.rs / routine.rs / message.rs：依赖 orchestrator / agent::routine / history / db / channels |

T0+T1+T2 = 22 文件 / ~10000 LOC：**短期可下沉**。
T3 = 5 文件 / ~3600 LOC：**需逐个评估是否下沉对应服务**。
T4 = 3 文件 / ~7900 LOC：**短期不可下沉**，与 orchestrator 同属"桌面后端独占"。

## 3. 三方对账：codex / claw-code 有没有同类能力

按 AGENTS.md 任务启动 4 问（涉及跨项目对账），必须三层验证。本 ADR 引用 40-tool-ecosystem-inventory.md §3 表格的已验证结论：

| 工具能力 | claude-code | codex | ironclaw 现状 |
|---------|-------------|-------|---------------|
| 文件读写（Read/Write/Edit/ApplyPatch）| ✅ | ✅ | ✅ |
| Bash/Shell 执行 | ✅ | ✅ | ✅ |
| Glob / Grep 搜索 | ✅ | ✅ | ✅ |
| WebFetch / WebSearch | ✅ | ✅（plan_tool）| ✅ |
| LSP 查询 | ❌ | ❌ | ✅（已薄壳指向 `dasclaw_lsp`）|
| Git 7 工具集 | ❌（用 Bash）| ❌（用 Bash）| ✅（已部分搬到 `dasclaw_git_tools` 空壳）|
| Routine / Job / Message 系列 | ❌ | ❌ | ✅（ironclaw 独有的多任务编排）|
| Skill 工具集 | ❌（claude-code 是 .md skill 不是工具）| ❌ | ✅ |
| Extension 工具集 | ❌ | ❌ | ✅ |
| Memory 4 工具 | ❌ | ❌ | ✅ |
| Image Gen / Analyze / Edit | ✅（部分）| ✅（部分）| ✅ |
| Plan Mode | ❌ | ✅ | ✅ |
| Sub Agent / Session Fork | ❌ | ✅（task tool）| ✅ |

**结论**：

- 文件/搜索/网络/shell 类基础工具，三方都有但实现各异——**不能换 codex/claw-code 代码**，按 ADR-129 §1.3 verbatim 红线只能搬 ironclaw 自己的实现
- LSP / Git 工具集 / Routine / Job / Message / Skill / Extension / Memory **是 ironclaw 独有**，搬迁路径只能在 ironclaw 内部摆放
- 这进一步证实 F4.6 不是"融合三方"工作，而是"重排 ironclaw 内部 path 依赖"工作

## 4. 关键判断：F4.6 的真正目标是什么

回到 ADR-153 §4.2 的无头 agent 最小可用形态：

```rust
let agent = Agent::builder()
    .llm(llm)
    .workspace(std::env::current_dir()?)
    .tools_default()
    .hooks_default()
    .build()?;
```

`.tools_default()` 的语义是"给 agent 一组开箱即用的工具"。如果这组工具仍然耦合 desktop 的 `channels / db / orchestrator / history`，那 `Agent::builder()` 就**没法在 dasclaw_cli 或第三方进程中实例化**——因为引入 `dasclaw_runtime` 等于拖上整个桌面后端。

**F4.6 的真正目标**：把 builtin 工具按反向依赖切成两堆——

- **A 堆**：无头 agent 必需的基础工具（文件/搜索/网络/shell/git/image/plan_mode/time/json），下沉到 crates/ 形成可在 dasclaw_cli 中复用的工具仓
- **B 堆**：桌面后端独占的工具（job/routine/message/orchestrator 联动类），留在 desktop-client 不动

这与 ADR-155 的判断逻辑一致：**"是否对无头 agent 必需" 是落点判断的主轴**。

## 5. 候选方案与权衡

### 候选 A：一刀切搬到 `dasclaw_tool` 现有 crate

- ✗ `dasclaw_tool` 当前是 trait + framework crate（977 LOC），不是工具实现集
- ✗ 把 21K LOC 灌进去会把它撑爆，违反 SRP
- ✗ 不解决依赖分层问题（T3/T4 工具仍带 desktop 反向依赖）

**否决**。

### 候选 B：建一个 `crates/dasclaw_builtin_tools` 大 crate，全部搬过去

- ✓ 命名直接对应 `tools/builtin/`
- ✗ T3/T4 工具反向依赖 desktop 7 个模块（channels / db / orchestrator / extensions / skills / agent / history / workspace / bootstrap），verbatim 搬迁就是把 desktop 内部模块全拖到新 crate 里
- ✗ 重蹈 ADR-155 候选 B 的覆辙（orchestrator 7 个反向依赖问题）
- ✗ 违反 ADR-152 §11.9 路径倒置红线

**否决**。

### 候选 C：按反向依赖 tier 分多个 crate，分批搬

按 §2.4 的 5 个 tier 切分（仅搬 T0+T1+T2 + 部分 T3）：

| 新 crate | 来源 | 包含工具 | LOC | 反向依赖 |
|---------|------|---------|-----|---------|
| `dasclaw_fs_tools` | T0+T1 | ApplyPatch / ListDir / ReadFile / WriteFile / CodeEdit / GlobSearch / GrepSearch / path_utils / file_guard | ~3700 | 仅 dasclaw_tool + dasclaw_runtime |
| `dasclaw_shell_tools` | T2 | Shell（含 classify_command_risk）| 1666 | dasclaw_tool + dasclaw_sandbox + dasclaw_bash_validation |
| `dasclaw_net_tools` | T2 | Http / WebFetch / WebSearch / html_converter | ~2400 | dasclaw_tool + dasclaw_safety + dasclaw_wasm_tools |
| `dasclaw_git_tools`（扩充已有空壳）| T0 | 7 个 Git 工具 + lsp 薄壳合入 | 1185 + 10 | dasclaw_tool |
| `dasclaw_image_tools` | T0 | ImageGen / ImageAnalyze / ImageEdit | 833 | dasclaw_tool + dasclaw_runtime |
| `dasclaw_misc_tools` | T0 | Echo / Time / Json / PlanMode / Restart / ToolInfo / SessionFork / SecretDelete / SecretList | ~1900 | dasclaw_tool + dasclaw_runtime |
| `dasclaw_memory_tools` | T0 | MemoryRead / MemorySearch / MemoryTree / MemoryWrite | 974 | dasclaw_tool + dasclaw_runtime |
| `dasclaw_sub_agent_tools` | T0 | SubAgent | 400 | dasclaw_tool + dasclaw_runtime |

**T3/T4 不下沉**（留桌面）：

| 留守工具 | 文件 | LOC | 留守理由 |
|---------|------|-----|---------|
| Extension 8 工具 | extension_tools.rs | 828 | 依赖 desktop `extensions::*` 子系统 |
| Skill 4 工具 | skill_tools.rs | 1322 | 依赖 desktop `skills::catalog / skills::registry` |
| Job 6 工具 | job.rs | 2359 | 依赖 orchestrator + channels + db + history |
| Routine 7 工具 | routine.rs | 2639 | 依赖 agent::routine_engine + db |
| Message 1 工具 | message.rs | 1075 | 依赖 channels + extensions + bootstrap |

**T3/T4 留守合计 8223 LOC**，与 ADR-155 orchestrator 留守 3330 LOC 加起来 = 11553 LOC 桌面独占工具，与 40 文档 §5 的"留 desktop 残余清单"对得上。

- ✓ 完全符合 ADR-129 §1.3 verbatim 红线（每个 crate 都是搬不改）
- ✓ 每个 crate 反向依赖目标都已存在（不需要前置下沉新模块）
- ✓ 8 个新 crate 各自单 PR 可 revert，符合 ADR-152 §4.2 回滚契约
- ✓ T0+T1+T2 = ~11300 LOC 下沉后，desktop 工具层只剩 ~10100 LOC，与 41 文档目标对齐
- ✗ 新增 8 个 crate，crates/ 数量从 45 增到 53，但 41 文档已识别蓝图遗漏 16 个，本身蓝图就要升级
- ⚠ 需要单独 PR 把 desktop 现有 `tools/builtin/mod.rs` 改成薄壳 re-export（与 F4.5.1 channels 薄壳模式一致）

**推荐**。

### 候选 D：按"代码风格 / 来源"分组

- ✗ 没有客观分类标准
- ✗ 不解决依赖分层问题

**否决**。

### 候选 E：暂缓 F4.6，等无头 agent 框架先跑起来再决定

- ✓ 风险最低
- ✗ 但无头 agent 框架（ADR-153）的 `.tools_default()` 必须有工具仓才能落地
- ✗ 暂缓等于 F4 阶段烂尾

**否决**。

## 6. 决策

**采用候选 C**：F4.6 按反向依赖 tier 切成 8 个新 crate 分批搬，T3/T4 共 8223 LOC 工具留桌面。

### 6.1 8 个新 crate 与切分边界（最终落点表）

| # | 新 crate | LOC | 来源文件 | 反向依赖（搬完后）|
|---|---------|-----|---------|-------------------|
| 1 | `dasclaw_fs_tools` | ~3700 | code_edit, file, file_guard, glob_search, grep_search, path_utils | dasclaw_tool, dasclaw_runtime |
| 2 | `dasclaw_shell_tools` | 1666 | shell.rs | dasclaw_tool, dasclaw_sandbox, dasclaw_bash_validation |
| 3 | `dasclaw_net_tools` | ~2400 | http, web_fetch, web_search, html_converter | dasclaw_tool, dasclaw_safety, dasclaw_wasm_tools |
| 4 | `dasclaw_git_tools`（扩充空壳）| ~1200 | git/* 7 文件（含 lsp 薄壳）| dasclaw_tool |
| 5 | `dasclaw_image_tools` | 833 | image_gen, image_analyze, image_edit | dasclaw_tool, dasclaw_runtime |
| 6 | `dasclaw_misc_tools` | ~1900 | echo, time, json, plan_mode, restart, tool_info, session_fork, secrets_tools | dasclaw_tool, dasclaw_runtime |
| 7 | `dasclaw_memory_tools` | 974 | memory.rs | dasclaw_tool, dasclaw_runtime |
| 8 | `dasclaw_sub_agent_tools` | 400 | sub_agent.rs | dasclaw_tool, dasclaw_runtime |

合计下沉 ~13073 LOC。原 LOC 估计 11300 与新 crate 合计 13073 存在偏差，原因：`path_utils.rs` 737 行被两个 tier 共享算入 fs_tools，`http` 系列 4 个文件合计 2407 行更准。

### 6.2 不在 F4.6 范围（留桌面）

| 文件 | LOC | 留守理由 |
|------|-----|---------|
| `extension_tools.rs` | 828 | 强依赖 desktop `extensions::*` |
| `skill_tools.rs` | 1322 | 强依赖 desktop `skills::*` |
| `job.rs` | 2359 | 强依赖 orchestrator + channels + db + history |
| `routine.rs` | 2639 | 强依赖 agent::routine_engine + db |
| `message.rs` | 1075 | 强依赖 channels + extensions + bootstrap |

合计留守 8223 LOC。这部分按 ADR-155 同款逻辑作为"桌面后端独占"，不阻塞无头 agent 落地。

### 6.3 PR 切分节奏（F4.6.1 ~ F4.6.8）

| 子波次 | crate | 启动门 | 验证最低标准 |
|--------|-------|--------|--------------|
| F4.6.1 | `dasclaw_misc_tools` | F4.5 全绿 | `cargo nextest run -p dasclaw_misc_tools` + 该 crate 自带 unit 测试 |
| F4.6.2 | `dasclaw_image_tools` | F4.6.1 merge | 同上 |
| ~~F4.6.3~~ | ~~`dasclaw_memory_tools`~~ | **取消下沉** | 见 §6.3.1 决策依据 |
| F4.6.4 | `dasclaw_sub_agent_tools` | F4.6.3 决策 | 同上 |
| F4.6.5 | `dasclaw_git_tools`（扩充）| F4.6.4 merge | 同上 + 把 lsp 薄壳合入 |
| F4.6.6 | `dasclaw_fs_tools` | F4.6.5 merge | 同上 + 跨 crate path 校验测试 |
| F4.6.7 | `dasclaw_shell_tools` | F4.6.6 merge | 同上 + `classify_command_risk` 回归 |
| F4.6.8 | `dasclaw_net_tools` | F4.6.7 merge | 同上 + wasm_tools 集成 |

**节奏特征**：先搬 T0 简单工具（F4.6.1~F4.6.5），再搬 T1/T2 有依赖工具（F4.6.6~F4.6.8）。每个 PR 单独可 revert，符合 ADR-152 §4.2。

### 6.3.1 F4.6.3 决策：memory_tools 取消下沉，标记 desktop-only

**结论**：`memory.rs`（MemorySearch / MemoryWrite / MemoryRead / MemoryTree 四个工具，974 行）**不下沉到 `crates/`**，继续留在 `desktop-client/ironclaw/src/tools/builtin/`。

**依据**：

1. **真实依赖偏离 T0**：原表格把 `dasclaw_memory_tools` 列为 T0（依赖 `dasclaw_tool + dasclaw_runtime`），但 memory.rs 实际依赖 `desktop-client/ironclaw/src/workspace/` 运行时模块——一整套 PostgreSQL/SQLite 双后端 + 向量 embeddings + 文档分块（chunker）+ 隐私分层（layer / privacy）+ 内容卫生（hygiene）+ 混合检索（FTS + 余弦相似度）的 RAG 系统。这与 F4.6.6 file.rs 只用 `dasclaw_workspace_cap::document::paths`（轻量常量）的情况完全不同。

2. **无头 agent 框架不需要 RAG 记忆**：
   - codex 无头：无持久记忆，每次 session 独立
   - claw-code / ironclaw-main：用文件系统 + `CLAUDE.md` / `AGENTS.md` 做文件级记忆
   - `dasclaw_fs_tools` 的 `read_file` / `write_file` / `list_directory` 已覆盖纯无头 CLI 的记忆需求

3. **强行下沉会污染共享层依赖**：把 PostgreSQL、sqlx、向量 embeddings、隐私分类等重依赖引入 `crates/` 会让所有下游（包括纯 CLI）背上数据库依赖，违反 ADR-156 §4「无头 agent 共享层应保持依赖最小」的原则。

4. **desktop 部署不受影响**：Tool trait 注册机制是开放的——desktop-client 在自己的 agent 启动代码里实例化 `Workspace` 并注册四个 MemoryTool，与"代码住在哪个 crate"无关。memory_tools 留在 desktop 不影响 desktop 的 agent 用上它们。

5. **未来需求的正确解法**：如果将来出现第三方客户端需要 RAG 向量记忆能力，应单独立 ADR 规划 "workspace 运行时下沉" 工程（含数据库选型、embeddings 后端、隐私层定义等），届时 memory_tools 跟随一起搬。**不属于 F4.6 节奏。**

**后续编号调整**：F4.6.3 槽位保留（不复用，避免历史 PR 编号混乱），F4.6.4 ~ F4.6.8 编号不变。本决策由 PR #775 之后的对账发现并落入 ADR。

### 6.3.2 配套评估：`llm/session.rs` 不在 F4.6 范围（保持 desktop）

**结论**：`desktop-client/ironclaw/src/llm/session.rs`（823 行，NEAR AI session token / OAuth renewal / 文件 + DB 持久化）**不**列入 F4.6 任何子波次，也不另立 `dasclaw_session` crate。

**依据**：

1. **真实依赖 T3 级**：`SessionManager` 持有 `Arc<dyn crate::db::Database>` store，依赖 desktop 的 db 抽象；与 `memory.rs` 依赖 desktop `workspace` 同属 T3，下沉同样会把 desktop db trait 牵到共享层。

2. **抽象边界已正确**：`SessionConfig` 已在 [`dasclaw_llm_provider::provider::config`](../../crates/dasclaw_llm_provider/src/provider/config.rs) 落地，desktop 仅 re-export；剩余 823 行是 NEAR AI OAuth 回调端口 + `~/.ironclaw/session.json` 文件持久化 + DB settings 表读写，全部是 desktop UX/部署细节。

3. **无头 agent 不需要 NEAR AI OAuth**：
   - `dasclaw_runtime`（ADR-153）走 API key 直注入，无 OAuth 流程；
   - 第三方无头集成走自己的认证；
   - `SessionConfig` trait 抽象足够，不需要把 `SessionManager` 实现搬到共享层。

4. **代码评审三层确认（2026-05-25）**：
   - `semantic_search` / `grep_search`：无 `dasclaw_session` crate，无同名子模块；`crates/dasclaw_*` 全部 47 个 crate 名单中无 session 项。
   - `desktop-client/ironclaw/src/agent/session.rs` 已是 4 行 shim → `dasclaw_core::session`（F3 已下沉）；
   - `desktop-client/ironclaw/src/tools/mcp/session.rs` 已是 11 行 shim → `dasclaw_mcp::session`（F3.2 phase 3 PR #661 已下沉）；
   - 仅剩 `llm/session.rs` 仍在 desktop，且按本节判定不应下沉。

**与 §6.3.1 关系**：本节与 §6.3.1 同形——两者都是「依赖 desktop 运行时抽象 → 强行下沉会污染共享层 → 抽象 trait 已下沉，实现留 desktop」的 T3 模式。本节并入 ADR-156 是为了把同类决策集中归档，避免日后误以为「凡是带 `session` 命名的都该有 `dasclaw_*` crate」。

### 6.4 desktop `tools/builtin/mod.rs` 薄壳化（独立 PR）

F4.6.8 全部 merge 后，单独开 PR 把 `desktop-client/ironclaw/src/tools/builtin/mod.rs` 改为：

```rust
//! Built-in tools that come with the agent.
//!
//! 按 ADR-156 §6 决策，工具实现已下沉到 crates/dasclaw_*_tools，
//! 本模块降级为 re-export 入口 + T3/T4 留守工具。

pub use dasclaw_fs_tools::*;
pub use dasclaw_shell_tools::*;
pub use dasclaw_net_tools::*;
pub use dasclaw_git_tools::*;
pub use dasclaw_image_tools::*;
pub use dasclaw_misc_tools::*;
pub use dasclaw_memory_tools::*;
pub use dasclaw_sub_agent_tools::*;

// T3/T4 留守工具
pub mod extension_tools;
pub mod skill_tools;
pub mod job;
pub mod routine;
pub mod message;
```

这一步与 F4.5.1 channels 薄壳模式（PR #733）一致，最小化对 desktop 上游代码的扰动。

## 7. 配套动作

1. **修订 ADR-152 §3 F4.6 条款**：把"`tools/builtin` 分批搬（21.5K LoC、28 工具，建议 6-8 刀按职能分组）"改为"`tools/builtin` 按 ADR-156 切 8 刀下沉，T3/T4 共 8223 LOC 留桌面"。该修订**不在本 ADR PR 范围**，留待单独 PR
2. **升级 31-target-architecture.md 到 v2.5**：补全 §4 P0/P1/P2 现实清单（按 41 文档结论），新增 §X F4.6 章节引用本 ADR。同样**不在本 ADR PR 范围**
3. **不改任何代码**：本 ADR PR 只提交 ADR 文档，不动 crates/ / desktop-client/
4. **下游契约**：F4.6.1 ~ F4.6.8 各自走 ADR-129 §1.3 verbatim 红线，每个 PR 自带门控（cargo check + nextest + check_no_panics）
5. **41-target-architecture-drift-analysis.md 同步更新**：F4.6.8 全部 merge 后把"F4.6 蓝图缺口"标记为已闭环

## 8. 后续路径

T3/T4 留守工具的未来：

- **Extension / Skill 工具**：随 desktop `extensions::*` / `skills::*` 子系统的下沉决策一起处理，需要单独 ADR（候选触发：ADR-153 步骤 3 `dasclaw_session`）
- **Job / Routine / Message 工具**：与 orchestrator 同属"桌面后端独占"，按 ADR-155 决策逻辑保持留守。如未来有 `dasclaw_cli` 容器化需求，再触发新 ADR

## 9. 不在本 ADR 范围

- ADR-152 §3 F4.6 条款文本如何修订（留待 ADR-152 自身修订 PR）
- 31-target-architecture.md v2.5 升级（留待独立 PR）
- T3/T4 留守工具的未来下沉路径（需要 desktop 服务皮先下沉，触发新 ADR）
- F4.6.1 ~ F4.6.8 各子波次的具体 PR 内容（每个子波次自带验证清单，按 ADR-129 §1.3 verbatim 推进）
- desktop `tools/builtin/mod.rs` 薄壳化的具体改写（留待 F4.6.8 后独立 PR）
- builtin 工具自身的代码质量问题（如 `routine.rs` 2639 行单文件是否拆分），属于 desktop 内部演化，与下沉决策无关

## 10. Sources read

- [`adr-152-agent-and-capability-fusion.md`](adr-152-agent-and-capability-fusion.md) §3 阶段 F4 / §4.2 回滚契约 / §11.9 路径倒置
- [`adr-153-headless-agent-framework-draft.md`](adr-153-headless-agent-framework-draft.md) §2 现状盘点 / §4.2 步骤 2
- [`adr-155-f44-orchestrator-landing-decision.md`](adr-155-f44-orchestrator-landing-decision.md) 模板与对账方法
- [`adr-152-f45-wasm-slicing.md`](adr-152-f45-wasm-slicing.md) F4.5 切分模式参考
- ADR-129 §1.3 verbatim port mandate（按惯例引用）
- [`40-tool-ecosystem-inventory.md`](40-tool-ecosystem-inventory.md) §3 四方工具对照 / §4 F4.6 真正要搬清单
- [`41-target-architecture-drift-analysis.md`](41-target-architecture-drift-analysis.md) F4.6 蓝图缺口识别
- `desktop-client/ironclaw/src/tools/builtin/mod.rs` 全文 87 行（re-export 入口）
- `desktop-client/ironclaw/src/tools/builtin/*.rs` 全部 35 个文件的 `use crate::*` 反向依赖扫描
- 文件体量统计（`find ... -name '*.rs' -exec wc -l`，21505 LOC 验证 ADR-152 §3 原估精度）

---

## 11. 附录 A：F4.6 与 ADR-155 决策对齐

| 维度 | ADR-155（F4.4 orchestrator）| ADR-156（F4.6 builtin tools）|
|------|-----------------------------|-------------------------------|
| 决策主轴 | 是否对无头 agent 必需 | 是否对无头 agent 必需 |
| 三方对账 | codex/claw-code 0 等价 | 基础工具三方各异，独有工具 0 等价 |
| 决策结论 | 候选 C：留桌面 | 候选 C：分 tier 切，T0~T2 下沉、T3/T4 留桌面 |
| verbatim 红线 | 完全遵守（不搬不改）| 完全遵守（搬不改）|
| 路径倒置红线 | 完全遵守（不引入反向依赖）| 完全遵守（按 tier 切分避开）|
| PR 切分 | 0 个 PR（不搬）| 8 个 PR + 1 个薄壳化 PR |
| 留守 LOC | 3330 | 8223 |
| 下沉 LOC | 0 | ~13073 |
| 配套蓝图升级 | 不必 | 推荐 31 v2.5 |

两个 ADR 共同的逻辑：

1. **决策主轴**：是否对 ADR-153 无头 agent 最小可用形态必需
2. **判断方法**：反向依赖图 + 三方对账双重验证
3. **保守倾向**：能不搬就不搬，宁可留桌面也不破坏 verbatim 红线
4. **配套机制**：每个 ADR 显式列出"不在本 ADR 范围"，避免范围蔓延

## 12. 附录 B：F4.6 与 31-target-architecture.md 升级关系

按 41-target-architecture-drift-analysis.md 推荐顺序：

```
本 ADR（ADR-156）merge
    │
    ▼
ADR-152 §3 F4.6 条款修订 PR（独立）
    │
    ▼
31-target-architecture.md v2.5 升级 PR（独立）
    │  - §4 补全 16 个遗漏 crate
    │  - §4 新增 F4.6 8 个 crate
    │  - 索引 ADR-111 ~ ADR-156
    │  - 新增 §X "F4.6 工具壳分层" 章节
    ▼
F4.6.1 ~ F4.6.8 实施 PR（按 §6.3 节奏）
    │
    ▼
desktop builtin/mod.rs 薄壳化 PR（F4.6.8 后）
    │
    ▼
41-target-architecture-drift-analysis.md 关闭 F4.6 缺口标记
```

这与 ADR-155 走过的路径完全对齐：**先决策、再修订 §3 条款、再实施、最后回写漂移分析文档**。
