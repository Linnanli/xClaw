# 11 · 路线 B 目标架构设计 (供仔细 Review)

> 本文档专注**目标架构**, 不重复 09/10 的能力清单与质量评估。
> 路线 B = codex 内核 port + ironclaw 外围保留 + claw-code 能力源聚合。
> 命名规范: 新 crate 统一 `dasclaw_*` 前缀; 存量 `ironclaw_*` 后续分阶段改名。

---

## 1. 总览: 五层架构

```mermaid
flowchart TB
    subgraph L1["L1 · 用户入口 (UI)"]
        UI1["Tauri Window<br/>desktop-client/src-ui (React)"]
        UI2["Web Browser"]
        UI3["REPL CLI"]
        UI4["Webhook Caller"]
    end

    subgraph L2["L2 · 通信层 (Channels & IPC)"]
        IPC["Tauri IPC<br/>desktop-client/src/ipc/<br/>(79 command, 14 module)"]
        WEB["channels/web/<br/>(HTTP+SSE+WS+OpenAI 兼容)"]
        REPL["channels/repl/"]
        WH["channels/webhook_server"]
        RELAY["channels/relay/"]
    end

    subgraph L3["L3 · 编排层 (Agent Orchestration)"]
        DISP["ironclaw agent/dispatcher.rs<br/>★ 并行 JoinSet 工具调度<br/>★ 保留, 内部调用 L4 kernel"]
        ROUT["ironclaw agent/router.rs<br/>(/command 路由)"]
        SUB["ironclaw agent/submission.rs<br/>(用户输入解析)"]
        ROUTINE["ironclaw routines/<br/>scheduler + cron + event"]
        ORCH["ironclaw orchestrator/<br/>job_manager + reaper"]
    end

    subgraph L4["L4 · Agent 内核 (★ 路线 B 新建, codex port)"]
        K1["dasclaw_agent_kernel<br/>← codex core/agent<br/>★ mailbox + registry + role"]
        K2["dasclaw_session<br/>← codex core/session<br/>(25 文件分层)"]
        K3["dasclaw_tasks<br/>← codex core/tasks<br/>(含 ghost_snapshot)"]
        K4["dasclaw_context_mgr<br/>← codex core/context_manager"]
        WRAP["x_claw_agent (legacy)<br/>agentic_loop wrapper<br/>+ Hook trait → 迁到 K1"]
    end

    subgraph L5["L5 · 工具与运行时 (Tools & Runtime)"]
        TOOLS["ironclaw tools/builtin/<br/>33 builtin tool · 50k LOC<br/>★ 100% 保留"]
        APPLY["dasclaw_apply_patch<br/>← codex apply-patch"]
        GIT["dasclaw_git_utils<br/>← codex git-utils"]
        HOOKS["dasclaw_hooks_engine<br/>← codex hooks"]
        FEAT["dasclaw_features<br/>← codex features"]
        SANDLIN["dasclaw_sandbox_linux"]
        SANDWIN["dasclaw_sandbox_windows"]
        POL["dasclaw_policy<br/>← claw policy_engine + 等"]
        BRANCH["dasclaw_branch_guard<br/>← claw branch_lock + stale_*"]
        EXT["ironclaw extensions/<br/>+ claw mcp_lifecycle_hardened"]
        SKILL["ironclaw skills/<br/>+ codex skills/assets"]
    end

    subgraph L6["L6 · 持久化与服务 (Infra)"]
        LLM["ironclaw llm/<br/>28 provider + failover<br/>★ 100% 保留"]
        DB["ironclaw db/"]
        SEC["ironclaw secrets/<br/>+ dasclaw_device_identity"]
        OBS["ironclaw observability/<br/>+ dasclaw_rollout_trace"]
        AUTH["crates/ironclaw_auth"]
        WS["crates/ironclaw_workspace_cap"]
    end

    UI1 --> IPC
    UI2 --> WEB
    UI3 --> REPL
    UI4 --> WH

    IPC --> DISP
    WEB --> DISP
    REPL --> DISP
    WH --> DISP
    RELAY --> DISP

    DISP --> ROUT
    DISP --> SUB
    ROUTINE --> DISP
    ORCH --> DISP

    DISP --> WRAP
    WRAP --> K1
    K1 --> K2
    K1 --> K3
    K1 --> K4

    DISP --> TOOLS
    TOOLS --> APPLY
    TOOLS --> GIT
    TOOLS --> HOOKS
    TOOLS --> FEAT
    TOOLS --> SANDLIN
    TOOLS --> SANDWIN
    TOOLS --> POL
    TOOLS --> BRANCH
    TOOLS --> EXT
    TOOLS --> SKILL

    K1 --> LLM
    K2 --> DB
    OBS -.观察.-> K1
    SEC -.读密钥.-> TOOLS
    AUTH -.认证.-> WEB
    WS -.工作区.-> TOOLS

    classDef newCap fill:#ffe6cc,stroke:#e67e22,stroke-width:2px
    classDef retained fill:#d5e8d4,stroke:#27ae60
    classDef legacy fill:#f8cecc,stroke:#c0392b
    class K1,K2,K3,K4,APPLY,GIT,HOOKS,FEAT,SANDLIN,SANDWIN,POL,BRANCH newCap
    class TOOLS,LLM,DISP,WEB,IPC,EXT,SKILL,ROUTINE,ORCH retained
    class WRAP legacy
```

**色标**:
- 🟧 橙色: 新建 `dasclaw_*` crate (路线 B 新增, 14 个)
- 🟩 绿色: ironclaw 100% 保留模块
- 🟥 红色: legacy (保留为 wrapper, 部分内部下线)

---

## 2. Crate 依赖关系图

```mermaid
flowchart LR
    subgraph apps["Application Layer"]
        DT["desktop-client (Tauri)"]
        AB["admin-backend"]
    end

    subgraph main["Main Library"]
        IC["ironclaw (主库 232k LOC)<br/>agent/ + tools/builtin/ + channels/ + llm/<br/>+ extensions/ + skills/ + routines/ + orchestrator/"]
    end

    subgraph kernel["★ Agent Kernel (新建)"]
        K1["dasclaw_agent_kernel"]
        K2["dasclaw_session"]
        K3["dasclaw_tasks"]
        K4["dasclaw_context_mgr"]
    end

    subgraph runtime["★ Runtime crates (新建)"]
        R1["dasclaw_apply_patch"]
        R2["dasclaw_git_utils"]
        R3["dasclaw_hooks_engine"]
        R4["dasclaw_features"]
    end

    subgraph sec["★ Security & Sandbox (新建)"]
        S1["dasclaw_sandbox_linux"]
        S2["dasclaw_sandbox_windows"]
        S3["dasclaw_policy"]
        S4["dasclaw_branch_guard"]
    end

    subgraph obs["★ Observability & Identity (新建)"]
        O1["dasclaw_rollout_trace"]
        O2["dasclaw_device_identity"]
    end

    subgraph legacy["Legacy Shims"]
        L1["x_claw_agent<br/>(Hook trait + loop wrapper)"]
    end

    subgraph shared["Existing Shared Crates"]
        E1["ironclaw_auth → 后续改名 dasclaw_auth"]
        E2["ironclaw_workspace_cap → 后续改名 dasclaw_workspace_cap"]
    end

    DT --> IC
    AB --> IC
    IC --> kernel
    IC --> runtime
    IC --> sec
    IC --> obs
    IC --> legacy
    IC --> shared
    legacy --> kernel
    K1 --> K2
    K1 --> K3
    K1 --> K4
    K3 --> K2

    classDef new fill:#ffe6cc,stroke:#e67e22,stroke-width:2px
    classDef ret fill:#d5e8d4,stroke:#27ae60
    classDef leg fill:#f8cecc,stroke:#c0392b
    class K1,K2,K3,K4,R1,R2,R3,R4,S1,S2,S3,S4,O1,O2 new
    class IC,DT,AB,E1,E2 ret
    class L1 leg
```

---

## 3. 数据流: 一次用户消息的处理路径

```mermaid
sequenceDiagram
    autonumber
    participant U as User (Tauri UI)
    participant T as Tauri IPC<br/>desktop-client/src/ipc/chat
    participant D as ironclaw<br/>agent/dispatcher
    participant W as x_claw_agent<br/>agentic_loop wrapper
    participant K as dasclaw_agent_kernel<br/>(mailbox/registry)
    participant S as dasclaw_session
    participant C as dasclaw_context_mgr
    participant L as ironclaw llm/<br/>(28 provider)
    participant Tl as ironclaw tools/builtin
    participant P as dasclaw_policy<br/>(safety check)

    U->>T: invoke("ic_chat_send", msg)
    T->>D: dispatch(message)
    D->>W: run_agentic_loop(delegate)
    W->>K: kernel.start_turn()
    K->>S: session.append_user(msg)
    S->>C: context.update(msg)

    loop 直到完成或需要审批
        K->>L: client.respond(messages)
        L-->>K: RespondOutput { tool_calls? }
        alt 有工具调用
            K->>D: emit ToolCallRequest
            D->>P: safety.check(tool, args)
            P-->>D: SafetyDecision::Allow | NeedApproval
            par 并行 JoinSet
                D->>Tl: tool_a.execute(args)
                D->>Tl: tool_b.execute(args)
            end
            Tl-->>D: results
            D->>K: kernel.tool_results(...)
            K->>S: session.append_tool_results(...)
            K->>C: context.compact_if_needed()
        else 纯文本响应
            K->>S: session.append_assistant(text)
            K-->>W: LoopOutcome::Response(text)
        end
    end

    W-->>D: outcome
    D-->>T: ChatResponse
    T-->>U: stream events via channel
```

---

## 4. 多 Agent 协调架构 (路线 B 的最大升级点)

```mermaid
flowchart TB
    subgraph main_session["主会话 Session A"]
        MA["Main Agent A<br/>(role: planner)"]
    end

    subgraph kernel_inner["dasclaw_agent_kernel"]
        REG["AgentRegistry<br/>HashMap&lt;agent_id, AgentMetadata&gt;<br/>active_agents tree<br/>nicknames"]
        MBA["Mailbox A<br/>mpsc + watch + seq"]
        MBB["Mailbox B"]
        MBC["Mailbox C"]
        MBD["Mailbox D"]
    end

    subgraph subs["Sub-Agents (无深度限制)"]
        SA["Sub-Agent B<br/>(role: explore)"]
        SB["Sub-Agent C<br/>(role: verify)"]
        SC["Sub-Agent D<br/>(role: custom)"]
        SD["Sub-Sub-Agent E<br/>(由 D spawn)"]
    end

    MA <-->|InterAgentCommunication| MBA
    SA <--> MBB
    SB <--> MBC
    SC <--> MBD
    SD <--> MBD

    REG -.管理.-> MBA & MBB & MBC & MBD
    REG -.tree.-> MA & SA & SB & SC & SD

    MA -->|spawn| SA
    MA -->|spawn| SB
    MA -->|spawn| SC
    SC -->|spawn (depth 2+)| SD

    classDef new fill:#ffe6cc,stroke:#e67e22,stroke-width:2px
    class REG,MBA,MBB,MBC,MBD new
```

**对比 ironclaw 现状**:

| 能力 | ironclaw 现状 | 路线 B 后 |
|---|---|---|
| Agent 注册表 | ❌ 无 | ✅ `AgentRegistry` 维护活跃 agent + nickname + metadata |
| Agent 间通信 | ❌ 无, 仅父→子单向 spawn | ✅ `Mailbox` mpsc + watch + 序号保证 |
| Agent 树 | ❌ 平铺 | ✅ `agent_tree: HashMap<String, AgentMetadata>` |
| 深度限制 | `MAX_SUB_AGENT_DEPTH = 1` | ✅ 移除硬限, 由 registry 配额管控 |
| 角色解析 | 3 个硬编码 enum (Explore/Verify/Custom) | ✅ `agent_resolver.rs` 动态加载 + role registry |

---

## 5. Hook 注入点 (Phase 3 设计延续)

Phase 3 已在 `x_claw_agent::hooks` 定义 4 个 Hook trait, 路线 B 把它们**迁到 `dasclaw_agent_kernel` 顶层**, 接口契约不变, 实现层下沉到新 crate。

```mermaid
flowchart LR
    K[dasclaw_agent_kernel<br/>定义 trait]

    subgraph traits["Hook Trait (4 个)"]
        T1[SafetyHook]
        T2[SandboxExecutor]
        T3[SecretProvider]
        T4[ApprovalGate]
    end

    subgraph impl_crates["Hook 实现 crate"]
        I1["dasclaw_policy<br/>impl SafetyHook"]
        I2["dasclaw_sandbox_linux<br/>impl SandboxExecutor"]
        I3["dasclaw_sandbox_windows<br/>impl SandboxExecutor"]
        I4["ironclaw secrets/<br/>impl SecretProvider"]
        I5["ironclaw safety/<br/>impl ApprovalGate"]
    end

    K --- traits
    T1 -.被.-> I1
    T2 -.被.-> I2
    T2 -.被.-> I3
    T3 -.被.-> I4
    T4 -.被.-> I5

    classDef new fill:#ffe6cc,stroke:#e67e22
    classDef ret fill:#d5e8d4,stroke:#27ae60
    class K,T1,T2,T3,T4,I1,I2,I3 new
    class I4,I5 ret
```

---

## 6. 文件级目录规划 (最终态)

```
x-claw/
├─ Cargo.toml                          # workspace root
├─ crates/                             # 共享 crate (16 个)
│   ├─ ironclaw_auth/                  # 保留, 后续改名 dasclaw_auth
│   ├─ ironclaw_workspace_cap/         # 保留, 后续改名 dasclaw_workspace_cap
│   ├─ x_claw_agent/                   # legacy: 仅保留 agentic_loop wrapper + bash_validation + permissions
│   │
│   ├─ dasclaw_agent_kernel/           # ★ W1: codex core/agent port (5.8k)
│   │   └─ src/
│   │       ├─ lib.rs
│   │       ├─ registry.rs             # AgentRegistry / ActiveAgents / AgentMetadata
│   │       ├─ mailbox.rs              # Mailbox / MailboxReceiver (mpsc+watch+seq)
│   │       ├─ control.rs              # AgentControl
│   │       ├─ role.rs                 # AgentRole / RoleRegistry
│   │       ├─ status.rs
│   │       ├─ agent_resolver.rs
│   │       └─ hooks.rs                # ← 从 x_claw_agent 迁来 (4 个 trait)
│   │
│   ├─ dasclaw_session/                # ★ W2: codex core/session port (~15k, 25 文件)
│   ├─ dasclaw_tasks/                  # ★ W3: codex core/tasks port (3k, 含 ghost_snapshot)
│   ├─ dasclaw_context_mgr/            # ★ W4: codex core/context_manager port (2k)
│   │
│   ├─ dasclaw_apply_patch/            # ★ W5: codex apply-patch (2k)
│   ├─ dasclaw_git_utils/              # ★ W5: codex git-utils (3k)
│   ├─ dasclaw_hooks_engine/           # ★ W6: codex hooks (6.9k schema+engine)
│   ├─ dasclaw_features/               # ★ W6: codex features (1k feature flag)
│   │
│   ├─ dasclaw_sandbox_linux/          # ★ W7: codex linux-sandbox
│   ├─ dasclaw_sandbox_windows/        # ★ W7: codex windows-sandbox-rs (12.5k)
│   ├─ dasclaw_policy/                 # ★ W8: claw policy_engine+permission_enforcer+trust_resolver 聚合
│   ├─ dasclaw_branch_guard/           # ★ W8: claw branch_lock+stale_base+stale_branch 聚合
│   │
│   ├─ dasclaw_rollout_trace/          # ★ W9: codex rollout-trace
│   └─ dasclaw_device_identity/        # ★ W9: codex device-key + agent-identity

├─ desktop-client/
│   ├─ src/                            # Tauri 层 (79 IPC, 14 module) — ★ 100% 保留
│   │   ├─ lib.rs                      # all_tauri_commands! 宏
│   │   ├─ ipc/                        # chat/threads/skills/extensions/jobs/...
│   │   ├─ engine.rs
│   │   ├─ safety_bridge.rs
│   │   ├─ dlp/
│   │   └─ src-ui/                     # React 前端
│   │
│   └─ ironclaw/src/                   # 主库 232k LOC — ★ 大部分保留
│       ├─ agent/
│       │   ├─ agentic_loop.rs         # re-export shim (沿用)
│       │   ├─ dispatcher.rs           # 并行 JoinSet 调度 (★ 保留, 内部调 kernel)
│       │   ├─ router.rs               # ★ 保留
│       │   ├─ submission.rs           # ★ 保留
│       │   ├─ thread_ops.rs           # 重写: 调 dasclaw_session
│       │   ├─ commands.rs             # ★ 保留
│       │   ├─ session.rs              # ❌ 下线 (换 dasclaw_session)
│       │   ├─ session_manager.rs      # ❌ 下线
│       │   ├─ task.rs                 # ❌ 下线 (换 dasclaw_tasks)
│       │   ├─ compaction.rs           # ❌ 下线 (换 dasclaw_context_mgr)
│       │   ├─ context_monitor.rs      # ❌ 下线
│       │   ├─ self_repair.rs          # ★ 保留 (合并 claw recovery_recipes)
│       │   ├─ heartbeat.rs            # ★ 保留 (ironclaw 独有)
│       │   ├─ cost_guard.rs           # ★ 保留 (ironclaw 独有)
│       │   ├─ routine.rs              # ★ 保留
│       │   └─ routine_engine.rs       # ★ 保留
│       ├─ tools/builtin/              # ★ 33 tool 100% 保留
│       │   ├─ apply_patch.rs          # ★ 新增 (薄 adapter 调 dasclaw_apply_patch)
│       │   ├─ code_edit.rs            # 改写: 调 dasclaw_apply_patch
│       │   ├─ git/                    # 改写: 调 dasclaw_git_utils
│       │   ├─ sub_agent.rs            # 重写: 移除 MAX_DEPTH=1, 调 dasclaw_agent_kernel
│       │   └─ ... (其余 30 个 tool 不变)
│       ├─ channels/                   # ★ 100% 保留 (web/repl/relay/webhook/wasm/signal)
│       ├─ llm/                        # ★ 100% 保留 (28 provider)
│       ├─ extensions/                 # ★ 保留 (融合 claw mcp_lifecycle_hardened)
│       ├─ skills/                     # ★ 保留 (引入 codex skills/assets)
│       ├─ routines/                   # ★ 保留 (融合 claw recovery_recipes)
│       ├─ orchestrator/               # ★ 100% 保留
│       ├─ hooks/                      # 改写: 基于 dasclaw_hooks_engine schema/engine
│       ├─ safety/                     # 改写: 基于 dasclaw_policy
│       ├─ sandbox/                    # 改写: 分发到 dasclaw_sandbox_linux/windows
│       ├─ secrets/                    # ★ 保留 (融合 dasclaw_device_identity)
│       └─ observability/              # ★ 保留 (融合 dasclaw_rollout_trace)
```

---

## 7. 关键能力归属表 (一览)

| 能力 | 所在 crate / 模块 | 来源 | 路线 B 状态 |
|---|---|---|---|
| **Agentic Loop** | `x_claw_agent::agentic_loop` (wrapper) + `dasclaw_agent_kernel` (内核) | ironclaw 原生 + codex port | 重组 |
| **多 Agent Mailbox** | `dasclaw_agent_kernel::mailbox` | codex port | ★ 新增 |
| **Agent Registry / Tree** | `dasclaw_agent_kernel::registry` | codex port | ★ 新增 |
| **Agent Role 解析** | `dasclaw_agent_kernel::role + agent_resolver` | codex port | ★ 新增 |
| **Session 生命周期** | `dasclaw_session` | codex port | ★ 替换 |
| **Tasks (regular/review/compact/undo/ghost_snapshot)** | `dasclaw_tasks` | codex port | ★ 替换 |
| **Context Manager** | `dasclaw_context_mgr` | codex port | ★ 替换 |
| **Hook 4 trait** | `dasclaw_agent_kernel::hooks` | Phase 3 自写 | 迁移 |
| **并行工具调度** | `ironclaw agent/dispatcher.rs` JoinSet | ironclaw 原生 | ★ 保留 |
| **Plan Mode** | `ironclaw tools/builtin/plan_mode.rs` | ironclaw 原生 | ★ 保留 |
| **Session Fork** | `ironclaw tools/builtin/session_fork.rs` | ironclaw 原生 | ★ 保留 (调 dasclaw_session) |
| **Sub-Agent** | `ironclaw tools/builtin/sub_agent.rs` | ironclaw 原生 | 重写 (调 kernel) |
| **33 Builtin Tool** | `ironclaw tools/builtin/` | ironclaw 原生 | ★ 100% 保留 |
| **Apply Patch 协议** | `dasclaw_apply_patch` | codex port | ★ 新增 |
| **Git utils (ghost_commits/baseline)** | `dasclaw_git_utils` | codex port | ★ 新增 |
| **Hooks schema + engine** | `dasclaw_hooks_engine` | codex port | ★ 新增 |
| **Feature flags** | `dasclaw_features` | codex port | ★ 新增 |
| **Linux/Windows 沙箱** | `dasclaw_sandbox_linux/windows` | codex port | ★ 新增 |
| **合规策略** | `dasclaw_policy` | claw 聚合 | ★ 新增 |
| **分支治理** | `dasclaw_branch_guard` | claw 聚合 | ★ 新增 |
| **错误恢复** | `ironclaw routines/self_repair.rs` 合并 | ironclaw + claw 融合 | 增强 |
| **MCP 加固** | `ironclaw extensions/` 内部 | ironclaw + claw 融合 | 增强 |
| **bash_validation** | `x_claw_agent::bash_validation` | claw port (Phase 3) | ★ 保留 |
| **permissions** | `x_claw_agent::permissions` | claw port (Phase 3) | ★ 保留 |
| **Rollout Trace** | `dasclaw_rollout_trace` | codex port | ★ 新增 |
| **Device Identity** | `dasclaw_device_identity` | codex port | ★ 新增 |
| **多 Channel 入口** | `ironclaw channels/` | ironclaw 原生 | ★ 100% 保留 |
| **28 LLM Provider** | `ironclaw llm/` | ironclaw 原生 | ★ 100% 保留 |
| **Routine + cron** | `ironclaw routines/` | ironclaw 原生 | ★ 100% 保留 |
| **Cost Guard** | `ironclaw agent/cost_guard.rs` | ironclaw 独有 | ★ 100% 保留 |
| **Heartbeat** | `ironclaw agent/heartbeat.rs` | ironclaw 独有 | ★ 100% 保留 |
| **Tauri 79 IPC** | `desktop-client/src/ipc/` | ironclaw 原生 | ★ 100% 保留 |
| **Skills 资产** | `ironclaw skills/` + codex bundled assets | ironclaw + codex 融合 | 增强 |

---

## 8. Wave 路线图 (路线 B 完整版)

| Wave | 主题 | 新增 / 改动 | 依赖 |
|---|---|---|---|
| **W1** | Agent Kernel | port codex `core/agent/` → `dasclaw_agent_kernel`; 把 Hook trait 从 `x_claw_agent` 迁过来 | - |
| **W2** | Session 分层 | port codex `core/session/` → `dasclaw_session`; ironclaw `session.rs/session_manager.rs` 下线 | W1 |
| **W3** | Tasks 与快照 | port codex `core/tasks/` → `dasclaw_tasks`; 含 ghost_snapshot 任务级回滚 | W1, W2 |
| **W4** | Context Manager | port codex `core/context_manager/` → `dasclaw_context_mgr`; ironclaw `compaction.rs/context_monitor.rs` 下线 | W1 |
| **W5** | Patch 与 Git | port `dasclaw_apply_patch` + `dasclaw_git_utils`; ironclaw `code_edit.rs / tools/builtin/git/` 改 adapter | - (并行) |
| **W6** | Hooks 与 Features | port `dasclaw_hooks_engine` + `dasclaw_features`; ironclaw `hooks/` 改 adapter | - (并行) |
| **W7** | 沙箱平台覆盖 | port `dasclaw_sandbox_linux` + `dasclaw_sandbox_windows`; ironclaw `sandbox/` 加分发层 | - (并行) |
| **W8** | 合规与分支治理 | 聚合 claw → `dasclaw_policy` + `dasclaw_branch_guard`; ironclaw `safety/` 改 adapter | - (并行) |
| **W9** | 可观测与身份 | port `dasclaw_rollout_trace` + `dasclaw_device_identity` | - (并行) |
| **W10** | 集成测试 + 老 sub_agent 重写 | `tools/builtin/sub_agent.rs` 重写为 kernel adapter; 契约/冒烟测试覆盖 14 个新 crate | W1-W9 |

**并行性**: W5/W6/W7/W8/W9 互不依赖, 可并行推进。W1→W4 是顺序的 (kernel 定义接口, 后续依赖)。

---

## 9. 编排层 (L3) 为什么保留 ironclaw 实现? — 用户关键提问回答

**一句话**: ironclaw 的 dispatcher/router/thread_ops **不是 agent 内核**, 而是连接 codex kernel 与 ironclaw 特有外围 (skills/jobs/channels/extensions) 的**胶水层**。这些胶水能力是 codex/claw-code 都没有的, 替换成本极高且没有对等物可换。

### 9.1 ironclaw dispatcher 的 4 项独有能力 (证据)

**文件规模**:

```
dispatcher.rs      3,017 行   (工具调度 + skill 注入 + channel 感知)
thread_ops.rs      2,583 行   (thread 操作 + undo/redo + 审批)
commands.rs        1,033 行   (/help /model /status /skills 等命令)
router.rs            200 行   (自然语言 → MessageIntent)
submission.rs          4 行   (re-export)
─────────────────────
合计                6,837 行
```

#### 能力 1: Skill 动态注入 (dispatcher.rs:132-199) ⭐

```rust
// dispatcher.rs 第 132-144 行
// Select and prepare active skills (if skills system is enabled)
let (disabled_skills, disabled_extensions) = if message.channel == "tauri" {
    (
        disabled_names_from_metadata(message, "disabled_skills"),
        disabled_names_from_metadata(message, "disabled_extensions"),
    )
} else { ... };

let active_skills = self.select_active_skills(&message.content, &disabled_skills);

// 第 147-174 行: 注入 <skill> XML block
let skill_context = if !active_skills.is_empty() {
    for skill in &active_skills {
        let trust_label = match skill.trust {
            SkillTrust::Trusted => "TRUSTED",
            SkillTrust::Installed => "INSTALLED",
        };
        format!("<skill name=\"{}\" version=\"{}\" trust=\"{}\">...</skill>", ...)
    }
}
```

**价值**: 这是**整个系统 skill 系统的唯一注入点**。根据用户消息内容**动态选择** trusted/installed skill, 注入为 XML block 到 system prompt。**codex / claw-code 完全没有这个机制**。

#### 能力 2: Channel 感知差异化 (dispatcher.rs 多处)

```rust
if message.channel == "tauri" { ... }  // 仅 Tauri 前端从 metadata 读 disabled 列表
```

**价值**: ironclaw 有 **7 个 channel 入口** (tauri / web / repl / relay / webhook / wasm / signal), dispatcher 是它们的**统一收敛点**, 针对不同 channel 差异化读取 metadata / 启用 skill / 审批策略。codex 只有 tui+app-server 两个入口, 没有多 channel 抽象。

#### 能力 3: 并行 JoinSet 工具调度 (dispatcher.rs:880-920)

```rust
let mut join_set = JoinSet::new();
for (pf_idx, tc) in &runnable {
    join_set.spawn(async move {
        // 每个工具独立 status event + safety check + execute + post-flight
    });
}
```

**价值**: 工业级并行调度, 集成 status event 流式推送, 与 codex 的并行实现水平相当。**路线 B 直接保留**, 无需重写。

#### 能力 4: Job 语义路由 (router.rs)

```rust
pub enum MessageIntent {
    CreateJob { title, description, category },
    CheckJobStatus { job_id },
    CancelJob { job_id },
    ListJobs { filter },
    HelpJob { job_id },
    Chat { content },
    Command { command, args },
    Unknown,
}
```

**价值**: 把自然语言对话**语义映射到 Job 系统**, 连接 `routines/` 与 `orchestrator/`。**codex / claw-code 没有 job 概念**, 它们只管对话 turn。

### 9.2 codex 编排层对应物 (做什么)

**codex `codex_delegate.rs` (852 行) + `thread_manager.rs` (1,129 行)** 职责:

| 职责 | 是否与 ironclaw 重叠 |
|---|---|
| Guardian approval 路由 (企业审批流) | ⚠️ ironclaw `safety/` + `approval` IPC 已有不同实现 |
| MCP tool 批准缓存 (ACCEPT / ACCEPT_FOR_SESSION / DECLINE) | ⚠️ 可借鉴到 dispatcher |
| Collaboration mode 切换 (collaboration_mode_presets) | ❌ ironclaw 无此概念 |
| Thread 生命周期 (spawn/prune) | ✅ 路线 B 已切到 `dasclaw_session` |
| Model preset 管理 | ⚠️ ironclaw `llm/models.rs` 已有 |

**codex 的编排层没有**:
- ❌ Skill 动态注入
- ❌ Channel 感知差异化
- ❌ Job 语义路由
- ❌ 7 channel 入口汇聚

### 9.3 claw-code 编排层对应物

**claw-code `runtime/conversation.rs` (1,811 行) + `session_control.rs` (966 行)**:

- 对话状态机 (turn / message / tool_call 推进)
- session 生命周期控制
- **没有**: skill 动态注入、multi-channel、job 路由

### 9.4 三家对比表

| 编排层职责 | ironclaw dispatcher | codex codex_delegate | claw-code conversation |
|---|---|---|---|
| LLM call → tool → repeat 主循环 | ✅ (调 x_claw_agent loop) | ✅ | ✅ |
| 并行工具调度 | ✅ JoinSet (3k LOC) | ✅ | ✅ |
| **Skill 动态选择 + 注入** | ✅ **独有** | ❌ | ❌ |
| **Channel 感知 metadata** | ✅ **独有 (7 入口)** | ❌ (2 入口硬编码) | ❌ |
| **Job 语义路由** | ✅ **独有** | ❌ | ❌ |
| **Extensions 动态禁用** | ✅ **独有** | ⚠️ (plugin 系统不同) | ⚠️ |
| **/help /model /status 命令** | ✅ commands.rs 1k | ✅ | ✅ |
| Guardian approval | ⚠️ 不同实现 | ✅ 更全 | ⚠️ |
| MCP approval 缓存 | ⚠️ 基础 | ✅ 更细 | ⚠️ |
| Collaboration mode | ❌ | ✅ | ❌ |
| Thread 生命周期 | 部分 (thread_ops) | ✅ | ✅ |
| undo/redo checkpoint | ✅ agent/undo.rs | ✅ core/tasks/undo.rs | ❌ |

### 9.5 结论: 为什么编排层用 ironclaw?

**3 条理由**:

1. **ironclaw 编排层集成了系统特有能力**: skill 注入 / 7 channel / job 路由 / extensions 禁用 — 这些都是 codex/claw 没有的, 替换 = 从零重写
2. **替换成本 ≈ 重构整个应用层**: dispatcher + thread_ops + commands + router = **6,837 行深度耦合代码**, 与 skills/channels/jobs/extensions 双向引用
3. **codex 的编排层强项可选择性融合**, 不需要整替换:
   - **MCP approval 缓存** (ACCEPT_FOR_SESSION 等) 可借鉴思路合并到 dispatcher
   - **Guardian approval** 可参考设计, 但 ironclaw 已有自己的 approval IPC (`ic_approve_plan`)
   - **Thread 生命周期** 已通过 `dasclaw_session` 替换内核部分

**路线 B 的正确边界划分**:

```
codex → 内核 (agent_kernel / session / tasks / context_mgr)  ← 算法标准化, 越替换越优
ironclaw → 编排层 (dispatcher / router / thread_ops / commands)  ← 应用胶水, 越保留越省
codex/claw → 工具与治理 (apply_patch / git / policy / sandbox)  ← 独立子能力, 整 crate 移植
ironclaw → 外围生态 (channels / llm / tools / routines / extensions / skills)  ← 7年积累, 完全保留
```

**简言之**: dispatcher 不是"agent 引擎", 而是"**把 agent 引擎和公司应用接起来的胶水**"。胶水是 ironclaw 独有的, 所以保留; 引擎是 codex 更优的, 所以替换。这就是路线 B 的本质。

---

## 10. 待确认事项

请仔细看以下决策点:

1. **§9 的编排层保留理由是否充分?** 如果您仍觉得 codex codex_delegate + thread_manager 比 ironclaw dispatcher 更优, 请指出具体哪项能力; 我可以再做更细致对比。

2. **架构图是否清晰?** 5 个 mermaid 图分别覆盖: 五层总览 / crate 依赖 / 数据流 / 多 agent 协调 / Hook 注入。是否还需要补充某个视角的图? (例如: skill 注入流程? channel 汇聚图?)

3. **14 个新 dasclaw_* crate 是否过多?** 建议合并选项:
   - `dasclaw_sandbox_linux` + `dasclaw_sandbox_windows` → `dasclaw_sandbox` 单 crate 两 feature?
   - `dasclaw_policy` + `dasclaw_branch_guard` → `dasclaw_governance`?
   - `dasclaw_rollout_trace` + `dasclaw_device_identity` → `dasclaw_enterprise`?

4. **下线 5 个 ironclaw 文件激进吗?** `session.rs / session_manager.rs / task.rs / compaction.rs / context_monitor.rs` 直接换成 codex port, 是否分阶段灰度更稳?

5. **Wave 顺序**: 当前是 W1-W4 内核优先, W5-W9 周边并行。或先做 W5-W8 周边能力再吃内核?

6. **legacy `x_claw_agent` 是否改名**为 `dasclaw_agent_legacy` 强调过渡性?
