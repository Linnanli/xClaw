# ADR-115: ProjectDoc 加载不进 hook 系统（构建期 DI）

- **Status**: Draft
- **Date**: 2026-04-29
- **Approver**: pending
- **Supersedes**: 修订 [32-execution-plan.md W3 §3.6](32-execution-plan.md) 中"集成到 dasclaw_hooks::OnSessionStart"的写法
- **Related**: [ADR-106](adr-106-codex-namespace-dual-read.md)、[ADR-113 §2.4](adr-113-hook-engine-unification.md)（反跨界原则）

---

## 1. Context

`32-execution-plan.md` W3 §3.6 (v2.1) 写：

> 集成到 dasclaw_hooks::OnSessionStart：加载后注入 system prompt 的 dynamic boundary 之后

issue #59 验收第 1 条：

> Hook 注册到 dasclaw_hooks::HookRegistry

落地时发现两条致命矛盾：

### 1.1 Schema 不承载

`crates/dasclaw_hooks/src/hook.rs::HookEvent::SessionStart { user_id, session_id }` 当前 schema **没有 cwd / 没有 system_prompt mutation 通道**。`HookOutcome::Continue { modified }` 的 `modified` 字段语义是 audit/transform 用途的字符串描述，**不是为 prompt prepend 设计**。

### 1.2 三参考库一致结论：构建期 DI

| 参考库 | 调用站 | 模式 |
|---|---|---|
| codex | `core/src/session/mod.rs:501` `AgentsMdManager::user_instructions(...)` | 构建期 DI |
| claw-code | `runtime/src/conversation.rs:925` `SystemPromptBuilder.with_project_context(...)` | 构建期 DI |
| ironclaw legacy | `llm/reasoning.rs:488` `Reasoning.with_workspace_system_prompt(p)` | 构建期 DI |

三库**无一**把 ProjectDoc 加载放进 hook 系统。

### 1.3 ADR-113 §2.4 反跨界原则

ADR-113 已规定：
> 禁止跨界：声明式 bundle 中不得注册"对 prompt 做 safety scan"类规则——这是 ② 的职责，由 `before_prompt` 走结构化 `SafetyDecision` 返回。

类比：ProjectDoc 加载是有结构化输入（cwd）和结构化输出（拼装的 system prompt 段）的业务核心数据装载，**不属于事件型 hook 的"横切扩展点"语义**。

---

## 2. Decision

### 2.1 时机 vs 机制 二分

| 维度 | 决定 |
|---|---|
| **时机** | OnSessionStart 时机点（session 创建时）—— 不变 |
| **机制** | SessionManager 构建期 DI 调用 `LayeredProjectDocLoader::load()` + `assemble_section()` —— **不进 hook 系统** |

### 2.2 强制契约

- ProjectDoc 加载逻辑**不得**注册为任何 `HookPoint::OnSessionStart` handler
- ProjectDoc 加载逻辑**不得**以 `HookEvent::SessionStart` 触发副作用为依赖
- `dasclaw_hooks` crate **不直接依赖** `dasclaw_project_docs` crate（防止反向闭环）
- `crates/x_claw_agent/src/session.rs` 或 `agentic_loop.rs` 的 `ReasoningContext.system_prompt` 是唯一允许的注入点

### 2.3 SessionStart 事件保留（解耦）

`HookEvent::SessionStart { user_id, session_id }` 事件本身**保留**——它仍是 audit / outbound webhook / 通知类 hook 的合法触发点。

但其触发与否、触发时序与 ProjectDoc 加载**完全解耦**：
- SessionManager **可以** fire SessionStart（用于 audit）
- 但 ProjectDoc 加载**不依赖**该事件触发，亦**不被**该事件触发

### 2.4 issue #59 / #59a / #59b 路线修订

按本 ADR 重写：
- **#59a** ([Issue #59](https://github.com/Linnanli/xClaw/issues/59))：纯 helper（assemble_section），不涉及 hook
- **#59b** ([Issue #111](https://github.com/Linnanli/xClaw/issues/111))：SessionManager 构建期 DI 注入，不调用 dasclaw_hooks

---

## 3. Consequences

### 3.1 正面

- 与三参考库设计一致，降低后续维护时阅读跳转成本
- ADR-113 反跨界原则得到执行
- `HookEvent::SessionStart` schema 不被破坏（不需要新增 cwd / prompt mutation 字段）
- ProjectDoc 加载失败不会阻塞 hook 链

### 3.2 负面 / 成本

- 32-execution-plan.md §3.6 文字需修订（已修）
- issue #59 验收需重写（已规划）
- 用户文档若提及"OnSessionStart hook 加载 ProjectDoc"需澄清术语

### 3.3 风险

- 若未来出现"插件想拦截 ProjectDoc 加载"需求，本 ADR 阻止其走 hook 路径——届时应通过新增 trait seam（类似 SafetyHook）解决，而非妥协回 hook 系统

---

## 4. Enforcement

### 4.1 编译期

`dasclaw_project_docs` crate `Cargo.toml`：
- **禁止**依赖 `dasclaw_hooks`（CI grep guard）

`dasclaw_hooks` crate `Cargo.toml`：
- **禁止**依赖 `dasclaw_project_docs`（CI grep guard）

### 4.2 运行期

`crates/x_claw_agent/src/session.rs` 或等价接入点的 #59b 实现 PR 必须：
- 直接持有 `Arc<dyn ProjectDocLoader>` 字段
- **不**通过 `HookEngine::dispatch(HookEvent::SessionStart { .. })` 触发加载

---

## 5. 决策范围外

- 是否 fire `HookEvent::SessionStart` 用于 audit/通知（独立决策，不绑定本 ADR）
- 其他 lifecycle hook（BeforeInbound/BeforeToolCall 等）的设计（ADR-113 已定）
