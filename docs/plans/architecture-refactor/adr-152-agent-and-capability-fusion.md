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
| F3.4 | `src/workspace/{chunker,document,embedding_cache,embeddings,hygiene,layer,privacy,repository,search,mod}` | `dasclaw_workspace_cap` | 新建子 issue |
| F3.5 | `src/observability/{log,multi,noop,prompt_cache,traits,mod}` | `dasclaw_observability` | 新建子 issue |
| F3.6 | `src/context/{manager,memory,fallback,state,mod}` + `agent/context_monitor.rs` | `dasclaw_core::context` 子模块 | 新建子 issue |

**搬迁规则**：按 ADR-129 §1.3 verbatim port，禁止任何 simplification；如必须改动，单独 ADR 论证。

### 阶段 F4：C 类清理

仅做依赖图收敛，不动业务逻辑：

- `ironclaw_safety` 通过 SafetyHook trait 装配到 core HookEngine（F1.2 完成后自动生效），保留代码不动。
- `routines` / `orchestrator` / `channels` / `import` / `secrets` / `tools_builtin` 检查依赖是否还指向 ironclaw 内部已删除的模块；如有则改 import 到 core。

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

---

Closes #621
