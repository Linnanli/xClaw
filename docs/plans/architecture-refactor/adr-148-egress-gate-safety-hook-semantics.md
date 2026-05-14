# ADR-148: EgressGate Pattern — Safety Hook 语义清洗 + IPC Facade 拆分

- **Status**: Accepted（2026-05-13，单 PR 方案 v1.2，对话评审通过）
- **Date**: 2026-05-13
- **Approver**: 用户对话评审通过（"接受 Pattern C 修正版" + "保证代码质量的情况下不要拆的太散"）
- **Supersedes**: 部分覆盖 ADR-113 §2.3（Safety trait seam 命名与方法粒度），不涉及 §2.1 双层抽象决策
- **Related**:
  - ADR-001（trait seam crate 独立性）
  - ADR-112 §5.4.1（模块 4 Hook 测试族）
  - ADR-113（Hook 系统收口 5→1，双层抽象保留）
  - ADR-117（LayeredPromptBuilder ownership）
  - GitHub Issue #61（SafetyBridge → hooks seam 接入；本 ADR 落地后 close as superseded）
  - GitHub Issue #92（attachment DLP gate；本 ADR 落地后由 LlmRequest egress 自然覆盖）

---

## 1. Context — 现状盘点

### 1.1 ADR-113 已完成的收口

ADR-113 P0-3（PR #46/#48/#51 均已合并）确立了双层抽象：

- **Layer A** = `dasclaw_hooks::HookEngine` 事件总线（N:1 链式，audit / declarative regex / outbound webhook）
- **Layer B** = `x_claw_agent::HookBundle` trait seams（1:1 强类型决策注入）：
  - `SafetyHook`、`SandboxExecutor`、`SecretProvider`、`ApprovalGate`

agent loop 4 处 safety 触发点已通过 `IronclawSafetyHook` adapter 走 Layer B。Layer A 中的安全规则触发已被 ADR-113 §2.4 删除（消除双调用）。

### 1.2 ADR-113 未覆盖的两类残余问题

| 问题 | 实证 |
|---|---|
| **R1：Safety trait method 命名违和** | `SafetyHook::before_prompt` 与业界惯例（LangChain `on_llm_start`、Claude Code `UserPromptSubmit`、Express middleware）冲突——`before_prompt` 在通用语义中是"装配/注入 prompt"，本 trait 却用于"扫描决策" |
| **R2：tool_output 散点未走 Layer B** | grep 实证 9 处直调 `SafetyLayer::sanitize_tool_output`：`agent/dispatcher.rs:567,601,650`、`worker/job.rs:720,843,1707,1720`、`routines/routine_engine.rs:1709,1714` |
| **R3：IPC 层 `SafetyBridge` 双职责** | 单一类型 `BridgeScanResult` 同时承担 UX 友好统计（`stats / sanitized_content`）+ fail-closed 安全决策（`was_blocked / block_reason`），耦合两个无关关切 |
| **R4：data_reporter 持久化路径未扫** | 用户消息 / 对话 / 附件 `extracted_text` 在 `ConversationAttachment` 入库前**无 safety 扫描**（issue #92 同源 GAP） |

### 1.3 复合问题：扫描"维度"错位

旧设计按 **LLM 调用生命周期 4 段** 切分：`before_prompt / after_completion / before_tool_call / after_tool_output`。

实际安全本质是 **数据离开信任边界**的事件，与生命周期不正交：
- 同一份数据在内部传递 N 次，旧设计可能扫 N 次（用户消息：IPC + before_prompt 重复）
- 数据走向 UI 显示 / 持久化时，旧设计无对应 hook（覆盖漏洞）
- `after_tool_output` 是"已发生"事件，无法用 `Allow/Block` 决策语义，故 trait 强制为 `-> Result<()>` 不返回 Decision（命名与签名不一致）

---

## 2. Decision

### 2.1 保留 ADR-113 §2.1 双层抽象，不引入新模式

明确**不采纳** "fat hook"（单 hook 复合 N 职责）方案，理由：
- ADR-113 §1.2 已分析：强类型决策无法降级为通用 string 语义
- fail-closed 保证依赖结构化返回类型，复合 hook 会让 fail-safe 兜底逻辑漂移到各实现
- Pattern C（双层 per phase）已是当前架构，业界对齐（K8s admission webhook / Envoy filter / LangChain）

### 2.2 Layer B 中 safety 部分按 "egress 维度" 重命名

引入新 trait seam `EgressGate` 替代 `SafetyHook`：

```rust
// crates/dasclaw_governance/src/egress.rs (新文件 / 或 crates/x_claw_agent/src/hooks.rs 重构)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EgressKind {
    /// 数据将通过 HTTP 出网到 LLM API
    LlmRequest,
    /// 数据将传递给 tool 执行器（bash / fs / mcp）
    ToolExecution,
    /// 数据将显示给用户 UI
    UserDisplay,
    /// 数据将持久化到 admin-backend / 本地存储
    Persistence,
}

#[derive(Debug, Clone)]
pub enum EgressDecision {
    Allow,
    Redact { sanitized: String, stats: RedactionStats },
    Block  { reason: String,    stats: RedactionStats },
}

#[async_trait]
pub trait EgressGate: Send + Sync {
    async fn check(
        &self,
        kind: EgressKind,
        payload: &str,
    ) -> EgressDecision;
}
```

**约束**：
- 单方法 + enum kind 取代 4 个 lifecycle method
- 返回 enum 取代 `Result<SafetyDecision>` 与 `Result<()>` 混合
- `EgressGate` 内部错误**默认转换为 `Block`**（fail-closed），不再有"Internal error"逃逸
- trait 居住 `crates/dasclaw_governance`（新增子模块 `egress`）或 `crates/x_claw_agent`（保持 trait seam 集中），由 PR-1 决定（见 §4）

### 2.3 Layer A 不动，作为未来扩展点

任何新的"非决策"功能（prompt 装配 / persona 注入 / memory / telemetry / A/B test 等），**必须**通过 Layer A `HookEngine` 注册 `Hook` 实现到对应 lifecycle point（`BeforeInbound` / `BeforeOutbound` / `BeforeToolCall` 等），**禁止**扩展 Layer B trait seam。

启动期契约（增强 `dasclaw_hooks::contract::no_safety_rule_in_event_hooks`）：
- 既保留 ADR-113 §5 "Layer A 不承担 safety" 校验
- 增加对偶校验 `no_mutation_in_egress_gate`：`EgressGate` 实现禁止对 payload 做无关于安全的 mutation（如 prompt 装配）

### 2.4 删除 IPC 层 `SafetyBridge` facade

`desktop-client/src/safety_bridge.rs` 拆为两个职责独立的组件：

| 旧职责 | 新去处 |
|---|---|
| UX 预览（`scan_user_input` 返回 stats / had_sensitive_data 供前端弹窗） | `InputAdvisor` trait（IPC 层私有，仅 advisory，**不**作 fail-closed） |
| 安全决策（`scan_user_input` 的 `was_blocked` 字段） | 直接调 `EgressGate.check(LlmRequest, ...)` |
| 持久化扫描（`sanitize_for_storage`） | `EgressGate.check(Persistence, ...)` |
| Outbound HTTP 扫描（`scan_outbound`） | `EgressGate.check(LlmRequest, ...)`（合并语义） |
| 配置/统计 getter | 独立模块 `dlp_stats` / 配置层 |

```rust
// desktop-client/src/dlp/advisor.rs（新）— 仅 UX，无决策权
#[async_trait]
pub trait InputAdvisor: Send + Sync {
    async fn preview(&self, payload: &str) -> AdvisoryReport;
}

pub struct AdvisoryReport {
    pub has_potential_secrets: bool,
    pub pii_count: usize,
    pub hint_message: Option<String>,
}
```

前端调 advisor 只为给用户实时提示（"您似乎输入了密钥，建议移除"），用户可忽略继续发送；真正阻断在 `EgressGate.check(LlmRequest)` 处发生（fail-closed）。

### 2.5 删除 9 处 `sanitize_tool_output` 散点

tool output 在 ironclaw 内部保持 raw 状态传递，仅在以下 4 个 egress 出口前过 `EgressGate.check`：

- 下一轮 LLM 调用前 → `LlmRequest`
- 流式推送给 UI 前 → `UserDisplay`
- 入 `ConversationTracker` / `DataReporter` 前 → `Persistence`
- （tool args 已在 `ToolExecution` egress 扫，与 tool output 无关）

由此 issue #92 attachment GAP 自然解决：attachment.extracted_text 在装配进 prompt 时统一被 `LlmRequest` egress 扫描。

### 2.6 SafetyLayer 内部算法不动

`ironclaw_safety::SafetyLayer` 及其子模块（`leak_detector` / `sanitizer` / `validator` / `policy` / `credential_detect`）保持 0 改动。仅 `agent_hook.rs` 适配器替换为 `egress_gate.rs`。

---

## 3. 目标架构

### 3.1 静态组件持有图

```
                         ┌────────────────────────────────────────────────┐
                         │  desktop-client/  (Tauri host)                  │
                         │                                                 │
                         │   IPC commands  ─── chat.rs / threads.rs        │
                         │                       │                         │
                         │                       ▼                         │
                         │   InputAdvisor.preview()  (advisory only)       │
                         │                       │                         │
                         │                       ▼                         │
                         │   msg_sender ──► agent loop                     │
                         │                       │                         │
                         │   ipc/dlp.rs  ──► EgressGate.check(Persistence) │
                         │                                                 │
                         │   data_reporter ──► EgressGate.check(           │
                         │                       Persistence)              │
                         └───────────────────────┬─────────────────────────┘
                                                 │
                         ┌───────────────────────▼─────────────────────────┐
                         │  desktop-client/ironclaw/  (agent runtime)      │
                         │                                                 │
                         │   agent loop / dispatcher / worker / routine    │
                         │                                                 │
                         │   ╞═══════════════════════════════════════╡    │
                         │   ║ Phase: BeforeLlmRequest                ║    │
                         │   ║   Layer A: HookEngine.run(BeforeInbound)║    │
                         │   ║   Layer B: egress.check(LlmRequest)     ║    │
                         │   ╞═══════════════════════════════════════╡    │
                         │                                                 │
                         │   ╞═══════════════════════════════════════╡    │
                         │   ║ Phase: AfterCompletion                 ║    │
                         │   ║   Layer A: HookEngine.run(TransformResponse) ║│
                         │   ║   Layer B: egress.check(UserDisplay)    ║    │
                         │   ╞═══════════════════════════════════════╡    │
                         │                                                 │
                         │   ╞═══════════════════════════════════════╡    │
                         │   ║ Phase: BeforeToolCall                  ║    │
                         │   ║   Layer A: HookEngine.run(BeforeToolCall)║   │
                         │   ║   Layer B: egress.check(ToolExecution)  ║    │
                         │   ║          + approval.request(...)        ║    │
                         │   ╞═══════════════════════════════════════╡    │
                         │                                                 │
                         │   sandbox.exec(...)  ──► tool output (raw)       │
                         │   (output 流回 next turn LlmRequest egress)      │
                         └───────────────────────┬─────────────────────────┘
                                                 │
                         ┌───────────────────────▼─────────────────────────┐
                         │  ironclaw_safety/  (扫描引擎)                    │
                         │                                                 │
                         │   IronclawEgressGate  (新，~120 行)              │
                         │     impl EgressGate {                           │
                         │       check(LlmRequest|Persistence) {           │
                         │         secret_scan + pii_redact                │
                         │       }                                         │
                         │       check(UserDisplay) {                      │
                         │         secret_scan only (无 PII redact)         │
                         │       }                                         │
                         │       check(ToolExecution) {                    │
                         │         validator.validate_tool_params          │
                         │       }                                         │
                         │     }                                           │
                         │                                                 │
                         │   SafetyLayer (不动) ── 5 子模块               │
                         │     • sanitizer / validator / policy            │
                         │     • leak_detector / credential_detect         │
                         └───────────────────────┬─────────────────────────┘
                                                 │
                         ┌───────────────────────▼─────────────────────────┐
                         │  crates/dasclaw_governance/  (新增 egress 模块)  │
                         │                                                 │
                         │   pub mod egress;                               │
                         │     pub trait EgressGate                        │
                         │     pub enum EgressKind / EgressDecision        │
                         │     pub struct NoopEgressGate                   │
                         │                                                 │
                         │   reexport from dasclaw_hooks::                 │
                         │   (HookBundle 同时含 egress + sandbox/secrets/  │
                         │    approval；HookEngine 与之独立)               │
                         └─────────────────────────────────────────────────┘
```

### 3.2 HookBundle 改动

```diff
  pub struct HookBundle {
-     pub safety:   Arc<dyn SafetyHook>,
+     pub egress:   Arc<dyn EgressGate>,
      pub sandbox:  Arc<dyn SandboxExecutor>,
      pub secrets:  Arc<dyn SecretProvider>,
      pub approval: Arc<dyn ApprovalGate>,
  }
```

agent loop 调用站对应一处替换（见 §4 PR-2）。

---

## 4. 实施切片（单 PR 原子提交）

### 4.1 PR 设计

**PR**: `adr146-egress-gate-safety-semantics-cleanup`  
**Base**: `xClaw`  
**预估 LOC**: 净 ~2000 行 diff（增 ~1180 / 删 ~900；含 ~500 行测试）  
**预估文件**: 修改 ~15 + 新增 3 + 删除 2  
**理由**：变更虽量大但单点（SafetyLayer 不动 + 机械重命名 + 调用站搬迁），单 PR 比 stacked PR **更符合仓库"禁止补丁式代码"原则**——无需 `#[deprecated]` 过渡层，直接物理删除旧代码，原子切换。

### 4.2 改动清单（按文件粒度，可并行 review）

**新增（3 个文件）**

| 文件 | LOC | 内容 |
|---|---|---|
| `crates/dasclaw_governance/src/egress.rs` | ~150 | `EgressGate` trait + `EgressKind` / `EgressDecision` / `RedactionStats` / `NoopEgressGate` |
| `desktop-client/ironclaw/crates/ironclaw_safety/src/egress_gate.rs` | ~150 | `IronclawEgressGate` 实现，包装 `SafetyLayer` |
| `desktop-client/src/dlp/advisor.rs` | ~120 | `InputAdvisor` trait + 默认 `DesktopInputAdvisor` 实现 |

**修改（~15 个文件）**

| 文件 | 改动 |
|---|---|
| `crates/dasclaw_hooks/src/lib.rs` | reexport `EgressGate` / `EgressKind` / `EgressDecision`；移除 `SafetyHook` reexport |
| `crates/dasclaw_hooks/src/contract.rs` | 新增 `no_mutation_in_egress_gate(&dyn EgressGate) -> bool` |
| `crates/x_claw_agent/src/hooks.rs` | 删除 `SafetyHook` trait + `SafetyDecision` + `NoopSafetyHook`；`HookBundle.safety` → `egress: Arc<dyn EgressGate>` |
| `crates/x_claw_agent/src/agentic_loop.rs` | 3 处调用站从 `bundle.safety.*` 迁移到 `bundle.egress.check(EgressKind::*, ...)`；测试 mock 重写 |
| `crates/x_claw_agent/src/lib.rs` | export 名单更新 |
| `desktop-client/ironclaw/crates/ironclaw_safety/src/lib.rs` | `pub mod agent_hook` → `pub mod egress_gate`；feature `agent-hook` → `egress-gate` |
| `desktop-client/ironclaw/src/agent/agentic_loop.rs` | `IronclawSafetyHook::new` → `IronclawEgressGate::new` |
| `desktop-client/ironclaw/src/agent/dispatcher.rs` | 删除 3 处 `sanitize_tool_output` 直调（line 567 / 601 / 650） |
| `desktop-client/ironclaw/src/worker/job.rs` | 删除 4 处 `sanitize_tool_output` 直调（line 720 / 843 / 1707 / 1720） |
| `desktop-client/ironclaw/src/routines/routine_engine.rs` | 删除 2 处 `sanitize_tool_output` 直调（line 1709 / 1714） |
| `desktop-client/ironclaw/src/{hook_bootstrap.rs,tools/execute.rs}` | doc 注释更新（不再提 `SafetyHook`） |
| `desktop-client/src/ipc/chat.rs` | 用 `InputAdvisor.preview` 替代 `safety_bridge.scan_user_input` 的 UX 部分；加 `EgressGate.check(LlmRequest)` 拦截（含 attachment.extracted_text，**修复 #92**） |
| `desktop-client/src/ipc/dlp.rs` | 6 Tauri 命令重写为直调 `EgressGate.check(Persistence/LlmRequest)` |
| `desktop-client/src/data_reporter.rs` | 持久化前增加 `EgressGate.check(Persistence)` 钩子 |
| `desktop-client/src/{state.rs,engine.rs,ipc/extensions.rs}` | `safety_bridge: Arc<SafetyBridge>` → `egress: Arc<dyn EgressGate>` + `advisor: Arc<dyn InputAdvisor>` 双字段 |

**删除（2 个文件）**

| 文件 | LOC | 理由 |
|---|---|---|
| `desktop-client/ironclaw/crates/ironclaw_safety/src/agent_hook.rs` | -220 | 被 `egress_gate.rs` 取代 |
| `desktop-client/src/safety_bridge.rs` + `safety_bridge_tests.rs` | -480 | 拆为 `InputAdvisor` + 直调 `EgressGate` |

**测试族（28 项，合并到 ~4 个文件）**

| 文件 | 测试项 |
|---|---|
| `crates/dasclaw_governance/tests/egress_contract.rs` | trait 单测 + 4 个 `EgressKind` dispatch 等价性（5 项） |
| `desktop-client/ironclaw/crates/ironclaw_safety/tests/egress_gate_integration.rs` | `IronclawEgressGate` 与旧 `IronclawSafetyHook` 行为等价回归（6 项：block_secret / redact_completion / validate_tool_args / sanitize_tool_output / fail_closed_on_internal_err / no_mutation_check） |
| `desktop-client/ironclaw/tests/egress_in_agent_loop.rs` | agent loop 端到端 + 双调用消除 + 9 散点路径覆盖（11 项：3 dispatcher + 4 job + 2 routine + block_terminates + redact_in_place） |
| `desktop-client/tests/ipc_egress_e2e.rs` | IPC + advisor 不阻塞 + Persistence 拦截 + attachment 扫描（8 项：chat_attachment / advisor_not_blocking / dlp_ipc × 4 / persistence_intercept / safetybridge_removed） |

### 4.3 单 PR 优势 vs 风险

| 维度 | 优势 |
|---|---|
| 原子性 | 一次切换，无中间 `#[deprecated]` 兼容层 |
| 仓库规约 | 符合"禁止补丁式代码"：无 feature flag、无 dual-impl |
| Review | 文件粒度独立可读；reviewer 看完整画面 |
| 回滚 | 单 commit revert |
| CI 总时间 | 1 次完整运行 vs 4 次累积 |

| 维度 | 风险 + 缓解 |
|---|---|
| Review 体量大 | PR body 提供"3 层阅读梯度"（30min / 2h / 完整）+ 文件级独立性引导 |
| Bisect 粒度粗 | commit 内部用 conventional commit `feat:` / `refactor:` / `test:` 分段（即使最终 squash 也保留历史于 PR body） |
| 隐藏 bug | 28 测试项覆盖每个删除点 + 每个 egress kind + 每个 IPC 入口 |

### 4.4 PR body 必填模板

```markdown
## 背景/目标
ADR-148 §1 - §2

## 改动范围
本 PR 完整落地 ADR-148 §4.2 清单（新增 3 + 修改 15 + 删除 2）。

## 非目标（What's NOT in this PR）
- SafetyLayer 5 子模块算法（不动）
- `SandboxExecutor` / `SecretProvider` / `ApprovalGate` 三 trait seam（不动）
- `HookEngine` 事件总线（不动，ADR-113 §2.1 双层抽象保留）
- claw-code / admin-backend 各自的 safety 实现（不在范围）

## 验证
- cargo nextest run -p dasclaw_governance: PASS
- cargo nextest run -p ironclaw_safety: PASS
- cargo nextest run -p ironclaw --lib hooks: PASS
- cargo nextest run -p desktop-client: PASS
- cargo build --bins --tests: 0 错误 0 警告
- python3 scripts/check_no_panics.py --base origin/xClaw: PASS
- skill audit: code-quality-audit ✓ / code-simplifier ✓ / code-review-expert ✓

## Sources read
- AGENTS.md §...
- docs/plans/architecture-refactor/adr-148-...md（全文）
- docs/plans/architecture-refactor/adr-113-...md §1.2 + §2.1
- docs/plans/architecture-refactor/adr-001-...md
- crates/x_claw_agent/src/hooks.rs（删除前快照）
- desktop-client/ironclaw/crates/ironclaw_safety/src/{lib.rs, agent_hook.rs}
- desktop-client/src/safety_bridge.rs（删除前快照）

## 复杂度分级阅读建议
- 30 分钟：crates/dasclaw_governance/src/egress.rs + HookBundle diff + tests 目录
- 2 小时：上 + IronclawEgressGate impl + agent loop 调用站
- 完整：上 + 9 散点路径覆盖测试 + IPC 端到端

Closes #61
Closes #92
```

### 4.5 拆分应急条款（fallback）

单 PR 是**首选**方案。若开发期间遇到以下任一情形，允许 1 次拆分为 2 PR：

| 触发条件 | 拆分边界 |
|---|---|
| 单 PR 编译失败链超过 2 小时无法收敛 | **PR-A**（core）：新增 trait + IronclawEgressGate + HookBundle 字段改名 + agent loop 3 调用站 + 删 SafetyHook & agent_hook.rs；**PR-B**（IPC + 散点）：删 9 散点 + 删 SafetyBridge + 新 InputAdvisor + ipc 重写 |
| CI 总耗时超过 60 分钟单 PR 难以迭代 | 同上边界 |
| Reviewer 明确要求拆分 | 按 reviewer 指示边界 |

**禁止**事项（即使拆分也守住）：
- 不使用 feature flag / `if cfg!()` 切换新旧路径
- 不保留 `#[deprecated]` 过渡层超过 1 个 PR 周期
- 不保留 trait alias
- 重命名用 `git mv`、删除用 `git rm`，保留 git history D 状态

### 4.6 禁止补丁式代码（无论是否拆分）

- 不使用 feature flag 或 `if cfg!()` 切换新旧路径
- 单 PR 路径：不保留 `#[deprecated]` 过渡层
- 拆分路径：`#[deprecated]` 仅允许 1 个 PR 周期，PR-B 必须物理删除
- 不保留 trait alias（4 method → 1 method 签名不兼容，alias 不可行）
- 重命名通过 `git mv` + `cargo fix --edition` 保留 blame 历史
- 删除文件用 `git rm`

---

## 5. 测试与启动期契约

### 5.1 启动期契约函数

```rust
// crates/dasclaw_hooks/src/contract.rs（增量补充，与 ADR-113 §5 共存）

/// 校验 EgressGate 实现没有承担非安全的 mutation 职责。
/// 反向于 `no_safety_rule_in_event_hooks` —— 后者禁止 Layer A 干安全，
/// 本函数禁止 Layer B 干装配。
pub fn no_mutation_in_egress_gate(gate: &dyn EgressGate) -> bool {
    // 实现：注入测试 payload "PROBE-NEUTRAL-12345" 跑 4 个 kind，
    // 断言 Allow 分支返回的 payload 与输入一致（无 mutation）。
    // PR-1 引入 noop 占位，PR-3 在 bootstrap 末尾强制调用。
}
```

### 5.2 测试族（共 28 项）

| PR | 测试族 | 覆盖维度 |
|---|---|---|
| 单 PR | 28 项测试合并到 4 个文件（见 §4.2） | egress trait + IronclawEgressGate 等价回归 + agent loop 集成 + 9 散点路径覆盖 + IPC 端到端 |

每 PR 同步 H1 + H2，对齐 ADR-112 §5.4.1 模块 4 Hook 测试族。

### 5.3 红线 assertion 转绿

单 PR 合并后：
- `dasclaw_hooks::count_hook_systems() == 1`（保持 ADR-113 §5）
- `no_safety_rule_in_event_hooks(&engine)` 通过（保持 ADR-113 §5）
- `no_mutation_in_egress_gate(&gate)` 通过（**本 ADR 新增**）

---

## 6. 偏离声明与风险

### 6.1 偏离

| # | 偏离 | 理由 |
|---|---|---|
| 1 | 删除 `SafetyHook` trait 而不保留 trait alias | 4 method → 1 method 签名不兼容，alias 不可行；改名透传只在适配器层做（`IronclawSafetyHook` deprecated 一个 PR 周期） |
| 2 | `after_tool_output` 不映射到独立 `EgressKind::ToolOutput`，而是后移到下一轮 `LlmRequest` | tool output 在内部传递期间属于信任边界内，扫不扫无安全意义；只在 LLM / UI / 持久化 egress 出口扫一次即可，避免重复 |
| 3 | `InputAdvisor` 不进 Layer B（trait seam），保持 IPC 层私有 | advisor 是 UX 工具，无 fail-closed 含义；放进 Layer B 会再次混淆决策 vs UX 分界 |
| 4 | `EgressGate` 内部错误强制转 Block | fail-closed 优先；目前 `SafetyError::Internal` 几乎不被触发，转 Block 实质收紧 |

### 6.2 风险

| # | 风险 | 等级 | 缓解 |
|---|---|---|---|
| R1 | 9 处 `sanitize_tool_output` 删除后某路径漏扫 | 🟡 中 | PR-3 H2 测试族 9 项，每处旧调用站对应路径都断言"output 最终经 LlmRequest/UserDisplay/Persistence egress" |
| R2 | `EgressKind::Persistence` 覆盖路径过广，影响性能 | 🟢 低 | 持久化是 batch 异步操作，扫描成本可接受；提供 `benches/egress_persistence.rs` 基准 |
| R3 | `IronclawSafetyHook` 外部 caller（claw-code / admin-backend / extensions）? | 🟢 低 | grep 实证仅 `ironclaw` 内部使用；claw-code / admin-backend 各自有 safety 实现 |
| R4 | 与 ADR-113 §5 `count_hook_systems` 红线冲突 | 🟢 低 | `EgressGate` 是 Layer B trait seam reexport，不计入"hook 系统数"（ADR-113 §2.2 已定义） |
| R5 | issue #92 用户期待"立即修复"，新 ADR 推迟 attachment 修复 | 🟡 中 | PR-1+PR-2 周期内 attachment GAP 仍存在；通过 #92 评论说明新方案 + 时间线，避免双重实现 |

---

## 7. 验收标准（ADR-148 完成定义）

- [ ] 单 PR `adr146-egress-gate-safety-semantics-cleanup` 合并到 `xClaw`
- [ ] `cargo nextest run -p dasclaw_governance` 0 失败
- [ ] `cargo nextest run -p ironclaw_safety` 0 失败（含新 `egress_gate.rs` 测试）
- [ ] `cargo nextest run -p ironclaw --lib hooks` 0 失败
- [ ] `cargo nextest run -p desktop-client` 0 失败
- [ ] `cargo build --bins --tests` 0 错误 0 警告
- [ ] `python3 scripts/check_no_panics.py` 通过
- [ ] 28 项测试族全绿
- [ ] 启动期 3 个契约 assertion 全绿（含本 ADR 新增 `no_mutation_in_egress_gate`）
- [ ] 三 skill 自审（code-quality-audit / code-simplifier / code-review-expert）记录到 PR 描述
- [ ] PR body 含"3 层阅读梯度"引导
- [ ] PR body `Closes #61` + `Closes #92`
- [ ] ADR-113 §2.3 标注 "Safety trait seam 详见 ADR-148"

---

## 8. 设计原则总结

1. **双层抽象保留**：Layer A（HookEngine 事件 N:1）不变，Layer B（trait seams 决策 1:1）safety 部分重命名
2. **egress 为安全维度**：数据离开信任边界 4 个方向（LLM / Tool / UI / Storage）= 4 个扫描点
3. **fail-closed 默认**：EgressGate 内部错误 → Block；advisor 仅给 UX 不可决策
4. **职责单一**：每 hook 1 件事；新功能扩展走 Layer A，新决策走 Layer B
5. **算法不动**：SafetyLayer 5 子模块 0 改动
6. **物理删除**：通过 stacked PR + `#[deprecated]` 过渡，最终物理删除旧代码（无 feature flag、无补丁式分支）

---

*v1.2（2026-05-13）：单 PR 方案 + 拆分应急条款，对话评审通过转 Accepted。*
