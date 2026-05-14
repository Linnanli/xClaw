# ADR-146: `SafetyDecision` 枚举扩展（4 → 5 态）

- **Status**: Draft（W4 #73 slice C 前置）
- **Date**: 2026-05-29
- **Approver**: pending
- **Supersedes**: 无
- **Related**: ADR-112（兼容性矩阵）、ADR-113（HookEngine 收口）、ADR-147（CompositeSafetyHook 设计）
- **Tracking Issue**: #73 slice C

---

## 1. Context — 现状盘点

### 1.1 当前 `SafetyDecision` 定义

`crates/x_claw_agent/src/hooks.rs:37`：

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum SafetyDecision {
    Allow,
    Redact { content: String, reason: String },
    Block { reason: String },
}
```

3 态，无 `#[non_exhaustive]`，外部 `match` 必须穷尽 3 个分支。

### 1.2 上游对照（claude-code-main）

`claude-code-main/src/utils/permissions/PermissionResult.ts` 是 4 态：

| TS 变体 | 字段 | 等价 Rust 现状 |
|---|---|---|
| `{ behavior: 'allow', decisionReason?: DecisionReason }` | `decisionReason` 结构化 | `Allow` 无字段 |
| `{ behavior: 'deny', reason: string, decisionReason?: DecisionReason }` | 同上 | `Block { reason: String }` |
| `{ behavior: 'ask', reason: string, suggestions: RuleSuggestion[] }` | 提示用户确认 | **缺失** |
| `{ behavior: 'redact', ... }` | 修改 args 后继续 | `Redact { ... }` |

`Passthrough` 在 claude-code 不是显式 enum 变体，而是 8 步 pipeline 内部"未表态"语义。本 ADR 将其显式化为 enum 变体以支持 `CompositeSafetyHook`（ADR-147）的链式语义。

### 1.3 #73 issue body 验收要求

> SafetyDecision 扩展 `Ask` + `Passthrough` 接口位

### 1.4 当前外部 `SafetyDecision` 使用面

通过 `vscode_listCodeUsages` + grep 三层验证：

| 位置 | 用途 |
|---|---|
| `crates/dasclaw_hooks/src/bash_validation_hook.rs` | 内部构造 `Allow / Block` |
| `desktop-client/ironclaw/crates/ironclaw_safety/src/agent_hook.rs` | 内部构造 `Allow / Block` |
| `crates/x_claw_agent/src/safety_noop.rs` | 实现 `NoopSafetyHook` |
| 测试模块约 12 处 `match` / `assert_eq!(...)` | 需要随枚举改动同步 |

外部 crate（含 dasclaw_governance / dasclaw_features）未使用 `SafetyDecision::*` literal。**爆炸半径有限**，可一次性升级。

---

## 2. Decision

### 2.1 新 enum 形态（MVP — 最小破坏面）

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SafetyDecision {
    /// Allow the operation unchanged.
    Allow,
    /// Caller mutated payload in-place (e.g. redacted secrets); continue.
    Redact,
    /// Refuse the operation; `reason` is safe to surface to the user.
    Block { reason: String },
    /// **NEW** — request user confirmation; UX renders suggestions buttons.
    Ask {
        reason: String,
        suggestions: Vec<RuleSuggestion>,
    },
    /// **NEW** — this hook abstains; CompositeSafetyHook continues chain.
    /// In single-hook context: semantically equivalent to `Allow`.
    Passthrough,
}
```

**MVP 设计原则**（与初稿差异）：
- 现有 3 变体 `Allow / Redact / Block` **形态完全不变**（向后兼容所有内部 literal 与测试断言）。
- 仅新增 2 变体 `Ask / Passthrough` 及配套 `RuleSuggestion` / `RuleAction`。
- `#[non_exhaustive]` 强制外部 `match` 加 `_` 兜底；未来加变体不破外部代码。
- 结构化 `DecisionReason` 作为**未来扩展**（见 §2.2），不进 slice C，避免一次性破坏 20+ 处内部 literal。

### 2.2 `DecisionReason` —— 未来扩展（不进 slice C）

后续如需把审计 / UI 渲染按决策来源分类，可在不破坏 MVP 的前提下：

1. 新增独立 `pub enum DecisionReason { RuleMatch / ModeEnforcement / PathValidation / DestructiveCommand / SedValidation / CommandInjection }`（强类型 + `#[non_exhaustive]`）
2. 用 `tracing` field 携带（slice C 已落实），不进入 enum 结构
3. 真正需要在 enum 中携带时，加新变体 `BlockWithReason(String, DecisionReason)` 而非修改 `Block` 形态

**slice C 不做 §2.2 的理由**：审计需求未量化，过早结构化会被推倒重来。当前 tracing log 配合 `tool` / `mode` / `reason` 字段已足够。

> 与上游对照：claude-code TS 的 `DecisionReason` discriminated union 是 UI 层渲染需要。Rust 端的 desktop-client UI 工作落在 slice D，届时按真实 UI 需要再设计强类型 enum。

### 2.3 `RuleSuggestion` 结构

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct RuleSuggestion {
    /// 用户可点击的展示文本，如 "Always allow `git status`"。
    pub label: String,
    /// 实际规则模式，如 "Bash(git status: allow)"。
    pub rule_pattern: String,
    /// 该 suggestion 对应的动作（allow / deny / ask）。
    pub action: RuleAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleAction {
    Allow,
    Deny,
    Ask,
}
```

### 2.4 `Passthrough` 语义

> `Passthrough` 表示"此 hook 不表态，请 chain 继续问下一个 hook"。

- 单 hook 上下文（无 CompositeSafetyHook）：`Passthrough` 等价于 `Allow { decision_reason: None }`（向后兼容）
- `CompositeSafetyHook` 上下文：见 ADR-147

### 2.5 Warn → Decision 映射（PermissionMode 依赖）

`BashValidationHook` 当前的 `ValidationResult::Warn { message }` 5-mode 映射：

| PermissionMode | 当前（PR #489 scaffold TODO #73）| 本 ADR 目标 |
|---|---|---|
| ReadOnly | Allow + log（待修） | `Block { reason }` —— ReadOnly 下任何 Warn 都拒绝（Fail-Safe） |
| WorkspaceWrite | Allow + log | `Allow` + `tracing::warn!` 结构化日志 |
| DangerFullAccess | Allow + log | `Allow` + `tracing::info!` 结构化日志（更宽松） |
| Allow | Allow + log | `Allow`（无 log，与 PermissionMode::Allow 自身语义一致） |
| Prompt | Allow + log | `Ask { reason: message, suggestions: vec![] }` —— 待用户确认 |

`Ask` 变体仅在 Prompt mode 触发，UI 弹窗『是否继续』（slice D / desktop-client UX 工作）。WorkspaceWrite/DangerFullAccess 保持 Allow 不打断用户，仅留 tracing log 供审计。`suggestions` 字段在 slice C MVP 中传空 `vec![]`；后续切片再根据 Warn 类型生成具体建议规则。

---

## 3. Consequences

### 3.1 破坏性变更面（MVP 后）

| 影响 | 严重度 | 缓解 |
|---|---|---|
| 所有 `match SafetyDecision` 必须加 `_` 兜底或显式列 5 变体（因为 `#[non_exhaustive]`） | 低 | 一次性迁移；项目内约 4-6 处生产 + ~5 处测试 |
| 现有 `SafetyDecision::Allow / Redact / Block { reason }` literal | **零影响** | MVP 保持形态不变 |
| `NoopSafetyHook` 行为 | 零影响 | 仍返回 `Allow` |
| `BashValidationHook` Warn 路径行为 | 中 | 5-mode 映射表见 §2.5：Prompt mode 现在返回 Ask（之前 Allow+log）；ReadOnly mode 返回 Block（之前 Allow+log，Fail-Open 修复） |
| 测试断言 `assert_eq!(d, SafetyDecision::Allow)` | 零影响 | unit 变体 PartialEq 不变 |

### 3.2 与 ADR-112 兼容性矩阵的关系

ADR-112 §"枚举禁止破坏性扩展"条款要求枚举变更走 major version。`x_claw_agent` 当前 `0.1.0` (path = ...) 未发布，本次变更落在 0.1.x，属内部演进。后续真正发版前再走 ADR-112 兼容性流程。

### 3.3 ADR-113 红线影响

不动 `count_hook_systems()` 编译期断言。枚举扩展对"系统数"无影响。

### 3.4 性能

`Ask` 变体携带 `Vec<RuleSuggestion>`，使 `SafetyDecision` 整体 size 增大到约 56 字节（之前 ~32 字节）。仅在 Prompt mode 触发，Vec 容量预期 0-3 之间。**slice C 不优化**；后续若热路径 profile 发现开销，再改 `Box<[RuleSuggestion]>` 或 `SmallVec`。

---

## 4. Alternatives Considered

### 4.1 不加 `#[non_exhaustive]`

理由：减少外部 `match` 的 `_` 强制。  
拒绝：违反 ADR-112 长期兼容性原则，每次扩展都是 breaking。

### 4.2 `DecisionReason` 用 `String` 而不是 enum

理由：实现简单。  
拒绝：失去 UI / 审计的分类能力，后续无法演进。

### 4.3 `Passthrough` 不进 enum，仅作为 `CompositeSafetyHook` 内部协议

理由：减少 public API 表面。  
拒绝：外部自定义 hook 也可能需要"不表态"语义；不暴露则无法表达。

---

## 5. Implementation Plan

slice C PR 内一次性完成（MVP 范围）：

1. 枚举改动：加 `#[non_exhaustive]` + 新增 `Ask / Passthrough` 变体
2. 新增 `RuleSuggestion` struct + `RuleAction` enum
3. 所有内部 `match SafetyDecision` 加 `_` 兜底或显式列 5 变体
4. `NoopSafetyHook` 行为保持（继续返回 `Allow`）
5. `BashValidationHook` Warn 映射按 §2.5 表实现（5 个 PermissionMode 分支）
6. 测试矩阵：5 决策态 × 5 PermissionMode = 25 case 覆盖
7. `IronclawSafetyHook::before_tool_call` 适配新枚举（PR #493 的 `if let Block` 改为 `match` 处理 `Block` + `Ask` 两种短路 + 显式 `_` 兜底）

**不在 slice C 范围**（未来扩展）：
- 结构化 `DecisionReason` enum（§2.2）
- `RuleSuggestion.suggestions` 自动生成逻辑
- desktop-client `Ask` 弹窗 UX（归 slice D）

---

## 6. References

- claude-code-main `src/utils/permissions/PermissionResult.ts`
- claude-code-main `src/utils/permissions/DecisionReason.ts`
- ADR-112 §"枚举兼容性"
- ADR-113 §"trait seam 收口"
- Issue #73 §"接口扩展"
- PR #493（slice A1，本 ADR 前置）
