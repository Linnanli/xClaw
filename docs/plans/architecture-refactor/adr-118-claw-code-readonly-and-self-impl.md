# ADR-118 — claw-code 子仓只读化 + LLM provider 中立迁出

> **Status**: Accepted (W4 milestone, supersedes ADR-117 D5/D7)
> **Date**: 2026-05-02
> **Authors**: x-claw 架构组
> **Supersedes (partial)**: [ADR-117 v1.0 D5/D7](adr-117-p0c-prompt-builder-unification.md) — 「在 claw-code 子仓内删除 prompt.rs / rusty-claude-cli / tools / compat-harness / mock-anthropic-service」的方向被推翻
> **Related**: [31-target-architecture.md L139](31-target-architecture.md)（已陈述「`claw_code_provider.rs` 1334 行... 中立迁出」但 32 未具体落 task）、[32-execution-plan.md W6](32-execution-plan.md)
> **Confirms**: claw-code 子仓本身**保留为只读参考库**（不解绑 submodule，不删原代码），主仓**自实现** LLM provider，**删除** `desktop-client/ironclaw` 对 `claw-code-api` 的 path-dep

---

## 1. 背景与问题陈述

### 1.1 ADR-117 v1.0 的方向错误

ADR-117 P0-C 的核心目标「提示词单源 = 主仓 `LayeredPromptBuilder`」**已通过 PR #142 完成**（删除 `desktop-client/ironclaw` 中的 `static_hash` / legacy 字段）。但 ADR-117 v1.0 的 D5/D7 工作项把「删除 claw-code 子仓内的 `prompt.rs` / `rusty-claude-cli` / `tools` / `compat-harness` / `mock-anthropic-service`」作为收尾步骤，方向错误：

- **claw-code 是 fork-only 参考库**（已偏离 anthropics 上游 42 个 fork-only commit），其角色是**只读参考材料**，不应作为我们能修改的实施对象
- 在子仓内删除代码 = 把参考材料修改成「我们的精简版」，破坏了「只读参考」的清晰边界
- 真正应该做的是：**主仓自实现** 等价能力，**删除主仓对子仓的 path-dep**，让 claw-code 子仓保持原貌

### 1.2 ADR-117 v1.1 修订（PR #143）的进一步偏离

PR #143 试图扩大 D7 删除半径至「档 1.5」（5 项 crate 整删），仍然走在「在子仓内删」这个错误方向上，因此被 close。子仓上的 c36c0ef 删除 commit 已通过 db8ff4d **revert**，子仓远端 myfork/x-claw 现状 = e88762a 等价（原代码完整）。

### 1.3 真正的问题

实测父仓对 claw-code 的依赖图：

```text
$ grep -rn "claw-code\|claw_code" desktop-client/ironclaw/Cargo.toml
desktop-client/ironclaw/Cargo.toml:149  claw-code-api = { path = "../../claw-code/rust/crates/api", package = "api" }
desktop-client/ironclaw/Cargo.toml:235  claw-code-llm = []   # 历史 feature flag, 已 no-op

$ grep -rn "claw_code_api::" desktop-client/ironclaw/src/ --include="*.rs" | wc -l
# 约 30 处，全部集中在 1 个文件
$ grep -rln "claw_code_api::" desktop-client/ironclaw/src/ --include="*.rs"
desktop-client/ironclaw/src/llm/claw_code_provider.rs
```

**唯一实质性生产依赖路径**：

```
desktop-client/ironclaw
   └── claw_code_provider.rs (~1300 LOC)
       └── claw-code-api (= claw-code/rust/crates/api)
           ├── AnthropicClient (= ApiClient)         — Anthropic streaming HTTP client
           ├── OpenAiCompatClient / OpenAiCompatConfig — OpenAI / xAI / DashScope / Kimi / Ollama 适配
           ├── ProviderClient enum                    — 统一发送入口
           ├── AuthSource                             — ApiKey / BearerToken / 双轨
           ├── InputContentBlock / InputMessage / OutputContentBlock  — 消息类型
           ├── ToolDefinition / ToolChoice / ToolResultContentBlock   — Tool calling 类型
           ├── MessageRequest / MessageResponse / Usage               — 请求/响应模型
           ├── resolve_model_alias / detect_provider_kind / ProviderKind  — 模型别名 + 厂商探测
           └── ApiError                                                — 统一错误类型
```

`claw-code-api` 在子仓内还间接消费 `runtime` + `telemetry` 的少量类型（`SessionTracer` 等），形成 5 crate 互依赖簇。

### 1.4 三仓 LLM provider 血脉关系（W6-C 决策的关键背景）

为澄清「desktop-client/ironclaw 的 LLM provider 是不是从 ironclaw-main 直接复制」这个常被问到的问题，三层验证如下：

| 验证 | 命令 | 结果 |
|---|---|---|
| ironclaw-main 是否依赖 claw_code | `grep -rn "claw_code\|claw-code" ironclaw-main/Cargo.toml ironclaw-main/src/` | **0 命中** |
| desktop-client 是否还依赖 rig-core | `grep -n "^rig\|rig-core" desktop-client/ironclaw/Cargo.toml` | 仅 1 行注释「Phase 2 Step I 之前用来切换 rig-core / claw-code-api 两套 LLM」 |
| ironclaw-main 用 rig 的规模 | `grep -rln "use rig\|rig::" ironclaw-main/src/` | 2 个 .rs 文件（`mod.rs` + `rig_adapter.rs`）+ Cargo 1 处依赖 |
| 文件清单 diff | `diff <(ls .../desktop-client/ironclaw/src/llm/*.rs) <(ls .../ironclaw-main/src/llm/*.rs)` | desktop 多 `claw_code_provider.rs` + `prompt/` + `schema_utils.rs`；ironclaw-main 多 `anthropic_oauth.rs` + `rig_adapter.rs` + `runtime.rs` |

**结论**：desktop-client/ironclaw 的 LLM provider **不是从 ironclaw-main 简单复制**。共同祖先确实是 ironclaw-main，但在 fork 之后做过一次重大架构替换 —— **Phase 2 Step I**：

```
ironclaw-main/src/llm/                  desktop-client/ironclaw/src/llm/
─────────────────────────────────       ──────────────────────────────────
LlmProvider trait (相同)        ────►   LlmProvider trait (相同，已下沉到 x_claw_agent)
rig::CompletionModel (rig-core)         claw_code_api::ProviderClient
rig_adapter.rs (适配 rig→Llm)   ────►   claw_code_provider.rs (适配 claw-code-api→Llm)
                                        prompt/ (LayeredPromptBuilder，新增)
                                        schema_utils.rs (从 rig_adapter 抽出)
nearai_chat / openai_codex /            nearai_chat / openai_codex /
codex_chatgpt / github_copilot /        codex_chatgpt / github_copilot /
gemini_oauth / bedrock                  gemini_oauth / bedrock
(独立 reqwest 直连，不走 rig)           (独立 reqwest 直连，与 ironclaw-main 几乎相同)
                  ⬆同源⬆                                  ⬆同源⬆
```

「Anthropic + OpenAI + xAI + Kimi + DashScope + Groq + OpenRouter + Tinfoil + Ollama」这 9 种协议在 ironclaw-main 时代走 **rig-core CompletionModel trait**，desktop fork 之后改为走 **claw-code-api ProviderClient enum**。其他 6 个独立 backend（nearai / codex / copilot / gemini / bedrock）的实现两端**几乎一致**（独立 reqwest 直连，未受 Phase 2 Step I 影响）。

**W6-C 的演化路径定位**：

```
ironclaw-main 时代：rig-core (外部 crate)              ← 上游 fork 起点
                       ↓ Phase 2 Step I 重大替换
desktop-client 当前：claw-code-api (子仓 path-dep)     ← 当前状态（W4）
                       ↓ W6-C 二次替换
W6 完成态：dasclaw_llm_provider (主仓自实现 crate)     ← 终态
```

**这告诉我们 W6-C 不是回退到 rig-core 时代，而是把"外部依赖 + 子仓 fork"两层耦合一起卸掉**：从 rig-core（外部 crate 锁定）→ claw-code-api（子仓 fork 锁定）→ dasclaw_llm_provider（主仓自治）。这是架构控制权的逐步收回，不是来回切换。

## 2. 决策

### 2.1 claw-code 子仓 = 只读参考库

| 维度 | 决策 |
|---|---|
| Submodule pointer | **保留**（不解绑，不删 `.gitmodules` 中条目） |
| 子仓代码内容 | **保持原貌**（不删 prompt.rs / rusty-claude-cli / tools 等） |
| 子仓 fork remote | **保留** myfork=`git@github.com:Linnanli/xclaw-claw-code.git`（用于偶尔 cherry-pick 上游修复） |
| 用法定位 | **只读参考材料** — 浏览代码以学习 Anthropic streaming / Tool calling / Prompt cache 等机制，**禁止**作为生产 path-dep 消费 |
| ADR-117 D5/D7 子仓内删除 | **撤回**（已通过子仓 db8ff4d revert 落地） |

### 2.2 主仓自实现 LLM provider crate

**新建** `crates/dasclaw_llm_provider`（暂定名，最终命名以 31 章节 ADR 提议为准），**完全主仓维护**，**不依赖** `claw-code-api`。该 crate 实现：

| 能力 | 来源 / 参考 |
|---|---|
| Anthropic streaming HTTP client | 参考 `claw-code/rust/crates/api/src/providers/anthropic.rs` 实现，**不直接消费**；按主仓需要重新组织接口 |
| OpenAI-compat 客户端（xAI / DashScope / OpenAI / Kimi / Ollama） | 参考 `claw-code/rust/crates/api/src/providers/openai_compat.rs` |
| 消息 / Tool 类型 | 参考 `claw-code/rust/crates/api/src/types.rs`，可直接抄 schema 但 type 名归属主仓 |
| 模型别名表 | 参考 `claw-code/rust/crates/api/src/providers/mod.rs` 的 `resolve_model_alias` |
| Prompt cache 1h marker | 参考 `claw-code/rust/crates/api/src/prompt_cache.rs`（已部分通过 ADR-117 P0-C `LayeredPromptBuilder` 接管） |
| 错误类型 | 主仓自定义（`crate::ApiError` 或映射到 `LlmError`） |

### 2.3 desktop-client/ironclaw 切换路径

1. **新建** `crates/dasclaw_llm_provider`，pub re-export 等价类型
2. **重写** `desktop-client/ironclaw/src/llm/claw_code_provider.rs`：
   - 改名为 `dasclaw_llm_provider.rs` 或保持原文件名内部切换
   - `use claw_code_api::{...}` → `use dasclaw_llm_provider::{...}`
   - 内部映射逻辑（`build_chat_message_request` / `map_message_response` 等）保留
3. **删除** `desktop-client/ironclaw/Cargo.toml:149` 的 `claw-code-api = { path = ".." }`
4. **删除** `desktop-client/ironclaw/Cargo.toml:235` 的 `claw-code-llm = []` no-op feature
5. 验证：`cargo check -p ironclaw` + `cargo nextest run -p ironclaw` 通过

### 2.4 子仓 path-dep 终态

完成 §2.3 后，主仓**整个 workspace 不再有任何 path 指向 `claw-code/`**。`Cargo.toml:33` 的 `exclude = ["claw-code"]` 保持不变（已对，编译期不参与）。

### 2.5 与 ironclaw 现有 LLM provider 的关系（**不冲突**，平行架构）

> 用户提问澄清：「ironclaw 原本的 llm provider 是否和 claw_code_provider.rs 冲突？」答：**不冲突**。

`desktop-client/ironclaw/src/llm/` 目录下当前共有 **8+ 个 provider 实现**，按 backend 平行分布（参见 `desktop-client/ironclaw/src/llm/CLAUDE.md`）：

| Provider 文件 | 后端 | 是否依赖 claw_code_api |
|---|---|---|
| `claw_code_provider.rs` (`ClawCodeLlmProvider`) | **Anthropic / OpenAI / Ollama / OpenAI-compat**（Phase 2 Step I 之后唯一生产路径） | ✅ **是**（W6 切换为 `dasclaw_llm_provider`） |
| `nearai_chat.rs` (`NearAiChatProvider`) | NEAR AI | ❌ 否（独立 reqwest） |
| `github_copilot.rs` (`GithubCopilotProvider`) | GitHub Copilot | ❌ 否（独立 reqwest，Copilot API 与 OpenAI 不兼容） |
| `openai_codex_provider.rs` (`OpenAiCodexProvider`) | OpenAI Codex（ChatGPT subscription） | ❌ 否（独立 OAuth + Responses API） |
| `bedrock.rs` (feature gated) | AWS Bedrock | ❌ 否（aws-sdk-bedrockruntime） |
| `gemini_oauth.rs` (`GeminiOauthProvider`) | Google Gemini | ❌ 否 |
| `failover.rs` / `smart_routing.rs` / `circuit_breaker.rs` / `recording.rs` / `response_cache.rs` / `retry.rs` / `token_refreshing.rs` | 装饰器（包装其他 provider） | ❌ 否 |

**结论**：W6 任务组 C **只动 `claw_code_provider.rs` 一个文件**，其它 7 个 backend 的实现完全不受影响。`ClawCodeLlmProvider` 这个 type 的对外 `LlmProvider` trait 实现保持不变，仅替换其内部使用的「上游 HTTP client」依赖（`claw_code_api::Client` → `dasclaw_llm_provider::Client`）。从 `mod.rs` 的 wire 点（`create_llm_provider_from_registry_config` 调用 `claw_code_provider::ClawCodeLlmProvider::from_registry_config`）来看，**调用方完全无感知**。

> 命名建议（W6 实施时定）：`ClawCodeLlmProvider` 这个 type 名也建议同步改为 `DasclawLlmProvider` 或 `AnthropicOpenAiCompatProvider`，避免暗示对 claw-code 的依赖。但这是文件 / type rename，不影响行为。

---

## 3. 工作量与影响

### 3.1 LOC 估算

| 项 | LOC | 来源 |
|---|---|---|
| 新增 `crates/dasclaw_llm_provider` | ~3,500-4,500 | 参考 claw-code/rust/crates/api 总 ~5k LOC，去除 prompt_cache 部分（已在主仓） + http_client 简化 |
| 修改 `desktop-client/ironclaw/src/llm/claw_code_provider.rs` | ~1,300（保留映射逻辑，仅改 import） | 当前文件大小 |
| 修改 `desktop-client/ironclaw/Cargo.toml` | ~3 行 | 删 path-dep + 删 no-op feature + 加 dasclaw_llm_provider 依赖 |
| **总增量** | **~3.5-4.5k LOC（净新增）** | |

注：claw-code 子仓内代码 0 行变化（保留只读）。

### 3.2 与现有 Wave 的关系

参照 [32-execution-plan.md](32-execution-plan.md) v2.4：

- **W1-W3**：不影响（基线收敛 / Sandbox / Hooks）
- **W4-W5**：claw-code 治理 6 件套 + MCP 6 transport + bash_validation 等的「主仓自实现」工作**继续按原计划走**，参考 claw-code 源码但不消费
- **W6（客户端外壳整合）**：本 ADR 新增 task **W6.7 — LLM provider 中立迁出** 列入此 Wave（自然融入 desktop-client 整合）。详见 §4.2。
- **W7-W9**：不影响

### 3.3 负面真相

- 写 LLM client 涉及 SSE 解析 / 鉴权头 / OAuth 双轨 / Tool calling JSON shape 兼容，**不是简单复制粘贴** — 必须配契约测试（mock HTTP server）保证与 claw-code-api 行为等价
- claw-code 子仓上游有偶发修复（reasoning_content 字段、anthropic-beta header 更新等），主仓自实现后**不再自动跟进**，需偶尔人工回看子仓 diff 判断是否同步

---

## 4. ADR-117 状态调整

### 4.1 ADR-117 P0-C Done 部分（保留）

- **D8.2 + D8.3**: cache refactor — 已通过 PR #141 完成（merged b8049f4c）
- **D6 + D8.4**: 删除 desktop-client/ironclaw 的 static_hash / legacy 字段 — 已通过 PR #142 完成（merged 51e14910，closes #130）
- **核心目标「提示词单源 = LayeredPromptBuilder」** — 已达成

### 4.2 ADR-117 D5 + D7 部分（撤回）

| 工作项 | v1.0 计划 | ADR-118 决策 |
|---|---|---|
| D5 — 删 `claw-code/rust/crates/runtime/src/prompt.rs` | 在子仓内删 | **撤回**（保留作只读参考） |
| D7 — 删 `claw-code/rust/crates/{rusty-claude-cli, tools, compat-harness, mock-anthropic-service}` | 在子仓内删 | **撤回**（保留作只读参考） |
| **新增 (ADR-118 §2.3)** | — | **删 desktop-client/ironclaw → claw-code-api 的 path-dep** |

### 4.3 ADR-117 文档头部状态

ADR-117 文档**保留 Accepted**（核心目标已达成），但需在头部加 v1.2 备注：

> v1.2 (2026-05-02): D5 + D7 子仓内删除工作项已撤回，由 [ADR-118](adr-118-claw-code-readonly-and-self-impl.md) 取代为「主仓自实现 + 删 path-dep」。本 ADR 其余部分（D6 / D8.2 / D8.3 / D8.4 / 提示词单源核心目标）保持有效。

---

## 5. PR 计划

### 5.1 本 PR（ADR-118 起草 + 32 修订 + ADR-117 v1.2 备注）

仅文档：
- 新增 [adr-118-claw-code-readonly-and-self-impl.md](adr-118-claw-code-readonly-and-self-impl.md)（本文）
- 修订 [32-execution-plan.md](32-execution-plan.md) W6 节加 task W6.7
- 修订 [adr-117-p0c-prompt-builder-unification.md](adr-117-p0c-prompt-builder-unification.md) 头部加 v1.2 备注 + 撤销 §2.2 D5/D7 措辞

### 5.2 后续实施 PR（在 W6 阶段）

- PR-A：新建 `crates/dasclaw_llm_provider` 骨架 + Anthropic / OpenAI-compat client 主仓自实现 + 契约测试
- PR-B：`desktop-client/ironclaw/src/llm/claw_code_provider.rs` 切到 `dasclaw_llm_provider`
- PR-C：删 `Cargo.toml:149` path-dep + 删 `:235` no-op feature

---

## 6. 验证账本

### 6.1 三层验证（否定性结论）

> 「desktop-client/ironclaw 对 claw-code 的实质性依赖只有 1 个文件 / 1 条 path-dep」

| 层 | 命令 | 结果 |
|---|---|---|
| 1. semantic_search | "claw_code_api consumers in desktop-client" | 仅 `src/llm/claw_code_provider.rs` |
| 2. grep | `grep -rln "claw_code_api::" desktop-client/ironclaw/src/ --include="*.rs"` | 1 个文件 |
| 3. Cargo.toml | `grep -rn "claw-code\|claw_code" desktop-client/ironclaw/Cargo.toml` | 3 行命中：line 149 path-dep（实质）+ line 232/235 no-op feature 注释（非实质） |

### 6.2 子仓回滚验证

- 子仓远端 `myfork/x-claw` 现状 = `db8ff4d`（`Revert "chore: implement ADR-117 D7 cleanup (extended scope)"`）
- 父仓 xClaw 分支 submodule pointer = `e88762a`（未 bump，`git submodule update` 拉到原代码）
- `claw-code/rust/crates/` 完整保留：`api / commands / compat-harness / mock-anthropic-service / plugins / runtime / rusty-claude-cli / telemetry / tools` 9 个 crate

### 6.3 panic 红线

```bash
python3.12 scripts/check_no_panics.py --base origin/xClaw
# 仅文档改动，无生产代码影响
```

---

## 7. 关联文档

- [31-target-architecture.md](31-target-architecture.md) §139（已陈述「私货中立迁出」）
- [32-execution-plan.md](32-execution-plan.md) W6（本 ADR 增补 task W6.7）
- [36-claw-code-capability-inventory.md](36-claw-code-capability-inventory.md)（claw-code 能力盘点 — 视为只读参考材料指南）
- [adr-104-routine-vs-runtime-classification.md](archive/legacy-v1/adr-104-routine-vs-runtime-classification.md) non-goal 列表（CLI/TUI/mock-anthropic-service/compat-harness 永久不进生产）
- [adr-117-p0c-prompt-builder-unification.md](adr-117-p0c-prompt-builder-unification.md) v1.2（D5/D7 撤回标记）

---

## 8. 实施事实账本（基于 desktop-client/ironclaw/src/llm/ 深度剖析）

> 本节为 **W6 实施依据**。基于直接读源（2026-05-02 subagent 全模块剖析）锁定，不是设计建议。所有 type 行号引自 `claw-code/rust/crates/api/src/lib.rs` re-export 表。

### 8.1 ironclaw LLM 子系统 6 个独立后端 provider 矩阵

| Provider 文件 | 主 type | 行数 | 协议 | 鉴权 | streaming | 走 claw_code_api |
|---|---|---|---|---|---|---|
| `claw_code_provider.rs` | `ClawCodeLlmProvider` | 1 351 | Anthropic Messages / OpenAI Chat Completions / xAI / Ollama | ApiKey + OAuth Bearer | ❌（trait 默认）| ✅ **是**（W6-C 切走） |
| `nearai_chat.rs` | `NearAiChatProvider` | 2 286 | OpenAI Chat（NEAR AI 端）| Bearer key/session | ✅ | ❌ |
| `openai_codex_provider.rs` | `OpenAiCodexProvider` | 1 226 | OpenAI Responses API | OAuth JWT | ✅ SSE | ❌ |
| `codex_chatgpt.rs` | `CodexChatGptProvider` | 934 | Responses API（chatgpt.com）| Bearer + 401 refresh | ✅ SSE | ❌ |
| `github_copilot.rs` | `GithubCopilotProvider` | 679 | OpenAI Chat（Copilot 端）| GitHub OAuth → session token | ✅ | ❌ |
| `gemini_oauth.rs` | `GeminiOauthProvider` | 2 617 | Gemini generateContent | Google OAuth + key fallback | ✅ | ❌ |
| `bedrock.rs` (feature) | `BedrockProvider` | 1 419 | Bedrock Converse | AWS IAM | ❌ | ❌ |

**关键反直觉**：`claw_code_provider.rs` 这个名字 **同时承载 Anthropic / OpenAI / xAI / Kimi / DashScope / Groq / OpenRouter / Tinfoil / Ollama 等所有非专属后端**——因为 `claw_code_api::ProviderClient` 是个三分支枚举（`Anthropic / Xai / OpenAi`）。**它不是 Anthropic 专属适配器**。这是命名混淆点，W6-C 完成后建议改名为 `multi_protocol_provider.rs` 或 `dasclaw_provider.rs`。

### 8.2 装饰器栈（外→内，由 `build_provider_chain` 组装）

```
6. RecordingLlm (env IRONCLAW_RECORD_TRACE)
5. CachedProvider (仅 cache complete()，不 cache tool calls)
4. CircuitBreakerProvider (Closed/Open/HalfOpen，5 失败→30s 半开)
3. FailoverProvider (多 provider + per-provider cooldown)
2. SmartRoutingProvider (13 维 complexity scorer，cheap/primary 分流)
1. RetryProvider (指数退避 1/2/4s + jitter)
0. 内层 Provider
```

`TokenRefreshingProvider` 是 `OpenAiCodexProvider` 专用，套在主链之内。**所有装饰器配置都从 `LlmConfig.nearai` 字段读取**（命名遗留——`NearAiConfig` 实际担当通用运行时开关角色，覆盖 retry / circuit_breaker / cache / failover / smart_routing 等全部装饰器）。

### 8.3 W6 任务组 C 必须从 `dasclaw_llm_provider` 迁出的 type / function 完整清单

来自 `claw_code_provider.rs` 行 13–17 的 `use claw_code_api::{...}` 单一导入语句 + 模块内全路径调用：

#### Type（13 个，1:1 复刻）

| 类型 | 形式 | 必保语义 |
|---|---|---|
| `ProviderClient` | `enum { Anthropic(_), Xai(_), OpenAi(_) }` | `send_message` / `stream_message` / `provider_kind()` 调度 |
| `ProviderKind` | `enum { Anthropic, Xai, OpenAi }` | 测试断言用，公开比较语义 |
| `AnthropicClient` | struct | `from_auth(AuthSource)` / `with_base_url` / `send_message` / `stream_message` |
| `OpenAiCompatClient` | struct | `new(api_key, OpenAiCompatConfig)` / `with_base_url` / `send_message` / `stream_message` |
| `OpenAiCompatConfig` | struct + 关联函数 | `::openai()` / `::xai()` / `::dashscope()` 三预设 |
| `AuthSource` | enum 3 变体 | `ApiKey(String) / BearerToken(String) / ApiKeyAndBearer { api_key, bearer_token }` |
| `MessageRequest` | struct（13 字段） | `model / max_tokens / messages / system / tools / tool_choice / stream / temperature / top_p / frequency_penalty / presence_penalty / stop / reasoning_effort` |
| `MessageResponse` | struct | `content: Vec<OutputContentBlock> / usage: Usage / stop_reason: Option<String>` |
| `InputMessage` | struct | `role: String / content: Vec<InputContentBlock>` |
| `InputContentBlock` | enum | `Text { text } / ToolUse { id, name, input } / ToolResult { tool_use_id, content, is_error }` |
| `OutputContentBlock` | enum | `Text / ToolUse / Thinking / RedactedThinking` |
| `ToolDefinition` (alias `ApiToolDefinition`) | struct | `name / description / input_schema(JsonValue)` |
| `ToolChoice` (alias `ApiToolChoice`) | enum | `Auto / Any / Tool { name }` |
| `ToolResultContentBlock` | enum | 至少 `Text { text }` |
| `Usage` | struct | `input_tokens / output_tokens / cache_read_input_tokens / cache_creation_input_tokens` |
| `ApiError` | struct | `to_string()` 含 trace ID + HTTP 状态 + 上游错误体 |

#### Free function（2 个）

- `resolve_model_alias(model: &str) -> String`（"opus" → "claude-opus-4-6"）
- `detect_provider_kind(model: &str) -> ProviderKind`（启发式分类）

#### Client 方法（仅 1 个核心）

```rust
async fn send_message(&self, request: &MessageRequest) -> Result<MessageResponse, ApiError>
```

> 当前 ironclaw 端**没调** `stream_message`（详见 §8.5 风险）。但 `dasclaw_llm_provider` **必须保留** `stream_message` 与 SSE 解析以便后续启用。

#### HTTP / SSE 设施

- `build_http_client() -> Result<reqwest::Client, ApiError>`
- `build_http_client_with(&ProxyConfig) -> Result<reqwest::Client, ApiError>`
- `ProxyConfig` struct
- `parse_frame` / `SseParser`

#### 模型别名表

`MODEL_REGISTRY` const 数组 9 条（`opus / sonnet / haiku → claude-*` + `grok-* → xAI` + `kimi → DashScope`）+ 每条 `ProviderMetadata`。**编译期内嵌**，不能依赖运行时配置。

#### Prompt cache（可选，W6-C 决策）

`PromptCache / PromptCacheConfig / PromptCachePaths / PromptCacheRecord / PromptCacheStats / CacheBreakEvent` + `ProviderClient::with_prompt_cache` / `prompt_cache_stats` / `take_last_prompt_cache_record`。⚠ **当前 ironclaw 端没调用这些**——`RegistryProviderConfig.cache_retention` 字段为它预留。W6-C 可先 stub。

### 8.4 **不需要迁出**的（保留在 ironclaw 内部）

下列是 ironclaw 应用层的「翻译胶水」，与 `dasclaw_llm_provider` 解耦：

- `LlmProvider` trait（属于 ironclaw + x_claw_agent）
- `LlmError` 枚举（应用层错误模型）
- `ChatMessage / CompletionRequest / ToolCompletionRequest`（已在 `x_claw_agent::messages`）
- `build_chat_message_request` / `build_tool_message_request`
- `split_system_and_messages` / `map_user_message` / `map_assistant_message` / `map_tool_result_message` / `map_tool_definition` / `map_tool_choice`
- `map_message_response` / `map_finish_reason` / `map_api_error`
- `from_registry_config` 路由（`RegistryProviderConfig` 是 ironclaw 私有概念）
- `build_anthropic_client` / `build_openai_compat_client` / `build_ollama_client` / `pick_openai_compat_config`
- 全部 35+ 个单元测试（输入/输出契约不变，仅改 import）

### 8.5 W6 实施时顺势处理的架构债（DO NOT FIX 在本 ADR）

仅登记，不在本 PR / W6-C 范围内：

1. **streaming 缺口**：`ClawCodeLlmProvider` 不实现 `supports_streaming = true`，所有走 claw-code 通道的后端（Anthropic / OpenAI / xAI / Kimi / DashScope / Groq / OpenRouter / Tinfoil / Ollama）**当前都是非流式**——这是显著产品缺口。建议 W6-D 或 W7 单独 issue 跟进。
2. **错误归一过粗**：`map_api_error` 把所有 `ApiError` 都归为 `LlmError::RequestFailed`，丢失 `RateLimited`（`Retry-After` 失效）/ `AuthFailed`（401 不能触发 OAuth re-login）/ `ContextLengthExceeded`（被错误判 retryable）。`is_retryable` 用字符串扫描补救，脆弱契约。`dasclaw_llm_provider::ApiError` 设计时**应直接拆强类型变体**便于精确映射。
3. **cost 默认 0**：`ClawCodeLlmProvider::cost_per_token` 在未显式提供 rates 时返回 `(0, 0)`，且 `from_registry_config` / `from_model` **不主动调** `costs::model_cost(&self.resolved_model)`。所有走 claw-code 通道的请求成本统计**默认归零**，与 `NearAiChatProvider` 不对称。
4. **命名遗留**：W6-C 完成后建议把 type 名 `ClawCodeLlmProvider` 改为 `DasclawLlmProvider` 或 `MultiProtocolProvider`（避免暗示对 claw-code 的依赖）；文件 `claw_code_provider.rs` 改为 `dasclaw_provider.rs`。这是 type/file rename，不影响行为。
5. **`NearAiConfig` 命名误导**：实际承担通用运行时装饰器开关（max_retries / circuit_breaker / response_cache / failover / smart_routing），即便 backend ≠ NearAI 也必填。建议 W7 拆出 `LlmRuntimeConfig`。
6. **`is_codex_chatgpt` 隐式状态机**：`RegistryProviderConfig.protocol = OpenAiCompletions + is_codex_chatgpt = true` 这个组合没有强类型表达。建议 W7 在 `ProviderProtocol` 加 `CodexChatGpt` 变体。

### 8.6 落地建议 crate 边界（最小可行）

```
crates/dasclaw_llm_provider/
├── src/
│   ├── lib.rs                        # 全部 pub use
│   ├── error.rs                      # ApiError（强类型变体）
│   ├── http_client.rs                # build_http_client + ProxyConfig
│   ├── types.rs                      # MessageRequest/Response, InputMessage,
│   │                                 # InputContentBlock, OutputContentBlock,
│   │                                 # ToolDefinition, ToolChoice, Usage,
│   │                                 # ToolResultContentBlock
│   ├── providers/
│   │   ├── mod.rs                    # ProviderKind + MODEL_REGISTRY +
│   │   │                             # resolve_model_alias + detect_provider_kind
│   │   ├── anthropic.rs              # AnthropicClient + AuthSource
│   │   └── openai_compat.rs          # OpenAiCompatClient + OpenAiCompatConfig
│   ├── client.rs                     # ProviderClient enum + send/stream dispatch
│   ├── sse.rs                        # SseParser + parse_frame
│   └── prompt_cache.rs               # （可选 stub）
└── Cargo.toml
   依赖：reqwest / serde / serde_json / async-trait / thiserror /
        eventsource-stream / secrecy / chrono
```

迁完后 `desktop-client/ironclaw/src/llm/claw_code_provider.rs` 只需把 `use claw_code_api::{...}` 改成 `use dasclaw_llm_provider::{...}`，35 个单元测试断言不需要改逻辑（除了 `claw_code_api::ProviderKind` → `dasclaw_llm_provider::ProviderKind`）。


