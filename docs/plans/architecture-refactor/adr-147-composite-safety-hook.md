# ADR-147: `CompositeSafetyHook` —— SafetyHook 链式串联

- **Status**: Draft（W4 #73 slice B 前置）
- **Date**: 2026-05-29
- **Approver**: pending
- **Supersedes**: 无
- **Related**: ADR-112、ADR-113（HookEngine 收口）、ADR-146（SafetyDecision 5 态）
- **Tracking Issue**: #73 slice B

---

## 1. Context

### 1.1 现状

`HookBundle.safety: Arc<dyn SafetyHook>` 是单 hook 槽位。`IronclawSafetyHook`（PR #493 落地）通过持有 `Option<Arc<BashValidationHook>>` 字段手工组合两个 hook —— 这是补丁式硬编码，无法扩展到 secrets / project rules / mcp-permission 等更多 hook。

### 1.2 #73 issue body 要求

> CompositeSafetyHook：可顺序组合多个 SafetyHook，Passthrough 时继续下一个。

### 1.3 上游对照（claude-code-main）

claude-code-main `src/utils/permissions/pipeline.ts` 实现 8 步 pipeline：

```
canUseTool → planMode → bashSecurity → permissions → bashPermissions
  → claudeRules → mcpPermission → askPermissionResult
```

每一步返回 `PermissionResult`，behavior !== 'allow' 短路。pipeline 是固定顺序硬编码的。

本 ADR 采用更通用的 `Vec<Arc<dyn SafetyHook>>` 顺序执行模型，让上层动态决定 chain。

---

## 2. Decision

### 2.1 `CompositeSafetyHook` 定义

落在 `crates/x_claw_agent/src/composite_safety_hook.rs`（与 `SafetyHook` trait 同 crate）。

```rust
pub struct CompositeSafetyHook {
    hooks: Vec<(HookId, Arc<dyn SafetyHook>)>,
}

pub type HookId = &'static str;

impl CompositeSafetyHook {
    pub fn builder() -> CompositeSafetyHookBuilder { ... }
}

pub struct CompositeSafetyHookBuilder {
    hooks: Vec<(HookId, Arc<dyn SafetyHook>)>,
}

impl CompositeSafetyHookBuilder {
    pub fn add(mut self, id: HookId, hook: Arc<dyn SafetyHook>) -> Self { ... }
    pub fn build(self) -> CompositeSafetyHook { ... }
}

#[async_trait]
impl SafetyHook for CompositeSafetyHook {
    async fn before_tool_call(...) -> Result<SafetyDecision> {
        // 见 §2.2 短路语义
    }
    // before_prompt / after_completion / after_tool_output 见 §2.3
}
```

`HookId` 为 `&'static str`，用于 tracing log 和审计（追踪哪条 hook 出的决策）。

### 2.2 `before_tool_call` 短路语义

| 当前 hook 返回 | 行为 |
|---|---|
| `Allow { decision_reason }` | **继续**问下一个 hook，但记录该 hook 的 Allow（用于 audit） |
| `Redact { ... }` | **修改 args 后继续**问下一个 hook（下一个 hook 看到 redacted args） |
| `Block { reason, decision_reason }` | **短路**返回 Block |
| `Ask { ... }` | **短路**返回 Ask |
| `Passthrough` | **继续**问下一个 hook（不记录 Allow，纯透传） |

**末端语义**：所有 hook 都跑完后，最终决策按以下规则汇总：
- 如果有任何 hook 返回 `Allow`：返回**最后一个** `Allow { decision_reason }`
- 如果所有 hook 都返回 `Passthrough`：返回 `Allow { decision_reason: None }`（与上游一致）
- Block / Ask 已经在中途短路，不会走到末端

> **Fail-Safe 通过约定保证**：CompositeSafetyHook 末端必须挂"非 Passthrough"hook 作为兜底（如 `IronclawSafetyHook`）。运行时不强制；文档约束 + builder pattern review 时审查。

### 2.3 4 个 SafetyHook 方法的串联差异

| 方法 | 串联策略 |
|---|---|
| `before_prompt` | 链式 redact：第一个 hook redact 后第二个看到 redacted 版本；Block 短路；最终返回最后非 Passthrough 的 Allow |
| `before_tool_call` | §2.2 短路语义 |
| `after_completion` | **全部串联**（不短路）：每个 hook 顺序应用 sanitize；Block / Ask 在 after_* 无意义，遇到时降级为 Allow + 错误 log |
| `after_tool_output` | 同 `after_completion` |

`after_*` 不短路的理由：output 已经产生，目标是 sanitize，不是拦截。每个 hook 都有机会清洗。

### 2.4 与 `HookBundle.safety` 的接入

**B5.b 方案**（最小破坏面）：`CompositeSafetyHook: SafetyHook`，外部代码不感知。

```rust
// 现状（PR #493 后）
let hook = IronclawSafetyHook::new(safety_layer).with_bash_validation(workspace);
let bundle = HookBundle::default().with_safety(Arc::new(hook));

// 本 ADR 后
let composite = CompositeSafetyHook::builder()
    .add("bash-validation", Arc::new(BashValidationHook::new(workspace, mode)))
    .add("ironclaw-safety", Arc::new(IronclawSafetyHook::new(safety_layer)))
    .add("project-rules", Arc::new(ProjectRulesHook::from_config(...)))
    .build();
let bundle = HookBundle::default().with_safety(Arc::new(composite));
```

`HookBundle.safety` 类型不变，仍是 `Arc<dyn SafetyHook>`。

### 2.5 `IronclawSafetyHook` 的内部 `bash_hook` 字段下线

PR #493 引入的 `bash_hook: Option<Arc<BashValidationHook>>` 字段在 slice B 落地后**移除**。BashValidationHook 直接挂在 CompositeSafetyHook chain 第一位。`IronclawSafetyHook::with_bash_validation()` builder 标记 `#[deprecated]` 一个版本后删除。

### 2.6 ADR-113 红线影响

ADR-113 §2.2 "1 = 编排入口数"。CompositeSafetyHook 是 trait seam 的组合器，**不是新的编排系统**——它仍然走 `HookBundle.safety` 单槽位。`count_hook_systems()` 编译期断言**不变**。

需在 ADR-113 中追加一句澄清：
> trait seam 的内部组合（如 `CompositeSafetyHook`）不计入"hook 系统数"。

### 2.7 错误传播

任何 hook 返回 `Err(_)` 立即短路，整个 chain 失败。**不允许"某个 hook 失败但继续问下一个"**（Fail-Safe）。

---

## 3. Consequences

### 3.1 PR #493 的回退

| PR #493 代码 | slice B 后 |
|---|---|
| `IronclawSafetyHook.bash_hook: Option<Arc<BashValidationHook>>` | 移除字段 |
| `IronclawSafetyHook::with_bash_validation(workspace)` | `#[deprecated]` → 删除 |
| `before_tool_call` 内 `if let Some(bash) = &self.bash_hook && let Block...` | 移除（chain 由 CompositeSafetyHook 负责）|
| `hook_bundle_with_safety(safety, workspace)` | 签名改为 `hook_bundle_with_safety(safety: Arc<dyn SafetyHook>)`，workspace 走 builder |

调用方需迁移到 CompositeSafetyHook builder。**PR #493 不是白做**——slice A1 验证了 hook 接线在 production 跑通，是 slice B 的可执行基础。

### 3.2 与 ADR-146 的耦合

CompositeSafetyHook 的短路语义依赖 `SafetyDecision::Passthrough` 和 `Ask` 变体存在。**slice B 必须在 slice C 之后**。

### 3.3 测试

- 单元：每种短路语义（Allow / Redact / Block / Ask / Passthrough / Err）×（first hook / middle hook / last hook）位置 = 18 case
- 集成：3-hook chain 端到端（BashValidation + IronclawSafetyHook + ProjectRules mock）
- 性能：100-hook chain 延迟 < 1ms（基线）

### 3.4 后续扩展

slice D / E 落地后，CompositeSafetyHook chain 形态：

```rust
CompositeSafetyHook::builder()
    .add("bash-validation", Arc::new(BashValidationHook::new(workspace_cap, mode_provider)))
    .add("ironclaw-safety", Arc::new(IronclawSafetyHook::new(safety_layer)))
    .add("project-rules", project_rules_hook)
    .add("mcp-permission", mcp_perm_hook)
    .build()
```

---

## 4. Alternatives Considered

### 4.1 仿照 claude-code 硬编码 8 步 pipeline

理由：与上游 100% 对齐。  
拒绝：硬编码 chain 顺序无法适配 desktop-client 与 CLI 的差异；project-rules / mcp-permission 在 ironclaw 端尚未存在。

### 4.2 `Vec<Box<dyn SafetyHook>>` 而不是 `Vec<(HookId, Arc<...>)>`

理由：减少 boilerplate。  
拒绝：丢失 audit / tracing 的 hook 来源标识；`Arc` 比 `Box` 适配多 chain 共享。

### 4.3 `before_*` / `after_*` 全部统一短路 OR 全部串联

理由：语义一致。  
拒绝：`before_*` 拦截语义和 `after_*` sanitize 语义本质不同；强行统一会让某一端语义错误。

---

## 5. Implementation Plan

slice B PR 内：

1. 新增 `crates/x_claw_agent/src/composite_safety_hook.rs`（+ 模块导出）
2. `CompositeSafetyHook` + Builder 实现
3. `impl SafetyHook for CompositeSafetyHook` 实现 4 方法
4. 单元测试：18 短路 case + 错误传播
5. 集成测试：3-hook chain 端到端
6. `IronclawSafetyHook` 内 `bash_hook` 字段标记 `#[deprecated(note = "use CompositeSafetyHook")]`
7. ironclaw `hook_bundle_with_safety_*` 签名重构，调用方迁移到 builder
8. ADR-113 文档追加 §2.2 澄清条款

---

## 6. References

- claude-code-main `src/utils/permissions/pipeline.ts`
- ADR-112 §"枚举 + trait 兼容"
- ADR-113 §"1 = 编排入口数"
- ADR-146（SafetyDecision 5 态）
- PR #493（slice A1，本 ADR 的前置接线）
- Issue #73 §"CompositeSafetyHook"
