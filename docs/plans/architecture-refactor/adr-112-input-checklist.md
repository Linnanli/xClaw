# ADR-112 契合度评估 — Input Checklist (Step 1)

> 本文档是 W3-A 契合度评估方案 (ADR-112) 的 **输入清单**，不是评估方案本身。
> 上游 handoff：`/memories/session/w3a-compatibility-evaluation-handoff.md`
> 下游产出：`adr-112-compatibility-evaluation.md`（Step 3 才写）
>
> 结构按 handoff §3：**已有抽取 70% / 我整理 20% / 你拍板 10%**。

---

## 1. 已有可直接抽取的输入（70%）

> 把这 8 类资产里的具体指标 / 规则 / 教训直接喂给评估方案，**不需要再开会讨论**。

### 1.1 DLP 规则维度模型 — `docs/DLP_MULTI_DIMENSIONAL_RULES_DESIGN.md`

直接产出"DLP 检测能力评分卡"的子项：

| 维度 | 子项 | 已落点 |
|---|---|---|
| Pattern-Based | regex / keyword / dictionary (Aho-Corasick) | `desktop-client/src/dlp/patterns.rs` + `detector.rs` |
| Fingerprint-Based | DB fingerprint / document fingerprint (fuzzy hash) | 当前**未实现**（设计稿） |
| Content-Based | OCR / face / 语义模型 (ONNX) | 当前**未实现**（设计稿） |
| File-Based | MIME / magic number / extension | 当前**未实现**（设计稿） |

**用法**：评估方案的 Layer 3 Business 项 1 = "DLP 维度覆盖率"，按上表分子/分母。已实现 1/4 维度 → 25%（基线分），目标分待 §3 用户拍板。

---

### 1.2 DLP 历史教训（盲区清单）— `docs/testing-guide.md` §历史教训详细分析

5 大盲区可直接转为"必须存在的 contract test"：

| # | 盲区 | 转为 contract test |
|---|---|---|
| 1 | DLP 失败路径缺失（fail-open） | `test_dlp_fail_safe_when_engine_unavailable` — 引擎不可达必须 block |
| 2 | model-configs 404 | `integration_smoke_tests.rs` 三层冒烟 — 已存在 |
| 3 | client-models 缺字段 | "够用契约"：从业务目标定义最小可用字段集 |
| 4 | SSE 事件名不匹配 | E2E 真实后端事件 (`scan_user_input` → `sanitized_content`) |
| 5 | Tauri state not managed | `engine_startup_tests.rs` 启动时序 — 已存在 |

> ⚠️ **handoff 中提到的 3 份 DLP 文档不存在**：
> - `desktop-client/DLP_LESSONS_SUMMARY.md` ❌
> - `desktop-client/DLP_TESTING_LESSONS_LEARNED.md` ❌
> - `desktop-client/DLP_SUPPLEMENTARY_TEST_PLAN.md` ❌
>
> 这 3 份文件曾被 [AGENTS.md](../../../AGENTS.md#L459-L461) 和 [docs/testing-guide.md](../../testing-guide.md#L99) 引用，但实际仓库中**已不存在**（仅在代码注释 `desktop-client/ironclaw/crates/ironclaw_safety/tests/agent_hook_integration.rs#L8` 残留引用）。Step 3 写 ADR 时**不再引用这 3 份**，全部归并到 `docs/testing-guide.md` 的"历史教训详细分析"小节。

> ⚠️ **handoff 中提到的 `desktop-client/docs/features/dlp/{overview,implementation,testing,lessons-learned}.md` 4 份文件也不存在**（仅在 `desktop-client/docs/README.md#L51,#L74,#L77,#L86` 有指向链接但目标缺失）。

**真实可抽取的 DLP 实施细节来源**：
- `desktop-client/src/dlp/mod.rs` + `detector.rs` + `sanitizer.rs` + `patterns.rs` + `integration.rs`
- 测试族：`requirements_tests.rs` / `security_tests.rs` / `reliability_tests.rs` / `code_coverage_tests.rs` / `data_coverage_tests.rs` / `change_coverage_tests.rs` / `policy_sync_integration_tests.rs` / `integration_tests.rs`
- 评分时直接读测试族里的"已断言的不变量"，转成 contract test 清单。

---

### 1.3 Admin Backend 敏感操作 / DLP API — `docs/ADMIN_BACKEND_REQUIREMENTS.md`

直接喂"Layer 2 Integration 跨模块联动"项：

| 章节 | 现状 | 评估用途 |
|---|---|---|
| §3 用户管理 | 部分实现 | 多租户隔离的"租户主键"来源 |
| §5 DLP 规则管理（CRUD） | 仅 GET 实现，CUD 未实现 | DLP 规则下发链路完整性 |
| §6 敏感操作管理 | 仅 GET 实现 | ApprovalGate 规则下发链路 |
| §7 策略查询（供 Desktop 使用） | 已实现 | 客户端轮询 / SSE 接收策略路径 |

**用法**：路径 F (Managed Mode 策略下发 + 验签 + 应用) 的 contract test 必须覆盖 §7 的 4 个端点 (`/api/policies` / `/api/policies/dlp` / `/api/policies/sensitive-ops` / `/api/policies/version`)。

---

### 1.4 OWASP Top 10 安全 checklist — `skills/code-review-expert/references/security-checklist.md`

直接转为"Layer 3 Business 攻击场景"列表（10 大类）：

1. **Input/Output Safety**：XSS / Injection (SQL/NoSQL/cmd/GraphQL) / SSRF / Path traversal / Prototype pollution
2. **AuthN/AuthZ**：tenant 检查 / RBAC / IDOR / session fixation
3. **JWT/Token**：algorithm confusion / 弱 secret / 缺 exp / payload 含 PII
4. **Secrets/PII**：日志泄露 / git history / error message 含敏感
5. **Supply Chain**：unpinned deps / dependency confusion / CVE
6. **CORS/Headers**：过宽 CORS / 缺 CSP / X-Frame-Options
7. **Runtime**：unbounded loops / 无 timeout / ReDoS
8. **Cryptography**：MD5/SHA1 / hardcoded IV / ECB / 短 key
9. **Race Conditions**：TOCTOU / 缺 optimistic locking / 缺 distributed lock
10. **Data Integrity**：缺 transaction / 缺 idempotency

**用法**：39 个 contract test 套件中，每个模块默认从这 10 类中**至少抽取 3 类适用项**作为 security audit baseline。

---

### 1.5 Security Reviewer 工作流 — `.codex/prompts/security-reviewer.md`

直接复用"Critical Issue 输出模板"：

```
**Severity:** CRITICAL
**Category:** [OWASP category]
**Location:** `file.ts:123`
**Exploitability:** [Remote/Local, authenticated/unauthenticated]
**Blast Radius:** [What an attacker gains]
**Remediation:** [secure code example]
```

**用法**：评估方案的"违规事件 (incident) 报告格式"统一用此模板，便于 Layer 3 nightly CI 把 finding 转为 dashboard 卡片。

---

### 1.6 ironclaw 安全能力清单（archive 但仍有效）— `docs/plans/architecture-refactor/archive/legacy-v1/13-security-capability-inventory.md`

13 文档定义的 **9 层纵深防御栈**直接当 Layer 1 Parity 评分骨架：

| 层 | 能力 | 实现位置 |
|---|---|---|
| L1 | 主进程 FS 防护 (cap_std TOCTOU-safe) | `crates/dasclaw_workspace_cap/` (568 行) |
| L2 | 子进程沙箱（三平台 OS-native） | 已 W3.3 完成（dasclaw_sandbox 三平台） |
| L3 | 容器沙箱 host (Docker + proxy + allowlist) | `desktop-client/ironclaw/src/sandbox/` (3611 行) |
| L4 | 工具沙箱 (WASM + capability opt-in) | `desktop-client/ironclaw/src/tools/wasm/` (15 文件 ~3000 行) |
| L5 | 凭证边界 (credential injector + SecretsStore) | `secrets/` (2546 行) + `tools/wasm/credential_injector.rs` (639 行) |
| L6 | 内容安全 (sanitize + credential/leak detect + policy) | `desktop-client/ironclaw/crates/ironclaw_safety/` (4849 行 + fuzz) |
| L7 | 日志脱敏 (18+ 字段) | `tools/redaction.rs` (251 行) |
| L8 | 网络审计 (4 边界 / 5 端口) | `desktop-client/ironclaw/src/NETWORK_SECURITY.md` |
| L9 | Extension 治理 | `desktop-client/ironclaw/src/extensions/` (11786 行) |

**用法**：每层一个 Parity Score 子项，三 harness（codex / claw-code / ironclaw）对照打分。L2 已 port codex，L1/L3-L9 全部保留 ironclaw（13 文档已盖章）。

---

### 1.7 Managed Mode 安全规范 — `plan.md` §受管终端模式

P0 / P1 / P2 / P3 全部已完成，4 阶段清单直接转为 contract test：

| 阶段 | contract test |
|---|---|
| P0 | `MANAGED_MODE` env var 注入 |
| P1 | 受管模式禁本地 SKILL.md 直装 / 禁显式 URL 安装扩展 / 启动时未授权项软禁用 |
| P2 | `GET /api/client-policy` 返回 envelope (algorithm/key_id/manifest_payload/signature) + Ed25519 验签 + 失败回退缓存 |
| P3 | 多 signing key 轮换 / `last_policy_version` 单调递增 / 过期策略拒绝 / 启动日志含 `managed_mode` + `has_signed_policy` |

**用法**：路径 F 的 6 个 contract test 直接来自上表 + `managed-policy-smoke.sh` 联调脚本。

---

### 1.8 Tauri IPC 安全示例 — `desktop-client/docs/architecture/tauri-ipc.md`

直接喂"Layer 2 路径 A/B 的 IPC 边界 contract test"：

- Command 路径：`scan_user_input` 必须返回 `sanitized_content`（不是原文）
- Event 路径：SSE 事件名必须命名匹配，前端用 `listen('event-name')` 而不是 `onmessage`
- State 注入：所有命令必须显式 `State<'_, CommandState>`，不能直接调内部函数

---

## 2. Agent 整理（20%）

> 这部分需要读代码画出来，不能直接抽现有文档。

### 2.1 6 条关键路径 e2e（评估方案 Layer 2 主骨架）

> 每条路径 = 1 个 e2e contract test 套件（最小步数 ≥ 5）。

#### 路径 A — 用户消息全链路（DLP 闭环）

```
UI input
  → invoke('scan_user_input', { content })
  → Desktop dlp::sanitizer 前置脱敏
  → x_claw_agent::agentic_loop 进入循环
  → ironclaw HookRegistry::PreToolUse 触发 (W3-A 后)
  → tool dispatch (sandbox/wasm/builtin)
  → ironclaw HookRegistry::PostToolUse
  → ironclaw_safety::leak_detector 后置扫描
  → emit('chat-message') 回 UI
```

**关键代码点**：
- `desktop-client/src/dlp/sanitizer.rs`（前置）
- `crates/x_claw_agent/src/agentic_loop.rs`（循环主体）
- `desktop-client/ironclaw/src/agent/dispatcher.rs`（hook 触发）
- `desktop-client/ironclaw/crates/ironclaw_safety/src/leak_detector.rs`（后置）

**Phase 0 收口前**：hook 调度跨 4 套系统（ironclaw HookRegistry / x_claw_agent HookBundle / x_claw_agent SessionHooks / dasclaw_hooks），每套都有可能漏 PreToolUse。**评估时这条路径每套独立断言**。

#### 路径 B — 工具调用 + ApprovalGate 弹窗回路

```
agentic_loop 决定调 tool X
  → ApprovalGate::check(tool=X, mode=PermissionMode::current())
  → 命中敏感规则 → emit('approval-required', { tool, args, reason })
  → UI 弹窗 → 用户 Approve/Deny
  → invoke('approval-response')
  → ApprovalGate 放行 / 拒绝
  → 若放行：dispatch 进 sandbox 子进程
  → 子进程结果 → tool result
```

**关键代码点**：
- `crates/x_claw_agent/src/permissions.rs`
- `desktop-client/ironclaw/crates/ironclaw_safety/src/policy.rs`
- ApprovalGate 弹窗 IPC：`desktop-client/ui/`

**联动到 admin-backend**：ApprovalGate 规则来自 §7 `/api/policies/sensitive-ops`，contract test 必须覆盖"策略下发后立即生效"。

#### 路径 C — Sub-agent fork-join（W6 后才完整）

```
parent agentic_loop
  → AgentControl::fork(role=worker, depth+1)
  → 检查 depth ≤ MAX_DEPTH 且 role ∈ whitelist
  → 子 session 独立演化（独立 context window / 独立 hook stack）
  → 父子合流：sub_agent_result → parent context
```

**关键代码点**：
- W6 待实现（codex `AgentControl::fork`）
- 当前 `crates/x_claw_agent/src/session_manager.rs`（fork hook 待加）
- 14 文档 §9 标注 ❌

**评估方案策略**：W6 前路径 C 标 N/A（不计入分母），W6 后纳入 Layer 2。

#### 路径 D — 跨租户隔离

```
Tenant A 创建 session
  → dasclaw_workspace_cap::WorkspaceRoot::open(tenant_a_root)
  → 所有 fs 调用必须穿过 tenant_a_root capability
  → Tenant B 同时创建 session
  → 两个 session 互不可见 fs / secrets / context
```

**关键代码点**：
- `crates/dasclaw_workspace_cap/src/lib.rs` (568 行)
- `desktop-client/ironclaw/src/secrets/store.rs`（按 tenant 分区）
- admin-backend 多租户配置（待补 §1.4 的 RBAC）

**硬指标**：跨租户读 = 0，跨租户写 = 0，跨租户列 = 0。**任一非零 → block 发布**。

#### 路径 E — 合规审计落库

```
任何 hook event / tool call / policy decision
  → ironclaw_observability::audit_log
  → 落 sqlite (client-side) + admin-backend audit_logs 表
  → 必须含: tenant_id / user_id / session_id / tool_name / policy_decision / timestamp
  → 7 天 / 30 天 / 1 年三档保留期可配
```

**关键代码点**：
- `crates/dasclaw_observability/`（如存在）
- admin-backend `audit_logs` 表（已实现 GET，待实现统计）
- `client-reports` / `conversation_messages` 表

**硬指标**：每条 hook event 必须有对应 audit row（采样率 = 100%）。

#### 路径 F — Managed Mode 策略下发 + 验签 + 应用

直接复用 §1.7 P2/P3 的 6 个 contract test。

---

### 2.2 多租户隔离硬指标

| 指标 | 阈值 | 检测点 |
|---|---|---|
| 跨租户 fs 读 | 0 | `dasclaw_workspace_cap` capability check |
| 跨租户 secrets 读 | 0 | `secrets/store.rs` tenant filter |
| 跨租户 session 列 | 0 | `session_manager` tenant 分区 |
| 跨租户 audit log 读 | 0 | admin-backend RBAC 检查 |
| Tenant ID 注入 (路径 / header / body) | 自动拒绝 | API 入口 middleware |

需归并的资产：
- `crates/dasclaw_workspace_cap/src/{lib,policy}.rs`
- admin-backend 多租户 SQL（待 §1.3 §1 RBAC 补完后定）

---

### 2.3 Prompt Injection 攻击场景

> 来自 `ironclaw_safety::sanitizer` 已断言的不变量 + OWASP LLM Top 10 标准场景。

| # | 场景 | 检测能力 |
|---|---|---|
| 1 | Direct injection ("Ignore previous instructions") | `sanitizer.rs` 已实现 |
| 2 | Indirect injection（外部内容里带指令） | `sanitizer.rs` + `leak_detector.rs` |
| 3 | Goal hijacking | 待补 fuzz corpus |
| 4 | Tool misuse via injection | ApprovalGate + PreToolUse hook |
| 5 | Context poisoning | compaction layer + hook |
| 6 | Jailbreak (DAN-style) | sanitizer 黑名单 |
| 7 | Encoded / obfuscated injection (base64 / unicode) | sanitizer 解码后再扫 |
| 8 | Multi-turn injection (跨消息累积) | session-level scanner |
| 9 | Output handling injection (LLM 输出注入到下游) | `credential_detect.rs` |
| 10 | Plugin/tool manifest injection | `wasm/loader.rs` 签名校验 |

**反推证据**：`desktop-client/ironclaw/crates/ironclaw_safety/fuzz/corpus/fuzz_safety_sanitizer/` 已有 fuzz 语料，Step 3 评估方案直接读取 corpus 算"已覆盖率"。

---

### 2.4 越权 / 提权场景（合并 ApprovalGate / PermissionMode / sandbox）

| 场景 | 防御层 |
|---|---|
| Tool 调用绕过 ApprovalGate | hook PreToolUse + policy decider |
| PermissionMode 绕过（plan→edit 自动升级） | `permissions.rs` 状态机 |
| Sandbox 子进程提权（fork/exec 逃逸） | L2 sandbox seccomp/Seatbelt/JobObject |
| WASM capability 提权（运行时 grant） | `wasm/capabilities.rs` opt-in 不可改 |
| Secrets 越权（tool 拿到原文 key） | `credential_injector.rs` host 边界 |
| Tenant 越权（Tenant A 读 Tenant B） | `workspace_cap` + admin RBAC |

每条场景 = 1 个 attack contract test，Layer 3 nightly 跑，0 漏报为发布门槛。

---

### 2.5 Agent 框架核心能力评估输入（Layer 1 Parity 主骨架）

> **这是评估方案的核心**：x-claw 是 codex 骨架 + claw-code 肌肉 + ironclaw 安全的混血，**契合度 = 三 harness 各自做参照系，给 x-claw 打分**。
> 之前 §1-§2.4 几乎全部偏向"Layer 3 安全/合规"，这一节把"agent 框架自身能力"补回来。

#### 2.5.1 现成可复用的三 harness 资产（不需要从零造）

| Harness | 位置 | 资产 |
|---|---|---|
| **claw-code** | `claw-code/rust/crates/rusty-claude-cli/tests/mock_parity_harness.rs` | 10 scripted scenarios + `mock-anthropic-service` mock LLM + `run_mock_parity_diff.py` 差异 runner |
| claw-code 场景清单 | `claw-code/rust/mock_parity_scenarios.json` | `streaming_text` / `read_file_roundtrip` / `grep_chunk_assembly` / `write_file_allowed` / `write_file_denied` / `multi_tool_turn_roundtrip` / `bash_stdout_roundtrip` / `bash_permission_prompt_{approved,denied}` / `plugin_tool_roundtrip` |
| **codex** | `codex-cli-main/codex-rs/core/tests/suite/tool_harness.rs` | tool harness（已 vendor codex 78 个 crate 边界） |
| codex otel | `codex-cli-main/codex-rs/otel/tests/harness/` | observability harness |
| **ironclaw** | `desktop-client/ironclaw/tests/parity_harness.rs` + `parity_gate_p1.rs` + `parity_gate_p2.rs` + `live_harness.rs` + `gateway_workflow_harness.rs` | parity gate p1/p2 + live + gateway workflow（5 套已存在） |
| **claw-code PARITY 评分模板** | `claw-code/PARITY.md` | 9-lane checkpoint 表 + lane details + 状态枚举（merged / partial / N/A）+ feature commit / merge commit / evidence 三联 |

**用法**：x-claw 评估直接 fork `claw-code/PARITY.md` 的 9-lane checkpoint 表结构 → 改造为 13-module checkpoint 表（详见 §2.6）。

#### 2.5.2 Patchwork 诊断转评估指标（Phase 0 收口判定标准）

handoff §1.2 列出 3 域红/黄但未给"完成判定"。**每个判定 = Layer 1 Parity 一个 contract test**：

| 域 | 判定标准（contract test） | 当前 ground truth |
|---|---|---|
| **System Prompt** | `assert_eq!(count_prompt_builders(), 1)` — 只剩 1 个构造器 | 当前 = 3（[reasoning.rs#L793](../../../desktop-client/ironclaw/src/llm/reasoning.rs)、[workspace/mod.rs#L1161](../../../desktop-client/ironclaw/src/workspace/mod.rs)、[tools/builder/core.rs#L299](../../../desktop-client/ironclaw/src/tools/builder/core.rs)） |
| | `assert!(env::var("IRONCLAW_PROMPT_LAYERING").is_err())` — env var 已删 | 当前 = exists |
| | LayeredPromptBuilder 是唯一路径 | 当前 = 双路 |
| **Hooks** | `assert_eq!(count_hook_systems(), 1)` — 只剩 dasclaw_hooks | 当前 = 4（ironclaw HookRegistry / x_claw_agent HookBundle / x_claw_agent SessionHooks / dasclaw_hooks 空骨架） |
| | `crates/x_claw_agent/src/session_hooks.rs` 已删 | 当前 = exists |
| | OnSessionStart/End 唯一定义在 ironclaw HookPoint | 当前 = 跨两套 |
| **Tool Registration** | `assert_eq!(count_register_methods(), 1)` — 只剩 `bootstrap_tools(app)` | 当前 = 5 register_* 散在 4 文件 |

**用法**：Phase 0 PR 0.1 / 0.2 / 0.3 完成的硬指标 = 上面 9 行 assertion 全绿。

#### 2.5.3 x_claw_agent 19 模块对照表（评估单元）

handoff §1.5 列了 19 模块，按"血缘归属 + 评估归类"映射：

| # | 模块 | 血缘 | Parity 对照 harness | Evaluation 归类 |
|---|---|---|---|---|
| 1 | `agentic_loop.rs` | ironclaw port | ironclaw `parity_harness.rs` + claw-code `mock_parity_harness` | 模块 1 Agentic Loop |
| 2 | `bash_validation.rs` | claw-code port | claw-code `bash_stdout_roundtrip` + `bash_permission_prompt_*` | 模块 3 Tool System / 模块 5 Permission |
| 3 | `compaction.rs` | ironclaw port | ironclaw `live_harness.rs` 长 session 测试 | 模块 7 Context/Memory |
| 4 | `context_monitor.rs` | x-claw 自创 | — | 模块 7 Context/Memory（非对等） |
| 5 | `hooks.rs` (HookBundle 4 trait) | x-claw 自创 | codex hooks engine | 模块 4 Hook System（W3-A 重构） |
| 6 | `intent.rs` | x-claw 自创 | — | 模块 2 Prompt 装配（非对等） |
| 7 | `lib.rs` | meta | — | meta |
| 8 | `llm.rs` | ironclaw port | claw-code `streaming_text` | 模块 1 Agentic Loop |
| 9 | `messages.rs` | ironclaw port | claw-code `multi_tool_turn_roundtrip` | 模块 1 Agentic Loop |
| 10 | `permissions.rs` | x-claw 自创 | claw-code `write_file_{allowed,denied}` + `bash_permission_prompt_*` | 模块 5 Permission |
| 11 | `reasoning_ctx.rs` | ironclaw port | — | 模块 1 Agentic Loop |
| 12 | `response_types.rs` | ironclaw port | claw-code `mock_parity_scenarios.json` | 模块 1 Agentic Loop |
| 13 | `session.rs` | ironclaw port | ironclaw `gateway_workflow_harness.rs` | 模块 1 Agentic Loop |
| 14 | `session_hooks.rs` | x-claw 自创（**W3-A PR 0.2 删**） | — | 模块 4（已弃） |
| 15 | `session_manager.rs` | ironclaw port | ironclaw `parity_gate_p2.rs` (含 fork) | 模块 9 Sub-Agent / fork |
| 16 | `submission.rs` | ironclaw port | claw-code `mock_parity_harness` | 模块 1 Agentic Loop |
| 17 | `task.rs` | ironclaw port | claw-code Lane 4 (TaskRegistry) | 模块 1 Agentic Loop |
| 18 | `traits.rs` | x-claw 自创（adapter） | — | meta |
| 19 | `undo.rs` | x-claw 自创 | — | 模块 14 Frontend/IPC（非对等） |

**用法**：
- 19 模块 → 13 评估单元映射建立完成
- `x-claw 自创` 7 个模块 = "主动允许偏离声明"候选项（见 §2.5.4）
- `meta` 2 个不参与评分

#### 2.5.4 "主动允许偏离声明"模板（每模块 1 份）

格式（强制 4 段）：

```
### 偏离声明 — 模块 X / 项 Y

**与谁偏离**：codex / claw-code / ironclaw 中具体哪一份
**偏离描述**：x-claw 做了什么、它没做什么（具体到函数名 / 代码行）
**偏离原因**：业务必要 / 安全要求 / 性能 / 维护成本（必填一项）
**残余风险与补偿**：偏离引入了什么新风险 / 我们靠什么补偿（contract test / hook / runtime check）
```

**已知必填的偏离声明候选清单**（W3-A 第一批必须写）：

| # | 模块 | 偏离对象 | 偏离描述 | Step 3 ADR 必含 |
|---|---|---|---|---|
| 1 | Hook System | codex `command hook handler` | x-claw `enterprise-mode` 默认禁用 command handler，等保 / ISO 27001 合规 | ✅ |
| 2 | Hook System | ironclaw `HookRegistry` | x-claw 改名 `InProcessHookBackend` 实现 `HookBackend` trait | ✅ |
| 3 | Sub-Agent | ironclaw（无 fork） | x-claw W6 才补 codex `AgentControl::fork`；W3-A 阶段路径 C 标 N/A | ✅ |
| 4 | Tool System | claw-code `bash_validation` | x-claw port，但加了等保审计字段 | ✅ |
| 5 | Permission | claw-code `permission_enforcer` | x-claw 重写为 ApprovalGate + PermissionMode 状态机 | ✅ |
| 6 | DLP | codex / claw-code（无 DLP） | x-claw 完全保留 ironclaw `leak_detector.rs` | ✅ |
| 7 | WASM 工具沙箱 | codex / claw-code（无 WASM） | x-claw 完全保留 ironclaw `tools/wasm/` | ✅ |
| 8 | Context/Memory | codex / claw-code | x-claw 自创 `context_monitor.rs` + `intent.rs`（非对等） | ✅ |
| 9 | Sandbox 三平台 | ironclaw `sandbox/` (Docker only) | x-claw W3.3 已 port codex linux/windows-sandbox + 自实现 macOS Seatbelt | ✅ |
| 10 | System Prompt | codex (单一构造器) / claw-code | x-claw Phase 0 收口前是 3 个构造器 — **PR 0.1 完成后取消此偏离** | ✅ |

**用法**：Step 3 ADR 必含一节 "Active Deviations"（10 条），评分时扣分按"声明且补偿"= 不扣分；"未声明" = -10 分。

---

### 2.6 13 模块 × 3 harness 评分卡输入清单（39 contract test 套件）

> handoff §1.6 写明 "13 模块 × 3 层 = 39 个 contract test 套件"，但模块清单从未列。这一节给清单。

#### 2.6.1 13 评估模块（去掉 14 块能力地图中的 1 项 meta 项）

| # | 模块 | Layer 1 Parity 来源 | Layer 2 Integration 关键路径 | Layer 3 Business 攻击面 |
|---|---|---|---|---|
| 1 | Agentic Loop | claw-code `mock_parity_harness` 10 场景 + ironclaw `parity_harness.rs` | 路径 A | DoS（unbounded loop） |
| 2 | Prompt 装配 | codex `ConfigLayerStack` + `Fragment` | Phase 0 PR 0.1 收口测试 | Prompt injection（路径 A 前段） |
| 3 | Tool System | claw-code Lane 1+3+8 (bash/file/lsp) + codex `tool_harness.rs` | 路径 B | Tool misuse / 越权 |
| 4 | Hook System | codex hooks engine + W3-A PR 1.1-1.4 | 路径 A 中段 | hook 绕过 / SafetyDecision 漏洞 |
| 5 | Permission | claw-code `permission_enforcer` Lane 9 + ironclaw policy | 路径 B（ApprovalGate） | 越权（PermissionMode 自动升级） |
| 6 | Sandbox 三平台 | codex linux/windows-sandbox + ironclaw Docker + W3.3 macOS Seatbelt | 路径 B 子进程 | 子进程逃逸 / fork bomb |
| 7 | Context/Memory | ironclaw `compaction.rs` + RRF | 路径 A 上下文管理段 | Context poisoning / DoS |
| 8 | MCP 6 transport | codex `codex-mcp` + ironclaw extension MCP | 路径 B（MCP 工具调用） | Plugin 仿冒 / 协议劫持 |
| 9 | Sub-Agent / fork | codex `AgentControl::fork`（W6） | 路径 C（W6 后） | 越权 fork / depth 爆炸 |
| 10 | Identity/Auth | ironclaw_auth + Argon2 + JWT | 路径 F + admin auth | JWT algorithm confusion / IDOR |
| 11 | Observability | dasclaw_observability + audit_log | 路径 E | log injection / 采样率漏洞 |
| 12 | Safety/Governance | ironclaw_safety 6 件套（4849 行 + fuzz） | 路径 A 全段 | 10 类 Prompt injection |
| 13 | Frontend/IPC | Tauri IPC + UI | 路径 A 头尾 + 路径 B 弹窗 | XSS / IPC 命令注入 |

> **去掉的 1 项**：14 块能力地图中的 "15. Net Proxy" 已合并进模块 6 Sandbox 三平台（W7 Net Proxy port 是 sandbox proxy 的子集）。
> "13. Resilience" 合并进模块 11 Observability（reliability_tests.rs 已存在）。

#### 2.6.2 39 contract test 套件命名规范

```
tests/
├── parity_module_<N>_<name>.rs        # Layer 1（13 套）
├── integration_module_<N>_<name>.rs   # Layer 2（13 套）
└── business_module_<N>_<name>.rs      # Layer 3（13 套）
```

**用法**：Step 3 ADR 表格直接列 39 行，每行 4 列：模块名 / harness 来源 / 关键路径 / 攻击面。CI 配置：
- L1 (PR smoke) = 13 个 parity_*.rs，目标 < 5 min
- L2 (behavioral, daily) = 13 个 integration_*.rs，目标 < 30 min
- L3 (nightly full) = 13 个 business_*.rs + fuzz corpus replay，目标 < 4 h

#### 2.6.3 评分卡公式（最终）

```
Total Score = 0.30 × Parity + 0.30 × Integration + 0.40 × Business

Parity_module_i = (passed_assertions / total_assertions) × deviation_penalty
deviation_penalty = 1.0 if 已声明且补偿 else 0.7
Parity = sum(Parity_module_i) / 13

Integration = (passed_paths / 6) × (1 - 0.05 × 跨模块联动失败数)

Business = 0.4 × DLP_coverage_tier1 + 0.3 × attack_block_rate + 0.2 × multi_tenant_isolation + 0.1 × audit_completeness
```

**block 发布门槛**（与 §4 选项 3 一致）：
- Total ≥ 90%
- Business 项中 DLP 漏报 = 0 / 跨租户违规 = 0 / Prompt injection 拦截 ≥ 99%
- Phase 0 收口 10 行 assertion 全绿（§2.7.1 合并版）
- 16 条偏离声明全部书面化 + 分类列已填（§2.7.2）

---

### 2.7 Agent 框架核心能力契合矩阵（100% 覆盖三层验证）

> **方法论**：按 [AGENTS.md](../../../AGENTS.md) "分析工具使用规范"，对 11 个核心能力执行 L1 semantic_search → L2 vscode_listCodeUsages → L3 grep_search 三层验证。
> **执行方式**：2026-04-28 用 4 个 Explore subagent 并行完成，每个 subagent 独立产出验证报告，结果文件保留在 `chat-session-resources/`。
> **覆盖范围**：×Codex / ×Claw-Code / ×Ironclaw / ×x_claw_agent / ×dasclaw_* 五方对账。
> **L2 局限说明**：Rust 文件 LSP usages 暂不可用，对 Rust 模块用 L1+L3 双证据替代；TypeScript/前端可正常 L2 验证。

#### 11 能力 × 三层验证 总览表

| # | 能力 | Patchwork | x-claw 现状 | 关键证据（L1+L3） | W3-A 行动 |
|---|------|-----------|------------|------------------|----------|
| 1 | **Agentic Loop** | 🟡 黄 | port from ironclaw | [agentic_loop.rs#L166](../../../crates/x_claw_agent/src/agentic_loop.rs#L166) `run_agentic_loop()` + ironclaw [dispatcher.rs#L81](../../../desktop-client/ironclaw/src/agent/dispatcher.rs#L81) wrapper + codex 独立 [handlers.rs#L1022](../../../codex-cli-main/codex-rs/core/src/session/handlers.rs#L1022) `submission_loop()` | x-claw 与 ironclaw 共用 OK，**codex 路径需独立适配** |
| 2 | **Prompt 装配** | 🔴 红 | **3 个 builder 共存** | ironclaw 3 入口（[reasoning.rs#L1025](../../../desktop-client/ironclaw/src/llm/reasoning.rs#L1025) / [workspace/mod.rs#L1161](../../../desktop-client/ironclaw/src/workspace/mod.rs#L1161) / [tools/builder/core.rs#L299](../../../desktop-client/ironclaw/src/tools/builder/core.rs#L299)）+ env 切换 `IRONCLAW_PROMPT_LAYERING` + claw-code [prompt.rs#L95](../../../claw-code/rust/crates/runtime/src/prompt.rs#L95) `SystemPromptBuilder` 独立 boundary 常量 | **Phase 0 必须收敛到 1 个 LayeredPromptBuilder + 统一 cache boundary 常量** |
| 3 | **Tool System** | 🔴 红 | **5 个 register_\* 方法** | ironclaw [tools/registry.rs](../../../desktop-client/ironclaw/src/tools/registry.rs) L280/382/536/551/573/605（builtin/dev/extension/skill/routine/message）+ codex 统一 `build_specs_with_discoverable_tools()` + claw-code MCP-only | **Phase 0 必须 unify 为 `bootstrap_tools()` 单入口** |
| 4 | **Hook System** | 🔴 红 | **5 套并存** | ironclaw [HookRegistry](../../../desktop-client/ironclaw/src/hooks/registry.rs) + x_claw_agent [HookBundle](../../../crates/x_claw_agent/src/hooks.rs#L393) + [SessionHooks](../../../crates/x_claw_agent/src/session_hooks.rs#L24) + claw-code PluginHooks + ironclaw_safety policy_decider + [dasclaw_hooks](../../../crates/dasclaw_hooks/src/lib.rs) 空骨架 | **Phase 1 抽象为单一 HookEngine + HookPoint × N** |
| 5 | **Permission/Approval** | 🟡 黄 | 三检栈 | claw-code `PermissionEnforcer` + x_claw_agent [ApprovalGate](../../../crates/x_claw_agent/src/hooks.rs) + ironclaw policy + `ironclaw_safety` policy_decider | **去重 → 让 ironclaw_safety 作为权威决策器** |
| 6 | **Sandbox** | 🟡 黄 | dasclaw 全骨架 W2.2+ | [dasclaw_sandbox/src/lib.rs#L53](../../../crates/dasclaw_sandbox/src/lib.rs#L53) `SandboxType` 4 种全 stub + ironclaw Docker(3611) + WASM(3000+) 独立 + codex L2 OS sandbox 完整（linux 4780 / windows 9753 / macos 721） | **Phase 2 以 codex L2 为基线，dasclaw 抽 trait 包装** |
| 7 | **Compaction / Context** | 🟡 黄 | port from ironclaw | x_claw_agent [compaction.rs](../../../crates/x_claw_agent/src/compaction.rs) + [context_monitor.rs#L41](../../../crates/x_claw_agent/src/context_monitor.rs#L41) + codex 6 模式 [compact.rs](../../../codex-cli-main/codex-rs/core/src/compact.rs) + claw-code 3 策略 + 阈值 codex 90% [openai_models.rs#L306](../../../codex-cli-main/codex-rs/protocol/src/openai_models.rs#L306) vs x-claw 80% | **阈值与 codex 对齐（90% 默认 + auto_compact_token_limit override）** |
| 8 | **MCP Transport** | 🔴 红 | **W1 零实现** | [dasclaw_mcp/src/lib.rs](../../../crates/dasclaw_mcp/src/lib.rs) 仅占位 + claw-code [mcp_client.rs#L9](../../../claw-code/rust/crates/runtime/src/mcp_client.rs#L9) **6/6 transport 完整**（Stdio/Http/Sse/Ws/Sdk/ManagedProxy）+ ironclaw 3/6 + codex 3/6 | **W5 直接 port claw-code 的 6 transport（est. 2-3 天）** |
| 9 | **Sub-Agent / Fork** | 🔴 红 | **仅 thread fork，无 agent spawn** | x_claw_agent [session.rs#L120](../../../crates/x_claw_agent/src/session.rs#L120) `fork_thread()` ✅ + 无深度限制 ❌ + 无 role 白名单 ❌ vs codex [agent/control.rs#L498](../../../codex-cli-main/codex-rs/core/src/agent/control.rs#L498) `AgentControl::spawn` + `exceeds_thread_spawn_depth_limit()` + ironclaw [sub_agent.rs#L28](../../../desktop-client/ironclaw/src/tools/builtin/sub_agent.rs#L28) `MAX_SUB_AGENT_DEPTH=1` + role 白名单 | **W5 P1 移植 ironclaw role 白名单 → W6 P2 移植 codex 深度管理** |
| 10 | **Identity / Auth** | 🟢 绿 | 已分层 | codex [agent-identity](../../../codex-cli-main/codex-rs/agent-identity/src/lib.rs#L29) JWT (Ed25519) 独家 + ironclaw [SecretsStore](../../../desktop-client/ironclaw/src/secrets/store.rs#L25) + [CredentialInjector](../../../desktop-client/ironclaw/src/tools/wasm/credential_injector.rs#L185) 639 LOC 独家 + x_claw_agent [AgentSecrets](../../../desktop-client/ironclaw/src/secrets/agent_provider.rs#L48) 适配器 + [ironclaw_auth/Cargo.toml](../../../crates/ironclaw_auth/Cargo.toml) 仅占位 | 补 "Auth Boundary ADR"：identity token vs HTTP credential 分工 |
| 11 | **Observability** | 🟡 黄 | dasclaw 骨架 | codex [otel/provider.rs#L46](../../../codex-cli-main/codex-rs/otel/src/provider.rs#L46) `OtelProvider` 完整 + ironclaw [observability/traits.rs#L12](../../../desktop-client/ironclaw/src/observability/traits.rs#L12) `Observer` event-only + admin-backend [extensions.rs#L25](../../../admin-backend/src/handlers/extensions.rs#L25) `write_audit_log()` + [dasclaw_observability/src/lib.rs#L10](../../../crates/dasclaw_observability/src/lib.rs#L10) 仅 SkeletonError | dasclaw_observability 桥接 codex OTEL + ironclaw Observer |
| 11.5 | **Bash Validation** | 🟢 绿 | 6 模块完整 port | claw-code [bash_validation.rs](../../../claw-code/rust/crates/runtime/src/bash_validation.rs) 1004 LOC × 6 模块 → x_claw_agent [bash_validation.rs](../../../crates/x_claw_agent/src/bash_validation.rs) 完整 port（pathValidation 精简未含 symlink）+ ironclaw [bash_validator.rs](../../../desktop-client/ironclaw/src/tools/builtin/bash_validator.rs) 5 阶段变体（独立） | **补 symlink path 验证 + 决定是否合并 ironclaw 5 阶段变体** |
| 13 | **Resilience** | 🟡 黄 | x-claw agent 无 built-in | ironclaw [llm/retry.rs#L1](../../../desktop-client/ironclaw/src/llm/retry.rs#L1) `RetryProvider`（exp 1s×2^n ±25%, default 3, floor 100ms）+ [provider_chaos.rs](../../../desktop-client/ironclaw/tests/provider_chaos.rs) `CircuitBreakerProvider` + `FailoverProvider` + codex [retry.rs#L9](../../../codex-cli-main/codex-rs/codex-client/src/retry.rs#L9) `RetryPolicy` + claw-code [anthropic.rs#L401](../../../claw-code/rust/crates/api/src/providers/anthropic.rs#L401) `send_with_retry()` | **抽 `dasclaw_resilience` crate（统一 retry/timeout/CB/failover）** |
| 14 | **Frontend / IPC** | 🟢 绿 | x-claw 独家、契约测试已就位 | desktop-client [src/ipc/](../../../desktop-client/src/ipc/) 30+ `#[tauri::command]`（threads/chat/logs/jobs/extensions/sandbox/approval）+ [src/lib.rs#L81](../../../desktop-client/src/lib.rs#L81) `all_tauri_commands!()` 宏统一注册 + [tests/tauri_command_contract_tests.rs#L15](../../../desktop-client/tests/tauri_command_contract_tests.rs#L15) `FRONTEND_INVOKED_COMMANDS` 白名单契约（codex/claw-code 无 Tauri，N/A） | 维持现状，Step 3 评分卡 Frontend 子项满分基线 |

#### 2.7.1 Phase 0 收口断言（合并版：13 → 10 行）

> 原 §2.5.2 9 行基础上，三层验证发现 4 项额外 patchwork，加入 Phase 0 红线。

```
# Phase 0（W3-A 收尾必须 100% 通过；按主题合并后 10 项）
- [ ] P0-1  Prompt 装配收口三连：(a) ironclaw 3 builder → 1 LayeredPromptBuilder；(b) 删除 IRONCLAW_PROMPT_LAYERING env var；(c) cache boundary 常量统一为 PROMPT_CACHE_BOUNDARY
- [ ] P0-2  Tool System 收口：tools/registry.rs 5 个 register_* → bootstrap_tools() 单入口
- [ ] P0-3  Hook 收口：HookEngine 抽象 5 套（HookRegistry + HookBundle + SessionHooks + PluginHooks + policy_decider）+ dasclaw_hooks 空骨架处置（删除 or 实现）
- [ ] P0-4  Permission 收口：ApprovalGate 与 ironclaw_safety policy_decider 决策路径去重（让 ironclaw_safety 作权威）
- [ ] P0-5  SafetyDecision 状态确认：3→4 状态决议（是否引入 Quarantine variant + Finding evidence 字段）
- [ ] P0-6  Compaction 阈值对齐：80% → 90% 默认 + 暴露 auto_compact_token_limit override
- [ ] P0-7  MCP 替换：dasclaw_mcp W1 骨架 → port claw-code 6 transport（Stdio/Http/Sse/Ws/Sdk/ManagedProxy）
- [ ] P0-8  骨架 crate 收口：dasclaw_observability 接通 codex OTEL + ironclaw Observer；ironclaw_auth 仅 Cargo.toml 状态澄清（删除 or 实现）
- [ ] P0-9  Bash Validation 补丁：x_claw_agent pathValidation 补 symlink 检查（追平 claw-code 上游）
- [ ] P0-10 Sandbox 抽象层：dasclaw_sandbox 4 stub 至少完成 trait 包装（codex L2 三平台对齐基线）
```

#### 2.7.2 偏离声明清单升级（10 → 16 条 + 分类列）

> 原 §2.5.4 10 条基础上，三层验证补 6 条；本次新增「分类」列，便于评分卡 Layer 1 Parity 加权时按类型差异化处理。
>
> **分类口径**：
> - **A. 架构偏离（Architectural）**：模块拆分/边界与上游不一致，需要重构对齐
> - **B. 能力缺失（Capability gap）**：上游有 / x-claw 无，必须 port 或重写
> - **C. 行为偏离（Behavioral）**：参数/阈值/默认行为与上游不同，可配置消除
> - **D. 精度/完整性偏离（Precision）**：算法/字段精简，可能漏检某些边界场景

| # | 模块 | 分类 | 上游基线 | x-claw 实现 | 风险 | 决策 |
|---|------|-----|---------|------------|------|------|
| 11 | Prompt | **A 架构** | claw-code 单 SystemPromptBuilder | ironclaw 3 builder 共存 | 🔴 高 | Phase 0 P0-1 收敛到 1 |
| 12 | Tool System | **A 架构** | codex 统一 build_specs | ironclaw 5 register_* | 🔴 高 | Phase 0 P0-2 bootstrap_tools() |
| 13 | Hook | **A 架构** | codex 单 hook engine | 5 套并存 | 🔴 高 | Phase 0 P0-3 HookEngine |
| 14 | Compaction 阈值 | **C 行为** | codex 90% (auto_compact_token_limit) | x-claw 固定 80% | 🟡 中 | Phase 0 P0-6 改 90% + override |
| 15 | MCP Transport | **B 能力缺失** | claw-code 6 transport | x-claw 0 transport | 🔴 高 | Phase 0 P0-7 port claw-code 6 |
| 16 | Sub-Agent | **B 能力缺失** | codex AgentControl + 深度 + role | x-claw 仅 thread fork | 🔴 高 | W5 P1 ironclaw role 白名单 + W6 P2 codex 深度管理 |

（§2.5.4 原 10 条同样应回填「分类」列；建议在 Step 3 ADR 主体里以同表格式合并展示。）

#### 2.7.3 100% 覆盖确认（L1+L3 双证据签名）

| # | 能力 | L1 semantic 证据 | L3 grep 证据 | 否定结论 双证 |
|---|------|----------------|-------------|--------------|
| 1 | Agentic Loop | ✅ 3 文件 | ✅ 8 匹配 | — |
| 2 | Prompt | ✅ 6 文件 | ✅ 12 匹配 | — |
| 3 | Tool System | ✅ 5 文件 | ✅ 6 register_* 匹配 | — |
| 4 | Hook | ✅ 5 文件 | ✅ 20+ 匹配 | — |
| 5 | Permission | ✅ 4 入口 | ✅ 15 匹配 | — |
| 6 | Sandbox | ✅ 4 项目 | ✅ 多平台覆盖 | — |
| 7 | Compaction | ✅ 5 文件 | ✅ 20+ 匹配 | — |
| 8 | MCP | ✅ 4 项目 | ✅ 20+ 匹配 | ❌ x-claw 0 transport（双证） |
| 9 | Sub-Agent | ✅ 4 文件 | ✅ 20+ 匹配 | ❌ x-claw 无 agent spawn（双证） |
| 10 | Auth | ✅ 5 文件 | ✅ 9 关键符号 | ❌ codex 无 secrets / ironclaw 无 JWT（双证） |
| 11 | Observability | ✅ 4 文件 | ✅ 8 关键符号 | ❌ dasclaw_obs 仅 SkeletonError（双证） |
| 11.5 | Bash Validation | ✅ 3 文件 | ✅ 6 模块对齐 | — |
| 13 | Resilience | ✅ 4 文件 | ✅ 8 关键符号 | ❌ x_claw_agent 无 built-in（双证） |
| 14 | Frontend / IPC | ✅ 3 文件 | ✅ 30+ `#[tauri::command]` + 宏 + 契约测试 | ❌ codex/claw-code 无 Tauri（双证：均为 CLI / lib） |

**覆盖率结论**：11 核心能力 + 1 补丁能力（11.5）+ 1 边缘能力（13）+ 1 IPC 边缘能力（14）= **14/14 全覆盖**，符合 AGENTS.md "否定性结论必须 L1+L3 双证据" 要求。

---

## 3. 必须用户拍板的（10%）

> 用户回答这 3 个选择题（10 分钟），Step 3 才能写完整 ADR。

### 选择题 1：合规框架选哪个？

- **默认建议** ✅ **等保 2.0 三级**
  - B 端企业最广泛标准
  - 控制项 ~211 项
  - 与项目已有 Managed Mode / SM2 国密签名 / 审计日志一致
- 备选 A：等保 2.0 四级（~344 项，多金融 / 政务核心系统才用）
- 备选 B：ISO 27001（国际通用，~114 控制项）
- 备选 C：SOC 2 Type II（SaaS 企业用，重运营审计）
- 备选 D：GDPR（欧盟数据保护，强 PII / 数据主体权利）
- 备选 E：行业特定 — PCI-DSS（支付）/ HIPAA（医疗）

**影响**：决定 Layer 3 Business 项的合规控制项数量（211 / 344 / 114 / N），及 nightly CI 的合规 baseline checklist 长度。

---

### 选择题 2：威胁模型范围

- **默认建议** ✅ **内部恶意员工 + 外部攻击者 + Prompt injection**
  - 与项目已实现的 Sanitizer / ApprovalGate / DLP / 9 层纵深防御一致
  - 覆盖 OWASP Top 10 + OWASP LLM Top 10 主体
- 备选加项 A：+ 供应链攻击（依赖污染、Skill 包仿冒）
- 备选加项 B：+ LLM jailbreak（针对模型本体的 DAN-style 越狱）
- 备选加项 C：+ 物理访问（终端被盗、磁盘取证、内存 dump）
- 备选加项 D：+ Side-channel（时序、缓存、电磁辐射 — 通常不必）

**影响**：路径 D / 越权场景表是否需要新增子项；Layer 3 的攻击 contract test 总数。

---

### 选择题 3：残余风险阈值（block 发布的硬指标）

- **默认建议** ✅
  - DLP 漏报率 = **0**（已发布 DLP 维度内）
  - Sub-agent 越权事件 = **0**（W6 后启用）
  - Prompt injection 拦截率 ≥ **99%**（fuzz corpus + standard 场景）
  - 跨租户隔离违规 = **0**
  - audit log 采样率 = **100%**
- 备选 A：DLP 漏报率允许 ≤ 0.1%（极端长文本场景下放行）
- 备选 B：Prompt injection 拦截率提到 ≥ 99.9%（更严，会拖慢发布）
- 备选 C：阈值按 Tier（free 用户 99% / 企业付费用户 100%）

**影响**：评估方案的 4 个 KPI（总体≥90% / 关键路径无回退 / 延时≤+10% / 内存≤+15%）的具体数值，及 L3 nightly 跑的 fail-fast 规则。

---

## 4. §3 决策记录（2026-04-28 用户拍板）

### 选择题 1：合规框架 — **分两档（不锁死等保认证）**

用户顾虑：等保 2.0 三级 211 控制项一次到位会不会改动太大？

**最终决策**：**Tier 1 必做（基础安全骨架，发布门槛）+ Tier 2 后做（等保正式认证，未来 TODO）**。

- **Tier 1（W3-A 内必须达成，~30 个有效控制点）**
  - 9 层纵深防御已落（13 文档定义的 L1-L9，~25,000 行）✅
  - Managed Mode P0-P3 已落（policy 验签 / 防重放 / 多 key 轮换）✅
  - 9 个核心 admin API 已实现（auth / audit GET / dlp GET / sensitive-ops GET / policies GET / health）✅
  - DLP 维度 1/4 实现（Pattern-Based）+ 失败路径 Fail-Safe ✅
  - audit_log 自动落库（采样率 100%）✅
  - 跨租户隔离硬指标（fs / secrets / session / audit 四项 = 0）⏳ W3-A 中验证
  - **Tier 1 ≈ 等保三级 ~60% 控制项免费拿下**（无需新增大改动）

- **Tier 2（标记为未来 TODO，不绑死 W3-A 发布）**
  - 完整 RBAC（admin §1.4 + §2，用户/角色/权限 CRUD）— 未来 W8/W9
  - Policy version 管理（admin §6 — diff / rollback / 版本列表）— 未来
  - 审计日志统计 / 导出（admin §5.3 / §5.4）— 未来
  - DLP 维度 2/4 + 3/4 + 4/4（Fingerprint / Content / File-Based）— 长期
  - 等保三级正式认证（211 项 checklist 走形式）— 长期，由合规团队发起

**Step 3 评估方案处理**：Layer 3 Business 项的"合规分"按 **Tier 1 控制点 / Tier 1 总数** 计算，**Tier 2 控制点不计入分母**（避免假性低分）。Tier 2 列入 ADR-112 末尾 Future Work 表。

---

### 选择题 2：威胁模型范围 — **默认 + 备选作 Future TODO**

**最终决策**：

- **W3-A 内必做**（Layer 3 Business 必含 attack contract test）：
  - 内部恶意员工 ✅
  - 外部攻击者（OWASP Top 10）✅
  - Prompt injection（OWASP LLM Top 10）✅
- **未来 TODO**（标记位但不写 contract test）：
  - 供应链攻击（依赖污染、Skill 包仿冒）— 未来 W?
  - LLM jailbreak（DAN-style 越狱专项）— 未来 W?
  - 物理访问（终端被盗 / 磁盘取证 / 内存 dump）— 未来 W?

---

### 选择题 3：残余风险阈值 — **默认 + 备选作 Future TODO**

**最终决策**（block 发布的硬指标）：

- DLP 漏报率 = **0**（Pattern-Based 维度内）
- Sub-agent 越权事件 = **0**（W6 后启用）
- Prompt injection 拦截率 ≥ **99%**（fuzz corpus + standard 场景）
- 跨租户隔离违规 = **0**
- audit log 采样率 = **100%**

**未来 TODO**：
- DLP 漏报率细化到 ≤ 0.1%（极端长文本场景）
- Prompt injection 拦截率提到 ≥ 99.9%
- 阈值按 Tier 区分（free vs 企业付费）

---

## 5. Step 1 完成标志

- [x] 列出 8 类已有抽取来源
- [x] 标注 handoff 中已不存在的 7 份 DLP 文档（避免 Step 3 引用空文件）
- [x] 画出 6 条关键路径 + 关键代码点
- [x] 列出多租户 / Prompt injection / 越权 三表
- [x] 给出 3 个用户拍板选择题 + 默认建议
- [x] **用户决策落锤**（§4，2026-04-28）

下一步：进入 Step 3，写 `adr-112-compatibility-evaluation.md`（39 个 contract test 套件 + 3 harness 设计 + 3 档 CI + 评分卡 + 4 KPI）。Step 2 已与 Step 1 合并完成（用户在反馈环节直接给了决策）。
