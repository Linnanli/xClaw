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

### 2.1 新 enum 形态

```rust
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum SafetyDecision {
    Allow {
        decision_reason: Option<DecisionReason>,
    },
    Redact {
        content: String,
        reason: String,
    },
    Block {
        reason: String,
        decision_reason: Option<DecisionReason>,
    },
    Ask {
        reason: String,
        decision_reason: DecisionReason,
        suggestions: Vec<RuleSuggestion>,
    },
    Passthrough,
}
```

**关键设计**：
- 所有变体加 `#[non_exhaustive]`（外部 `match` 必须显式 `_` 兜底，未来再加变体不破坏外部代码）
- `Allow` / `Block` 都可携带结构化 `decision_reason`（Block 必带；Allow 选填）
- `Ask` 必带 `decision_reason` + 至少一个 `suggestions` 候选

### 2.2 `DecisionReason` 结构

```rust
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum DecisionReason {
    /// 命中具体的 deny/allow/ask 规则。
    RuleMatch {
        rule_id: String,
        rule_pattern: String,
    },
    /// 由 PermissionMode 决定（如 ReadOnly mode 拒绝 write 命令）。
    ModeEnforcement {
        mode: PermissionMode,
    },
    /// 路径越界（workspace 边界）。
    PathValidation {
        path: String,
        category: PathEscapeCategory,
    },
    /// 命中破坏性命令分类。
    DestructiveCommand {
        command_class: String,
    },
    /// sed 表达式校验失败。
    SedValidation {
        detail: String,
    },
    /// 命令注入检测（bashSecurity）。
    CommandInjection {
        injection_kind: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum PathEscapeCategory {
    OutsideWorkspace,
    HomeDirReference,
    SystemPath,
    Redirect,
}
```

**为什么强类型 enum 而不是 `struct { source: &str, detail: String }`**：
1. 后续 desktop-client `Ask` 弹窗按 `DecisionReason` 类型渲染不同 UI（规则命中 vs 路径越界 vs 命令注入 文案完全不同）
2. 审计日志按 `DecisionReason` 类型分类统计
3. 与 claude-code TS 的 discriminated union 同源

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

`BashValidationHook` 当前的 `ValidationResult::Warn { message }` 映射：

| PermissionMode | 当前（PR #493 / TODO #73）| 本 ADR 目标 |
|---|---|---|
| ReadOnly | Block | `Block { reason, decision_reason: Some(...) }` |
| WorkspaceWrite | Allow + log（TODO #73）| `Allow { decision_reason: Some(...) }` + tracing log |
| DangerFullAccess | Allow + log | 同上 |
| Allow | Allow | `Allow { decision_reason: None }` |
| Prompt | Allow + log | `Ask { reason, suggestions, decision_reason }` |

`Ask` 变体在 Prompt mode 才触发；WorkspaceWrite/DangerFullAccess 走结构化 `Allow + reason`，**不打断用户**。

---

## 3. Consequences

### 3.1 破坏性变更面

| 影响 | 严重度 | 缓解 |
|---|---|---|
| 所有 `SafetyDecision::Allow` literal 必须改为 `Allow { decision_reason: None }` | 中 | 一次性迁移；项目内约 20-30 处 |
| 所有 `match SafetyDecision { Allow => ..., Redact => ..., Block => ... }` 必须加 `_` 兜底（因为 `#[non_exhaustive]`） | 中 | 一次性迁移；测试断言用 `matches!()` 替代 |
| `NoopSafetyHook` 默认返回值改为 `Allow { decision_reason: None }` | 低 | 行为等价 |

### 3.2 与 ADR-112 兼容性矩阵的关系

ADR-112 §"枚举禁止破坏性扩展"条款要求枚举变更走 major version。`x_claw_agent` 当前 `0.1.0` (path = ...) 未发布，本次变更落在 0.1.x，属内部演进。后续真正发版前再走 ADR-112 兼容性流程。

### 3.3 ADR-113 红线影响

不动 `count_hook_systems()` 编译期断言。枚举扩展对"系统数"无影响。

### 3.4 性能

`DecisionReason` enum 加入后 `SafetyDecision` size 增大（最大变体 `Ask` 含 `Vec`）。考虑用 `Box` 包装大字段：

```rust
Ask {
    reason: String,
    decision_reason: DecisionReason,
    suggestions: Box<[RuleSuggestion]>,  // 而不是 Vec
},
```

实测后决定。

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

slice C PR 内一次性完成：

1. 枚举改动 + 新增 `DecisionReason` / `RuleSuggestion` / `PathEscapeCategory` / `RuleAction` 类型
2. 所有内部 `SafetyDecision::Allow` literal 升级
3. 所有内部 `match` 加 `_` 兜底
4. `NoopSafetyHook` 行为保持
5. `BashValidationHook` Warn 映射按 §2.5 表实现（5 个 PermissionMode 分支）
6. 测试矩阵：5 决策态 × 5 PermissionMode = 25 case + 5 `DecisionReason` 类型分类测试
7. `IronclawSafetyHook::before_tool_call` 适配新枚举（PR #493 落地的 `if let SafetyDecision::Block` 改为 `match` 处理 5 态）

---

## 6. References

- claude-code-main `src/utils/permissions/PermissionResult.ts`
- claude-code-main `src/utils/permissions/DecisionReason.ts`
- ADR-112 §"枚举兼容性"
- ADR-113 §"trait seam 收口"
- Issue #73 §"接口扩展"
- PR #493（slice A1，本 ADR 前置）
