# ADR-149: Tool Visibility Triple Gate — Prompt / Definitions / Executor 三层强制契约

- **Status**: Accepted（2026-05-13，对话评审通过）
- **Date**: 2026-05-13（v1.0 Draft）/ 2026-05-13（v2.0 Accepted —— 新增 §1.3 三仓库对比 + §2.7 Source-Aware Default + §2.8 与 #484 衔接 + §6.5 备选 E）
- **Approver**: 用户对话评审通过（Phase J/K）
- **Supersedes**: 无（新增）
- **Related**:
  - GitHub Issue #84（P0-B/W3 Tool visibility triple gate：prompt + tool defs + executor — 本 ADR 落地后 close）
  - ADR-112（输入清单 §484 codex `build_specs_with_discoverable_tools` 工具注册统一）
  - ADR-113（Hook 系统收口 5→1，trait seam 双层抽象，本 ADR 复用 Layer B 注入模式）
  - ADR-148（EgressGate 模式，本 ADR 沿用"维度切分 + fail-closed enum 决策"思路）
  - claude-code-parity（`docs/plans/claude-code-parity-architecture.md` §680-687 disabled_tools HashSet 模型）
  - 现有实现：
    - [`desktop-client/ironclaw/src/tools/feature_flags.rs`](../../desktop-client/ironclaw/src/tools/feature_flags.rs) — 简单 `disabled_tools: HashSet<String>` 黑名单（仅 L3）
    - [`desktop-client/ironclaw/src/tools/registry.rs`](../../desktop-client/ironclaw/src/tools/registry.rs) — 5 个 `register_*` 入口待 P0-2 统一
    - [`desktop-client/ironclaw/tests/parity_harness.rs:1987,2033`](../../desktop-client/ironclaw/tests/parity_harness.rs) — sp_016/sp_017 disabled_tool 测试
  - 上游依赖（现成）：
    - `dasclaw_identity`（actor / tenant 输入）
    - `dasclaw_workspace_cap`（capability scope 输入）
    - `dasclaw_governance::trust_resolver`（#67 已完成，提供 RequireApproval 维度）
  - **非依赖**：`dasclaw_governance::policy_engine`（#65 已完成，但维度是 lane merge/escalate，与本 ADR 无关——避免语义污染）

---

## 1. Context — 现状盘点

### 1.1 #84 的"三层 gate"目标

issue #84 要求：当某个工具被 policy 决策为"对当前 actor 不可见 / 拒绝"时，必须在以下**三个独立层面**全部生效，缺一不可：

| Layer | 位置 | 强制内容 |
|---|---|---|
| **L1 Prompt visibility** | system prompt / tool catalog 段拼装时 | 该工具的描述/示例**不进入** prompt |
| **L2 Tool definitions** | 发给 LLM API 的 `tools` / `functions` JSON spec | 该工具**不出现**在 spec 列表 |
| **L3 Executor preflight** | tool dispatcher 收到 LLM 返回的 tool_call | 即使 LLM"幻觉"调用了被禁工具，executor **拒绝执行**并 audit |

三层任一缺失即构成攻击面：
- L1 漏 → LLM 看到禁用工具描述，可能"指挥"自己绕路
- L2 漏 → 模型直接调用禁用工具（即使 prompt 没提）
- L3 漏 → prompt injection / 越狱 / 模型幻觉绕过 L1+L2

### 1.2 dasclaw 现状（实证）

| 维度 | 现状 | 缺口 |
|---|---|---|
| L1 prompt | tool catalog 段拼装无 policy 过滤 | ❌ 完全无 |
| L2 tool defs | `ToolRegistry::list_tools()` 全量返回，5 个 `register_*` 入口分散 | ❌ 完全无 |
| L3 executor | `feature_flags::disabled_tools.contains(name)` 简单黑名单 | ⚠️ 仅静态，无 context-aware（无 tenant / actor / env / args 维度） |
| Audit metadata | 无（拒绝时只有 bool） | ❌ 完全无 |
| Dynamic tool 覆盖 | MCP / WASM / extension 注册路径未经 policy | ❌ 完全无 |

测试覆盖（`parity_harness.rs`）只验证 L3，未验证 L1/L2 共契约：
- `sp_016_disabled_tool_rejected`
- `sp_017_multiple_disabled_tools_rejected`

### 1.3 上游参考的取舍

#### 1.3.1 三仓库工具拦截机制全景对比

本节系统化对比 codex-cli-main / claw-code / ironclaw-main 三个上游参考仓库的工具拦截/白名单设计（证据 grep 自仓内 `codex-cli-main/`、`claw-code/`、`ironclaw-main/`，仅供设计借鉴，禁止直接复制实现）：

| 维度 | codex-cli-main | claw-code | ironclaw-main | **dasclaw 现状** | **本 ADR 目标** |
|---|---|---|---|---|---|
| **拦截模型** | Plan-based spec 构建 | PermissionMode 模式机 | Capability + Lease + GatePipeline | Blocklist HashSet | `ToolVisibilityPolicy` trait + Source-aware default |
| **拦截层数** | 1（spec 期）+ 网络层 | 1（enforce 期） | 1（gate pipeline，可扩展） | 2（L2 spec + L3 execute） | **3**（L1 prompt + L2 spec + L3 execute） |
| **核心 API** | `build_tool_registry_plan(config, params)` | `PermissionEnforcer.check(tool, input)` | `ExecutionGate.evaluate(ctx)` | `feature_flags.is_enabled(tool)` | `ToolVisibilityPolicy.check(ctx)` |
| **决策类型** | 函数式（不出现即拒绝） | `EnforcementResult::{Allowed, Denied}` | `GateDecision::{Allow, Pause, Deny}` closed enum | bool | `ToolGateDecision::{Allow, Hide, DenyArgs, RequireApproval}` closed enum |
| **Fail-closed** | 隐式 | 显式 Denied | **结构性**（无 None 变体） | 显式 Disabled err | **结构性** + 内部错误自动转 Hide |
| **黑白名单** | 无（namespace 选择） | 模式 × per-tool 最小权限 | Lease + Tier + `AUTONOMOUS_TOOL_DENYLIST` | 黑名单（HashSet） | Source-aware（BuiltIn 默认 Allow + Blocklist 修剪） |
| **强制 deny 名单** | 无 | 由 PermissionMode 间接 | ✅ `AUTONOMOUS_TOOL_DENYLIST` 18 项常量 | ❌ 无 | 留 #484 跟踪 |
| **暂停/审批语义** | spec 决定是否含审批元工具 | `Prompt` mode 延迟到 caller | `Pause { ResumeKind }` 3 种 closed | 无 | `RequireApproval { spec }` 复用 `trust_resolver` |
| **Hook 注入** | 无 | `PermissionOverride::{Allow,Deny,Ask}` | gate 自身就是 hook | ❌ 无 | Layer A HookEngine + Layer B policy 天然支持 |
| **Tier 分类** | 无 | 无（mode 是全局） | ✅ 4 档：ReadOnly < Stateful < Privileged < Administrative | ❌ 无 | 暂不引入，留 #484 评估 |
| **关键文件 LOC** | `tool_registry_plan.rs` 622 + network-proxy | `permissions.rs` 683 + `permission_enforcer.rs` 585 = 1268 | `gate/*.rs` + `capability/planner.rs` | `feature_flags.rs` 97 | 预估 ~2240 净 LOC |
| **License** | Apache-2.0（可 vendor） | 复杂（不 vendor） | 复杂（不 vendor，仅借鉴设计） | — | Apache-2.0 vendor codex 子集 |

#### 1.3.2 dasclaw 决定吸收的设计点

| 来源 | 设计 | 本 ADR 是否采纳 | 落地位置 |
|---|---|---|---|
| **codex** `tool_registry_plan.rs` 集中 spec 构建 | ✅ **选择性 vendor** | §2.4 |
| **codex** `build_specs_with_discoverable_tools` 模板 | ✅ 作为 L2 实现骨架 | §2.2 |
| **ironclaw-main** `GateDecision` closed enum + fail-closed by construction | ✅ **已对齐**（`ToolGateDecision` 无 None 变体） | §2.1 |
| **ironclaw-main** `RequireApproval / Pause { ResumeKind }` 三态决策 | ✅ 借鉴方向（`RequireApproval { spec }`） | §2.1 |
| **ironclaw-main** `AUTONOMOUS_TOOL_DENYLIST` 硬编码强制名单 | ✅ **强烈推荐采纳**（但留 #484 落地，避免本 ADR 范围扩大） | #484 Phase 4 |
| **ironclaw-main** `ToolTier` 4 档分类 | ❌ 暂不引入 | 留 #484 评估 |
| **ironclaw-main** `GatePipeline` 多 gate priority 串行 | ❌ 暂不引入（P0 单 policy 注入足够） | 未来可演进 |
| **claude-code-parity** `disabled_tools: HashSet<String>` | ✅ 作为 `BlocklistPolicy` 最简实现（向后兼容） | §2.5 |
| **claw-code** `PermissionOverride` hook 注入 | ✅ 通过 Layer A + Layer B 天然支持 | §3.2 |
| **claw-code** `PermissionMode` 全局模式机 | ❌ 不采纳（dasclaw 倾向 per-tool policy） | — |
| **codex** `network-proxy/policy.rs` host allowlist | ❌ 维度正交，由 ADR-148 `EgressGate` 覆盖 | §3.2 |
| **claude-code** MCP `tools/list` 单层 client filter | ❌ 维度不够 | — |
| 本仓库 `dasclaw_governance::policy_engine` (#65) | ❌ 维度无关（lane merge/escalate），**不复用同名 trait** | — |

#### 1.3.3 dasclaw 选三层 gate 的独特理由（区别于三仓库单层）

三仓库都只有单层强制（codex 在 spec 期；claw-code 在 enforce 期；ironclaw-main 在 gate 期），dasclaw 提出三层的独特理由：

> dasclaw 是 **企业多 channel agent platform**（Discord / Slack / Web / IPC 同时存在），**LLM prompt injection 攻击面比单一 IDE/CLI 大得多**。L1 prompt 层的工具描述若不过滤，攻击者可通过 prompt injection 让 LLM 试图调用未注册工具，即使 L2/L3 已拒，**prompt 的"暗示"已经污染对话**。这是三仓库当前部署形态（CLI/IDE/Single-channel）没遇到的威胁模型。

→ #84 的"三层"不是 over-engineering，是**多 channel 部署形态独有的需求**。

### 1.4 关键架构问题：当前没有"工具维度策略" trait

dasclaw 已有 governance 6-pack（policy_engine / recovery_recipes / trust_resolver / branch_lock / stale / green / lane_events），但**没有任何 trait 描述"工具是否对此 actor 可见 / 可调用"**。`feature_flags::disabled_tools` 是 hardcoded 静态配置，无法表达：
- 租户隔离（Tenant A 不可见 Tenant B 的 dynamic tools）
- 角色 RBAC（junior 禁用 `git push --force`）
- 环境敏感（prod 禁用 `mock_*`）
- 参数白名单（`shell.exec` 仅允许特定 cmd）
- 时间窗口
- 配额

---

## 2. Decision

### 2.1 引入新 trait seam `ToolVisibilityPolicy`（Layer B）

参照 ADR-148 `EgressGate` 模式：**单方法 + enum kind + enum decision + fail-closed**。

```rust
// crates/dasclaw_governance/src/tool_visibility.rs (新文件，feature-gated)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolGateLayer {
    /// L1：拼装 system prompt 中的 tool catalog 段
    PromptCatalog,
    /// L2：构建发给 LLM 的 tool spec JSON
    LlmDefinitions,
    /// L3：dispatcher 收到 tool_call，准备执行前
    ExecutorPreflight,
}

#[derive(Debug, Clone)]
pub struct ToolGateContext<'a> {
    pub tool_name: &'a str,
    pub tool_source: ToolSource,        // BuiltIn / MCP / WASM / Extension
    pub layer: ToolGateLayer,
    pub actor: &'a ActorRef,            // 来自 dasclaw_identity
    pub workspace_cap: &'a WorkspaceCap, // 来自 dasclaw_workspace_cap
    pub environment: Env,                // Dev / Staging / Prod
    /// 仅 L3 提供（L1/L2 时 None）
    pub proposed_args: Option<&'a Value>,
}

#[derive(Debug, Clone)]
pub enum ToolGateDecision {
    /// 三层均通过
    Allow,
    /// L1+L2：从输出中过滤掉；L3：拒绝执行 + 上 audit（视为幻觉/伪造）
    Hide { audit: AuditMetadata },
    /// L3 专用：参数命中黑名单
    DenyArgs { reason: String, audit: AuditMetadata },
    /// 接 trust_resolver：需要用户审批
    RequireApproval { spec: ApprovalSpec, audit: AuditMetadata },
}

#[derive(Debug, Clone)]
pub struct AuditMetadata {
    pub policy_id: String,           // 命中的 policy 规则 ID
    pub decision_reason: String,     // 人类可读理由
    pub redacted_payload: Option<String>, // L3 args 经 redaction 后留痕
    pub tenant_id: TenantId,
    pub actor_id: ActorId,
}

#[async_trait]
pub trait ToolVisibilityPolicy: Send + Sync {
    async fn check(&self, ctx: &ToolGateContext<'_>) -> ToolGateDecision;
}
```

**约束**：
- 内部错误（policy 实现 panic / 超时 / unwrap）**默认转 `Hide`**（fail-closed，绝不 fail-open）
- 同一份 `policy: Arc<dyn ToolVisibilityPolicy>` 注入到三层调用点，**保证决策一致性**（L1+L2+L3 必须看到同一份判定）
- trait 居住 `crates/dasclaw_governance/src/tool_visibility.rs`，feature `tool_visibility` 默认 off（W4 scaffolding 风格）

### 2.2 三层强制接入点

```rust
// L1: crates/x_claw_agent/src/prompts/catalog.rs (新建或扩展)
//     LayeredPromptBuilder 拼装 tool catalog 段时调用
for tool in registry.list_tools() {
    let ctx = ToolGateContext { layer: PromptCatalog, .. };
    match policy.check(&ctx).await {
        Allow | RequireApproval { .. } => include_tool_in_catalog(tool),
        Hide { audit } => { audit_log(audit); /* 跳过 */ }
        DenyArgs { .. } => unreachable!("L1 has no args"),
    }
}

// L2: desktop-client/ironclaw/src/tools/spec_builder.rs (从 codex vendor 改造)
//     build_tool_specs() 返回 tools JSON 前过 policy
for tool in registry.list_tools() {
    let ctx = ToolGateContext { layer: LlmDefinitions, .. };
    match policy.check(&ctx).await {
        Allow | RequireApproval { .. } => specs.push(tool.spec()),
        Hide { audit } => { audit_log(audit); /* 跳过 */ }
        DenyArgs { .. } => unreachable!("L2 has no args"),
    }
}

// L3: desktop-client/ironclaw/src/agent/dispatcher.rs (替换 feature_flags 检查)
//     dispatch_tool_call() 执行前
let ctx = ToolGateContext { layer: ExecutorPreflight, proposed_args: Some(&args), .. };
match policy.check(&ctx).await {
    Allow => executor.run(tool, args).await,
    Hide { audit } => {
        audit_log(audit);
        return Err(ToolError::HallucinatedDisallowedTool);  // 模型绕过 L1+L2，硬拒
    }
    DenyArgs { reason, audit } => {
        audit_log(audit);
        return Err(ToolError::DeniedArgs(reason));
    }
    RequireApproval { spec, audit } => {
        audit_log(audit);
        approval_gate.request(spec).await?  // 复用 ApprovalGate trait seam
    }
}
```

### 2.3 启动期共契约校验

在 `dasclaw_governance::tool_visibility::contract` 增加 `const _: () = assert!(...)` 编译期红线，参考 `dasclaw_hooks::count_hook_systems()` 模式：

- `assert_triple_gate_wired`：三个接入点必须都调用了同一份 `policy.check()`（通过启动期注册计数）
- `no_static_disabled_tools_in_executor`：禁止 dispatcher 中残留 `feature_flags.is_enabled()` 直调

### 2.4 vendor codex 取舍清单

新增 `crates/vendor/codex_tool_spec/`（结构对齐已有 `vendor/` 模式）：

| codex 文件 | LOC | 是否 vendor | 用途 |
|---|---|---|---|
| `tool_registry_plan_types.rs` | ~80 | ✅ 全量 | L2 spec 类型定义 |
| `tool_registry_plan.rs` | 622 | ⚠️ **部分**：保留 `build_tool_registry_plan` 主体（去掉 discoverable_tools 元工具逻辑 + UI client filter） | L2 spec 构建骨架 |
| `tool_discovery.rs` | ~150 | ❌ 不要 | codex CLI 客户端类型过滤，dasclaw 用 policy 替代 |
| `tool_discovery_tests.rs` | ~80 | ❌ 不要 | 同上 |
| `tool_registry_plan_tests.rs` | 1900+ | ❌ 不要 | 需求不同，dasclaw 自己写 req_tool_visibility_* |
| `lib.rs` | 152 | ⚠️ **片段**：仅 re-export | — |

**强制要求**：
- 保留 codex 原始 Apache-2.0 copyright header（每文件首部）
- 顶层 `NOTICE` 文件增加 codex 归属（如已存在则追加章节）
- vendor crate **禁止修改**已 vendor 的函数体（除明确删除部分外）；任何扩展通过 wrapper crate 完成

### 2.5 向后兼容迁移路径

`feature_flags::disabled_tools` 不立即删除，提供 `BlocklistPolicy` 默认实现：

```rust
/// 兼容 #84 落地前的简单黑名单模型。新部署建议直接配置完整 policy。
pub struct BlocklistPolicy {
    disabled_tools: HashSet<String>,
}

#[async_trait]
impl ToolVisibilityPolicy for BlocklistPolicy {
    async fn check(&self, ctx: &ToolGateContext<'_>) -> ToolGateDecision {
        if self.disabled_tools.contains(ctx.tool_name) {
            ToolGateDecision::Hide { audit: minimal_audit(ctx, "blocklist") }
        } else {
            ToolGateDecision::Allow
        }
    }
}
```

→ 现有 `feature_flags.rs` 读取的 config 不变，构造时包装为 `BlocklistPolicy` 注入到三层，**零行为退化**。

### 2.6 与 `trust_resolver` 的对接

`RequireApproval` 决策的 `ApprovalSpec` 直接复用 `dasclaw_governance::trust_resolver::TrustPolicy::RequireApproval` 路径，**不另起审批通道**。policy 实现内部可调 `trust_resolver` 作为子决策器（例如"prod 环境 + 写操作 → RequireApproval"）。

### 2.7 Source-Aware Default + Always-Has-Policy 不变式

本 ADR 不允许 `policy: Option<Arc<dyn ToolVisibilityPolicy>>` 的可空注入形态。**`Arc<dyn ToolVisibilityPolicy>` 是必选构造参数**——启动期保证非 None，零行为退化。

#### 2.7.1 `ToolSource` 枚举（本 ADR 仅强制 BuiltIn，其它占位等待 #484）

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolSource {
    /// 仓库内 PR review + CI 审计的工具（dasclaw_governance / x_claw_agent 注册）
    BuiltIn,
    /// 远端 MCP server 暴露 —— 本 ADR 范围占位，详细策略归 #484
    Mcp { server_id: String, verified: bool },
    /// 远端下载的 skill —— 本 ADR 范围占位，详细策略归 #484
    Skill { lock_hash_matched: bool },
    /// 用户/企业安装的扩展 —— 本 ADR 范围占位，详细策略归 #484
    Extension { bundle_id: String, verified: bool },
    /// WASM 工具（当前 build-from-source，未来动态加载占位）
    Wasm { build_from_source: bool },
}
```

#### 2.7.2 `BlocklistPolicy` 默认决策表（仅 BuiltIn 在本 ADR 范围内强制）

| `ToolSource` | `verified` | **`BlocklistPolicy` 默认决策** | 本 ADR 是否落地 |
|---|---|---|---|
| `BuiltIn` | N/A | **Allow**（仓内已审计） | ✅ 本 ADR |
| `Mcp` | `true` | Allow | ⏭ #484 Phase 3 |
| `Mcp` | `false` | **Hide**（fail-closed） | ⏭ #484 |
| `Skill` | `true` | Allow | ⏭ #484 |
| `Skill` | `false` | **Hide** | ⏭ #484 |
| `Extension` | `true` | Allow | ⏭ #484 |
| `Extension` | `false` | **Hide** | ⏭ #484 |
| `Wasm` | `build_from_source=true` | Allow | ✅ 本 ADR（占位） |
| `Wasm` | `build_from_source=false` | **Hide** | ⏭ #484 |

**本 ADR 范围**：`BlocklistPolicy` 仅对 `BuiltIn` 工具执行 disabled_tools HashSet 检查，其它 source 默认走 `Hide`（fail-closed），直到 #484 落地 verifier。

#### 2.7.3 Always-Has-Policy 启动期 contract

```rust
// crates/dasclaw_governance/src/tool_visibility/contract.rs

impl LayeredPromptBuilder {
    pub fn new(/* ... */, policy: Arc<dyn ToolVisibilityPolicy>) -> Self { /* ... */ }
    //                    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ 无 Option，无 default
}

impl ToolSpecBuilder {
    pub fn new(/* ... */, policy: Arc<dyn ToolVisibilityPolicy>) -> Self { /* ... */ }
}

impl ToolDispatcher {
    pub fn new(/* ... */, policy: Arc<dyn ToolVisibilityPolicy>) -> Self { /* ... */ }
}
```

→ 任何忘记注入 policy 的代码路径都在编译期 / 构造期失败，**不可能**在运行时遇到 "policy is None → fall through to allow-all" 这种 fail-open 退化。零退化部署：传 `Arc::new(BlocklistPolicy::default())`（空 disabled_tools = Allow-All for BuiltIn）。

### 2.8 与 #484 的衔接边界

本 ADR 严格限定为 **BuiltIn 工具的三层 gate 机制**。skills / MCP / extension 的可见性 + 完整性验证由 **GitHub issue #484** 独立跟踪，并按以下接口约定衔接：

| 接口 | 本 ADR（#84）提供 | #484 实现 |
|---|---|---|
| `ToolSource` enum | 声明 5 个变体（含 Mcp/Skill/Extension/Wasm 占位） | 实化 `verified` 字段语义（接 verifier） |
| `BlocklistPolicy` | 仅 BuiltIn 检查 disabled_tools，其它 source 默认 Hide | 扩展为 `SourceAwareBlocklistPolicy`，按 §2.7.2 决策表完整路由 |
| `IntegrityVerifier` trait | ❌ 不在本 ADR 范围 | #484 Phase 1 落地 |
| `ADMIN_TOOL_DENYLIST` 强制名单 | ❌ 不在本 ADR 范围 | #484 Phase 4 落地（借鉴 ironclaw-main `AUTONOMOUS_TOOL_DENYLIST`） |

**关键不变式**：#484 落地前，所有非 `BuiltIn` 的 `ToolSource` 在 `BlocklistPolicy` 默认决策下走 Hide —— 即"还没法验证的，就先不让看见"，**永不退化为 fail-open**。

---

## 3. 目标架构

### 3.1 静态组件持有图

```
                ┌──────────────────────────────────────────────────────┐
                │  Policy 注入（启动期一次）                            │
                │                                                       │
                │   Arc<dyn ToolVisibilityPolicy>                       │
                │     │                                                 │
                │     ├──► LayeredPromptBuilder  (L1 PromptCatalog)     │
                │     ├──► spec_builder          (L2 LlmDefinitions)    │
                │     └──► dispatcher            (L3 ExecutorPreflight) │
                └──────────────────────────────────────────────────────┘

  agent loop turn:
  ─────────────────────────────────────────────────────────────────
  ① Prompt 拼装阶段
       prompt_builder.assemble()
         └─ for tool in registry: policy.check(PromptCatalog) ──┐
                                                                  ▼
                                                          Hide → 跳过
                                                          Allow → 拼入

  ② LLM 请求构建阶段
       spec_builder.build()
         └─ for tool in registry: policy.check(LlmDefinitions) ─┐
                                                                  ▼
                                                          Hide → 跳过
                                                          Allow → 加入 tools[]

  ③ LLM 返回 tool_call
  ④ Dispatcher 接收
       dispatcher.dispatch(tool_call)
         └─ policy.check(ExecutorPreflight, args=Some(...))
              ├─ Allow             → executor.run()
              ├─ Hide              → ToolError::HallucinatedDisallowedTool + audit
              ├─ DenyArgs(reason)  → ToolError::DeniedArgs + audit
              └─ RequireApproval   → approval_gate.request() → ...
  ─────────────────────────────────────────────────────────────────
```

### 3.2 与 ADR-113 / ADR-148 的关系

- **ADR-113 双层抽象**：本 ADR 在 Layer B 新增 `ToolVisibilityPolicy` trait seam，**不**触碰 Layer A HookEngine
- **ADR-148 EgressGate**：维度正交（egress = 数据出网/持久化/显示；tool gate = 工具是否对 actor 可见/可调）。**两个 trait 并存**，分别注入不同位置。`EgressGate.check(ToolExecution, args)` 仍在 L3 之后执行（先 policy 决定能不能调，再 egress 决定 args 能不能出）。
- 调用顺序在 L3：`ToolVisibilityPolicy.check` → 通过 → `EgressGate.check(ToolExecution)` → 通过 → `executor.run`

### 3.3 测试族

| 测试 ID 前缀 | 验证内容 | 测试位置 |
|---|---|---|
| `req_tool_visibility_84_001` | L1 prompt 不含被 Hide 的工具描述 | `crates/x_claw_agent/tests/` |
| `req_tool_visibility_84_002` | L2 tool specs 不含被 Hide 的工具 | `desktop-client/ironclaw/tests/` |
| `req_tool_visibility_84_003` | L3 dispatcher 拒绝幻觉调用 + 留 audit | 同上 |
| `req_tool_visibility_84_004` | 三层决策一致性（同一 ctx 三次 check 返回同一决策） | governance 单测 |
| `req_tool_visibility_84_005` | 内部错误 fail-closed（policy panic → Hide） | governance 单测 |
| `test_security_84_jailbreak_via_l1_only` | 单独移除 L1 不应放行 | 集成测试 |
| `test_security_84_jailbreak_via_l2_only` | 单独移除 L2 不应放行 | 集成测试 |
| `test_security_84_hallucinated_tool_call` | LLM 凭空调用被 Hide 工具被 L3 拦截 | 集成测试 |
| `test_security_84_tenant_isolation` | Tenant A 看不见 Tenant B 的 dynamic tool | 集成测试 |
| `test_security_84_args_blocklist_bypass` | 已过 L3 的 tool 仍受 DenyArgs 约束 | 集成测试 |
| `req_tool_visibility_84_migration` | `BlocklistPolicy` 行为与旧 `feature_flags::disabled_tools` 完全等价 | parity_harness |
| sp_016 / sp_017 已有 | 保持绿（迁移期间不能退化） | parity_harness（不动） |

---

## 4. Implementation Plan

### 4.1 PR 拆分策略（默认单 PR，对齐 ADR-148 §4.5 经验）

**默认方案：单 PR**（~2500 LOC 净 diff），保证 reviewer 一次看完三层共契约。

仅当**触发以下任一条件**才拆为 2-PR 应急：
- vendor codex 部分超 800 LOC（含 copyright header + NOTICE 调整）
- L3 dispatcher 改动触及 >5 处分散调用点（参考 ADR-148 §2.5 9 处 sanitize_tool_output 教训）
- 跨 crate 改动 >4 个（governance / vendor / x_claw_agent / ironclaw / desktop-client）→ 拆为 "PR-1: trait + vendor + BlocklistPolicy" + "PR-2: 三层接入点"

### 4.2 LOC 估算

| 文件 | 类型 | LOC | 说明 |
|---|---|---|---|
| `crates/dasclaw_governance/src/tool_visibility.rs` | 新建 | ~350 | trait + types + BlocklistPolicy + contract |
| `crates/dasclaw_governance/tests/tool_visibility_test.rs` | 新建 | ~300 | req_tool_visibility_* 单测 |
| `crates/vendor/codex_tool_spec/` | 新建 vendor | ~500 | types + plan 主体（删减后） |
| `crates/vendor/codex_tool_spec/Cargo.toml + NOTICE` | 新建 | ~30 | 归属 |
| `desktop-client/ironclaw/src/tools/spec_builder.rs` | 新建/重构 | ~250 | L2 接入 + 替换 5 个 register_* 入口 |
| `desktop-client/ironclaw/src/agent/dispatcher.rs` | 修改 | ~80 | L3 接入，替换 feature_flags 直调 |
| `desktop-client/ironclaw/src/tools/feature_flags.rs` | 修改 | ~30 | 保留为 BlocklistPolicy 构造器 |
| `crates/x_claw_agent/src/prompts/catalog.rs` | 新建/扩展 | ~150 | L1 接入 |
| `desktop-client/ironclaw/tests/tool_visibility_integration.rs` | 新建 | ~400 | test_security_84_* 集成测试 |
| `desktop-client/ironclaw/tests/parity_harness.rs` | 修改 | ~50 | 迁移测试 + 保留 sp_016/sp_017 |
| ADR-149 本文件 + 顶层 NOTICE | 新建/修改 | ~100 | 文档 |
| **合计** | | **~2240** | 单 PR 可控范围 |

### 4.3 阶段顺序（PR 内部）

1. trait + types + BlocklistPolicy（无依赖，编译先过）
2. vendor codex（独立 crate，与 1 平行）
3. L3 dispatcher 接入（最关键的 fail-closed 路径，优先有测试覆盖）
4. L2 spec_builder 接入（依赖 vendor）
5. L1 prompt catalog 接入（依赖 LayeredPromptBuilder）
6. contract 启动期校验（最后加，避免循环阻塞）
7. parity_harness 迁移 + 新集成测试

### 4.4 PR Body 必填模板（对齐 [.github/pull_request_template.md](../../.github/pull_request_template.md)）

```markdown
Closes #84

## 背景/目标
落地 ADR-149：工具可见性三层强制契约（prompt + tool defs + executor）

## 改动范围
- 新 trait `ToolVisibilityPolicy` + `BlocklistPolicy` 默认实现
- vendor codex `tool_registry_plan` 选择性引入
- L1/L2/L3 三处接入点
- 14 个新测试（含 5 个 security 测试）

## 非目标（What's NOT in this PR）
- 完整租户/RBAC policy 实现（本 PR 只交付 trait + Blocklist 兼容实现）
- MCP/WASM 动态工具的 source 字段细化（留待后续）
- prompt catalog 段重排序优化

## 验证
- [ ] `cargo nextest run -p dasclaw_governance --features tool_visibility`
- [ ] `cargo nextest run -p ironclaw`（含 parity_harness sp_016/sp_017 不退化）
- [ ] `python3.12 scripts/check_no_panics.py --base origin/xClaw`
- [ ] 三层共契约 contract 编译期检查通过
- [ ] vendor NOTICE 已添加 codex 归属

## Sources read
- AGENTS.md §6（复用优先于新建）+ §2（红线）
- ADR-149（本 PR 落地的 ADR）§2.1 trait 设计、§2.4 vendor 取舍、§4.3 阶段顺序
- ADR-112 §484（codex tool 注册统一参考）
- ADR-113 §2.1（Layer B trait seam 双层抽象）
- ADR-148 §2.2 + §4.5（EgressGate enum 决策模式 + 拆分应急）
- codex-cli-main/codex-rs/tools/src/tool_registry_plan.rs（vendor 源）

## 三层 reviewer 阅读引导
- Tier 1（架构）：ADR-149 §2.1 + §2.2 + 本 PR 三层接入 diff
- Tier 2（实现）：BlocklistPolicy + vendor crate + spec_builder
- Tier 3（测试）：14 个新测试（5 个 security_*）
```

### 4.5 拆分应急条款（对齐 ADR-148 §4.5 经验）

满足以下任一条件，可临时拆为 2 PR 并在 PR-1 描述声明触发原因：

| 触发条件 | 拆分方式 |
|---|---|
| vendor codex 部分 NOTICE / copyright 处理引入超 800 LOC（含许可证审计） | PR-1: vendor + trait + Blocklist；PR-2: 三层接入 + 测试 |
| L3 dispatcher 改动牵连超过 5 处分散调用点 | PR-1: trait + L3；PR-2: L1 + L2 + contract |
| 编译失败级跨 crate 类型循环依赖 | 按依赖图细拆，最大 3 PR，每 PR 描述附依赖图 |

任何拆分都**必须**保证：
- 每个 PR 自身编译 + nextest 全绿
- 中间状态下 fail-closed 不退化（宁可三层全走 BlocklistPolicy 兜底，不允许 L3 暂时 Allow-All）

### 4.6 禁止补丁式代码（对齐 [AGENTS.md](../../AGENTS.md) §2）

明确**禁止**以下补丁手段：
- 在 `feature_flags::is_enabled()` 上追加 `if context.tenant_id ...` 分支扩展为 context-aware
- 复制 `disabled_tools` 检查到 `prompt_builder.rs` / `spec_builder.rs` 各自实现
- 通过 `cfg(feature = "old_disabled_tools")` flag 切换大块代码

必须通过 trait 注入完成三层接入。

---

## 5. Consequences

### 5.1 正面

- ✅ 三层 gate 强制契约，关闭 #84 攻击面
- ✅ 同一份 `Arc<dyn ToolVisibilityPolicy>` 注入，**保证决策一致**（不可能 L1 通过 L2 阻断）
- ✅ Fail-closed 默认，内部错误转 Hide
- ✅ 向后兼容：旧 `disabled_tools` 配置自动包装为 `BlocklistPolicy`，行为不退化
- ✅ vendor codex 的 spec 构建器，减少自造轮子（Apache-2.0 兼容）
- ✅ 维度正交：`ToolVisibilityPolicy`（谁能调什么工具）vs `EgressGate`（数据能不能出网） vs `trust_resolver`（要不要审批），无语义重叠
- ✅ 测试族明确（11 个 req_* + 5 个 test_security_*），TDD 路径清晰

### 5.2 负面 / 成本

- ⚠️ 引入新 crate `crates/vendor/codex_tool_spec`，仓库 vendor 边界扩大（需更新 deny.toml 白名单 + NOTICE）
- ⚠️ feature `tool_visibility` 启用后，三层每次都额外一次 async 调用（每 turn 大致 +2N async hop，N=工具数）—— `BlocklistPolicy` 实现已是 O(1) HashSet，影响可忽略，但自定义 policy 实现需关注延迟预算
- ⚠️ codex vendor 文件未来若上游有安全更新，需要手动同步（参考已有 vendor 模式）

### 5.3 不在本 ADR 范围

- 具体的多租户 policy 实现（本 ADR 只交付 trait + Blocklist 兼容实现）
- `ToolSource::MCP/WASM/Extension` 的 source 鉴定与签名校验（动态工具供应链安全，另立 ADR）
- prompt catalog 段的工具描述压缩 / 段重排序优化
- approval UX 流程（复用 `ApprovalGate`，不重做）

---

## 6. Alternatives Considered

### 6.1 备选 A：仅扩展 `feature_flags::disabled_tools` 为 context-aware HashMap

**否决理由**：仍是 L3 单层，不能保证 L1/L2 同步过滤（违反 #84 三层强制要求）；且违反 AGENTS.md §2 红线"不写补丁式代码"。

### 6.2 备选 B：复用 `dasclaw_governance::policy_engine` (#65) 同名 trait

**否决理由**：#65 是 lane merge/escalate 决策（`PolicyCondition::GreenAt / StaleBranch / LaneCompleted`），维度与工具访问完全无关。强行复用会让 trait 承担两个不相关的领域，最终需要再拆。直接新 trait `ToolVisibilityPolicy` 维度清晰。

### 6.3 备选 C：直接 fork 整个 codex `tool_registry_plan` crate

**否决理由**：codex 该 crate 包含 1900 LOC 与 dasclaw 需求无关的测试 + `tool-suggest` 元工具 + UI client-type filter，整体引入是过度耦合，维护成本高。选择性 vendor + wrapper 模式与现有 `crates/vendor/` 一致。

### 6.4 备选 D：用 Layer A `HookEngine` 注册 BeforeToolCall hook 实现 L3

**否决理由**：违反 ADR-113 §2.4 教训——Layer A 是事件总线 N:1 弱类型，无法承载 fail-closed 强类型决策（无 enum 返回，无 Block 语义）。Hook 实现 panic 时 HookEngine 的兜底是 LogAndContinue，不是 Block。安全场景必须用 Layer B trait seam。

### 6.5 备选 E：直接移植 ironclaw-main 的 Capability + Lease + GatePipeline 模型

**否决理由**：
- ironclaw-main 的 `Lease` / `Capability` / `ThreadType` 是其完整运行时模型的一部分（`LeasePlanner::plan_for_thread` 依赖 thread 类型 Foreground / Research / Mission），dasclaw 当前没有等价 thread 模型，**整体移植需要引入 ~6 个新概念**。
- ironclaw-main 仍是**单层 gate**（gate 在 dispatcher 前评估），不满足 #84 三层强制契约。
- `GatePipeline` 多 gate priority 串行设计虽然优秀，但 dasclaw P0 阶段单 `ToolVisibilityPolicy` 注入即可满足 #84，引入 pipeline 是过度工程。
- **选择性借鉴**已在 §1.3.2 列明：closed enum 决策、`AUTONOMOUS_TOOL_DENYLIST`（移交 #484）、`RequireApproval` / `Pause` 三态语义。

---

## 7. Rollout

1. **Stage 0**（本 ADR 评审）：✅ 用户对话评审通过（2026-05-13）→ Status 转 Accepted
2. **Stage 1**（GH issue 关联）：在 #84 顶 comment 引用本 ADR + 创建主跟踪 issue（类似 ADR-148 → #483 流程）
3. **Stage 2**（PR 开发）：单 PR 默认，触发 §4.5 条件时拆 2 PR
4. **Stage 3**（合并后）：
   - 默认部署仍走 `BlocklistPolicy`（BuiltIn 维度行为零变化）
   - 文档新增"如何实现自定义 ToolVisibilityPolicy" guide
   - 在 [`AGENTS.md`](../../AGENTS.md) §6 红线追加 "新增工具必须经过 ToolVisibilityPolicy 三层"
5. **Stage 4**（W3 后续）：#484 落地 skills / MCP / extension 完整性验证 + `ToolSource` 实化
6. **Stage 5**（W4 后续）：实现租户/RBAC/环境维度的具体 policy，由独立 issue 跟踪（**不在本 ADR 范围**）

---

## 8. References

- [#84 — Tool visibility triple gate](https://github.com/Linnanli/xClaw/issues/84)
- [#484 — Skills/MCP/Extension visibility + integrity verification (extends #84)](https://github.com/Linnanli/xClaw/issues/484)
- ADR-112 §484 — codex tool 注册统一参考
- ADR-113 — Hook 系统收口 5→1，双层抽象保留
- ADR-148 — EgressGate Pattern（语义清洗 + IPC Facade 拆分）
- claude-code-parity-architecture.md §680-687 — disabled_tools HashSet 模型源头
- codex-cli-main `codex-rs/tools/src/tool_registry_plan.rs` — vendor 源（Apache-2.0）
- codex-cli-main `codex-rs/network-proxy/src/policy.rs` — host allowlist 维度（与 ADR-148 EgressGate 关联）
- claw-code `rust/crates/runtime/src/permission_enforcer.rs` + `permissions.rs` — PermissionMode 模式机参考（不采纳，仅对比）
- ironclaw-main `crates/ironclaw_engine/src/gate/{mod,pipeline,lease,tool_tier}.rs` — closed enum + GatePipeline 设计参考
- ironclaw-main `crates/ironclaw_engine/src/capability/planner.rs` — LeasePlanner ThreadType-aware 设计参考
- [AGENTS.md](../../AGENTS.md) §2 红线 + §6 复用优先于新建
- [.github/pull_request_template.md](../../.github/pull_request_template.md) — PR body 模板

---

**文档版本**：v2.0 Accepted（2026-05-13）  
**版本历史**：
- v1.0 Draft（2026-05-13）— 初稿：trait + 三层接入 + vendor 取舍
- v2.0 Accepted（2026-05-13）— 新增 §1.3.1 三仓库全景对比表 / §1.3.2 设计点吸收表 / §1.3.3 dasclaw 三层独特理由 / §2.7 Source-Aware Default + Always-Has-Policy 不变式 / §2.8 与 #484 衔接边界 / §6.5 备选 E（ironclaw-main 整体移植否决）/ §7 Rollout 增加 Stage 4 衔接 #484。Approved by 用户对话评审。
