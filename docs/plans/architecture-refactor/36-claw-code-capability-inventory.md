# 36 — Claw Code 全量能力清单（路线 B 补充）

> **版本**：v1.0 (2026-04-25) · 配套 [13-security-capability-inventory.md](13-security-capability-inventory.md)（ironclaw 安全清单）与 [35-codex-capability-inventory.md](35-codex-capability-inventory.md)（codex 全量清单）
> **方法**：严格按 [`AGENTS.md`](../../../AGENTS.md) 三级工具链（semantic_search → vscode_listCodeUsages → rg/grep）
> **覆盖率**：≥ 95%（9 crate / 16 大能力域 / CLAUDE.md/PARITY.md/PHILOSOPHY.md 关键语录全覆盖）
> **总规模**：48,599 行 Rust 生产代码 + 2,568 行测试 / 9 crate / 292 commits

---

## 0. 结论先行

| 维度 | 数据 |
|---|---|
| 总规模 | 48,599 LOC（runtime 16,200 / tools 12,500 / plugins 7,800 / api 3,500 / cli 4,200 / 其它 4,400） |
| Crate 数 | 9（runtime / tools / plugins / api / rusty-claude-cli / commands / telemetry / mock-anthropic-service / compat-harness） |
| 提交活跃度 | 2026-03-31 → 2026-04-03 / 9-lane 合并完成度 100% |
| Mock parity | 10 scripted scenarios + 19 captured `/v1/messages` |

**最强 Top 3（claw 独家或最深，必须移植到 dasclaw）**：

1. **治理 6 件套**（branch_lock / green_contract / stale_branch / policy_engine / recovery_recipes / trust_resolver）共 1,478 LOC — codex 与 ironclaw 完全没有此能力体系，是 claw-code 的护城河
2. **MCP 6 transport bridge**（stdio / sse / http / ws / sdk / managed-proxy）— ironclaw 仅 stdio，移植后 dasclaw 直接超过 ironclaw
3. **Bash validation 6 模块**（1,004 LOC，readOnlyValidation / destructiveCommandWarning / modeValidation / sedValidation / pathValidation / commandSemantics）— codex 同类 ~800 LOC，ironclaw 无对标

**最弱 Top 3（不必投入）**：

1. ❌ Desktop GUI / TUI — 仅 CLI REPL，ironclaw 桌面端已完整
2. ❌ Cloud Tasks / Async Jobs — 无队列、无持久化调度，ironclaw jobs 已覆盖
3. ❌ Identity / Device-Key — 仅 PKCE + ED25519，弱于 ironclaw 完整体系

**对路线 B 的核心价值**：可直接移植核心约 28,850 LOC（占 claw-code 总产能 59.4%），是 W4 治理 + W5 MCP 的主要资产来源。

---

## 1. 16 大能力域总览表

| # | 域 | 精确行数 | 核心模块路径 | vs codex | vs ironclaw | 桌面端价值 | 路线 B 决策 |
|---|---|---:|---|:-:|:-:|:-:|---|
| A | Agent Runtime（Conversation Loop） | ~3,200 | `runtime/src/conversation.rs` (286) | ⚠️ | ⚠️ | ★★★★★ | 选择性移植 |
| B | Tool 系统（Builtin + Registry） | ~9,200 | `tools/src/lib.rs` (8,400) + `task_registry.rs` (336) + `team_cron_registry.rs` (441) | ⚠️ | ⚠️ | ★★★★★ | 完整移植 → `dasclaw_tools_v2` |
| C | Sandbox（Linux landlock + bwrap） | ~1,200 | `runtime/src/sandbox.rs` (385) | ✅ codex 更强 | ✅ ironclaw 更强 | ★★★★ | 优先用 codex 三平台方案 |
| D | Hooks（Schema/Registry/Engine 三分离） | ~1,050 | `plugins/src/hooks.rs` (957) | ✅ codex 设计更干净 | ✅ ironclaw 一体化 | ★★★★ | 用 codex 设计 |
| E | Project Docs（CLAUDE.md 单层） | ~250 | `runtime/src/prompt.rs` + `bootstrap.rs` | ⚠️ codex 多层更强 | ✅ ironclaw 多层 | ★★★★ | **用 codex AGENTS.md 多层方案** |
| F | **Bash Validation 6 模块** | **1,004** | `runtime/src/bash_validation.rs` | ⚠️ codex ~800 | ❌ 无对标 | ★★★★★ | **完整移植** → `dasclaw_bash_validation` |
| G | **治理 6 件套**（独家） | **1,478** | `runtime/src/{branch_lock,green_contract,stale_branch,policy_engine,recovery_recipes,trust_resolver}.rs` | ❌ codex 无 | ⚠️ 有限交集 | ★★★★★ | **完整移植** → `dasclaw_governance` |
| H | **MCP 6 Transport** | ~2,400 | `runtime/src/mcp*.rs` | ✅ codex 等价 | ⚠️ ironclaw 仅 stdio | ★★★★★ | **完整移植** → `dasclaw_mcp_v2` |
| I | ExecPolicy / 权限模型 | ~800 | `runtime/src/permission_enforcer.rs` | ✅ codex Starlark 更灵活 | ✅ ironclaw JSON | ★★★★ | 用 codex Starlark |
| J | Features Flags / 配置系统 | ~1,100 | `runtime/src/config.rs` + `config_validate.rs` | ⚠️ 各自独立 | ⚠️ ironclaw .env | ★★★★ | 保留 + 借鉴 codex |
| K | Observability（Trace / Metrics） | ~650 | `telemetry/src/lib.rs` + `runtime/src/usage.rs` | ⚠️ codex 更丰富 | ⚠️ ironclaw 基础 | ★★★ | session tracer 参考 |
| L | Identity / OAuth | ~400 | `runtime/src/oauth.rs` | ❌ 弱 | ❌ 弱 | ★★ | 用 codex device-key |
| M | Crash & Panic | 0 | （无实现） | ❌ codex 弱 | ❌ ironclaw 弱 | ★ | dasclaw 新建 |
| N | Network Proxy | ~450 | `api/src/http_client.rs` | ✅ codex 更强 | ✅ ironclaw 完整 | ★★★★ | 用 codex |
| O | **Prompt Caching（tool-use/result 边界保护）** | ~110 | `runtime/src/compact.rs:110` + `runtime/src/prompt.rs:480` + `prompt.rs:197` | ⚠️ codex 类似 | ⚠️ ironclaw 类似 | ★★★★★ | **完整移植** |
| P | **Session Compaction**（摘要 + 尾消息） | ~280 | `runtime/src/compact.rs` | ⚠️ 各有 | ⚠️ ironclaw apply-patch 互补 | ★★★★★ | **完整移植** |

**可移植核心合计**：~28,850 LOC（59.4% of claw-code）

---

## 2. 治理 6 件套深度分析（独家 ★★★★★）

> claw-code 相对 codex / ironclaw 最大的差异化优势。为多 agent 自治 Git Worktree 编排设计。

### 2.1 文件矩阵

| 件套 | 文件 | LOC | 单测数 | 核心职责 |
|---|---|---:|---:|---|
| 1. branch_lock | `runtime/src/branch_lock.rs` | 163 | 3+ | 检测 lane/branch/module 嵌套碰撞 |
| 2. green_contract | `runtime/src/green_contract.rs` | 189 | 12+ | CI 状态级别（TargetedTests/Package/Workspace/MergeReady） |
| 3. stale_branch | `runtime/src/stale_branch.rs` | 268 | 6+ | 陈旧分支检测 + auto-rebase / merge-forward |
| 4. policy_engine | `runtime/src/policy_engine.rs` | 398 | 24+ | PolicyCondition/PolicyAction 优先级规则评估 |
| 5. recovery_recipes | `runtime/src/recovery_recipes.rs` | 304 | 7+ | 7 个 FailureScenario 的恢复 recipe + escalation |
| 6. trust_resolver | `runtime/src/trust_resolver.rs` | 156 | 5+ | cwd 信任评估（allowlist/denied/RequireApproval） |
| **合计** | — | **1,478** | **57+** | — |

### 2.2 关键 enum 摘要

```rust
// branch_lock.rs:5
pub struct BranchLockIntent { lane_id, branch, worktree, modules: Vec<String> }
pub fn detect_branch_lock_collisions(intents: &[BranchLockIntent]) -> Vec<BranchLockCollision>;
// 检测嵌套模块冲突（"runtime" vs "runtime/mcp" 视为冲突）

// green_contract.rs:3
pub enum GreenLevel { TargetedTests, Package, Workspace, MergeReady }
// 相对比较：observed >= required 才算满足

// stale_branch.rs:6
pub enum BranchFreshness { Fresh, Stale { commits_behind, missing_fixes }, Diverged { ahead, behind, missing_fixes } }
pub enum StaleBranchPolicy { AutoRebase, AutoMergeForward, WarnOnly, Block }

// policy_engine.rs:1
pub enum PolicyCondition { And(Vec<_>), Or(Vec<_>), GreenAt { level }, StaleBranch, StartupBlocked, ... }
pub enum PolicyAction { MergeToDev, MergeForward, RecoverOnce, Escalate { reason }, CloseoutLane, ... }

// recovery_recipes.rs:14
pub enum FailureScenario {
    TrustPromptUnresolved, PromptMisdelivery, StaleBranch, CompileRedCrossCrate,
    McpHandshakeFailure, PartialPluginStartup, ProviderFailure,
}

// trust_resolver.rs:10
pub enum TrustPolicy { AutoTrust, RequireApproval, Deny }
pub enum TrustDecision { NotRequired, Required { policy, events } }
```

### 2.3 与 codex / ironclaw 的对照

| 件套 | codex | ironclaw | claw 独家点 |
|---|---|---|---|
| branch_lock | ❌ | ❌ | 嵌套模块碰撞算法 |
| green_contract | ❌ | ❌ | CI 级别有序枚举 |
| stale_branch | ⚠️ git-utils 部分 | ❌ | 自动 rebase/merge-forward 策略 |
| policy_engine | ✅ Starlark DSL | ❌ | claw 用 Rust enum，类型安全；codex 灵活但需 Starlark VM |
| recovery_recipes | ⚠️ degraded mode | ❌ | 7 场景 + max_attempts 编码 |
| trust_resolver | ⚠️ TUI 提示 | ⚠️ GUI dialog | claw 在 runtime 层，codex/ironclaw 在 UI 层 |

**结论**：除 policy_engine 与 codex Starlark 重叠外，其余 5 件套是 dasclaw 的独家优势。

---

## 3. MCP 6 Transport 深度分析（★★★★★）

**位置**：[runtime/src/mcp_client.rs:8-30](../../claw-code/rust/crates/runtime/src/mcp_client.rs)

```rust
pub enum McpClientTransport {
    Stdio(McpStdioTransport),                    // 本地进程 spawn + JSON-RPC
    Sse(McpRemoteTransport),                     // HTTP Server-Sent Events
    Http(McpRemoteTransport),                    // RESTful 请求-响应
    WebSocket(McpRemoteTransport),               // 双向 WebSocket
    Sdk(McpSdkTransport),                        // 内嵌 Rust SDK（如 postgres-mcp）
    ManagedProxy(McpManagedProxyTransport),      // 云代理（CCR /v2/session_ingress/）
}

pub enum McpClientAuth { None, OAuth(McpOAuthConfig) }  // OAuth 兼容
```

| Transport | claw-code | codex | ironclaw |
|---|:-:|:-:|:-:|
| Stdio | ✅ | ✅ | ✅ |
| SSE | ✅ | ✅ | ❌ |
| HTTP | ✅ | ✅ | ❌ |
| WebSocket | ✅ | ✅ | ❌ |
| SDK | ✅ | ✅ | ❌ |
| ManagedProxy | ✅ | ⚠️ concept | ❌ |

**移植后**：dasclaw 直接超越 ironclaw 当前 stdio-only 状态，与 codex 持平。

---

## 4. Bash Validation 6 模块（★★★★★）

**位置**：[runtime/src/bash_validation.rs](../../claw-code/rust/crates/runtime/src/bash_validation.rs)（1,004 LOC）

| 模块 | LOC | 职责 |
|---|---:|---|
| readOnlyValidation | ~180 | 在 read-only 模式下阻止 cp/rm/touch/chmod 等写操作 |
| destructiveCommandWarning | ~150 | 标记 rm -rf/shred/truncate 等不可逆操作 |
| modeValidation | ~180 | 根据 PermissionMode 强制约束命令 |
| sedValidation | ~140 | Sed 表达式验证（防 regex 溢出） |
| pathValidation | ~200 | 检测 `../` / symlink escape 等可疑路径 |
| commandSemantics | ~154 | CommandIntent 分类（ReadOnly/Write/Destructive/Network/Process/Package/SystemAdmin/Unknown） |

```rust
pub enum CommandIntent { ReadOnly, Write, Destructive, Network, ProcessManagement, PackageManagement, SystemAdmin, Unknown }
pub enum ValidationResult { Allow, Block { reason }, Warn { message } }

pub fn validate_read_only(cmd: &str, mode: PermissionMode) -> ValidationResult;
pub fn validate_destructive(cmd: &str) -> ValidationResult;
pub fn validate_mode(cmd: &str, mode: PermissionMode) -> ValidationResult;
pub fn validate_sed(expr: &str) -> ValidationResult;
pub fn validate_path(path: &str) -> ValidationResult;
pub fn classify_intent(cmd: &str) -> CommandIntent;
```

**vs codex** `core/src/tools/runtimes/shell/bash_validator.rs` ~800 LOC：claw 多 204 LOC，模块化更清晰。

---

## 5. Prompt Caching / Tool-use 边界保护（★★★★★）

**位置**：[runtime/src/compact.rs:110](../../claw-code/rust/crates/runtime/src/compact.rs)（核心算法约 110 LOC）

**问题**：LLM tool-use / tool-result 必须成对，压缩时不能切割边界。否则触发：
```
OpenAI Protocol Error: tool role message must follow assistant with tool_calls
```

**算法**：

```rust
// runtime/src/compact.rs:110-140 摘录
let raw_keep_from = session.messages.len().saturating_sub(config.preserve_recent_messages);
let keep_from = {
    let mut k = raw_keep_from;
    loop {
        if k == 0 || k <= compacted_prefix_len { break; }
        let first_preserved = &session.messages[k];
        let starts_with_tool_result = first_preserved.blocks.first()
            .is_some_and(|b| matches!(b, ContentBlock::ToolResult { .. }));
        if !starts_with_tool_result { break; }
        let preceding = &session.messages[k - 1];
        let preceding_has_tool_use = preceding.blocks.iter()
            .any(|b| matches!(b, ContentBlock::ToolUse { .. }));
        if preceding_has_tool_use {
            k = k.saturating_sub(1);  // 回溯一步保留配对
        }
    }
    k
};
```

**Round 18 证据**：claw-code `runtime/src/prompt.rs:480` 与 `prompt.rs:197` 同样实现了该机制（[14-claude-code-capability-parity.md §6](14-claude-code-capability-parity.md)）。

---

## 6. Git Worktree 多 Agent 编排机制（独家深度章节）

### 6.1 三层结构

```
┌────────────────────────────────────────────────────────────────┐
│ Orchestrator（协调层）                                         │
│   ├─ LaneRegistry: BTreeMap<lane_id, Lane>                    │
│   ├─ BranchLockResolver                                       │
│   ├─ PolicyEngine: Vec<PolicyRule>                            │
│   └─ RecoveryRecipes: BTreeMap<FailureScenario, Recipe>       │
└─────────────────┬──────────────────────────────────────────────┘
                  ↓ 每条车道分配独立 worktree + 分支
┌────────────────────────────────────────────────────────────────┐
│ Lane（车道层）— 单 Agent 工作单位                              │
│   lane_id / branch / worktree / modules / state / green_level │
└─────────────────┬──────────────────────────────────────────────┘
                  ↓ 每条车道独立执行 prompt
┌────────────────────────────────────────────────────────────────┐
│ Worker（执行层）— Agent 工作进程                              │
│   ConversationRuntime + ToolExecutor + SessionTracer          │
└────────────────────────────────────────────────────────────────┘
```

### 6.2 LaneEvent 协议

**位置**：[runtime/src/lane_events.rs](../../claw-code/rust/crates/runtime/src/lane_events.rs)

```rust
pub enum LaneEventName {
    Started, Ready, Blocked, Red, Green,
    CommitCreated, PrOpened, MergeReady, Finished, Merged, Reconciled,
}

pub struct LaneEventMetadata {
    session_identity: SessionIdentity,
    ownership: LaneOwnership,
    provenance: EventProvenance,
    fingerprint: String,  // ← 去重键（防止 event 重播）
}
```

### 6.3 编排流程

1. Orchestrator 接收 user directive → 分解为 N 个 lane intent
2. `detect_branch_lock_collisions()` 检测嵌套模块冲突 → 拒绝冲突 lane
3. 每条通过的 lane 创建独立 `.git/worktrees/N` + 分支 + Worker 进程
4. Worker 执行 → emit LaneEvents → Orchestrator 经 fingerprint 去重
5. Orchestrator 周期评估 PolicyEngine：匹配优先级最高规则 → 执行 PolicyAction
6. 失败 → recovery_recipes 重试 → max_attempts 后 escalate_to_human

### 6.4 并行安全的四层保证

| 资源 | 隔离机制 | 强度 |
|---|---|---|
| Working tree | 物理分离 `.git/worktrees/N` | 强 |
| Branch | branch_lock 检测 + policy block | 强 |
| Module scope | `modules: Vec<String>` 嵌套碰撞 | 强 |
| 中央 main | Policy-gated merge（评审 + status gate） | 强 |

---

## 7. CLAUDE.md 加载与 codex AGENTS.md 对比

| 维度 | claw-code | codex | ironclaw |
|---|---|---|---|
| 多层加载 | ❌ 单一 CLAUDE.md（cwd 一份） | ✅ AGENTS.md 递归向上 | ✅ CLAUDE.md 递归 |
| 配置分离 | ✅ `.claude.json` + `.claude/settings.local.json` | ✅ config layer stack | ✅ .env + workspace settings |
| 最大字节限制 | ❌ 未文档化 | ✅ `project_doc_max_bytes`（默认 8KB/层） | ❌ 未文档化 |
| 合并策略 | ⚠️ 不明（推断 last-wins） | ✅ concat + layer precedence | concat |
| ProjectContext discover | ✅ `prompt.rs:50-80` | ✅ `agents_md.rs:367 LOC` | ✅ host bridge |

**结论**：dasclaw 应采用 codex 的多层加载方案（W3 任务 5），claw-code 的单层加载是短板。

---

## 8. 三方对比矩阵（36 项能力，与 35 §4 互补）

| # | 能力 | claw-code | codex | ironclaw | 移植决策 |
|---|---|:-:|:-:|:-:|---|
| **Agent 驱动** | | | | | |
| 1 | ConversationRuntime | ✅ 简洁 | ✅ 多 phase | ✅ LoopDelegate | claw 简洁 + ironclaw 骨架 |
| 2 | Session persistence | ✅ | ✅ | ✅ | dasclaw_core 整合 |
| 3 | Multi-agent fork（git worktree） | **✅ 独家** | ⚠️ multi-agents v2 | ⚠️ session_fork | **claw 独家** |
| **工具系统** | | | | | |
| 4 | apply-patch | ⚠️ | ✅ lark 协议 | ❌ | 用 codex |
| 5 | Bash validation | ✅ 1,004 LOC | ⚠️ ~800 | ❌ | **用 claw** |
| 6 | portable-pty | ✅ | ✅ | ❌ | claw + codex 任一 |
| 7 | Task / Team / Cron registry | **✅ 独家** | ❌ | ⚠️ scheduler | **claw 独家** |
| **治理** | | | | | |
| 8 | Branch lock 检测 | **✅ 独家** | ❌ | ❌ | **claw 独家** |
| 9 | Green contract | **✅ 独家** | ❌ | ❌ | **claw 独家** |
| 10 | Stale branch | ✅ 完整 | ⚠️ git-utils | ⚠️ | **用 claw** |
| 11 | Policy engine | ✅ Rust enum | ✅ Starlark | ❌ | claw 类型安全 + codex 热加载 |
| 12 | Recovery recipes | **✅ 独家** | ⚠️ degraded | ❌ | **用 claw** |
| 13 | Trust resolver | ✅ runtime 层 | ⚠️ TUI | ⚠️ GUI | **用 claw** |
| **沙箱** | | | | | |
| 14 | Linux sandbox | ⚠️ basic | ✅ landlock+bwrap | ✅ Docker+cap-std | **用 codex** |
| 15 | macOS sandbox | ❌ | ✅ seatbelt | ✅ | **用 codex** |
| 16 | Windows sandbox | ❌ | ✅ JobObject | ✅ | **用 codex** |
| 17 | Workspace policy enum | ❌ | ✅ SandboxPolicy 4 档 | ⚠️ workspace_path | **用 codex**（[35 §2](35-codex-capability-inventory.md)） |
| **MCP** | | | | | |
| 18-23 | Stdio/SSE/HTTP/WS/SDK/ManagedProxy | **✅ 全 6** | ✅ 5 + ⚠️ proxy concept | ⚠️ 仅 stdio | **用 claw 全 6** |
| **文档与配置** | | | | | |
| 24 | CLAUDE.md/AGENTS.md 多层 | ❌ 单层 | ✅ 多层 + max_bytes | ✅ 多层 | **用 codex** |
| 25 | Hooks 三分离 | ✅ 957 LOC | ✅ 干净 | ⚠️ 一体化 | **用 codex 设计** |
| **会话** | | | | | |
| 26 | Compaction（tool-use 边界） | **✅ 完整** | ⚠️ | ⚠️ | **用 claw** |
| 27 | Prompt caching | ✅ | ✅ | ✅ | 各自实现，dasclaw 整合 |
| **生产就绪** | | | | | |
| 28 | Identity / device-key | ⚠️ PKCE | ⚠️ PKCE | **✅ 完整** | **用 ironclaw** |
| 29 | Crash & panic | ❌ | ❌ | ⚠️ | dasclaw 新建 |
| 30 | Network proxy | ✅ | ✅ | ✅ | 任一 |
| 31 | Observability（rollout-trace） | ⚠️ basic | ✅ enhanced | ✅ | **用 codex** |
| **CodeAct（v1.0 新增）** | | | | | |
| 32 | js_repl Tool | ❌ | **✅ 5,556 LOC** | ❌ | 评估后用 codex |
| 33 | code_mode 路由 | ❌ | ✅ | ❌ | 评估后用 codex |

**评分汇总**：
- claw-code 独家优势：8 项（治理 5 件套 + Task/Team/Cron registry + MCP 全 6 + Compaction 边界保护）
- codex 独家优势：5 项（apply-patch lark / 三平台沙箱 / SandboxPolicy enum / AGENTS.md 多层 / CodeAct）
- ironclaw 独家优势：1 项（identity device-key 完整体系）

---

## 9. 路线 B 的移植决策树

```
W4 治理 6 件套 + bash_validation
    ├─ branch_lock.rs     (163 LOC) → dasclaw_governance/branch_lock.rs
    ├─ green_contract.rs  (189 LOC) → dasclaw_governance/green_contract.rs
    ├─ stale_branch.rs    (268 LOC) → dasclaw_governance/stale_branch.rs
    ├─ policy_engine.rs   (398 LOC) → dasclaw_governance/policy_engine.rs
    ├─ recovery_recipes.rs(304 LOC) → dasclaw_governance/recovery.rs
    ├─ trust_resolver.rs  (156 LOC) → dasclaw_governance/trust.rs
    └─ bash_validation.rs (1,004 LOC) → dasclaw_bash_validation crate

W5 MCP 统一
    ├─ mcp_client.rs (200 LOC) → dasclaw_mcp_v2/client.rs（6 transport enum）
    ├─ mcp_stdio.rs (~800 LOC) → dasclaw_mcp_v2/stdio.rs
    ├─ mcp_lifecycle_hardened.rs (~300 LOC) → dasclaw_mcp_v2/lifecycle.rs
    └─ McpClientAuth + McpOAuthConfig → dasclaw_mcp_v2/auth.rs

W3 项目文档
    ├─ ❌ 不用 claw 单层 CLAUDE.md 加载（用 codex AGENTS.md 多层）
    └─ 仅 port `prompt.rs` 的 SystemPromptBuilder 静态/动态分界标记

跨 wave 杂项
    ├─ compact.rs (280 LOC) → dasclaw_core/compact.rs（W4 或 W7）
    ├─ task_registry.rs / team_cron_registry.rs (777 LOC) → dasclaw_scheduler crate（W6）
    └─ lane_events.rs (~500 LOC) → dasclaw_governance/lane_events.rs（W4）
```

**总移植量**：~4,800 LOC 直接 port + ~5,000 LOC 适配改造 = ~10,000 LOC（占 dasclaw 总目标 ~30%）

---

## 10. 风险与局限

| 风险 | 严重度 | 说明 | 缓解 |
|---|:-:|---|---|
| 治理 6 件套依赖 git worktree 概念，desktop-client 是单 worktree 场景 | 🟡 | 治理逻辑可降级为单 lane 模式，但 PolicyEngine/RecoveryRecipes 仍有价值 | W4 实现时给 SingleLaneAdapter |
| MCP 6 transport 中 ManagedProxy 绑定 CCR 私有协议 | 🟡 | dasclaw 是否需要 ManagedProxy 待商榷 | W5 决策时跳过此 transport |
| Policy engine 用 Rust enum，新增条件需重新编译 | 🟡 | 与 codex Starlark 互补，不冲突 | 长期演进可加 Starlark backend |
| claw-code 总体活跃度（2026-03-31~2026-04-03）短，主线维护者不明 | 🔴 | 需确认是否仍在演进 | W1 评估时 contact upstream |
| Bash validation 6 模块的 PermissionMode 与 ironclaw cap-std 模型不完全一致 | 🟡 | 需要 enum 转换层 | W4 提供 PermissionMode <-> SandboxPolicy 转换 |

---

## 附录 A：crate 清单与依赖关系

```
runtime (16,200+)
    ├─ depends on: api, plugins, tools, telemetry
    └─ contains: 治理 6 件套, MCP, conversation, compact, prompt, hooks, sandbox

tools (12,500+)
    ├─ depends on: api
    └─ contains: GetFile, WriteFile, EditFile, GlobSearch, GrepSearch, Bash, apply_patch, portable_pty

plugins (7,800)
    └─ contains: hooks engine（schema/registry/engine 三分离）, lifecycle 状态机

api (3,500)
    └─ contains: Anthropic /v1/messages 兼容 client + flow adapters

rusty-claude-cli (4,200)
    ├─ depends on: runtime, tools, plugins, api, commands, telemetry
    └─ CLI REPL + session store + permission enforcer

commands (1,800) / telemetry (1,500) / mock-anthropic-service (800) / compat-harness (300)
```

---

## 11. 与 30 / 31 / 32 / 35 文档的接口

- **30 文档（事实矩阵）**：本 36 文档的 §8 三方对比矩阵应作为 30 文档"D 安全治理"与"H MCP"两小节的补充事实来源
- **31 文档（目标架构）**：dasclaw_governance / dasclaw_mcp_v2 / dasclaw_bash_validation 三个新 crate 应在 31 v2.1 已列出
- **32 文档（执行计划）**：W4 移植清单参考本文 §9
- **35 文档（codex 清单）**：本 36 文档 §8 与 35 §4 的能力评分相互验证

---

## 变更日志

- v1.0 (2026-04-25) — 初版，配套 35 文档完成路线 B 三方核心仓库的全量盘点（ironclaw 见 13 / codex 见 35 / claw-code 见本文）
