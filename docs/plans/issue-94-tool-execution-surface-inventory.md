# 工具执行入口审批契约 — 现状盘点矩阵

> **Status**: research/draft（非实施 PR；仅供 #94 实施前的人类 review）
> **Related**: #94 — `[P0-H/W4] Tool execution surface parity across chat, jobs, routines, and containers`
> **Constraint**: #94 标了 `adr-redline`，本文件仅记录调研结果，不引入实施 PR
> **Last updated**: 2026-05-18

## 1. 目的

#94 要求"识别所有可执行工具的代码路径，对账每个路径是否都走了同一套安全门"。本文件是该盘点结果的**当前状态快照**，供人类 review 后再决定实施顺序。

## 2. 8 个安全门（gate）清单

按 #94 验收标准，每个工具执行入口都应该通过以下 8 个门：

| # | Gate | 实现位置 | 失败时行为 |
|---|------|---------|-----------|
| G1 | **策略可见性 (executor preflight)** | `ToolRegistry::policy_check_executor` (ADR-149) | Fail-safe → `ToolError::Disabled` |
| G2 | **审批检查** | `tool.requires_approval()` + `ApprovalContext::is_blocked_or_default()` | 后台路径 → `autonomous_unavailable_error` |
| G3 | **限流** | `RateLimiter::check_and_record` | `ToolError::RateLimited` |
| G4 | **BeforeToolCall hook** | `hooks.run(HookEvent::ToolCall)` | 拒绝/失败 → `ExecutionFailed` |
| G5 | **参数校验** | `SafetyLayer::validator().validate_tool_params` | `InvalidParameters` |
| G6 | **超时执行** | `tokio::time::timeout(tool.execution_timeout(), tool.execute(...))` | `Timeout` |
| G7 | **输出脱敏** | `safety::egress::sanitize_tool_output_via_egress` (ADR-148 R2 / PR #615) | 必须经过，否则原文外泄 |
| G8 | **审计/状态事件** | `StatusUpdate::tool_completed` + audit log | 缺失 → 取证盲区 |

## 3. 共享 helper：`execute_tool_with_safety`

**位置**: [`desktop-client/ironclaw/src/tools/execute.rs:20`](../desktop-client/ironclaw/src/tools/execute.rs#L20)

**文档注释原话**：
> "This is the single canonical implementation of tool execution. All consumers (chat dispatcher, job worker, container runtime, scheduler subtasks) use this function instead of maintaining their own copies."

**实际覆盖的 gate**：G1（已接） / G5 / G6 / 部分 G8（debug log，但不进 channel status）

**实际未覆盖的 gate**：G2 / G3 / G4 / G7 — 这些**全部留给调用方各自处理**。

> 也就是说，目前共享 helper 只解决了"参数校验 + 超时执行 + 策略可见性"，**审批 / 限流 / hook / 脱敏 4 项关键安全门仍然分散在各入口里**。这是 #94 的核心痛点。

## 4. 生产路径盘点矩阵

> 表中 ✅ = 当前已挂；❌ = 当前未挂；⚠️ = 部分实现 / 不一致

| # | 执行入口 | 代码位置 | G1<br/>策略 | G2<br/>审批 | G3<br/>限流 | G4<br/>hook | G5<br/>参数 | G6<br/>超时 | G7<br/>脱敏 | G8<br/>审计 | 当前用 helper |
|---|---------|---------|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| 1 | **聊天主路径** | [`agent/dispatcher.rs:587-1500`](../desktop-client/ironclaw/src/agent/dispatcher.rs#L587) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ (#615) | ✅ | ✅ `execute_tool_with_safety` |
| 2 | **后台 job 单次执行** | [`worker/job.rs:568-810`](../desktop-client/ironclaw/src/worker/job.rs#L568) | ⚠️ 手写 | ✅ 手写 | ✅ 手写 | ✅ 手写 | ✅ 手写 | ✅ 手写 | ✅ (#615) | ✅ | ❌ 自己拼 |
| 3 | **后台 job 并行执行** | [`worker/job.rs::execute_tools_parallel:504`](../desktop-client/ironclaw/src/worker/job.rs#L504) | ⚠️ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ 委托给 #2 |
| 4 | **agent 内 job loop** | [`worker/job.rs:1717`](../desktop-client/ironclaw/src/worker/job.rs#L1717) | ⚠️ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ 委托给 #2 |
| 5 | **容器子 agent** | [`worker/container.rs:560-606`](../desktop-client/ironclaw/src/worker/container.rs#L606) | ✅ | ⚠️ 部分 | ❌ | ❌ | ✅ | ✅ | ✅ | ⚠️ | ✅ `execute_tool_simple` |
| 6 | **routine 调度器** | [`routines/scheduler.rs:563-602`](../desktop-client/ironclaw/src/routines/scheduler.rs#L602) | ✅ | ⚠️ | ⚠️ | ⚠️ | ✅ | ✅ | ✅ | ⚠️ | ✅ `execute_tool_with_safety` |
| 7 | **routine 引擎（轻量）** | [`routines/routine_engine.rs::execute_routine_tool:1784-1830`](../desktop-client/ironclaw/src/routines/routine_engine.rs#L1784) | ❌ | ✅ 仅 allowlist | ❌ | ❌ | ✅ | ✅ | ✅ | ⚠️ | ❌ 自己拼 |
| 8 | **webhook 触发** | [`webhooks/mod.rs:124-200`](../desktop-client/ironclaw/src/webhooks/mod.rs#L190) | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ⚠️ | ❌ **全裸** |
| 9 | **builder 构建工具** | [`tools/builder/core.rs::execute_build_tool:780-800`](../desktop-client/ironclaw/src/tools/builder/core.rs#L780) | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ **全裸** |
| 10 | **CLI manual 调用** | [`cli/tool.rs::run_tool_command:119`](../desktop-client/ironclaw/src/cli/tool.rs#L119) | ? | ? | ? | ? | ? | ? | ? | ? | ❌ 待核 |

## 5. 关键发现

### 5.1 真正高风险的两个"全裸"入口

**#8 webhook 触发** (`webhooks/mod.rs:190`):
```rust
let output = match tool_impl.execute(params, &ctx).await {
    ...
}
```
直接调用 `tool.execute()`，**完全不过 G1-G7 任何一个 gate**。任意通过 webhook auth 的请求 → 直接执行工具 → 任何脱敏 / 限流 / hook 都不会触发。

**#9 builder 构建工具** (`tools/builder/core.rs:800`):
```rust
tool.execute(normalized_params, &ctx).await
```
同样直接调用，且使用 `JobContext::default()`（无 user_id！），所有 user-scoped 检查（限流 / 审批）天然失效。

### 5.2 三种"未走 helper"的模式

| 模式 | 案例 | 风险 |
|------|-----|------|
| **手写完整流程** | #2 worker/job (G1-G8 几乎全手写) | 维护成本高，新 gate 难同步 |
| **手写残缺流程** | #7 routine_engine（只有 allowlist + 参数 + 超时） | 漏 G1/G3/G4，且不易发现 |
| **完全不挂 gate** | #8 webhook, #9 builder | 真正的安全漏洞 |

### 5.3 共享 helper 自身的不足

`execute_tool_with_safety` 当前只覆盖 G1/G5/G6，**没有把 G2/G3/G4/G7 收进来**。这意味着即使 #2 worker/job 改成调用 helper，仍然需要在 helper 外再补 G2/G3/G4/G7 —— 这是真正需要修复的根因。

## 6. #94 实施建议（仅供人类决策）

### Phase 1 — 扩展共享 helper 让它覆盖全部 8 个 gate

```rust
// 提议的接口（不引入 PR，仅为讨论）
pub async fn execute_tool_with_full_safety(
    deps: &ToolExecutionDeps,  // 一个聚合 (tools, safety, hooks, rate_limiter, approval_context, channels)
    tool_name: &str,
    params: serde_json::Value,
    ctx: &JobContext,
    surface: ExecutionSurface,  // 标记调用来源，用于审计和默认策略
) -> Result<ExecutionOutcome, Error>;
```

`ExecutionSurface` 枚举提议：
```rust
pub enum ExecutionSurface {
    Chat,                          // 主聊天路径
    Job { job_id: Uuid },          // 后台 job
    ContainerSubAgent { container_id: Uuid },
    Routine { routine_id: Uuid },
    Scheduler { task_id: Uuid },
    Webhook { source: String },    // ← 当前完全裸奔的入口
    Builder,                       // ← 当前完全裸奔的入口
    Cli,                           // CLI 手动调用
}
```

不同 surface 可以**默认应用不同的策略基线**（例如 Webhook 默认 strict、Cli 默认 interactive）。

### Phase 2 — 9 个入口逐个迁移

按风险优先级：

1. **#8 webhook** 和 **#9 builder**（当前裸奔，最先修）
2. **#7 routine_engine**（残缺，第二批）
3. **#2/#3/#4 worker/job**（手写完整但分散，第三批合并）
4. **#5 container, #6 scheduler**（已用 helper 但需补 G2/G3/G4）

### Phase 3 — 契约测试 + 编译期防漏

提议的契约测试：
- `req_tool_execution_surface_chat_passes_all_gates` — 验证聊天入口跑过 G1-G8
- `req_tool_execution_surface_webhook_passes_all_gates` — 验证 webhook 入口跑过 G1-G8
- `req_new_execution_surface_fails_compile_without_full_safety` — 用 trait bound 或 marker 强制所有 surface 走 helper

编译期防漏的一种实现：让 `Tool::execute()` 标记成 `#[doc(hidden)]` + `pub(crate)`，**只允许 helper 内调用**。任何新写的 execution surface 要直接调用 `tool.execute()` 编译就过不了。

## 7. 与 OpenClaw 灵感 issue (#616) 的协调点

- #94 落地 Phase 1 的 `execute_tool_with_full_safety` 后，#616 的"exec approvals 上下文缓存"机制就可以挂在 G2 入口内
- #94 的 `ExecutionSurface::Webhook` 落地后，#616 的"webhook 配对授信"机制就可以挂在该 surface 的默认策略上
- 两个 issue 是**前后顺序**关系，不能并行

## 8. 估算（不作为承诺）

| Phase | 范围 | 估算 |
|-------|------|------|
| Phase 1 | helper 扩展 + ExecutionSurface 枚举 | 2-3 day |
| Phase 2 | 9 个入口迁移（每入口 1 PR） | 5-8 day |
| Phase 3 | 契约测试 + 编译期防漏 | 1-2 day |
| **合计** | | **8-13 day** |

> 与 #94 的 `effort:L` (3-7 day) 标签**略超**。建议人类 review 时考虑是否拆分成 #94 和 #94-followup。

## 9. 待人类决策的关键问题

1. **是否同意 helper 扩展方向**？还是更倾向于"每个 surface 自己写但加 trait bound 强制完整"？
2. **`ExecutionSurface` 枚举的粒度合适吗**？是否需要更细的子类型（如 `Webhook::Slack / Webhook::Discord`）？
3. **编译期防漏的 trait 化方案能接受吗**？这会要求所有外部 tool 实现都改入口可见性，影响范围较大。
4. **是否要趁此机会把 `tools::builder::core::execute_build_tool` 直接归并到 helper**？还是认为 builder 用例足够特殊（无 user_id）保留独立路径？

---

## 附录 A — gate 8 项实现的 ground truth 引用

| Gate | 代表实现 |
|------|---------|
| G1 | `dasclaw_governance::tool_visibility::ToolGateContextSeed::system()` + `tools.policy_check_executor()` (in `execute.rs:50-83`) |
| G2 | `worker/job.rs:593`: `let requirement = tool.requires_approval(...)` + `ApprovalContext::is_blocked_or_default()` |
| G3 | `worker/job.rs:603-616`: `RateLimiter::check_and_record` |
| G4 | `worker/job.rs:619-655`: `HookEvent::ToolCall` + `hooks.run` |
| G5 | `execute.rs:103-115` 和 `worker/job.rs:670-685`: `validate_tool_params` |
| G6 | `execute.rs:118`: `tokio::time::timeout(tool.execution_timeout(), tool.execute(...))` |
| G7 | `safety/egress.rs::sanitize_tool_output_via_egress` (ADR-148 R2 / PR #615) |
| G8 | `StatusUpdate::tool_completed` + tracing debug log + thread audit |

## 附录 B — 调研使用的工具命令记录

```bash
# 入口枚举（grep）
grep -rn "execute_tool_calls\|execute_tool_simple\|execute_tool_inner\|run_tool_command" \
  desktop-client/ironclaw/src/

# 共享 helper 调用者枚举
grep -rn "execute_tool_with_safety\|execute_tool_simple" desktop-client/ironclaw/src/

# 全裸 tool.execute() 调用者枚举
grep -rn "tool\.execute\|tool_impl\.execute" desktop-client/ironclaw/src/ \
  | grep -v test | grep -v "#\[" | grep -v "//"
```
