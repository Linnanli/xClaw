# ADR-160：框架架构重审 —— 砍 LoopDelegate 上帝接口、取消 Session 扩展槽

- 状态：Proposed
- 提案人：Coding Agent（基于 doc 56 调研 + codex / claw-code 双参照核验）
- 关联：
  - ADR-157（无头 agent 框架能力边界）§2.3 —— 本 ADR 修订其 3 前置条件的实施方向
  - ADR-153（headless agent framework draft）§4.2 / §4.4
  - ADR-148（EgressGate）—— 保留，作为 L4 出口闸门
  - ADR-113（hooks event-only）—— 保留，作为 L3 hook 约束
  - 56（framework architecture re-review，本 ADR 的详细论证材料）
  - issue #957（前置 1：持久化标准，方向已按本 ADR 调整）
  - issue #959（前置 2：派发器，方向已按本 ADR 调整）
  - PR #958（前置 1 原扩展槽 RFC，被本 ADR 超越，待重写或关闭）
  - PR #960（doc 56 草案）
- 时间：2026-05-29

---

## 1. 背景

ADR-157 §2.3 把"桌面端 `ChatDelegate` 迁到 `Agent` 门面"列为不做，并给出 3 个前置条件：

1. 持久化标准统一
2. `dasclaw_runtime` 加可插拔 3 阶段派发器
3. 审批统一到 ApprovalInbox

起草前置 1（PR #958）/ 前置 2（issue #959）RFC 时发现：**三件事都在给同一棵歪树打补丁**。根因不在 RFC 写得不细，而在 `LoopDelegate` 这个 trait 本身的层次切错了。

### 1.1 当前问题

[`crates/dasclaw_runtime/src/agent.rs`](../../crates/dasclaw_runtime/src/agent.rs) 的 `LoopDelegate` trait 把以下职责揉在一个 trait 里：

- 事件循环驱动
- 工具批次调度（并发 / 顺序）
- 审批门控
- 出口消毒（DLP）
- 会话持久化时机

结果：

- [`HeadlessDelegate::execute_tool_calls`](../../crates/dasclaw_runtime/src/agent.rs#L855)（约 170 行）
- [`ChatDelegate::execute_tool_calls`](../../desktop-client/ironclaw/src/agent/dispatcher.rs#L586)（约 600 行）

**不是同一接口的两个实现，是两份各自独立的事件循环代码**。共性逻辑（事件发射、ApprovalInbox 等待、EgressGate 调用、ChatMessage 追加）在两边复制；想给桌面端加并发只能改一边、headless 无法受益，反之亦然。

### 1.2 行业参照（已核验）

派子 agent 调研 `codex-cli-main` 与 `claw-code` 两个参照实现：

| 维度 | claw-code | codex |
|---|---|---|
| Session 类型 | concrete struct 12 字段（[session.rs:90](../../claw-code/rust/crates/runtime/src/session.rs#L90)） | concrete struct 12 字段 |
| 扩展槽 | **无** | **无** |
| 第三方扩展 | 改源码 + 版本迁移 | 改源码 + 版本迁移 |
| 持久化格式 | JSONL 即时追加 | JSONL 后台 mpsc 异步、延迟物化 |
| 派发器 trait | `ToolExecutor` trait **1 个方法**（[conversation.rs:57-60](../../claw-code/rust/crates/runtime/src/conversation.rs#L57)） | `ToolOrchestrator`（单工具粒度） |
| 批工具调度 | conversation.rs 顺序 for 循环 | turn 内具体代码 |

**关键启示**：业界两个 reference 实现都不做"框架公共扩展槽"。"框架核心字段 vs 业务扩展字段"是个伪需求——它把"框架不知道字段含义"的责任甩给使用者，最后人人都往 `serde_json::Value` 里塞 blob，没法 query / migrate / 校验。

---

## 2. 决策

### 2.1 砍 `LoopDelegate` 上帝接口

`LoopDelegate` trait 进入废弃路径。换为：

```
AgenticLoop (concrete struct, 单一实现)
    ├── tool_dispatcher : Box<dyn ToolDispatcher>   ← L2 新增 trait
    ├── hooks           : HookChain                  ← L3 已有 (event-only)
    ├── egress_gate     : Arc<dyn EgressGate>        ← L4 已有 (ADR-148)
    ├── session_store   : Arc<dyn SessionStore>      ← L5 已有
    ├── approver        : Box<dyn Approver>          ← 新 trait，替代 ApprovalInbox 直耦合
    └── (其他不变的依赖)
```

`AgenticLoop::run()` 是**所有 agent 共用的同一份**事件循环代码，不再是 trait method。差异点全部通过注入的 trait 表达。

### 2.2 取消 Session 扩展槽

`SessionSnapshot` 保持 concrete struct。新增：

- `version: u32` 字段
- `migrate_vN_to_vM` 函数族
- **不**引入 `extensions: serde_json::Value` 或 `metadata: Map<String, Value>`

第三方需要扩展 session 状态？两条路：

| 路径 | 适用场景 |
|---|---|
| **WrappedSession** | 第三方自己定义 `MyWrappedSession { inner: SessionSnapshot, my_state: Foo }`，各自管各自的存储 |
| **推动主线加字段** | 字段对所有 agent 都有意义时，走版本迁移加进 `SessionSnapshot` |

### 2.3 `HeadlessDelegate` / `ChatDelegate` 走废弃路径

不再是 `LoopDelegate` 的两个实现。改为 `AgenticLoop` 的两套装配：

| 当前类型 | 重构后等价物 |
|---|---|
| `HeadlessDelegate` | `AgenticLoop::new(SequentialDispatcher, StandardHookChain, IronclawEgressGate, JsonlSessionStore, AgentEventApprover)` |
| `ChatDelegate` | `AgenticLoop::new(DesktopDispatcher, DesktopHookChain, DesktopEgressGate, DesktopSessionStore, TauriApprover)` |

桌面端 `tool_output_stash` 双轨切分写成 `DesktopDispatcher`（包 `ConcurrentDispatcher` + 自己的后置逻辑）—— 是桌面端的实现策略，不进框架公共面。

---

## 3. 新分层模型

```
L1 AgenticLoop          concrete struct，单一实现
                        run loop = LLM call → tool_calls → dispatcher.dispatch → results → 继续

L2 ToolDispatcher trait async fn dispatch(calls: Vec<ToolCall>, ctx: &DispatchCtx) -> Vec<ToolResult>
                        默认 SequentialDispatcher / ConcurrentDispatcher
                        桌面端自己写 DesktopDispatcher（含 stash 切分）

L3 HookChain            纯事件通知 (ADR-113 event-only red line)
                        不改控制流；改控制流的需求走 L2 自定义 dispatcher 或 L4 EgressGate

L4 EgressGate trait     已有 (ADR-148)
                        消毒 / DLP / 闸门

L5 SessionStore trait   已有 (dasclaw_session)
                        SessionSnapshot concrete + version + migration
                        不做扩展槽

(横切) Approver trait   新增，替代 ApprovalInbox 直耦合
                        async fn approve(req: ApprovalRequest) -> ApprovalDecision
                        实现：AgentEventApprover (headless) / TauriApprover (desktop)
```

---

## 4. 对 ADR-157 §2.3 前置 1/2/3 的影响

| 前置 | 原 ADR-157 描述 | ADR-160 后的实施方向 |
|---|---|---|
| **1 持久化** | "持久化标准统一"（无具体方案） | **取消扩展槽**。`SessionSnapshot` + `version` + `migrate_vN_to_vM`。第三方走 `WrappedSession`（issue #957 已同步调整） |
| **2 派发器** | "`dasclaw_runtime` 加可插拔 3 阶段派发器" | **直接砍 `LoopDelegate`**。`AgenticLoop` concrete + 注入 `ToolDispatcher` trait（issue #959 已同步调整） |
| **3 审批** | "审批统一到 ApprovalInbox" | 引入 `Approver` trait，`AgentEventApprover` 包 `ApprovalInbox`，桌面端用 `TauriApprover` |

净结果：代码量大概率**减少**，因为消除了 `HeadlessDelegate` / `ChatDelegate` 两份 600+170 行的循环代码重复。

---

## 5. 不做什么（显式排除）

- ❌ **不立刻重写** `ChatDelegate` / `HeadlessDelegate`。走废弃路径，给桌面端 / 第三方迁移时间（至少 1 个 release cycle）
- ❌ **不改** `EgressGate` / `SessionStore` / `dasclaw_safety` 的公共面
- ❌ **不破坏**现有桌面端行为（`tool_output_stash` / DLP 双轨切分 / Tauri 直推审批保持原样，只是换实现位置）
- ❌ **不引入**新 crate（在 `dasclaw_runtime` 内部完成）
- ❌ **不强制**第三方迁。`LoopDelegate` 标 deprecated 后保留至少 1 个 release

---

## 6. 替代方案

| 方案 | 描述 | 评价 |
|---|---|---|
| **A. 维持现状，走原 §2.3 三补丁** | 按 #957 / #959 / 审批统一各加一层补丁 | ❌ 拒绝。补的是歪树枝，根问题不解 |
| **B. 给 `LoopDelegate` 加 default method** | trait method 给默认实现，子类按需 override | ❌ 拒绝。不解决上帝接口本质——默认 method 仍然在调上帝接口的其他 method，循环耦合 |
| **C. 本方案（5 分层 + 装配）** | 砍 `LoopDelegate`，引入 `AgenticLoop` concrete + 4 个注入 trait | ✅ 推荐 |
| **D. 完全废弃 `dasclaw_runtime`，第三方直接用 `dasclaw_core`** | 不给"通用 agent 框架"承诺，每家自己拼 | ❌ 拒绝。ADR-157 已承诺通用 agent 能力 |

---

## 7. 迁移计划

| 阶段 | 内容 | 状态 |
|---|---|---|
| W6 | 在 `dasclaw_runtime` 起 `AgenticLoop` concrete struct（旁路 `LoopDelegate`）+ `ToolDispatcher` / `Approver` trait | 待启动 |
| W7 | `ChatDelegate` 改为基于 `AgenticLoop` 的装配。保留旧 `ChatDelegate` API 作为薄门面 | 待启动 |
| W8 | `HeadlessDelegate` 同上 | 待启动 |
| W9 | `LoopDelegate` 标 `#[deprecated]`，保留 1 个 release | 待启动 |
| W10 | 删除 `LoopDelegate` 和 `HeadlessDelegate` / `ChatDelegate` 旧实现 | 待启动 |

每个阶段独立 PR，独立可回滚。

---

## 8. 风险

| 风险 | 缓解 |
|---|---|
| `ChatDelegate` 当前 15 字段，迁到装配模式切片复杂 | W7 分多个 sub-PR：先抽 `DesktopDispatcher`，再抽 `DesktopHookChain`，最后切 `AgenticLoop::new` |
| 第三方已 impl `LoopDelegate`（**目前没人，但 W3-A Phase 0 后可能有**） | 兼容 shim：保留 `LoopDelegate` blanket impl for `AgenticLoop`-with-defaults |
| `tool_output_stash` 抽到 `DesktopDispatcher` 后是否还能让 `json` 工具回查 | stash 本来就是桌面端业务，迁移时随实现迁移，不入框架公共面 |
| 桌面端 W3-A migration 已进行中，重叠风险 | ADR-160 不要求与 W3-A 同步推进；W3-A 完成后再启 W6 |

---

## 9. DoD（启用 ADR-160 的最小完成度）

- [ ] 本 ADR merge 到 xClaw（accepted）
- [ ] PR #958（前置 1 原扩展槽方向 RFC）改写或关闭
- [ ] issue #957 / #959 body 已按本 ADR 方向更新（已完成）
- [ ] W6 启动 PR：`crates/dasclaw_runtime/src/agent_loop.rs` 起 `AgenticLoop` concrete struct PoC
- [ ] W6 启动 PR：`crates/dasclaw_runtime/src/dispatcher/mod.rs` 起 `ToolDispatcher` trait + `SequentialDispatcher`

---

## 10. Sources read

- [ADR-157 §2.3](adr-157-headless-agent-framework-capability-boundary.md#23-不做的事)
- [doc 53 §4.3](53-headless-agent-capability-design.md)
- [doc 54（前置 1 RFC，扩展槽方向，被本 ADR 超越）](54-session-extensible-snapshot-rfc.md)
- [doc 56（本 ADR 详细论证材料）](56-framework-architecture-re-review.md)
- [ADR-148（EgressGate）](adr-148-egress-gate.md)
- [ADR-113（hooks event-only）](adr-113-hook-engine-unification.md)
- `crates/dasclaw_runtime/src/agent.rs` L807-979（`HeadlessDelegate::execute_tool_calls` 完整 5 步，亲自核验）
- `desktop-client/ironclaw/src/agent/dispatcher.rs` L586-1200（`ChatDelegate` 三阶段，亲自核验）
- `claw-code/rust/crates/runtime/src/session.rs` L90-105（`Session` struct 12 字段）
- `claw-code/rust/crates/runtime/src/conversation.rs` L57-60（`ToolExecutor` trait 1 个方法）
- `crates/dasclaw_safety/src/lib.rs` L1-60 + `egress_gate.rs`（框架已有完整 DLP + `IronclawEgressGate` 适配器）
- codex `codex-rs/core/src/state/session.rs` L1-35（`SessionState` 12 字段 concrete，无扩展槽）—— 子 agent 报告引用，未亲自核验

## 11. Cross-cuts

类 A：无新增 `.ironclaw` / `IRONCLAW_BASE_DIR` 字面量。
