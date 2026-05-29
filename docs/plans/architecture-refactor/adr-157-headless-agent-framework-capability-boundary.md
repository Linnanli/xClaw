# ADR-157：无头 agent 框架能力边界与迁移决策

- 状态：Proposed
- 提案人：Coding Agent（基于 PR #944 落地 + doc 52/53 调研 + PR #945/#950 实施切片）
- 关联：
  - ADR-153（headless agent framework draft）§4.2 / §4.4
  - 49（GUI readiness assessment）§5 / §9
  - 52（GUI readiness reassessment）§3
  - 53（headless agent capability design，本 ADR 的详细论证材料）
  - ADR-152（agent and capability fusion）§3 阶段 F4
  - ADR-129（verbatim port mandate）§1.3
  - PR #942（B5 三库融合证明）/ #944（B4 GUI approval facade）/ #945（headless starter README + example）/ #948（doc 52+53）/ #950（顶层嵌入指针）
- 时间：2026-05-29

---

## 1. 背景

`dasclaw_runtime::Agent` 作为 ADR-153 §4.2 B 系列出口已经全部落地（B1–B5 全绿，详见 [52 §3](52-gui-readiness-reassessment.md)）。但仓库里缺一份**正式决策文档**回答以下三类问题：

1. **哪些能力是"无头 agent 框架"边界内的**？第三方嵌入者能依赖哪些公共面？
2. **哪些跨端代码看着像重复，实际上是合理多态，不该抽**？
3. **桌面端 `ChatDelegate` 要不要迁到 `Agent` 门面**？什么前置条件下才考虑？

[doc 53](53-headless-agent-capability-design.md) 给出了详细盘点和论证。本 ADR 将其结论固化为一份可被未来 PR `cite` 的决策记录。

---

## 2. 决策

### 2.1 接受六层架构归属（doc 53 §3）

`dasclaw_runtime::Agent` 是无头框架的窄门面。以下分层作为"能力是否抽到通用 crate"的判定依据：

- L0 共享主循环：`dasclaw_core::agentic_loop::run_agentic_loop`
- L1 默认 Delegate：`dasclaw_runtime::HeadlessDelegate`（4 件套：responder + tool_executor + approval policy + event 流）
- L2 公共辅助：`dasclaw_runtime::{composite_executor, ...}` / `dasclaw_mcp::McpToolExecutor` / `dasclaw_hooks::HookBundle`
- L3 业务 Delegate：`ChatDelegate` / `JobDelegate` / `CodexDelegate`，共享 L0 但各自派发
- L4 业务编排：会话注册表、订阅、UI 事件桥
- L5 通道实现：Tauri / SSE / WASM / CLI / HTTP

**新 crate / 新模块前必须先在 L0–L2 找是否已有等价能力**，找不到再考虑下沉。

### 2.2 接受 doc 53 §4.1 P0 缺口清单作为唯一实施清单

- **P0-1**：起步指南 + 样例（PR #945, in flight）
- **P0-2**：顶层 README / INTEROP_DASCLAW.md 加嵌入指针（PR #950, in flight）
- **P0-3**：本 ADR（升格 doc 53 为正式决策 + 迁移指引）

doc 53 §6 初稿的 P1/P2 已全部撤销（详见 §3 不做什么）。

### 2.3 不迁桌面端 `ChatDelegate` 到 `Agent`

`ChatDelegate`（[desktop-client/ironclaw/src/agent/dispatcher.rs:354](../../desktop-client/ironclaw/src/agent/dispatcher.rs#L354)）与 `HeadlessDelegate` 在 6 个维度上结构性不同（doc 53 §4.3）：

| 维度 | `HeadlessDelegate` | `ChatDelegate` |
|---|---|---|
| 字段数 | 8 | 15 |
| 持久化 | 纯内存 | 写数据库 |
| 工具派发 | 单次顺序 | 3 阶段（preflight / parallel / post-flight） |
| 审批模型 | `ApprovalInbox` + AgentEvent | 直推 Tauri channel |
| 取消信号 | `CancellationToken` | `Session::ThreadState::Interrupted` |
| 多租户 / Skills | 无 | 有 |

**强迁的代价**：要让桌面端走 `Agent`，必须把上述 6 件事塞进 `dasclaw_runtime`，等同于把"窄门面"扩成"宽超集"。这**违反 ADR-153 §4.4 "headless CLI 不依赖 DB / HTTP"**。

**不迁的代价**：净重复约 ~150 LOC（两个 LoopDelegate 实现），但底层共享 `dasclaw_core::agentic_loop::run_agentic_loop` + `HookBundle` + `ToolExecutor` trait。

**重新评估的前置**（不在本 ADR 范围）：

1. `dasclaw_session` 能持久化 `ThreadState` / `Tenant` / `Skills` schema
2. `dasclaw_runtime` 加可插拔的"3 阶段派发器"
3. 审批完全统一到 `ApprovalInbox`

三件事都落地后，迁移成本会从"重写 ChatDelegate"降到"换三个 trait 实现"。

### 2.4 不集中以下重复（doc 53 §5）

- 审批 4 处实现 → 合理多态（trait 已抽，UX 必然不同）
- 会话查询 3 处 schema → 业务诉求不同
- 工具派发循环 2 处 → 桌面端三段式有产品诉求
- Guardian 自动复审 → codex 业务专属
- MCP 适配（`desktop-client/ironclaw/src/tools/mcp/client_tool.rs` vs `dasclaw_mcp::McpToolExecutor`）→ 结构性多态：前者走 `Tool` trait + `strip_top_level_nulls` 防守，后者走 `ToolExecutor` trait

---

## 3. 不做什么（显式排除）

- ❌ 桌面端 `ChatDelegate` 迁 `Agent`（条件未到）
- ❌ 抽新的 `dasclaw_runtime::mcp_executor`（重复造轮子，`dasclaw_mcp::McpToolExecutor` 早已存在；doc 53 初稿 §4.2 列为缺口属脑补，已勘误）
- ❌ 把 `CompositeToolExecutor` 从 cli 搬到 runtime（同上，已在 `dasclaw_runtime::composite_executor`）
- ❌ 统一 4 处审批 UX 实现
- ❌ 统一 3 处会话 schema
- ❌ 把 Guardian 复审从 codex 抽出来

---

## 4. 后果

### 4.1 正面

- 第三方嵌入路径有明确决策可 cite，不必每次重新论证"能不能用 dasclaw_runtime"
- 后续涉及"是否新建 crate / 是否抽公共能力"的 PR 必须按 §2.1 六层架构对照说明
- 桌面端两套并发模型分裂被显式接受为设计选择，不被误读为"技术债"
- doc 53 §4.2 的勘误记录写进 ADR，避免同样的伪缺口在未来 ADR 里复现

### 4.2 负面

- 桌面端长期保留 ~150 LOC 重复（`ChatDelegate` vs `HeadlessDelegate`）
- 想合并的人需要先推动 §2.3 的 3 个前置才能再开议

### 4.3 风险与缓解

| 风险 | 等级 | 缓解 |
|---|---|---|
| 第三方仍不知道怎么起 agent | 低 | P0-1 #945 starter example + P0-2 #950 顶层指针 |
| `dasclaw_runtime` 公共面被悄悄扩大（违反 §2.1） | 中 | 后续涉及 runtime 公共 API 的 PR 必须 cite 本 ADR；新增公共 trait 需要专门 ADR |
| `AgentError::ApprovalRequested` deprecate 后第三方代码迁移找不到指引 | 中 | 见附录 A 迁移示例 |

---

## 5. 附录 A — `AgentError::ApprovalRequested` 迁移指引

PR #944 引入 `AgentEvent::ApprovalNeeded` + `Agent::respond_to_approval` 作为新 GUI approval 通道后，旧的 `AgentError::ApprovalRequested` 标 `#[deprecated]`，将在后续 release 移除。

### 旧模式（已废弃）

```rust
match agent.run(prompt).await {
    Err(AgentError::ApprovalRequested(req)) => {
        // 用户决策...
        agent.run_with_approval(req.id, decision).await
    }
    other => other,
}
```

### 新模式（推荐）

```rust
use tokio::sync::mpsc;
use dasclaw_runtime::{Agent, AgentEvent, ApprovalDecision};

let (tx, mut rx) = mpsc::channel::<AgentEvent>(16);
let agent = Arc::new(build_agent()?);

let agent_for_task = Arc::clone(&agent);
let join = tokio::spawn(async move {
    agent_for_task.run_streaming(prompt, tx).await
});

while let Some(event) = rx.recv().await {
    if let AgentEvent::ApprovalNeeded { request_id, tool_name, .. } = event {
        let decision = ask_user_for(tool_name).await;
        agent.respond_to_approval(request_id, decision).await?;
    }
}

join.await?
```

完整可跑示例：[crates/dasclaw_cli/examples/headless_agent_starter.rs](../../crates/dasclaw_cli/examples/headless_agent_starter.rs)（来源 PR #945）

### 自定义 `LoopDelegate` 接入

如果你不用 `HeadlessDelegate` 而是自己实现 `LoopDelegate`，需要在 `execute_tool_calls` 里：

1. 查 `ApprovalPolicy::needs_approval(&call)`
2. 命中后构造 `request_id = Uuid::new_v4()`，发 `AgentEvent::ApprovalNeeded` 给 event channel
3. 通过 `ApprovalInbox::wait(request_id)` 阻塞等 `Agent::respond_to_approval` 推回的 `ApprovalDecision`
4. 按 decision 决定执行 / 跳过 / 上抛 `AgentError::ApprovalRejected`

参考实现：`crates/dasclaw_runtime/src/agent.rs` 的 `HeadlessDelegate::execute_tool_calls`。

---

## 6. 决策记录

| 日期 | 事件 |
|---|---|
| 2026-05-29 | 首版 Proposed，引用 doc 53 + PR #944 / #945 / #948 / #950 |
