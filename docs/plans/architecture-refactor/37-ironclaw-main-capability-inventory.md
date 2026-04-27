# 37 — IronClaw Main 全量能力清单（路线 B 后端基底）

> **版本**：v1.0 (2026-04-25) · 配套 [35-codex-capability-inventory.md](35-codex-capability-inventory.md)（codex）+ [36-claw-code-capability-inventory.md](36-claw-code-capability-inventory.md)（claw-code）+ [38-desktop-client-ironclaw-fork-inventory.md](38-desktop-client-ironclaw-fork-inventory.md)（嵌入 fork）
> **方法**：严格按 [`AGENTS.md`](../../../AGENTS.md) 三级工具链
> **总规模**：**464,222 LOC**（精确 `find | wc -l`，主目标 ironclaw-main 0.26.0）
> **覆盖率**：≥ 95%（18 大能力域 + 24 migration + Bridge 25k LOC 解构）

---

## 0. 结论先行

| 维度 | 数据 |
|---|---|
| **总 LOC（精确）** | **464,222 LOC**（src 主体 ~292k + crates ~57k + tests/channels-src/tools-src ~115k） |
| **核心 crate 数** | 6（ironclaw_common 2,172 / ironclaw_safety 5,490 / ironclaw_skills 6,304 / ironclaw_engine 30,414 / ironclaw_gateway 2,182 / ironclaw_tui 11,021） |
| **src 顶层模块（按 LOC 排名）** | channels 60,211 / tools 57,003 / llm 33,654 / agent 29,270 / **bridge 25,369** / extensions 15,348 / db 13,977 / config 11,568 / workspace 10,536 / cli 9,900 |
| **Migrations** | 24 版本（V1-V24，含 WASM/owner_id/job_token_budget/users/channel_identities/tool_scope） |
| **路线 B 可移植核心** | ~320k LOC（≈ 69%） |

**最强 Top 5（ironclaw 独家，路线 B 必须满额移植）**：

1. ✅ **Bridge 巨型适配层 25,369 LOC** — v1 dispatcher ↔ engine v2 的粘合剂（router 9,609 + effect_adapter 5,802 + store_adapter 3,351 + auth_manager 1,581 + llm_adapter 1,595）
2. ✅ **ironclaw_engine v2 - 30,414 LOC** — 统一 Thread/Step/Capability 模型，替代 10+ 散落抽象
3. ✅ **多 Channel 体系 60,211 LOC** — Web/CLI/HTTP/Signal/Telegram/Discord/Slack/WhatsApp/Feishu/WASM
4. ✅ **Workspace 多租户隔离 10,536 LOC** + bind-mount + libSQL F32_BLOB + hybrid FTS/vector RRF（k=60）
5. ✅ **Cost Guard + Approvals**（dispatcher.rs 内 + cost_guard_gate.rs 41 LOC）— LLM 预算守卫 + Tool 审批门

**最弱 Top 3（不优先）**：

1. ❌ Crash Dump / Panic Hook — 都弱
2. ⚠️ Sandbox 跨平台 — 仅 Linux Landlock + Docker，缺 macOS Seatbelt / Windows JobObject（codex 强）
3. ⚠️ apply-patch / portable-pty / CodeAct — 全部缺，需从 codex 移植

**对路线 B 的核心价值**：ironclaw-main 是**整个路线 B 后端基底**，提供 channels/bridge/db/workspace/extensions/jobs 完整生产级实现；codex 补 sandbox + tool 协议，claw-code 补 Git 治理 6 件套。

---

## 1. 18 大能力域总览表

| # | 域 | 精确 LOC | 核心模块 | vs codex | vs claw-code | 价值 | 决策 |
|---|---|---:|---|:-:|:-:|:-:|---|
| A | Agent Runtime / Loop | ~8,500 | `src/agent/{dispatcher,agentic_loop,agent_loop,session_manager}.rs` | ⚠️ | ⚠️ | ★★★★★ | **核心移植** |
| B | Tool 系统 | **57,003** | `src/tools/{registry,builtin,builder,wasm,mcp,permissions,execute}` | ⚠️ | ⚠️ 无 WASM | ★★★★★ | **完整移植** |
| C | Sandbox（Docker + bind-mount） | 3,599 | `src/sandbox/{config,manager,container,proxy}` | ❌ codex 三平台更强 | ✅ claw 无 | ★★★★ | **保留 + codex 补 Win/Mac** |
| D | Workspace 多租户隔离 | **10,536** | `src/workspace/{mod,chunker,embeddings,schema,search}.rs` + `src/db/libsql/workspace.rs` (1,777) | ❌ codex 无 | ⚠️ claw 简化 | ★★★★★ | **完整移植** → `dasclaw_workspace` |
| E | 多 Channel 体系 | **60,211** | `src/channels/{web,cli,signal,wasm,manager}` + channels-src/{discord,telegram,slack,whatsapp,feishu} | ❌ codex 无 | ❌ claw CLI only | ★★★★★ | **完整移植** → `dasclaw_channels_v2` |
| F | LLM Provider 编排 | **33,654** | `src/llm/{provider,nearai,codex,bedrock,failover,retry,circuit_breaker,response_cache,smart_routing}` | ⚠️ codex 类似 | ⚠️ claw 简化 | ★★★★★ | **完整移植** → `dasclaw_llm_chain` |
| G | Skills 体系 | 6,304 + 732 | `crates/ironclaw_skills/` + `src/skills/` | ❌ codex 无 | ⚠️ claw 6 件套不同 | ★★★★ | ironclaw + claw 6 件套融合 |
| H | Extensions / WASM | **15,348** | `src/extensions/` + `src/tools/wasm/` + `tools-src/` + `wit/tool.wit` | ⚠️ | ❌ claw 无 | ★★★★★ | **完整移植** |
| I | Jobs / Scheduler | 6,654 | `src/worker/{job}.rs` + `src/agent/{scheduler,job_monitor}.rs` | ❌ | ⚠️ claw 简化 | ★★★★ | **完整移植** |
| J | Memory / RAG / Embeddings | 含在 D 中 | `src/workspace/{embeddings,chunker,search}.rs` + `src/embeddings/` | ❌ | ⚠️ | ★★★★ | 跟随 D |
| K | Approvals / Cost Guard | ~2,500 | `src/agent/dispatcher.rs::approval` + `src/bridge/cost_guard_gate.rs` (41) + budget tracking | ❌ codex 无 | ❌ claw 无 | ★★★★★ | **完整移植** → `dasclaw_approvals` |
| L | Hooks / Lifecycle | 3,229 | `src/hooks/{registry,hook,bootstrap}.rs` | ✅ codex 三分离更干净 | ⚠️ | ★★★★ | **用 codex 设计 + ironclaw lifecycle** |
| M | **Bridge 巨型适配层** | **25,369** | router 9,609 + effect_adapter 5,802 + store_adapter 3,351 + auth_manager 1,581 + llm_adapter 1,595 + 其它 | ❌ codex 单体 | ❌ 无对标 | ★★★★★ | **完整移植** → `dasclaw_bridge`（路线 B 粘合剂） |
| N | OAuth / Identity | 2,988 | `src/auth/{oauth,code_challenge}.rs` + `src/bridge/auth_manager.rs` (1,581) | ⚠️ | ⚠️ 弱 | ★★★ | **用 ironclaw**（最完整） |
| O | Observability | 628 | `src/observability/` + `src/llm/costs.rs` + tracing_fmt | ❌ codex rollout-trace 更强 | ⚠️ | ★★★ | **用 codex 补 ironclaw 基底** |
| P | Config / Settings | **11,568** | `src/config/{40+ types}` + `src/settings.rs` + `.env.example` | ⚠️ | ⚠️ | ★★★★ | **完整移植** → `dasclaw_config_v2` |
| Q | DB 持久化（PG + libSQL 双后端） | **13,977** | `src/db/{postgres,libsql/*}` + 24 migration | ❌ codex PG only | ❌ claw 无 DB 层 | ★★★★★ | **完整移植** → `dasclaw_db` |
| R | Network Security | ~800 | `src/NETWORK_SECURITY.md` + `src/sandbox/proxy/` + http intercept | ⚠️ codex 三平台 | ⚠️ | ★★★★ | **完整移植** → `ironclaw_safety v2` |
| ironclaw_engine v2 | — | **30,414** | `crates/ironclaw_engine/` LoopDelegate + ThreadManager + ExecutionLoop | ❌ | ❌ | ★★★★★ | **完整移植** → `dasclaw_core` 骨架 |
| ironclaw_safety | — | 5,490 | `crates/ironclaw_safety/` DLP + prompt injection + safety pipeline | ⚠️ | ⚠️ | ★★★★★ | 已被 desktop-client 直接 path-dep |
| ironclaw_tui | — | 11,021 | `crates/ironclaw_tui/` Ratatui REPL | ❌ codex tui 142k | ❌ | ★ | **不移植**（桌面端用 Tauri） |
| ironclaw_gateway | — | 2,182 | `crates/ironclaw_gateway/` Axum + Web SPA | ⚠️ | ❌ | ★★ | 选择性参考 |

**核心可移植合计**：~320,000 LOC（69% of total），不含 ironclaw_tui + 部分外置 channels-src

---

## 2. Bridge 巨型适配层深度解构（25,369 LOC，独家 ★★★★★）

> 这是 ironclaw v1 dispatcher 与 engine v2 LoopDelegate **过渡期共存**的粘合剂。路线 B 必须理解这层，否则无法吸收 ironclaw 已有能力。

### 2.1 文件矩阵

| 文件 | LOC | 职责 |
|---|---:|---|
| `src/bridge/router.rs` | **9,609** | `handle_with_engine()` engine outcome → channel response 映射，auth gate 显示 |
| `src/bridge/effect_adapter.rs` | **5,802** | `EffectBridgeAdapter` — tool 执行 + safety sanitize + hook + rate limit |
| `src/bridge/store_adapter.rs` | **3,351** | `HybridStore` — engine v2 的 Store trait 实现，thread/step/event 持久化 |
| `src/bridge/auth_manager.rs` | **1,581** | 集中式鉴权状态机，credential → extension_name 单一解析器 |
| `src/bridge/llm_adapter.rs` | **1,595** | `LlmBridgeAdapter` — LLM provider 代理 + cost guard 整合 |
| `src/bridge/sandbox/intercept.rs` | 593 | sandbox hook 截获点 |
| `src/bridge/sandbox/containerized_backend.rs` | 521 | Docker 容器后端 |
| `src/bridge/skill_migration.rs` | 445 | legacy skill → engine capability 一次迁移 |
| `src/bridge/user_facing_errors.rs` | 448 | 用户友好错误信息映射 |
| `src/bridge/sandbox/docker_transport.rs` | 296 | Docker transport |
| `src/bridge/sandbox/workspace_path.rs` | 218 | per-project bind-mount 路径解析（详见 §3） |
| `src/bridge/sandbox/lifecycle.rs` | 204 | 容器 lifecycle |
| 其它 sandbox/* + cost_guard_gate.rs + workspace_reader.rs | ~706 | 杂项 |
| **合计** | **25,369** | — |

### 2.2 双模式切换

```
ENGINE_V2=true 时
    Channel msg → router.rs:handle_with_engine() → ironclaw_engine ThreadManager → ExecutionLoop
                                                          ↓
                                                bridge adapters
                                                ├─ EffectExecutor   (effect_adapter)
                                                ├─ Store trait      (store_adapter)
                                                ├─ LlmBackend       (llm_adapter)
                                                ├─ AuthManager      (auth_manager)
                                                └─ CostGuardGate    (cost_guard_gate)

ENGINE_V2=false 时
    Channel msg → src/agent/agent_loop.rs → dispatcher.rs → run_agentic_loop(ChatDelegate/JobDelegate)
                                                                    ↓
                                                          v1 直接调用 tool/llm/db
```

**路线 B 含义**：
- 短期（W1-W6）保留 v1 path 不破坏
- 中期（W7+）所有新功能上 v2 path
- 长期（6+ 月）逐步迁移 channel handler 到 v2 后弃用 v1

### 2.3 关键设计点

1. **Auth 集中化**：`resolve_auth_flow_extension_name()` 在 `auth_manager.rs` 是单一法则。修复了过去 4 处不同地方各自解析 identity 导致的 bug。
2. **Output 清理三步**：tool result → SafetyLayer sanitize → wrap_for_llm → inject 回 LLM
3. **Lease 恢复**：pending approval gate 可从快照恢复 lease，避免重复 approve
4. **Cost gate**：LLM 调用前预算检查，失败返回 `EngineError::OutOfBudget`

---

## 3. Workspace 多租户隔离深度分析（★★★★★）

### 3.1 关键文件与设计

| 文件 | LOC | 角色 |
|---|---:|---|
| `src/bridge/sandbox/workspace_path.rs` | 218 | **per-project host workspace path 解析** |
| `src/config/workspace.rs` | 266 | workspace 配置加载 |
| `src/db/libsql/workspace.rs` | 1,777 | workspace 持久化（FTS5 + F32_BLOB vector） |
| `crates/ironclaw_engine/src/traits/workspace.rs` | 20 | `trait WorkspaceReader` |
| `src/bridge/workspace_reader.rs` | 40 | host 实现 trait |
| `src/workspace/{mod,chunker,embeddings,schema,search,hygiene,layer,document,privacy,repository,...}` | 10,536 | 完整 facade |
| **合计** | **12,857** | — |

### 3.2 Bind-Mount 工作流

```
Host:  ~/.ironclaw/projects/<user_id>/<project_id>/
       ├── src/                   ← 用户可见可编辑可备份
       ├── Cargo.toml
       ├── target/
       ├── MEMORY.md              ← agent identity 文件
       └── TOOLS.md
              ↓ docker run -v <host>:/project/
Container:  /project/             ← 容器看到的工作区
            ├── src/              ← shared writable (取决于 SandboxPolicy)
            ├── Cargo.toml        ← shared (read-only if ReadOnly)
            ├── target/           ← shared
            └── MEMORY.md         ← shared

            /tmp/                 ← ephemeral container-local
            /ironclaw-worker/     ← orchestrator 通信
```

### 3.3 Hybrid Search RRF（k=60）

```rust
// src/workspace/search.rs
let merged_score = keyword_results.iter()
    .map(|(doc, rank)| (doc, 1.0 / (60.0 + rank)))
    .chain(vector_results.iter().map(|(doc, rank)| (doc, 1.0 / (60.0 + rank))))
    .fold(...);
```

- **Keyword**：FTS5（libSQL）/ tsvector（PG）
- **Vector**：F32_BLOB(N) cos similarity，N 来自 env `EMBEDDING_DIM`
- **Fusion**：Reciprocal Rank Fusion，k=60（行业标准）

### 3.4 与 codex / claw-code 对比

| 维度 | ironclaw | codex | claw-code |
|---|---|---|---|
| Workspace 概念 | ✅ per-project bind-mount | ⚠️ 仅 cwd + WritableRoot | ⚠️ 仅 cwd |
| 多租户 user_id | ✅ | ❌ | ❌ |
| Hybrid search | ✅ RRF k=60 | ❌ | ❌ |
| Vector storage | ✅ libSQL F32_BLOB + PG pgvector | ❌ | ❌ |
| 持久化 | ✅ DB | ⚠️ 文件系统 | ⚠️ 文件系统 |

**结论**：ironclaw workspace 是路线 B 后端的核心资产，codex/claw-code 完全无对标。

---

## 4. ironclaw_engine v2 关键架构（30,414 LOC）

**位置**：[ironclaw-main/crates/ironclaw_engine/](../../ironclaw-main/crates/ironclaw_engine/)

**核心抽象**：用统一 Thread/Step/Capability 模型替代 10+ 散落抽象（Session / Job / Routine / Channel / Tool / Skill / Hook / Observer / Extension / LoopDelegate）。

```rust
// crates/ironclaw_engine/src/traits/loop_delegate.rs (推断接口)
pub trait LoopDelegate: Send + Sync {
    async fn before_llm_call(&self, ctx: &mut TurnContext) -> Result<()>;
    async fn execute_tool(&self, call: ToolCall) -> Result<ToolResult>;
    async fn after_response(&self, response: &Response) -> Result<()>;
}

// 实例：ChatDelegate / JobDelegate / ContainerDelegate / ApprovalDelegate
```

**与 v1 dispatcher 关系**：
- v1：dispatcher.rs 直接调用各组件（无 trait 抽象）
- v2：dispatcher 实现 LoopDelegate，引擎调度 trait

**路线 B 移植路径**：
1. dasclaw_core 直接采用 LoopDelegate 骨架（已写入 [31 文档 ADR-104](31-target-architecture.md)）
2. v1 dispatcher 通过 bridge 适配层暂时共存
3. 最终统一在 v2 模型上

---

## 5. 多 Channel 体系（60,211 LOC，★★★★★）

### 5.1 Channel 实现矩阵

| Channel | 状态 | 实现位置 | LOC |
|---|---|---|---|
| Web (Axum SSE/WebSocket) | ✅ | `src/channels/web/` | ~28k |
| CLI (Ratatui TUI) | ✅ | `src/channels/cli/` + `crates/ironclaw_tui/` | 11k + 11k |
| HTTP webhook | ✅ | `src/channels/http.rs` | ~1k |
| Signal | ✅ | `src/channels/signal.rs` | ~2k |
| Telegram | ✅ | `channels-src/telegram/`（条件编译） | ~3k |
| Slack | ✅ | `channels-src/slack/` | ~2k |
| Discord | 🚧 | `channels-src/discord/` | ~3k |
| WhatsApp | ❌ P1 | `channels-src/whatsapp/` 占位 | 0 |
| Feishu/Lark | 🚧 | `channels-src/feishu/` | ~3k |
| WASM channel runtime | ✅ | `src/channels/wasm/` | ~5k |

### 5.2 Channel 抽象

```rust
// src/channels/channel.rs
pub trait Channel: Send + Sync {
    async fn next_message(&mut self) -> Option<IncomingMessage>;
    async fn send_response(&self, msg: OutgoingResponse) -> Result<()>;
    async fn send_status(&self, update: StatusUpdate) -> Result<()>;
    async fn pair(&self, ctx: PairingContext) -> Result<PairingResult>;
}
```

**核心设计**：channel-identity 与 sender 分离 — `credential_name`（存储）vs `extension_name`（UI 路由）

---

## 6. Migration 编年史（24 版本）

| 版本 | 主题 | 关键变更 |
|---|---|---|
| V1 | Initial | conversations / messages / jobs |
| V2 | WASM secure API | wasm_tools / tool_capabilities |
| V3 | Tool failures | tool_failures table |
| V4 | Sandbox columns | runtime info |
| V5 | Claude Code | job 表改名 |
| V6 | Routines | routines / routine_runs |
| V7 | Events rename | claude_code_events → job_events |
| V8 | Settings | user-level KV |
| V9 | Embedding dim | flexible dimension |
| V10 | WASM versioning | tool versioning |
| V11 | Conversation index | unique constraints |
| V12 | **Job token budget** | session_budget_usd 真金白银预算 |
| V13 | Owner scope notify | per-owner notify targets |
| V14 | **Users** | user identity |
| V15 | Conversation source | source_channel |
| V16 | Document versions | memory doc versioning |
| V17 | User identities | identity resolution |
| V18 | Tool scope | tool scope isolation |
| V19 | Channel identities | channel-specific identities |
| V20 | Pairing requests | pairing workflow |
| V21 | Backfill channel | data migration |
| V22 | Sandbox restart | restart parameters |
| V23 | List escape | LIKE escaping |
| V24 | LLM calls index | created_at index |

---

## 7. 三方对比矩阵（45 项）

| # | 能力 | ironclaw | codex | claw-code | 决策 |
|---|---|:-:|:-:|:-:|---|
| **Agent 内核** | | | | | |
| 1 | Agent loop | ✅ 多场景（chat/job/container） | ✅ 多 phase | ⚠️ worker_boot | **ironclaw v2 LoopDelegate 骨架** |
| 2 | Heartbeat | ✅ `agent/heartbeat.rs` | ❌ | ❌ | **ironclaw 独家** |
| 3 | Sub-agent | ✅ Explore/Verify | ✅ multi-agents v2 | ✅ subagent | 三方融合 |
| 4 | Compaction | ✅ MoveToWorkspace/Summarize/Truncate | ✅ | ✅ tool 边界保护 | **claw 边界算法 + ironclaw 三策略** |
| **工具** | | | | | |
| 5 | apply-patch | ❌ | ✅ lark 协议 | ❌ | **codex** |
| 6 | portable-pty | ❌ | ✅ | ✅ | codex 或 claw |
| 7 | Bash validation | ❌ | ⚠️ ~800 | ✅ 1,004 LOC 6 模块 | **claw** |
| 8 | WASM tools | ✅ 完整 | ⚠️ | ❌ | **ironclaw** |
| 9 | Tool builder（LLM 自动构建） | ✅ | ❌ | ❌ | **ironclaw 独家** |
| 10 | code_mode / JS REPL | ❌ | ✅ 5,556 LOC | ❌ | **codex（可选 W7+）** |
| **沙箱** | | | | | |
| 11 | Linux Landlock+bwrap | ⚠️ 简版 | ✅ | ❌ | **codex** |
| 12 | macOS Seatbelt | ❌ | ✅ | ❌ | **codex** |
| 13 | Windows JobObject | ❌ | ✅ | ❌ | **codex** |
| 14 | Docker bind-mount | ✅ 完整 | ❌ | ❌ | **ironclaw（ExternalSandbox 模式）** |
| 15 | SandboxPolicy enum | ✅ 3 档 | ✅ 4 档（最完整） | ❌ | **codex 4 档 + ironclaw cap-std** |
| 16 | cap-std kernel layer | ✅ | ⚠️ Landlock 等价 | ❌ | **ironclaw 保留** |
| **Workspace** | | | | | |
| 17 | per-project bind-mount | ✅ | ❌ | ❌ | **ironclaw 独家** |
| 18 | Hybrid FTS+vector RRF | ✅ k=60 | ❌ | ❌ | **ironclaw 独家** |
| 19 | F32_BLOB embedding | ✅ libSQL | ❌ | ❌ | **ironclaw 独家** |
| 20 | 多 user_id 隔离 | ✅ | ❌ | ❌ | **ironclaw 独家** |
| **MCP** | | | | | |
| 21-26 | Stdio/SSE/HTTP/WS/SDK/ManagedProxy | ⚠️ 1-2 transport | ✅ 5+1 | ✅ 全 6 | **claw-code** |
| **Channels** | | | | | |
| 27 | Web SSE/WebSocket | ✅ | ⚠️ app-server | ❌ | **ironclaw** |
| 28 | TUI / CLI | ✅ ironclaw_tui | ✅ codex tui | ✅ rusty-claude-cli | 桌面端不需要 |
| 29 | Telegram/Slack/Discord/WhatsApp/Feishu | ✅ 5 channel | ❌ | ❌ | **ironclaw 独家** |
| 30 | WASM channel runtime | ✅ | ❌ | ❌ | **ironclaw 独家** |
| **LLM Provider** | | | | | |
| 31 | Provider chain | ✅ failover/retry/circuit_breaker | ✅ | ⚠️ | **ironclaw** |
| 32 | SmartRouter（13-d 复杂度评分） | ✅ | ❌ | ❌ | **ironclaw 独家** |
| 33 | Response cache（SHA-256+LRU+TTL） | ✅ | ⚠️ | ❌ | **ironclaw 独家** |
| 34 | Multi-provider（OpenAI/Anthropic/Bedrock/NEAR/GitHub Copilot/Codex/Tinfoil/Ollama） | ✅ 8+ | ⚠️ | ⚠️ Anthropic 主 | **ironclaw** |
| **Bridge** | | | | | |
| 35 | v1/v2 双模式适配 | ✅ 25k LOC | ❌ 单体 | ❌ | **ironclaw 独家** |
| 36 | Auth 集中化 | ✅ resolve_auth_flow_extension_name | ⚠️ | ⚠️ | **ironclaw** |
| **DB** | | | | | |
| 37 | PG + libSQL 双后端 | ✅ | ❌ PG only | ❌ 无 DB | **ironclaw** |
| 38 | 24 migration | ✅ | ⚠️ | ❌ | **ironclaw** |
| **Cost / Approval** | | | | | |
| 39 | Cost guard（daily cents+hourly rate） | ✅ | ❌ | ❌ | **ironclaw 独家** |
| 40 | Tool approval 暂停机制 | ✅ pending lease | ⚠️ TUI 提示 | ⚠️ trust resolver | **ironclaw + claw** |
| **治理 6 件套** | | | | | |
| 41 | branch_lock/green_contract/stale_branch/policy_engine/recovery_recipes/trust_resolver | ❌ | ❌ | ✅ 1,478 LOC | **claw 独家** |
| **AGENTS.md/CLAUDE.md** | | | | | |
| 42 | 多层加载 | ✅ | ✅ max_bytes 8KB | ❌ 单层 | **codex 设计** |
| **Engine v2** | | | | | |
| 43 | LoopDelegate trait 抽象 | ✅ ironclaw_engine 30k | ❌ | ❌ | **ironclaw 独家** |
| **Identity** | | | | | |
| 44 | OAuth 多 provider | ✅ NEAR ED25519 + GH PKCE + Codex device flow | ⚠️ device-key | ⚠️ PKCE | **ironclaw** |
| **可观测性** | | | | | |
| 45 | rollout-trace + analytics | ⚠️ 基础 | ✅ 丰富 | ⚠️ | **codex 补 ironclaw** |

**决策汇总**：
- **ironclaw 独家或最强**：22 项
- **codex 独家或最强**：10 项
- **claw-code 独家或最强**：3 项（治理 6 件套 / Bash validation / 边界保护）

---

## 8. 路线 B 移植决策树

```
W1-W2 基础设施
    ├─ ironclaw_engine v2 (30,414) → dasclaw_core 骨架（ADR-104）
    ├─ ironclaw_safety (5,490) → 已被 desktop-client 直接 path-dep，保留
    ├─ src/bridge/sandbox/workspace_path.rs (218) → dasclaw_workspace_cap policy 层
    └─ codex linux-sandbox (5,984) + sandboxing (4,984) → dasclaw_sandbox 三平台

W3 Hooks + Apply Patch + Project Docs
    ├─ codex hooks (6,922) → dasclaw_hooks
    ├─ codex apply-patch (4,037) → dasclaw_apply_patch
    └─ codex agents_md.rs (367) → dasclaw_project_docs（多层）

W4 治理 + Bash Validation
    ├─ claw 治理 6 件套 (1,478) → dasclaw_governance
    ├─ claw bash_validation (1,004) → dasclaw_bash_validation
    └─ ironclaw cost_guard + dispatcher.approval → dasclaw_approvals

W5 MCP + ExecPolicy
    ├─ claw mcp_client.rs + 6 transport → dasclaw_mcp_v2
    └─ codex execpolicy (2,753) → dasclaw_exec_policy

W6 后端基底大移植（**本文档主舞台**）
    ├─ ironclaw bridge 25,369 → dasclaw_bridge
    ├─ ironclaw db 13,977 + 24 migration → dasclaw_db
    ├─ ironclaw workspace 12,857 → dasclaw_workspace
    ├─ ironclaw channels 60,211 → dasclaw_channels_v2
    ├─ ironclaw llm 33,654 → dasclaw_llm_chain
    ├─ ironclaw tools 57,003 → dasclaw_tools_v2
    ├─ ironclaw extensions 15,348 + WASM → dasclaw_extensions
    ├─ ironclaw config 11,568 → dasclaw_config_v2
    └─ ironclaw worker (jobs) 6,654 → dasclaw_scheduler

W7+ 可观测 + Identity + Crash + Net Proxy
    ├─ codex otel + analytics + rollout-trace
    ├─ codex login + device-key + keyring-store
    └─ codex network-proxy（rama + MITM + SOCKS5）
```

**总移植量**：~352k LOC（codex ~25k + ironclaw ~320k + claw ~7k）

---

## 9. 风险与局限

| 风险 | 严重度 | 缓解 |
|---|:-:|---|
| ironclaw bridge v1/v2 双模式复杂度高，移植后需保持 ENGINE_V2 切换 | 🔴 高 | W6 设独立 4-6 周，不与其它 wave 并行 |
| ironclaw_tui 11k LOC 不移植，但桌面端需要类似 REPL 体验 | 🟡 中 | 桌面端用 Tauri，UI 自建 |
| ironclaw_engine v2 仍在演进，10 lifecycle hook 接口可能变 | 🟡 中 | W7 前锁定 trait 版本 |
| Migration V1-V24 累计复杂，迁移到 dasclaw_db 需精确复制 | 🟡 中 | 用 ironclaw 现有 migration 文件直接搬 |
| channels-src/ 外置 channel 是 conditional build，路线 B 包装方式待定 | 🟡 中 | W6 决策时按 feature flag 处理 |

---

## 10. 与其他文档的接口

- **30 文档**：本 37 文档的 §7 三方矩阵 45 项应反向同步到 30 v2.1 的 A-O 大表（用户审查后执行）
- **31 文档**：dasclaw_bridge / dasclaw_db / dasclaw_workspace / dasclaw_channels_v2 等 9 个新 crate 已列入 31 v2.1 ADR-104
- **32 文档**：W6 移植清单参考本文 §8
- **35 文档**：codex sandbox / hooks / apply-patch / agents_md 互补
- **36 文档**：claw-code 治理 6 件套 + bash_validation 互补
- **38 文档**：[38-desktop-client-ironclaw-fork-inventory.md](38-desktop-client-ironclaw-fork-inventory.md) 描述桌面端嵌入 fork 与本主版的差异

---

## 变更日志

- v1.0 (2026-04-25) — 初版，464,222 LOC 精确（subagent 估算 386k 偏低 17% 已校准），18 大域 + 45 项三方矩阵 + Bridge 25k 解构
