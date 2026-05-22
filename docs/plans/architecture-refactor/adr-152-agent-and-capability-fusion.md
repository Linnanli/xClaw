# ADR-152：Agent 与能力栈融合方案（crates/ ↔ desktop-client/ironclaw/）

> 状态：Draft
> 作者：xClaw 架构小组
> 上下文 Issue：[#621](https://github.com/Linnanli/xClaw/issues/621) Step 3
> 依据证据：[dual-graph-stats.md](dual-graph-stats.md) + [capability-comparison.md](capability-comparison.md)
> 测试基线：[../testing/end-to-end-validation-plan.md](../testing/end-to-end-validation-plan.md)
>
> **命名说明**：Issue #621 原计划编号 ADR-149，但 adr-149 slot 已被 `tool-visibility-triple-gate` 占用，本 ADR 落入下一个空 slot **ADR-152**。

## 1. 决策摘要

本 ADR 把 PR #623 给出的双图谱能力对比落成可执行的融合方案：

1. **HookEngine 单一权威**：所有能力域统一通过 `dasclaw_hooks::HookEngine` + 4 个 trait（SafetyHook / SandboxExecutor / SecretProvider / ApprovalGate）接入，删除 ironclaw 内部 hook chain。（沿用 ADR-113）
2. **可立即切换的 8 项能力**：bash_validation / bash_permissions / sandbox / compaction / submission / undo / EgressGate / hook 入口，全部从 ironclaw 自实现改为依赖 core crate，无新代码。
3. **反向吸收的 6 项主航道**：LLM Provider / MCP / LSP / Workspace / Observability / Context manager 按 [40-ironclaw-internalization.md](40-ironclaw-internalization.md) 路径，把 ironclaw 实现下沉到对应 `dasclaw_*` crate。
4. **保留在 L2/L3 不下沉**：ironclaw_safety / routines / orchestrator / channels / import / secrets / tools_builtin（不含 bash_validator）。
5. **DLP 不在本 ADR 范围**：DLP 实现位于 `desktop-client/src/dlp/`（L2 客户端外壳），由 [ADR-148](adr-148-egress-gate-safety-hook-semantics.md) 配套 PR-A 跟进。
6. **红线**：`desktop-client/ironclaw/crates/ironclaw_safety/` 整体不删、不重写、不下沉到 core 命名空间。

## 2. 决策矩阵

> 完整 28 项见 [capability-comparison.md §2](capability-comparison.md#2-能力域对照表)。本节仅列动作分类。

| 类别 | 能力域 | 动作 | 触发风险 |
|---|---|---|---|
| **A. 立即切换**（仅改 import） | bash_validation / bash_permissions / sandbox / compaction / submission / undo / hook 入口 / EgressGate | ironclaw 调用方改 import → core | 低；ironclaw 现有调用点不多，行为应等价 |
| **B. 反向吸收**（W6 主航道） | LLM Provider / MCP / LSP / Workspace cap / Observability / Context manager | 把 ironclaw 实现搬到 `dasclaw_*`，按文件 verbatim port（ADR-129 §1.3） | 中；多文件移动会触发 reviewability + drift guard |
| **C. 留在 L2/L3** | ironclaw_safety（红线）/ routines / orchestrator / channels / import / secrets / tools_builtin / LLM prompt layers | 不动；后续仅做依赖图收敛 | 低 |
| **D. 范围外** | DLP（L2 客户端外壳） | 不在本 ADR 跟进 | 由 ADR-148 PR-A 处理 |

## 3. 迁移路径

按依赖图与风险排序，**串行**推进。每一阶段必须满足"上一阶段全部 e2e_headless 测试通过"才能开始。

### 阶段 F1：HookEngine 单一权威（前置 ADR-113）

**目标**：ironclaw 内部 hook 调用链全部走 `dasclaw_hooks::HookEngine`。

| 步骤 | 文件级动作 | 验证 |
|---|---|---|
| F1.1 | `desktop-client/ironclaw/src/hooks/registry.rs` 改为 `pub use dasclaw_hooks::HookRegistry`，删除自实现 | 单测 `req_hooks_registry_parity` |
| F1.2 | `src/hooks/{bootstrap,bundled,hook,mod}.rs` 改为薄包装，注册点路由到 core | 集成测 `e2e_headless::s2_bash_block` 调用序列与原 HookSpy 一致 |
| F1.3 | 删除 ironclaw `BeforeInbound`/`BeforeOutbound`/`BeforeToolCall` 双调用路径 | `dasclaw_hooks::count_hook_systems()` 编译期断言 = 1 |

**前置**：ADR-113 已 accepted；`dasclaw_hooks` 已含 SafetyHook/SandboxExecutor/SecretProvider/ApprovalGate 4 trait seam。

### 阶段 F2：A 类立即切换（8 项）

| 步骤 | ironclaw 当前 | 切换到 | 验证 |
|---|---|---|---|
| F2.1 bash_validation | `tools/builtin/bash_validator.rs` 自实现 | `dasclaw_bash_validation` | `e2e_headless::s2_bash_block` × 5 高危规则 parity |
| F2.2 bash_permissions | 未实现 | 通过 `dasclaw_hooks::bash_permission_hook` | 新增 `req_bash_perm_e2e` |
| F2.3 sandbox | `src/sandbox/` 客户端 wrapper 改为薄适配 | `dasclaw_sandbox` | `e2e_headless::s3_sandbox_block` |
| F2.4 compaction | `agent/compaction.rs` | `dasclaw_core::compaction` | `e2e_headless::a3_compaction` |
| F2.5 submission | `agent/submission.rs` | `dasclaw_core::submission` | 现有 agent loop 测试 |
| F2.6 undo | `agent/undo.rs` | `dasclaw_core::undo` | 现有 undo 测试 |
| F2.7 EgressGate | regression test only | `dasclaw_governance::egress` | `e2e_headless::s4_egress_redact` 三种 EgressKind |
| F2.8 Hook 入口 | F1 完成 | — | 见 F1 |

每项是一个独立 PR，base `xClaw`，互相不 stacked。

### 阶段 F3：B 类反向吸收（按 W6 主航道，按 40-ironclaw-internalization.md 推进）

> 每一项是 W6 子波次，单独 stacked PR 链。本 ADR 只列范围，不重复 W6 既有 issue。

| 子波次 | 移动 | 目标 crate | 关联 |
|---|---|---|---|
| F3.1 | `src/llm/{bedrock,codex_*,github_copilot*,gemini_oauth,nearai_chat,claw_code_provider,circuit_breaker,failover,response_cache,recording,retry,reasoning(+models),image_models,oauth_helpers,error,costs}` | `dasclaw_llm_provider` | W6 现有计划 |
| F3.2 | `src/tools/mcp/{auth,client,config,factory,*_transport,process,protocol,session,mod}` + `cli/mcp` | `dasclaw_mcp` | 新建 W6 子 issue |
| F3.3 | `src/tools/builtin/lsp/{client,protocol,server_config,tool,mod}` | `dasclaw_lsp` | 新建子 issue |
| F3.4 | `src/workspace/{chunker,document,embedding_cache,embeddings,layer,privacy,repository,search}` + `profile.rs` + `timezone.rs` + `WorkspaceError`（**收口 2026-05-22，见 §11**；`mod.rs` 本体、`hygiene.rs`、`db/*` 留 ironclaw） | `dasclaw_workspace_cap` | #698 / #700 / #703 / #705 / #707 / #709 / #711 |
| F3.5 | `src/observability/{log,multi,noop,prompt_cache,traits,mod}` | `dasclaw_observability` | 新建子 issue |
| F3.6 | `src/context/{manager,memory,fallback,state,mod}` + `agent/context_monitor.rs` | `dasclaw_core::context` 子模块 | 新建子 issue |

**搬迁规则**：按 ADR-129 §1.3 verbatim port，禁止任何 simplification；如必须改动，单独 ADR 论证。

### 阶段 F4：C 类清理

仅做依赖图收敛，不动业务逻辑。子波次清单（见 §11.9 修订）：

- **F4.0**：`ironclaw_safety` 整 crate 物理搬到 `crates/dasclaw_safety`（含 fuzz/、tests/、benches/），同步修复 `dasclaw_llm_provider`、`dasclaw_workspace_cap` 等 crate 对桌面端 path 的反向引用。**原"保留代码不动"条款作废**，理由见 §11.9。
- **F4.1**：F3.6 收尾——删 desktop 端 `context/mod.rs`、`agent/context_monitor.rs` 两个薄壳，约 130 处调用方 import 重写。
- **F4.2**：`routines` 实搬——填充 `dasclaw_routines`（当前为 W1 空壳，24 行 trait skeleton），把 desktop `routines/` 的 8.7K LoC verbatim 搬入。
- **F4.3**：`secrets` 桌面薄壳清理——约 60 处调用方 import 改写。
- **F4.4**：`orchestrator` 落点决策 + 搬迁（3.3K LoC）。需补 ADR 决定落点（候选：`dasclaw_runtime::orchestrator` 或新 crate）。
- **F4.5**：`channels` 拆分——抽核心子集（trait + relay + wasm + manager）到新 crate，REPL/HTTP/Signal/Webhook/Web/Telegram 留 ironclaw。
- **F4.6+**：`tools/builtin` 分批搬（21.5K LoC、28 工具，建议 6-8 刀按职能分组）。
- **不纳入 F4**：`import/openclaw`（桌面端历史数据迁移工具，非 headless 核心）。

每个子波次单独 PR、单独可 revert，按 ADR-129 §1.3 verbatim 红线推进。

## 4. 灰度策略

### 4.1 阶段切换前置门

| 阶段 | 启动门 |
|---|---|
| F2 启动 | F1 全部 PR merge + `e2e_headless::s2/s3/s4/s5` 全绿 |
| F3 启动 | F2 全部 PR merge + 完整 `e2e_headless::*` 5 安全 + 4 agent 全绿 |
| F4 启动 | F3 至少一个子波次完成 + 编译通过 + 该子能力 e2e 测试全绿 |

### 4.2 回滚契约

每个 F2/F3 PR 必须满足：
- **单 PR revertable**：commit 内部不依赖 squash；`git revert <merge-sha>` 必须能干净回滚。
- **行为旗标可选**：高风险切换（F2.3 sandbox / F2.7 EgressGate）允许临时引入 `cfg(feature = "fusion_v1")` 让 ironclaw 在 F3 启动前可选择 fallback；但 feature flag 必须在 F4 结束时移除（不允许长期共存，违反 ADR-113 "唯一入口"红线）。
- **降级路径 Fail-Safe**：任何后端切换异常一律拒绝操作，禁止 Fail-Open。

### 4.3 监测信号

接入 `dasclaw_observability::multi`，对每个能力切换记录三类信号：

1. **行为 diff 信号**：HookSpy / EgressSpy 记录在切换前后 1 周对比，必须 0 差异（除时间戳归一化）。
2. **错误率信号**：切换 PR merge 后 24h 内 `area:safety` / `area:agent` 类 issue 增量 ≤ 1。
3. **性能信号**：`benches/safety_check.rs` + `safety_pipeline.rs` 切换前后回归 ≤ 5%（沿用 ironclaw 现有 bench）。

任一信号超阈值 → 回滚该 PR + 单独开 ADR 论证。

## 5. 测试基线

对齐 [end-to-end-validation-plan.md](../testing/end-to-end-validation-plan.md)。

### 5.1 融合前（baseline）

`origin/xClaw` HEAD 必须满足：
- `cargo nextest run --workspace` 全绿
- `cargo nextest run -p dasclaw -E 'test(e2e_headless::)'` 现有套件全绿
- `python3.12 scripts/check_no_panics.py --base origin/xClaw` 0 违规

记录 baseline 报告到 `docs/plans/testing/runlog-fusion-baseline-YYYY-MM-DD.md`。

### 5.2 融合中（每个 F-PR）

每个 F2/F3 PR 必须跑：

```bash
# 1. crate 级 check
cargo check -p <touched-crate> --tests
cargo check -p dasclaw --tests        # 下游影响

# 2. 该 PR 触及能力的 e2e_headless 场景
cargo nextest run -p dasclaw -E 'test(e2e_headless::<scenario>)'

# 3. 关联 bench 不回归
cargo bench -p ironclaw --bench safety_check -- --baseline pre-fusion
```

PR 描述必须含上述三项命令的实际输出。

### 5.3 融合后（F4 结束）

整体验收：

| 维度 | 门槛 |
|---|---|
| `e2e_headless` 安全 S1–S5 | 5/5 全过 |
| `e2e_headless` agent A1–A5 | 5/5 全过（A5 这次必须过，因为多入口契约靠融合解决） |
| `cargo nextest run --workspace` | 全绿 |
| `ironclaw_safety` fuzz target | 1 小时无 panic |
| 性能回归 | ≤ 5% |
| `dasclaw_hooks::count_hook_systems()` 编译期断言 | = 1 |

任一不达标即 F4 不算完成。

## 6. 不动的红线

1. **`desktop-client/ironclaw/crates/ironclaw_safety/`** 不删、不重写、不下沉到 core 命名空间；仅通过 trait 装配。
2. **`desktop-client/ironclaw/src/{routines,orchestrator,channels,import,secrets}/`** 不下沉 core。
3. **`desktop-client/src/dlp/`** 不属于本 ADR 范围；由 ADR-148 PR-A 跟进。
4. **HookEngine 唯一入口**：F4 结束后 `dasclaw_hooks::count_hook_systems()` 必须恒等于 1，任何引入第二条 hook chain 的 PR 必须先废止该断言并写新 ADR。
5. **公共接口兼容**：F2 切换不得改 `dasclaw_*` 已公开 API；如必须改，先开 ADR 论证。
6. **verbatim port 纯度**：F3 反向吸收禁止"顺手简化"；任何重构必须延后到 F4 之后单独立项。

## 7. 风险与对策

| 风险 | 概率 | 对策 |
|---|---|---|
| F3 多文件移动触发 ADR-114 grep guard 假阳 | 高 | 每个 F3 子 PR 显式声明 ADR-114 类 A 或带 `adr-114-class-b` label |
| F2.3 sandbox 切换在 Linux/macOS 行为差异 | 中 | 5.2 中 `e2e_headless::s3` 同时跑 landlock + seatbelt；Windows 走 #481 单独 ADR |
| F3.1 LLM provider 反向吸收触发 W6 现有计划冲突 | 中 | F3 子波次开始前与 W6 owner 对账；同一 provider 不并行 |
| ironclaw_safety 通过 trait 接入后被绕过 | 低（编译期断言兜底） | F1.3 编译期断言 + F5.3 fuzz target 双重保险 |
| F4 验收期间发现 D 类（DLP）依赖意外渗入 core | 低 | 5.3 校验时 `grep -r '/dlp/' crates/` 必须为空 |

## 8. 时间与所有权

- 本 ADR 不锁定具体日期；推进节奏由 W6 主航道 + #621 owner 决定。
- F1 owner：dasclaw_hooks crate maintainer
- F2 owner：每项一个 issue assignee
- F3 owner：W6 sub-wave owner
- F4 owner：架构小组联合审

## 9. 关联 ADR / 文档

- 上游：[ADR-113 hook-engine-unification](adr-113-hook-engine-unification.md)、[ADR-112 compatibility-evaluation](adr-112-compatibility-evaluation.md)、[ADR-114 dasclaw-rebrand](adr-114-dasclaw-rebrand.md)、[ADR-129 sandbox-windows-windows-crate-adoption](adr-129-sandbox-windows-windows-crate-adoption.md) §1.3 verbatim port、[ADR-147 composite-safety-hook](adr-147-composite-safety-hook.md)、[ADR-148 egress-gate-safety-hook-semantics](adr-148-egress-gate-safety-hook-semantics.md)、[ADR-149 tool-visibility-triple-gate](adr-149-tool-visibility-triple-gate.md)
- 同级：[31-target-architecture.md](31-target-architecture.md) / [32-execution-plan.md](32-execution-plan.md) / [38-desktop-client-ironclaw-fork-inventory.md](38-desktop-client-ironclaw-fork-inventory.md) / [40-ironclaw-internalization.md](40-ironclaw-internalization.md)
- 证据底：[dual-graph-stats.md](dual-graph-stats.md) / [capability-comparison.md](capability-comparison.md)
- 测试：[../testing/end-to-end-validation-plan.md](../testing/end-to-end-validation-plan.md)

## 10. Sources read

- code-review-graph SQLite DB（commit `80d41c90`）
- [docs/plans/architecture-refactor/31-target-architecture.md](31-target-architecture.md) §L1–L6
- [docs/plans/architecture-refactor/32-execution-plan.md](32-execution-plan.md) §W3-A / W6
- [docs/plans/architecture-refactor/38-desktop-client-ironclaw-fork-inventory.md](38-desktop-client-ironclaw-fork-inventory.md) §fork 清单
- [docs/plans/architecture-refactor/40-ironclaw-internalization.md](40-ironclaw-internalization.md) §吸收路径
- [docs/plans/architecture-refactor/adr-112-compatibility-evaluation.md](adr-112-compatibility-evaluation.md) §14 能力 × 3 harness
- [docs/plans/architecture-refactor/adr-113-hook-engine-unification.md](adr-113-hook-engine-unification.md) §HookEngine + 4 trait
- [docs/plans/architecture-refactor/adr-114-dasclaw-rebrand.md](adr-114-dasclaw-rebrand.md) §grep guard
- [docs/plans/architecture-refactor/adr-129-sandbox-windows-windows-crate-adoption.md](adr-129-sandbox-windows-windows-crate-adoption.md) §1.3 verbatim port
- [docs/plans/architecture-refactor/adr-147-composite-safety-hook.md](adr-147-composite-safety-hook.md)
- [docs/plans/architecture-refactor/adr-148-egress-gate-safety-hook-semantics.md](adr-148-egress-gate-safety-hook-semantics.md)
- [docs/plans/architecture-refactor/adr-149-tool-visibility-triple-gate.md](adr-149-tool-visibility-triple-gate.md)
- [docs/plans/architecture-refactor/dual-graph-stats.md](dual-graph-stats.md)（PR #622）
- [docs/plans/architecture-refactor/capability-comparison.md](capability-comparison.md)（PR #623）
- [docs/plans/testing/end-to-end-validation-plan.md](../testing/end-to-end-validation-plan.md)

## 11. F3.4 修订（2026-05-22）

> 修订人：本次会话 agent；签字关联 issue #712。

### 11.1 原计划

把 `src/workspace/` 整目录 + Workspace 本体 verbatim 搬到 `dasclaw_workspace_cap`。

### 11.2 实际进展

10 个 preparatory slice 已合入 `xClaw`（PR #698 / #700 / #703 / #705 / #707 / #709 / #711 等）。已搬迁的零件：

- 数据层：`document`（含 `MemoryDocument` / `MemoryChunk` / `WorkspaceEntry`）、`embeddings`、`embedding_cache`、`chunker`
- 算法层：`search`（FTS / 向量 / RRF）、`layer`、`privacy`、`policy`
- 工具层：`sanitization`（含 `is_system_prompt_file` / `reject_if_injected`）、`profile`（含 `PsychographicProfile` + `ANALYSIS_FRAMEWORK` + `PROFILE_JSON_SCHEMA`）、`timezone`
- 基础：`error::WorkspaceError`、`config::WorkspaceSearchConfig`、`repository`（postgres feature 门控）

cap crate 当前 157 个测试全绿，含双 feature（default + postgres）。

### 11.3 阻塞事实（三层验证）

准备搬 Workspace 本体 + `db::Database` trait 时调研得到：

1. `desktop-client/ironclaw/src/db/mod.rs:32-42` import 头：

    ```rust
    use crate::agent::BrokenTool;
    use crate::agent::routine::{Routine, RoutineRun, RunStatus};
    use crate::context::{ActionRecord, JobContext, JobState};
    use crate::history::{...};
    ```

2. 这些类型进入 `Database` trait 方法签名（非临时局部使用）。

3. 三棵子树体量：
   - `src/context/` = 5 文件
   - `src/history/` = 3 文件
   - `src/agent/` = 18 文件

4. 这三棵子树属于宿主职责（IO 适配 / 对话状态 / 运行时调度），不属于 Workspace 能力域。把它们 verbatim 搬到 `dasclaw_workspace_cap` 会让 cap 失去"小而专"的属性，违背 ADR-152 §1 的"反向吸收"目标。

### 11.4 决策

**F3.4 收口于 slice 10。Workspace 本体（`workspace/mod.rs`）和 `hygiene.rs` 留 ironclaw 作为参考组合。**

理由：

1. **架构层面**：cap 已经提供所有可复用的数据 + 算法零件。其他宿主想用 Workspace，可以基于 cap 零件自行组装；不必继承 ironclaw 的那一份具体组合。这正符合 Hexagonal / Ports & Adapters 推荐方向：核心域不知道数据库存在。
2. **红线层面**：继续搬需要要么破 ADR-129 §1.3 verbatim 红线（引入新抽象 trait），要么把 context/history/agent 整树搬过去（cap 失去定位）。两个都比"收口"代价大。
3. **零返工**：已合入的 10 个 slice 全部保留收益，不需要回退任何代码。

### 11.5 未迁移项清单（明确留 ironclaw）

| 文件 | 原因 |
|---|---|
| `desktop-client/ironclaw/src/workspace/mod.rs`（Workspace 本体） | 内部持有 `Arc<dyn crate::db::Database>`，与 IO 抽象耦合 |
| `desktop-client/ironclaw/src/workspace/hygiene.rs` | 依赖 Workspace + `crate::bootstrap::dasclaw_base_dir`（宿主路径策略） |
| `desktop-client/ironclaw/src/db/` 整树 | 数据库适配层，属于宿主职责 |

### 11.6 后续宿主复用规约

其他宿主（codex / claw-code / 第三方）若想复用 Workspace 能力：

1. **零件**：直接 `use dasclaw_workspace_cap::{document, search, embeddings, ...}` 即可
2. **组合**：自行实现 Workspace 等价物，决定自己的持久化方式（不必 `dyn Database`）
3. **参考**：可对照 ironclaw `workspace/mod.rs` 的组合方式，但不强制继承

### 11.7 对 F3.5 / F3.6 的影响

无。F3.5（observability）和 F3.6（context manager）的范围、目标、ADR-129 verbatim 约束不变。

F3.6 涉及 `crate::context/`，如果后续也遇到类似"trait 方法签名拖入宿主类型"的阻塞，可参照本次修订模式，单独评估收口策略。

> **更新（#719）**：F3.6 推进到 slice 3 时确实遇到了类似阻塞。详见 §11.8。

---

## 11.8 修订 2：F3.6 阻塞与方向（#719，draft）

> **状态**：草案，待评审决策。
> **触发**：F3.6 slice 1（#715）+ slice 2（#717）合入后，准备搬 `context::manager` 时发现死结。

### 11.8.1 已完成的零件搬迁

| Slice | 内容 | PR | 落点 |
|---|---|---|---|
| 1/6 | `context::memory`（`Memory` / `ConversationMemory` / `ActionRecord`） | #714 | `dasclaw_core::context::memory` |
| 2/6 | `JobError` | #718 | `dasclaw_core::error` |
| — | 早先 F3.2 phase 2 PR 3（#641） | 已合入 | `dasclaw_runtime::job` 已含 `JobState` / `StateTransition` / `TokenBudgetExceeded` |

桌面端 `desktop-client/ironclaw/src/{context/mod.rs, error.rs}` 全部用 `pub use` re-export 保持源码层兼容。

### 11.8.2 阻塞事实（三层验证）

1. **manager 生产代码对 JobContext 的访问深度**（grep + 人读）：仅 `ctx.{job_id, state, started_at}` 字段 + `ctx.transition_to()` / `ctx.mark_stuck()` 方法。都属于状态机骨架。
2. **JobContext 类型定义**（`desktop-client/ironclaw/src/context/state.rs:25-110`）持有桌面端类型：
   - `Option<Arc<dyn HttpInterceptor>>`（来自 `crate::llm::recording`）
   - `SharedFeatureFlags` / `ToolFeatureFlags`（来自 `crate::tools::feature_flags`）
   - `Arc<tokio::sync::RwLock<HashMap<String, String>>>` 工具输出暂存
3. **依赖方向约束**：`dasclaw_core` 是底层 crate，不能反向依赖宿主 `dasclaw`。

结论：`manager.rs` 不能在 `JobContext` 形态保持现状的前提下 verbatim 搬到 `dasclaw_core`。同理 `state.rs`、`fallback.rs`。

### 11.8.3 第一性问题：无头 agent 框架需不需要 ContextManager？

在挑搬迁路线之前，先回答 ADR-152 的本来问题：**这块代码是不是"反向吸收"目标的一部分**。

`ContextManager`（1392 行 / 20 个公开方法）实际承担的能力：

| 能力 | 方法 | 无头 agent 框架是否需要 |
|---|---|---|
| 多 job 并发上限 + TOCTOU 保护 | `create_job` / `insert_context` / `MaxJobsExceeded` | **需要**：任何同时跑多个任务的 runtime 都要并发上限 |
| 按 user_id 分桶 | `active_jobs_for` / `all_jobs_for` / `parallel_blocking_count_for` | **需要**：多用户 / 多 tenant 隔离 |
| 容器化任务 ID 预分配 | `register_sandbox_job` | **需要**：sandbox / Docker 标签必须在容器创建前拿到 UUID |
| stuck job 检测 | `find_stuck_jobs` / `find_stuck_jobs_with_threshold` | **需要**：长跑 agent 必备的看门狗 |
| summary 聚合 | `summary` / `summary_for` | **需要**：监控 / 健康检查 |
| memory + context 双 RwLock 协调 | `update_context_and_get` / `update_memory` | **需要**：和 `Memory`（已搬入 dasclaw_core）天然配对 |

结论：**ContextManager 不是 ironclaw 私货，它是任何无头 agent 框架都要重写一遍的基础设施**。和 §11.4 的 Workspace（强 GUI / 强 DB 绑定）性质不一样。Workspace 适合收口，ContextManager **不适合收口**。

### 11.8.4 候选方向（修订）

#### 方向 X：JobContext 字段拆分 + ContextManager 搬迁（推荐）

关键观察：**ContextManager 没碰 `JobContext` 的桌面端字段**——它不读不写 `http_interceptor` / `feature_flags` / `tool_output_stash` / `tools` / `metadata`，只摸状态机骨架。"god-struct"的污染范围比看上去窄。

拆分方案：

```rust
// dasclaw_core::context::state
pub struct JobContextCore {
    pub job_id: Uuid,
    pub state: JobState,
    pub started_at: Option<DateTime<Utc>>,
    pub user_id: String,
    pub title: String,
    pub description: String,
    pub timezone: Option<Tz>,
    pub requester_id: Option<String>,
    pub cost_usd: f64,
    pub tokens_used: u64,
    pub token_budget: Option<u64>,
    // 状态机方法
    pub fn transition_to(...) -> Result<...>;
    pub fn mark_stuck(...) -> Result<...>;
    pub fn elapsed() -> ...;
    pub fn add_cost(...) / pub fn add_tokens(...);
    pub fn budget_exceeded() -> bool;
}
```

桌面端：

```rust
// desktop-client/ironclaw/src/context/state.rs
pub struct JobContext {
    pub core: JobContextCore,                              // 共享骨架
    pub http_interceptor: Option<Arc<dyn HttpInterceptor>>, // 桌面私货
    pub feature_flags: SharedFeatureFlags,
    pub tool_output_stash: Arc<RwLock<HashMap<String, String>>>,
    pub tools: ...,
    pub metadata: ...,
}
impl Deref for JobContext { type Target = JobContextCore; fn deref(&self) -> &JobContextCore { &self.core } }
impl DerefMut for JobContext { fn deref_mut(&mut self) -> &mut JobContextCore { &mut self.core } }
```

ContextManager 搬到 `dasclaw_core::context::manager`，泛型化为 `ContextManager<C = JobContextCore>` 或直接持 `JobContextCore`，桌面端实例化时仍可由 worker 单独维护一个 `HashMap<Uuid, DesktopExtensions>`（持有 http_interceptor 等）副向表。

**代价**：

- 破 ADR-129 §1.3 verbatim port 红线。本草案为这一次破例提供依据（详 §11.8.5 的红线豁免条款）。
- 触面：所有读 `ctx.http_interceptor` / `ctx.feature_flags` 等字段的位置要改为读 `desktop_extensions` 侧表，或保留 `JobContext` 直接字段（如上 struct 定义所示，桌面端 `JobContext` 仍持有这些字段）。
- 测试需要少量调整（mock JobContextCore 或直接构造 JobContext）。
- PR 体量中等，可切片化：拆 struct（slice A）→ 搬 manager（slice B）→ 搬 state.rs 剩余部分（slice C）→ 桌面端 mod.rs shim（slice D）。

**收益**：

- 真正达成 ADR-152 §1 的"反向吸收"目标：把 agent runtime 必需的多 job 控制器交给所有宿主复用。
- 不需要 trait 抽象（字段拆分比 trait 简单且零运行时开销）。
- `JobContext::core` 暴露后，桌面端外的宿主可以 `let ctx = JobContextCore::new(...)` 直接用。

#### 方向 X'：trait 化（不推荐）

抽 `JobContextCore` trait 而非拆 struct。比字段拆分复杂（要写 trait method 转发）、有动态分发开销、桌面端要写 `impl JobContextCore for JobContext`。除非有"同一 manager 同时管多种 context 类型"的需求（目前没有），否则不应选这条。

#### 方向 Y：F3.6 收口（不推荐）

> ~~本草案 v1 推荐 Y。修订后撤回。~~

参照 §11.4 模式，停在 slice 2。理由曾是"避免破 verbatim 红线"+"避免大重构"。

**为什么撤回**：

- §11.8.3 表格已证明 ContextManager 是无头 agent 必需基础设施，不是 ironclaw 私货。
- §11.4 收口的 Workspace 是 GUI / DB 强绑定，与 ContextManager 性质不同。
- 收口意味着任何想做无头 agent 的宿主都要重写 1392 行类似代码，违反 ADR-152 §1 的反向吸收目标。

### 11.8.5 ADR-129 §1.3 红线一次性豁免

本草案为 F3.6 的字段拆分申请 **ADR-129 §1.3 verbatim port 红线一次性豁免**，条件：

1. 仅限 `JobContext` 一个类型的字段拆分（不蔓延到其他 god-struct）。
2. 拆分后 `JobContextCore` 字段集合与原 `JobContext` 中可被桌面端外宿主复用的字段**一一对应**（不增不减、不改语义）。
3. 拆分 PR 必须附 before/after 字段映射表，由 reviewer 逐项核对。
4. 桌面端 `JobContext` 在拆分后仍能通过 `Deref` 提供原字段访问语法，调用方代码改动量限制在 **机械化 `ctx.field` → `ctx.core.field` 之外不动业务逻辑**。
5. 拆分 PR 自身不引入新功能、不改并发模型、不动状态机语义。

### 11.8.6 决策落定后的动作

若选 **X（字段拆分，推荐）**：

- 在 §11.8.7 写下字段分配最终清单（哪些进 `JobContextCore` / 哪些留桌面端）。
- 拆 **两刀**（不再细分到 5 个 slice，避免开发拖太久）：
  - **slice A — 字段拆分 + ContextManager 搬迁一刀到位**：
    `JobContextCore` struct 拆分 + `state.rs` 共享部分 + `ContextManager` / `manager.rs` 整体搬入 `dasclaw_core::context`；桌面端 `JobContext` 改组合 + `Deref`/`DerefMut`；桌面端 `context/mod.rs` 同步改 re-export shim。一个 PR 闭环。
  - **slice B — `fallback.rs` 评估 + 收尾**：
    判定 `fallback.rs` 是搬还是留 ironclaw（按是否含桌面端字段依赖决定）；清理桌面端剩余 import；关闭 umbrella #632。

若选 **X'（trait 化，不推荐但允许）**：

- 在 §11.8.7 冻结 trait method 清单 + dyn vs generic 选择。
- 同样按"字段/trait 拆 + manager 搬迁"两刀，不细切。

若选 **Y（收口，不推荐）**：

- §11.8.7 写明"F3.6 收口于 slice 2"+ 承认产品上放弃 ContextManager 复用。
- 更新 umbrella #632；标 §3 F3.6 行 "Scope clarified — see §11.8"。

### 11.8.7 决策记录

**决策**：**X（JobContext 字段拆分 + ContextManager 搬迁）**

**决策时间**：随本 PR commit v3。

**决策理由**：见 §11.8.3 表格——ContextManager 是任何无头 agent 框架必备基础设施，符合 ADR-152 §1 反向吸收目标；且 manager 不碰 JobContext 桌面端字段，字段拆分（X）比 trait 化（X'）干净。

**字段分配清单**（slice A 实施时按本表执行，PR 内附 before/after 字段映射核对表）：

进 `dasclaw_core::context::JobContextCore` 的字段（manager + 任何无头 agent runtime 都需要的状态机骨架 + 计费 + 元数据）：

- `job_id: Uuid`
- `state: JobState`
- `started_at: Option<DateTime<Utc>>`
- `user_id: String`
- `title: String`
- `description: String`
- `timezone: Option<Tz>`
- `requester_id: Option<String>`
- `cost_usd: f64`
- `tokens_used: u64`
- `token_budget: Option<u64>`
- （其他原 JobContext 中**不依赖桌面端类型**的字段，slice A PR 内 grep 一遍补齐）

进 `dasclaw_core::context::JobContextCore` 的方法（manager 已经在用 + 状态机基础动作）：

- `new` / `with_user` / `with_timezone` / `with_feature_flags`（如果不引入桌面端类型）/ `with_requester_id`
- `transition_to`
- `mark_stuck`
- `attempt_recovery`
- `elapsed`
- `add_cost` / `add_tokens` / `budget_exceeded`

留桌面端 `JobContext` 的字段（桌面端 GUI / LLM 录制 / 工具系统私货）：

- `http_interceptor: Option<Arc<dyn HttpInterceptor>>`
- `feature_flags: SharedFeatureFlags`
- `tool_output_stash: Arc<RwLock<HashMap<String, String>>>`
- `tools: ...`
- `metadata: ...`
- 任何其他依赖 `crate::llm::*` / `crate::tools::*` / `crate::gui::*` 的字段（slice A PR 内 grep 补齐）

**桌面端 `JobContext` 形态**：

```rust
pub struct JobContext {
    pub core: JobContextCore,
    pub http_interceptor: Option<Arc<dyn HttpInterceptor>>,
    pub feature_flags: SharedFeatureFlags,
    pub tool_output_stash: Arc<RwLock<HashMap<String, String>>>,
    // ...
}
impl Deref for JobContext { type Target = JobContextCore; ... }
impl DerefMut for JobContext { ... }
```

**slice A 验证清单**（PR 必跑）：

- `cargo check -p dasclaw_core -p dasclaw --tests`
- `cargo nextest run -p dasclaw_core -p dasclaw`
- `cargo fmt --all`
- `python3.12 scripts/check_no_panics.py --base origin/xClaw`
- `cargo clippy --no-deps -p dasclaw_core -p dasclaw --all-targets -- -D warnings`
- PR body 附 before/after 字段映射表（§11.8.5 第 3 条要求）

**待 slice A 完成后**：在本节追加"slice A 完成于 PR #N"标记；slice B 决策另写。

### 11.8.8 事实更正（v4，撤回 X 决策）

> 本节为 v4 commit 追加。v1（推荐 Y）→ v2（推荐 X）→ v3（决策 X + 字段清单）→ **v4：撤回 X，改纯 verbatim 搬迁**。

#### 11.8.8.1 事实错误

§11.8.2 第 2 条"`JobContext` 持有桌面端类型"是**事实错误**。重新核实当前代码：

| 之前以为是桌面端类型 | 真实落点 | 桌面端形态 |
|---|---|---|
| `HttpInterceptor` trait | `dasclaw_runtime::recording::HttpInterceptor` | `crate::llm::recording` 是 re-export shim（行 32 `pub use ...HttpInterceptor`） |
| `SharedFeatureFlags` / `ToolFeatureFlags` | `dasclaw_runtime::feature_flags` | `crate::tools::feature_flags` 是 re-export shim（行 20 `pub use dasclaw_runtime::feature_flags::{...}`） |
| `JobContextCore` trait | `dasclaw_runtime::job_context` | `JobContext` 已 `impl JobContextCore`（state.rs 行 269+） |

`dasclaw_core` 已经依赖 `dasclaw_runtime`，所以以上 trait/类型在 `dasclaw_core::context::state` 中可以直接 `use dasclaw_runtime::recording::HttpInterceptor` 等正常引用。

#### 11.8.8.2 真实情况

- **`state.rs` 共 492 行**：实际 outside-crate 引用只有 `std`、`chrono`、`rust_decimal`、`serde`、`uuid`、`dasclaw_runtime`、`crate::llm::recording`（shim）、`crate::tools::feature_flags`（shim）。
- **`manager.rs` 共 1392 行**：唯一 outside-context 引用是 `use crate::error::JobError`（已搬入 `dasclaw_core::error`）。
- **`fallback.rs` 共 319 行**：grep 显示无任何桌面端独有 crate 引用。

**结论**：三个文件全部满足 ADR-129 §1.3 verbatim port 形态——只改 import 路径，不动结构、不动业务逻辑、不动字段、不动 trait method 签名。

#### 11.8.8.3 撤回 X 路线、撤回红线豁免

- §11.8.4 推荐方向 X（字段拆分）：**撤回**。X 解决的是不存在的问题。
- §11.8.5 ADR-129 §1.3 一次性红线豁免条款：**撤回**。verbatim 搬迁本身就符合红线，无需豁免。
- §11.8.6 X 的 slice A 字段拆分计划：**撤回**。
- §11.8.7 决策 = X：**撤回**。

#### 11.8.8.4 新方向：verbatim 一刀搬迁

**slice A'（替代 §11.8.6 slice A）**：

- `git mv desktop-client/ironclaw/src/context/state.rs` → `crates/dasclaw_core/src/context/state.rs`
- `git mv desktop-client/ironclaw/src/context/manager.rs` → `crates/dasclaw_core/src/context/manager.rs`
- `git mv desktop-client/ironclaw/src/context/fallback.rs` → `crates/dasclaw_core/src/context/fallback.rs`
- 三个文件内部仅改 import：
  - `crate::llm::recording::HttpInterceptor` → `dasclaw_runtime::recording::HttpInterceptor`
  - `crate::tools::feature_flags::{SharedFeatureFlags, ToolFeatureFlags}` → `dasclaw_runtime::feature_flags::{SharedFeatureFlags, ToolFeatureFlags}`
  - `crate::error::JobError` → `crate::error::JobError`（在新 crate 内仍叫这个名，dasclaw_core::error::JobError 已就位）
  - `crate::context::{JobContext, JobState, Memory}` → `crate::context::{JobContext, Memory}` + `dasclaw_runtime::JobState`
- `crates/dasclaw_core/src/context/mod.rs` 暴露 `pub mod state; pub mod manager; pub mod fallback;` + 顶层 re-export `pub use state::JobContext; pub use manager::{ContextManager, ContextSummary};` 等。
- 桌面端 `desktop-client/ironclaw/src/context/mod.rs` 改为 `pub use dasclaw_core::context::{Memory, JobContext, ContextManager, ContextSummary, ...};`（与 slice 1 同形）。
- 桌面端 `state.rs` / `manager.rs` / `fallback.rs` 物理删除（git mv 已带走）。

**ADR-154 step 2 影响**：因为 `JobContext` 现在落在 dasclaw_core 而不再 ironclaw 私有，`impl JobContextCore for JobContext` 也跟着搬过去，但不构成结构性改动（同样是纯 import rebase）。

#### 11.8.8.5 slice A' 验证清单

- `cargo check -p dasclaw_core -p dasclaw --tests`
- `cargo nextest run -p dasclaw_core -p dasclaw`
- `cargo fmt --all`
- `python3.12 scripts/check_no_panics.py --base origin/xClaw`
- `cargo clippy --no-deps -p dasclaw_core -p dasclaw --all-targets -- -D warnings`
- PR body 列出三个 `git mv` 命令 + 改动的 import 行号（review 可对照确认 verbatim）。

#### 11.8.8.6 教训

读旧注释（"the god-struct split is the follow-up sub-PR"）就接受了它，没核实当前事实。源码注释会过期，做迁移决策前必须先 grep 当前代码确认类型实际落点。三层验证应该包含"事实核实"层。

#### 11.8.8.7 新决策

**新决策 = verbatim 一刀搬迁（slice A'）**。slice B（fallback.rs）已并入 slice A'，剩下只需在 slice A' 合入后关闭 umbrella #632 + 标 §3 F3.6 行 "Done"。

### 11.9 F4 修订：ironclaw_safety 路径倒置修复（F4.0）

**问题**

ADR-152 §3 F4 原条款"`ironclaw_safety` 保留代码不动"在 F3 推进过程中被实际依赖图证伪：

- `crates/dasclaw_llm_provider/Cargo.toml` 通过 `path = "../../desktop-client/ironclaw/crates/ironclaw_safety"` 反向引用桌面端 crate（`LeakDetector` 调用）。
- `crates/dasclaw_workspace_cap/Cargo.toml` 同样反向 path 引用（`Sanitizer`、`Severity`、`SafetyLayer`）。
- 顶层 `Cargo.toml` workspace `members` 已把 `desktop-client/ironclaw/crates/ironclaw_safety` 列为成员。
- 全工作区 `use ironclaw_safety::*` 调用共 65 处（排除上游参考目录 `ironclaw-main/`）。

这违反 `crates/` 作为底层框架不应反向依赖上层 `desktop-client/` 的依赖方向原则，且 crate 命名空间不符 `dasclaw_*` 约定。

**修订**

1. F4 增设 **F4.0 子波次**：把 `ironclaw_safety` 整 crate 物理搬到 `crates/dasclaw_safety`，含本体 + `fuzz/` + `tests/` + `benches/` + corpus 文件。
2. crate 重命名 `ironclaw_safety` → `dasclaw_safety`，所有 `use ironclaw_safety::*` 改为 `use dasclaw_safety::*`（约 65 处机械替换）。
3. 同步修正 8 处 `Cargo.toml`：
   - 顶层 `Cargo.toml` workspace `members` 列表
   - `crates/dasclaw_llm_provider/Cargo.toml` path 引用改为 `../dasclaw_safety`
   - `crates/dasclaw_workspace_cap/Cargo.toml` path 引用改为 `../dasclaw_safety`
   - `desktop-client/Cargo.toml`、`desktop-client/ironclaw/Cargo.toml` 改 path
   - 本体 `Cargo.toml`、`fuzz/Cargo.toml` 改 name
4. 上游参考目录 `ironclaw-main/crates/ironclaw_safety/` 不动（属上游镜像，不在工作区构建）。

**约束**

- 严格遵守 ADR-129 §1.3 verbatim：只动 crate 物理位置 + name + import path，**不动任何逻辑、API、测试用例**。
- 单 PR revertable：`git revert <merge-sha>` 必须能干净回滚。
- 验证门：`cargo nextest run -p dasclaw_safety` + `cargo nextest run -p dasclaw_llm_provider -p dasclaw_workspace_cap -p dasclaw` 全绿，`cargo clippy --workspace -- -D warnings` 通过。

**对其他子波次的影响**

- 修复路径倒置后，`dasclaw_llm_provider` / `dasclaw_workspace_cap` 不再反向引用 desktop。
- 不影响 F3.6 收尾（F4.1）、`routines`/`orchestrator`/`channels` 搬迁（F4.2–F4.5）的范围与顺序。
- SafetyHook trait 装配 HookEngine 的设计（原 F4 条款）仍有效，可在 F4.0 之后作为 F4 后续工作单独推进。

---

Closes #621
