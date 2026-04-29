# ADR-113: Hook 系统收口（HookEngine 5→1）

- **Status**: Draft（W3-A Phase 0 P0-3）
- **Date**: 2026-04-29
- **Approver**: pending
- **Supersedes**: 无
- **Related**: ADR-112 §5（Phase 0 路线图）、ADR-001（Sandbox hook 不在 Phase 3 接线）、ADR-112 §1 表 line 340"4 套并存"红线

---

## 1. Context — 现状盘点（事实基线）

ADR-112 input-checklist §2.7.1 列 "Hook 收口：5 套合 1（HookRegistry + HookBundle + SessionHooks + PluginHooks + policy_decider）"。Step 3 实证调研结果**修正**该列举：

| # | 系统 | 位置 | 抽象类型 | 实际状态 |
|---|------|------|---------|---------|
| ① | **ironclaw HookRegistry** | `desktop-client/ironclaw/src/hooks/{hook,registry,bundled,bootstrap}.rs` | 6 `HookPoint` × 优先级 + 声明式 bundle + plugin/workspace bootstrap + audit + outbound webhook | 生产可用，最丰富 |
| ② | **x_claw_agent HookBundle** | `crates/x_claw_agent/src/hooks.rs` | 4 trait seams：`SafetyHook` / `SandboxExecutor` / `SecretProvider` / `ApprovalGate` | 生产可用，crate 边界稳定（ADR-001 决策） |
| ③ | **x_claw_agent SessionHooks** | `crates/x_claw_agent/src/session_hooks.rs` | 单 trait（`on_session_start` / `on_session_end`） | 已通过 ironclaw `impl SessionHooks for HookRegistry` 桥接 |
| ④ | **dasclaw_hooks** | `crates/dasclaw_hooks/src/lib.rs` | 21 行 W1 placeholder（`trait HookEngine { fn dispatch }`） | 空骨架，无人调用 |
| ⑤ | "PluginHooks / policy_decider" | input-checklist §列举 | PluginHooks 实为 ① 的 declarative bundle 之一；`PolicyDecider` 实为 `dasclaw_net_proxy::NetworkPolicyDecider`（与 hook 无关） | **不构成独立 hook 系统**（误归类） |

**结论**：实际只有 **3 套真实抽象**（①②③）+ 1 空骨架（④）。复杂度低于 input-checklist 描述。

### 1.1 重叠分析（关键风险）

3/6 ironclaw HookPoint 与 ② SafetyHook 功能直接重叠：

| Lifecycle 时点 | ① HookRegistry | ② HookBundle.safety | 状态 |
|---|---|---|---|
| LLM prompt 发送前 | `BeforeInbound` | `SafetyHook::before_prompt` | 🔴 双调用 |
| LLM completion 返回后 | `TransformResponse` + `BeforeOutbound` | `SafetyHook::after_completion` | 🔴 双调用 |
| Tool call 执行前 | `BeforeToolCall` | `SafetyHook::before_tool_call` + `ApprovalGate::request` | 🔴 双重 |
| Tool output 后 | 无对应点 | `SafetyHook::after_tool_output` | ⚪ 仅 ② |
| Session 生命周期 | `OnSessionStart`/`End` | `SessionHooks` 独立 trait | 🟡 已桥接 |
| Sandbox / Secrets | 无 | `SandboxExecutor` / `SecretProvider` | ⚪ 仅 ② |

ironclaw 当前已通过 `IronclawSafetyHook` 把 `SafetyLayer` 装进 ②，但 dispatcher 同时独立调用 ① 的 `BeforeInbound`，导致**同一条 prompt 可被 SafetyLayer 经两条路径检查**——这是 ADR-112 §1 line 340 风险红线"4 套并存"的真实危害。

### 1.2 两套抽象的本质差异（不能粗暴合一）

| 维度 | ① HookRegistry | ② HookBundle 4 trait seams |
|------|-----|------|
| 执行模型 | **N:1 链式**（priority 排序，`Reject` 短路，`Modify` 链式累积） | **1:1 单实现**（一个具体类型实现 trait） |
| 返回类型 | `HookOutcome { Continue / Reject }` 字符串语义 | 各 trait 独立结构化返回（`SafetyDecision` / `SandboxExecOutput` / `SecretString`） |
| 配置来源 | 用户 / 插件可配置 declarative bundle | 系统级注入（启动时 `Arc<dyn>`） |
| 用例 | audit log、regex transform、outbound webhook | safety 决策、sandbox 执行、secret 读取、approval 流 |

强行合一为单 trait 会破坏 ② 的 per-trait 错误类型 + crate 独立性（ADR-001），同时让 ① 的 N:1 链式语义无处安放。

---

## 2. Decision

### 2.1 统一编排入口，保留双层抽象

把 ④ `dasclaw_hooks` 实化为**唯一编排入口** `HookEngine`，承载 ① 的全部能力（事件型 + 声明式 bundle）。 ② / ③ 的 trait seams **保留在 `x_claw_agent`**（保 crate 独立），由 `dasclaw_hooks` reexport 形成"统一前门"。

### 2.2 "1" 的术语澄清

> Phase 0 红线 `count_hook_systems() == 1` 中的 **"1" = 编排入口数 = 1**（即 `dasclaw_hooks::HookEngine`），**不是 trait 数 = 1**。trait seams 是承担不同职责的具体抽象，由 `dasclaw_hooks` 统一 reexport 后不计入"系统数"。

### 2.3 职责契约（强制分工）

| 职责 | 归属 | 形态 |
|------|------|------|
| 用户 / 插件可配置的横切（audit / declarative regex / outbound webhook） | `HookEngine`（事件型） | `Hook` trait + `HookEvent` × 6 lifecycle |
| 有返回类型语义的系统级拦截（safety 决策 / sandbox / secrets / approval） | trait seams（reexport 自 `x_claw_agent`） | `Arc<dyn SafetyHook>` 等 |
| Session lifecycle | `SessionHooks` trait（reexport） | `Arc<dyn SessionHooks>` |

**禁止跨界**：声明式 bundle 中不得注册"对 prompt 做 safety scan"类规则——这是 ② 的职责，由 `before_prompt` 走结构化 `SafetyDecision` 返回。启动期校验函数 `no_safety_rule_in_event_hooks()` 保证此契约不被违反。

### 2.4 删除双调用路径

- `BeforeInbound` + `SafetyHook::before_prompt`：**保留 `bundle.safety.before_prompt`**（agent loop 单一入口），dispatcher 删除 `BeforeInbound` 独立触发
- `BeforeOutbound` + `SafetyHook::after_completion`：保留 `after_completion`，`BeforeOutbound` 仅留 audit / webhook 用途
- `BeforeToolCall` + `SafetyHook::before_tool_call`：保留 `before_tool_call` + `ApprovalGate::request`，`BeforeToolCall` 仅留 audit / webhook 用途

---

## 3. 目标架构

```
crates/dasclaw_hooks/  ← 实化（不再是 W1 placeholder）
├── lib.rs
│   ├── pub use engine::HookEngine          ← 唯一编排入口
│   ├── pub use event::{HookEvent, HookPoint, HookOutcome, ...}
│   ├── pub use hook::Hook                  ← 事件型 hook trait
│   └── pub use x_claw_agent::{             ← reexport「前门」
│         SafetyHook, SafetyDecision, SafetyError,
│         SandboxExecutor, SandboxExecRequest, SandboxExecOutput, SandboxError,
│         SecretProvider, SecretString,
│         ApprovalGate,
│         HookBundle,                       ← trait seams 容器（不变）
│         SessionHooks,
│       }
├── engine.rs                                ← 迁自 ironclaw registry.rs（重命名 HookRegistry→HookEngine）
├── event.rs                                 ← 迁自 ironclaw hook.rs（HookPoint / HookEvent / HookOutcome）
├── bundle.rs                                ← 迁自 ironclaw bundled.rs（声明式 rules / outbound webhook）
├── bootstrap.rs                             ← 迁自 ironclaw bootstrap.rs（plugin / workspace 加载）
└── contract.rs                              ← 新增：no_safety_rule_in_event_hooks() 启动期校验

crates/x_claw_agent/                         ← 不动（trait seams 仍由本 crate 拥有）
├── src/hooks.rs                             ← 4 trait + HookBundle，不动
└── src/session_hooks.rs                     ← 不动

desktop-client/ironclaw/src/hooks/           ← Phase A 兼容期：thin reexport shim
└── mod.rs                                   ← `pub use dasclaw_hooks::*;` 全部
                                                Phase B 删除（PR #48）

desktop-client/ironclaw/src/agent/dispatcher.rs
                                              ← 删除 BeforeInbound / BeforeOutbound / BeforeToolCall 的 SafetyLayer 重复触发
```

---

## 4. 实施切片（stacked PR）

| PR | base | 内容 | 测试族 |
|----|------|------|--------|
| **PR #46** `p03-pr1-dasclaw-hooks-impl` | `xClaw` | (1) 把 ironclaw `hooks/{hook,registry,bundled,bootstrap}.rs` 整体迁入 `dasclaw_hooks`，重命名 `HookRegistry` → `HookEngine`；(2) `dasclaw_hooks` reexport `x_claw_agent` trait seams；(3) 新增 `contract.rs` 与 `no_safety_rule_in_event_hooks()`；(4) ironclaw `hooks/mod.rs` 改为 thin reexport shim（兼容） | H1 unit：迁过来的全部 ironclaw hook 测试 + `req_p03_pr1_*` 5 测试（reexport 等价 / 契约校验 / 启动期 assertion） |
| **PR #47** `p03-pr2-migrate-callers` | PR #46 | (1) ironclaw 内部所有 `crate::hooks::HookRegistry` 调用站改为 `dasclaw_hooks::HookEngine`；(2) `agent_loop.rs` 的 `hook_bundle_with_safety_and_secrets` 改用 `dasclaw_hooks::reexport`；(3) 删除 `dispatcher.rs` 的 `BeforeInbound`/`BeforeOutbound`/`BeforeToolCall` 与 SafetyHook 的双调用 | H2 integration：agent dispatcher 端到端 hook 触发不退化 + 双调用消除断言；`req_p03_pr2_*` 5 测试 |
| **PR #48** `p03-pr3-cleanup-shim` | PR #47 | (1) 删除 ironclaw `hooks/` 兼容 shim；(2) `count_hook_systems() == 1` 启动期 assertion 转生产代码；(3) `no_safety_rule_in_event_hooks()` 在 `bootstrap_hooks` 注册末尾强制调用 | `req_p03_pr3_*` 3 测试 + Phase 0 红线测试转绿 |

**禁止补丁式代码**：每个 PR 都是完整的代码搬移 + 调用站迁移，**不**在原模块上 `if cfg!(feature = "new_hooks")` 切换新旧路径。兼容期通过 Rust `pub use` reexport 实现（一行代码，零运行期开销）。

---

## 5. Phase 0 红线 assertion 定义

```rust
// crates/dasclaw_hooks/src/contract.rs
/// Phase 0 红线：仅一个 hook 编排入口（HookEngine）。
/// trait seams 是被 reexport 的具体抽象，不计入"系统数"。
pub fn count_hook_systems() -> usize { 1 }

/// 职责契约：声明式 bundle rule 不得承担 safety 职责。
/// 由 bootstrap_hooks 末尾在启动期调用，违反时 panic（启动失败优于隐性 fallback）。
pub fn no_safety_rule_in_event_hooks(engine: &HookEngine) -> bool {
    // 检查 engine 注册的所有 declarative rule，若 name 含 "safety"/"redact"/"secret"
    // 或注册到 BeforeInbound/BeforeOutbound/BeforeToolCall 且声明 reject_reason
    // 含 secret 关键字 → 返回 false
    // 详细规则在 PR #46 contract.rs 实现
}
```

---

## 6. 测试计划（B1 同步补测，遵循 ADR-112 §5.4.1）

每个 PR 都自带 H1 + H2，覆盖**模块 4 Hook**（参见 ADR-112 §5.4.1 表）。

### 6.1 PR #46 — H1 unit + 契约
- `req_p03_pr1_engine_dispatch_priority_order` — HookEngine 按 priority 排序执行
- `req_p03_pr1_engine_reject_short_circuits` — `Reject` 立即终止链
- `req_p03_pr1_engine_modify_chains` — `Modify` 链式累积
- `req_p03_pr1_reexport_trait_seams_identity` — `dasclaw_hooks::SafetyHook == x_claw_agent::SafetyHook`（type identity）
- `req_p03_pr1_no_safety_rule_in_event_hooks_rejects_violation` — 注册 safety 类规则时 contract 返回 false

### 6.2 PR #47 — H2 integration
- `req_p03_pr2_dispatcher_no_double_call_safety` — 同一条 prompt 仅经 SafetyLayer 一次
- `req_p03_pr2_agentic_loop_uses_reexport_bundle` — agent loop 走 `dasclaw_hooks::HookBundle`
- `req_p03_pr2_session_hooks_still_bridged` — `OnSessionStart` 仍触发 ironclaw 桥接的 SessionHooks
- `req_p03_pr2_declarative_bundle_audit_path_intact` — 用户配置的 audit rule 仍正常执行
- `req_p03_pr2_outbound_webhook_intact` — outbound webhook 路径不退化

### 6.3 PR #48 — Phase 0 红线
- `req_p03_pr3_count_hook_systems_equals_one` — 启动期断言
- `req_p03_pr3_contract_runs_at_bootstrap_end` — 启动期违规配置直接 panic
- `req_p03_pr3_compat_shim_removed` — `desktop-client/ironclaw/src/hooks/` 不再含具体实现

---

## 7. 偏离声明与风险

| # | 偏离 | 理由 |
|---|------|------|
| 1 | 不删除 `x_claw_agent::HookBundle` 4 trait seams | 保 crate 独立性是 ADR-001 决策。Phase 0 红线允许"1 个编排入口 + N 个 trait 抽象 reexport"。 |
| 2 | `policy_decider` 不在范围 | 网络代理决策与 hook 无关，input-checklist §的列举有误，本 ADR §1 表 ⑤ 已澄清。 |
| 3 | dasclaw_hooks W1 placeholder（`trait HookEngine { fn dispatch }`）整体替换 | 老 trait 无 caller，直接由新设计替换。 |
| 4 | 兼容期不引入 feature flag | 用 `pub use` reexport 替代 `if cfg!(feature)`，反"补丁式代码"原则。 |

### 风险

- **R1（中）**：`hook_bundle_with_safety_and_secrets` 在多个 caller 使用，PR #47 调用站迁移可能漏改 → 缓解：grep 全量替换 + `cargo build --bins --tests` 0 错误 0 警告 + H2 集成测试。
- **R2（低）**：删除 `BeforeInbound` 等独立触发可能影响**用户已配置**的声明式 rule 期望 → 缓解：PR #47 H2 测试族 `req_p03_pr2_declarative_bundle_audit_path_intact` 覆盖；ADR §2.4 明确仅删除 SafetyLayer 重复触发，audit/webhook 用途保留。
- **R3（低）**：`x_claw_agent::SessionHooks` reexport 后 ironclaw `impl SessionHooks for HookEngine` 桥接需重写 → PR #46 内同步迁移，无跨 PR 风险。

---

## 8. 验收标准（Phase 0 P0-3 完成定义）

- [ ] PR #46 / #47 / #48 均合并到 `xClaw`
- [ ] `cargo nextest run -p dasclaw_hooks` 0 失败
- [ ] `cargo nextest run -p ironclaw --lib hooks` 0 失败（迁移后）
- [ ] `cargo build --bins --tests` 0 错误 0 警告
- [ ] `python3 scripts/check_no_panics.py` 通过
- [ ] Phase 0 红线 `count_hook_systems() == 1` assertion 转绿
- [ ] H1 + H2 测试族（13 项）全绿
- [ ] 三 skill 自审（code-quality-audit / code-simplifier / code-review-expert）记录到 PR 描述
