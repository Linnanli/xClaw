# 双图谱能力对比：`crates/`（core 侧）vs `desktop-client/ironclaw/`（ironclaw 侧）

> Issue: [#621 step 2](https://github.com/Linnanli/xClaw/issues/621)
> 数据基线：commit `80d41c90`（xClaw HEAD），见 [`dual-graph-stats.md`](dual-graph-stats.md)
> 范围 A（core）：`crates/` — 总 333 文件 / 6871 节点
> 范围 B（ironclaw）：`desktop-client/ironclaw/` — 总 564 文件 / 14035 节点

本文件不重写架构方案，**只输出每个能力域当前两侧实际归属与吸收/保留建议**，作为 PR 3（ADR-149）决策矩阵的证据底。

---

## 1. 解读约定

- "core 节点 / 文件 / 测试" 三元组来自 code-review-graph SQLite DB：`SELECT count(*) FROM nodes WHERE file_path LIKE '%<pat>%'` / `count(DISTINCT file_path)` / `count(*) WHERE is_test=1`。
- **测试节点 (is_test=1) 是 tree-sitter 标记的 `#[test]` / `#[tokio::test]` 函数计数，不是测试用例覆盖率。** 高测试节点数代表"被测试覆盖深"。
- 结论列含义：
  - **core 权威**：能力在 `crates/` 已成熟，ironclaw 应该切换到 core；如有差距由 ADR-149 列迁移步骤。
  - **ironclaw 权威**：能力在 `desktop-client/ironclaw/` 成熟而 core 仅有骨架；融合应反向吸收到 core 或继续保留在 ironclaw（按 L2/L3 分层）。
  - **两侧职责不同**：同名但语义不同，互不替代。
  - **范围外**：能力位于 `desktop-client/src/`（L2 客户端外壳，非本图谱范围），需在 ADR-149 单独脚注说明。

---

## 2. 能力域对照表

| # | 能力域 | core 节点/文件/测试 | ironclaw 节点/文件/测试 | 哪侧更强 | 吸收/保留建议 |
|---|---|---|---|---|---|
| 1 | **Agent Loop**（主循环+派发） | 86 / 3 / 11<br>`dasclaw_core/src/agentic_loop.rs` 单文件 | 300+ / 11 / 50+<br>`agent/{agent_loop,agentic_loop,dispatcher,router,task,thread_ops,session,submission,attachments,commands,traits_impl}.rs`；dispatcher 89节点/49测试 | **ironclaw** | ironclaw 是端到端实战 loop（thread/router/task/dispatcher 分层）。融合方向：保留 ironclaw agent/ 作为 L3 实现，并改为通过 `dasclaw_hooks::HookEngine`（ADR-113）+ `dasclaw_governance` 接入安全栈，丢弃 ironclaw 内部小 hook chain。 |
| 2 | **Compaction**（上下文压缩） | 45 / 1 / 20<br>`dasclaw_core/src/compaction.rs` | 6 / 1 / 2<br>`agent/compaction.rs` | **core** | core 单测覆盖更深（20 vs 2 测试节点）。ironclaw 切换调用 `dasclaw_core::compaction`，删除 agent/compaction.rs 内 6 节点的薄包装。 |
| 3 | **Context Monitor**（上下文水位监控） | 81 / 5 / 6<br>`dasclaw_core/src/context_monitor.rs`（+ `dasclaw_bash_*` 共名干扰，实际仅 1 文件相关） | 214 / 7 / 76<br>`context/{manager,memory,fallback,state,mod}.rs` + `agent/context_monitor.rs`；76 测试节点 | **ironclaw** | ironclaw 多层（memory/fallback/manager/state 分离）+ 测试覆盖远厚。融合方向：把 ironclaw `context/` 模块按 L3 吸收回 core（新建 `dasclaw_context_manager` 或并入 `dasclaw_core`），dasclaw_core/context_monitor.rs 收编为子模块。 |
| 4 | **Hook 系统** | 300 / 14 / 37<br>`dasclaw_hooks/src/{hook,registry,bash_permission_hook,bash_validation_hook,bundled,contract,lib}.rs` + 7 测试 | 130 / 5 / 33<br>`hooks/{bootstrap,bundled,hook,registry,mod}.rs` | **core**（ADR-113 已定） | core 是新统一 `HookEngine`，ironclaw 必须切换到 core 唯一入口，删除 ironclaw 内部 hook chain。ironclaw bootstrap 路径改为注册到 core registry。**红线**：ironclaw_safety 不删，作为 SafetyHook trait 后端被 core 装配。 |
| 5 | **Egress Gate**（出网治理） | 47 / 3 / 1<br>`dasclaw_governance/src/egress.rs` + `dasclaw_core/src/egress_apply.rs` + `dasclaw_governance/tests/egress_contract.rs` | 22 / 3 / 6<br>仅出现在 `tests/gemini_oauth_regression.rs` + `tests/shell_risk_regression.rs`（regression 测试断言） | **core** | core 是 EgressGate 的唯一实现归属，ironclaw 仅有调用方与回归测试。无吸收，保持 core 权威；ironclaw 的回归测试保留以验证集成。 |
| 6 | **DLP（入口脱敏）** | 0 / 0 / 0 | 0 / 0 / 0 | **范围外** | DLP 实现位于 `desktop-client/src/dlp/`（L2 客户端外壳），不在本次双图谱范围内。ADR-149 必须单独列条目：DLP 留在 L2，作为客户端层的入口管控；core 侧不重复造轮子。参考 ADR-148 PR-A。 |
| 7 | **Bash Validation**（命令安全校验） | 932 / 31 / 44<br>`dasclaw_bash_validation/`（fs_resolver/path_validation/redirects/security/{ast,context,early}/readonly/{bash,gh,git}_allowlist 等） | 45 / 1 / 0<br>仅 `tools/builtin/bash_validator.rs` 1 文件 | **core**（压倒性） | ironclaw `tools/builtin/bash_validator.rs` 改为调用 `dasclaw_bash_validation`，删除其内嵌实现。 |
| 8 | **Bash Permissions**（细粒度规则） | 335 / 25 / 12<br>`dasclaw_bash_permissions/`（compound/exact/prefix match + rule_parser + dangerous_patterns + shadowed_rule_detection 等） | 0 / 0 / 0 | **core 独有** | ironclaw 未实现。融合后 ironclaw agent 通过 `dasclaw_hooks::bash_permission_hook` 自动获得能力。 |
| 9 | **Sandbox**（内核执行隔离） | 1221 / 87 / 11<br>`dasclaw_sandbox`/`_linux`/`_windows`/`_sandboxing` 四 crate（cgroup_v2/bwrap/memorystatus/job_object/launcher_ipc 等） | 88 / 7 / 8<br>仅 `src/sandbox/` 客户端 wrapper | **core**（压倒性） | ironclaw 切到 core sandbox，保留 ironclaw 的客户端 wrapper 仅作为 L2 适配薄层。 |
| 10 | **Governance**（任务/团队/分支治理） | 477 / 17 / 3<br>`dasclaw_governance/`（branch_lock/policy_engine/recovery_recipes/stale_base/stale_branch/task_packet/task_registry/team_cron_registry/tool_visibility/trust_resolver/lane_events/green_contract） | 0 / 0 / 0 | **core 独有** | 无需吸收。ironclaw 后续如需 governance，直接依赖 core。 |
| 11 | **Session**（会话语义） | 160 / 3 / 73<br>`dasclaw_core/src/{session,session_hooks,session_manager}.rs` | 133 / 6 / 38<br>`agent/session.rs` + `llm/{session,openai_codex_session}.rs` + `tools/{builtin/session_fork,mcp/session}.rs` + telegram session | **两侧职责不同** | core 是"会话状态机/历史/forking"核心；ironclaw 端是"传输层 / 渠道 / fork-tool"包装。融合方向：core 保留权威 session core；ironclaw 各 session 文件改为调用 core，删除重复 state 持有。 |
| 12 | **Submission**（任务提交） | 53 / 1 / 34<br>`dasclaw_core/src/submission.rs`，34 测试节点 | 1 / 1 / 0<br>`agent/submission.rs`（仅 1 节点） | **core** | ironclaw 立即切换。 |
| 13 | **Routines**（定时/巡检/自愈） | 3 / 1 / 0 | 596 / 18 / 152<br>`routines/{cost_guard,heartbeat,job_monitor,scheduler,self_repair,routine,routine_engine,mod}.rs` + 多端 handler/cli/db/config | **ironclaw 独有** | core 仅占位（`dasclaw_routines` crate）。融合方向：保留 ironclaw routines 实现在原位（L3），后续可下沉 trait 到 `dasclaw_routines` 但不抢功能。 |
| 14 | **Orchestrator**（多 job 编排） | 69 / 1 / 0<br>仅 `dasclaw_sandbox_windows/src/setup_orchestrator.rs`（语义为 sandbox setup 编排，不同语义） | 126 / 5 / 15<br>`orchestrator/{api,auth,job_manager,reaper,mod}.rs` | **ironclaw 独有** | 同名但语义不同。core 侧那一文件是 sandbox 启动器，不算 agent orchestrator。融合方向：保留 ironclaw orchestrator 在 L2/L3，不下沉 core（属于运行时应用层）。 |
| 15 | **Registry**（多种注册表） | 131 / 3 / 13<br>`dasclaw_governance/{task_registry,team_cron_registry}` + `dasclaw_hooks/registry` | 382 / 12 / 146<br>`registry/{artifacts,catalog,embedded,installer,manifest,mod}` + `cli/registry` + `extensions/registry` + `hooks/registry` + `llm/registry` + `skills/registry` + `tools/registry` | **两侧职责不同** | core 仅 hook/task 注册（运行时基础）；ironclaw 提供 artifact/skill/extension/tool 注册（应用层）。融合方向：hook registry 收敛到 core；其它 6 类继续留在 ironclaw。 |
| 16 | **LLM Provider** | 337 / 14 / 4<br>`dasclaw_llm_provider`（http/sse/retry/types + providers/{anthropic,openai_compat,client}） | 1439 / 38 / 448<br>30 文件含 bedrock / circuit_breaker / claw_code / codex_chatgpt+auth / failover / gemini_oauth / github_copilot+auth / nearai / openai_codex / image_models / reasoning(+models) / response_cache / recording / retry | **ironclaw 显著更厚** | core 仅 2 种 provider + 基础 retry。融合方向：把 ironclaw 多 provider 实现（bedrock/codex/copilot/gemini/nearai/claw_code）、circuit_breaker、response_cache、failover、recording 反向吸收回 `dasclaw_llm_provider`，按 40-ironclaw-internalization.md 推进；保留 ironclaw `llm/prompt/{static,dynamic}_layer.rs` 在应用层。 |
| 17 | **Observability**（日志/Trace/Cache 观测） | 4 / 1 / 0<br>`dasclaw_observability` crate 仅占位 | 65 / 6 / 0<br>`observability/{log,multi,noop,prompt_cache,traits,mod}.rs` | **ironclaw 更厚** | 反向吸收 `prompt_cache` + `multi`/`noop` trait fanout 进 `dasclaw_observability`；ironclaw 端改 import。 |
| 18 | **Context Import**（旧客户端历史导入） | 0 / 0 / 0 | 131 / 14 / 82<br>`import/openclaw/{credentials,history,memory,reader,settings,mod}` + 6 个 e2e 测试 | **ironclaw 独有** | 应用层迁移工具，保留在 ironclaw；core 不需要。 |
| 19 | **Undo**（最近编辑回滚） | 28 / 1 / 7<br>`dasclaw_core/src/undo.rs` | 1 / 1 / 0<br>`agent/undo.rs` 仅 1 节点 | **core** | ironclaw 切到 core。 |
| 20 | **MCP**（Model Context Protocol） | 3 / 1 / 0<br>`dasclaw_mcp` crate  仅占位 | 521 / 15 / 221<br>`tools/mcp/{auth,client,config,factory,http_transport,process,protocol,session,stdio_transport,transport,unix_transport,mod}` + `cli/mcp` + mock server | **ironclaw 独有/远更厚** | 反向吸收到 `dasclaw_mcp`，按 40-ironclaw-internalization.md 推进。 |
| 21 | **LSP** | 3 / 1 / 0 | 107 / 5 / 37<br>`tools/builtin/lsp/{client,mod,protocol,server_config,tool}.rs` | **ironclaw 独有** | 反向吸收到 `dasclaw_lsp`。 |
| 22 | **Channels**（IO 渠道：web/wasm/relay/repl/signal/telegram） | 0 / 0 / 0 | 1969 / 59 / 476<br>59 文件含 wasm host/runtime + web handlers + relay + signal + telegram_host_config 等 | **ironclaw 独有**（L2/L3 应用层） | 保留在 ironclaw 原位，不下沉。 |
| 23 | **Secrets**（密钥/keychain/crypto） | 0 / 0 / 0 | 164 / 9 / 61<br>`secrets/{agent_provider,crypto,keychain,store,types,mod}` + handler + config + tools/builtin/secrets_tools | **ironclaw 独有** | 反向吸收到一个新 crate `dasclaw_secrets`（或并入 `dasclaw_identity`），后续 PR；本 ADR 阶段保留 ironclaw。 |
| 24 | **Workspace Capability**（文件/嵌入/检索） | 84 / 2 / 0<br>`dasclaw_workspace_cap`（占位） | 361 / 10 / 64<br>`workspace/{chunker,document,embedding_cache,embeddings,hygiene,layer,privacy,repository,search,mod}.rs` | **ironclaw 独有** | 反向吸收到 `dasclaw_workspace_cap`，按 40-ironclaw-internalization.md 推进。 |
| 25 | **Tools Builtin** | 0 / 0 / 0 | 2719 / 86 / 875<br>file/code_edit/grep/glob/lsp/mcp/git/image_{gen,edit,analyze}/job/json/http/memory/echo/file_guard/html_converter/secrets_tools/routine 等 | **ironclaw 独有**（L3） | 保留在 ironclaw。其中 `bash_validator.rs` 需切换调用 `dasclaw_bash_validation`（见 #7）。 |
| 26 | **ironclaw_safety**（W1-W6 私货：脱敏 / 凭据检测 / 漏检校验） | 134 / 7 / 0（仅 `dasclaw_shell_command/command_safety`，语义为 Windows 安全命令名单） | 347 / 19 / 95<br>`crates/ironclaw_safety/src/{agent_hook,credential_detect,leak_detector,policy,sanitizer,validator,lib}` + 5 fuzz_targets + 2 benches + `src/safety/mod.rs` + `tests/e2e_safety_layer.rs` | **ironclaw 独有/不可丢**（红线） | **保留**。融合方向：ironclaw_safety 通过 `dasclaw_hooks::SafetyHook` trait 接入 core HookEngine，作为后端实现存在；不删、不重写、不下沉到 core 命名空间。ADR-113 + ADR-147 红线。 |
| 27 | **Auth / Identity** | 33 / 4 / 9<br>`dasclaw_identity` 部分 | ironclaw 端含 `llm/{codex_auth,gemini_oauth,github_copilot_auth}.rs` 等 OAuth 客户端 | **两侧职责不同** | core 是身份内核（identity/identifiers）；ironclaw 是 provider OAuth 客户端。互不替代。 |
| 28 | **Apply Patch / ExecPolicy / PTY / NetProxy / ProjectDocs / Features / Crash** | core 独有：apply_patch 88/6/10；execpolicy 153/11/1；pty 45/4/0；net_proxy 506/15/0；project_docs 73/4/3；features 26/1/0；crash 3/1/0 | 0 / 0 / 0 各项 | **core 独有** | ironclaw 后续接入即可，无需吸收回 ironclaw。 |

---

## 3. 红线与不动项

按 ADR-113 + ADR-147 + [40-ironclaw-internalization.md](40-ironclaw-internalization.md)：

1. **`desktop-client/ironclaw/crates/ironclaw_safety/` 整体保留**，作为 W1-W6 私货代表。融合策略是"装配到 core HookEngine"，而非"重写到 core"。
2. **`desktop-client/ironclaw/src/routines/`、`/orchestrator/`、`/channels/`、`/import/`、`/secrets/` 应用层保留**（L2/L3 归属）。
3. **DLP 不动**（位于 `desktop-client/src/dlp/`，超出本对比范围，由 ADR-148 PR-A 跟进）。
4. **Hook 系统统一**（ADR-113）：唯一入口是 `dasclaw_hooks::HookEngine` + 4 个 trait（SafetyHook / SandboxExecutor / SecretProvider / ApprovalGate）；ironclaw 内 hook chain 删除，hook registration 改走 core registry。

## 4. 反向吸收清单（从 ironclaw → core，按 40-ironclaw-internalization.md 推进）

| 能力 | 当前 ironclaw 文件 | core 目标 crate | 紧迫度 |
|---|---|---|---|
| LLM 多 provider（codex/copilot/gemini/bedrock/nearai/claw_code）+ circuit_breaker + response_cache + failover + recording | `src/llm/` 多文件 | `dasclaw_llm_provider` | 高（W6 主航道，按 40-ironclaw-internalization.md） |
| MCP client + transports | `src/tools/mcp/` | `dasclaw_mcp` | 高 |
| LSP client + protocol | `src/tools/builtin/lsp/` | `dasclaw_lsp` | 中 |
| Workspace embedding / chunker / search | `src/workspace/` | `dasclaw_workspace_cap` | 中 |
| Observability prompt_cache + multi/noop | `src/observability/` | `dasclaw_observability` | 中 |
| Context manager / memory / fallback / state | `src/context/` | `dasclaw_core` 或新 crate | 中 |

## 5. 切换清单（从 ironclaw → core，仅改 import 即可）

| 能力 | ironclaw 当前调用 | 切换到 |
|---|---|---|
| Compaction | `agent/compaction.rs` 自实现 | `dasclaw_core::compaction` |
| Submission | `agent/submission.rs` 自实现 | `dasclaw_core::submission` |
| Undo | `agent/undo.rs` 自实现 | `dasclaw_core::undo` |
| Bash Validation | `tools/builtin/bash_validator.rs` 自实现 | `dasclaw_bash_validation` |
| Bash Permissions | 未实现 | 新接入 `dasclaw_bash_permissions`（经 `dasclaw_hooks`） |
| Sandbox | `src/sandbox/` 客户端 wrapper 自实现 | `dasclaw_sandbox`（保留 wrapper 作为 L2 薄适配） |
| Hooks 入口 | `src/hooks/` 5 文件 | `dasclaw_hooks::HookEngine`（ADR-113） |
| EgressGate | regression 测试 | `dasclaw_governance::egress`（无代码改动，验证保留） |

---

## 6. 复现命令

```bash
cd /Users/nallylin/Documents/code/x-claw

# 重新采集节点/文件/测试三元组
CORE_DB=crates/.code-review-graph/graph.db
ICW_DB=desktop-client/ironclaw/.code-review-graph/graph.db
PAT='%agent%'   # 替换为目标关键字
for db in "$CORE_DB" "$ICW_DB"; do
  sqlite3 "$db" "
    SELECT
      (SELECT count(*) FROM nodes WHERE file_path LIKE '$PAT'),
      (SELECT count(DISTINCT file_path) FROM nodes WHERE file_path LIKE '$PAT'),
      (SELECT count(*) FROM nodes WHERE file_path LIKE '$PAT' AND is_test=1);
  "
done
```

## 7. Sources read

- [docs/plans/architecture-refactor/31-target-architecture.md](31-target-architecture.md) §L1–L6 分层
- [docs/plans/architecture-refactor/32-execution-plan.md](32-execution-plan.md) §W3-A / W6
- [docs/plans/architecture-refactor/40-ironclaw-internalization.md](40-ironclaw-internalization.md) §吸收路径
- [docs/plans/architecture-refactor/38-desktop-client-ironclaw-fork-inventory.md](38-desktop-client-ironclaw-fork-inventory.md) §fork 清单
- [docs/plans/architecture-refactor/adr-112-compatibility-evaluation.md](adr-112-compatibility-evaluation.md) §14 能力 × 3 harness
- [docs/plans/architecture-refactor/adr-113-hook-engine-unification.md](adr-113-hook-engine-unification.md) §HookEngine + 4 trait
- [docs/plans/architecture-refactor/adr-147-composite-safety-hook.md](adr-147-composite-safety-hook.md) §safety 复合 hook
- [docs/plans/architecture-refactor/dual-graph-stats.md](dual-graph-stats.md) §基线统计
- code-review-graph SQLite DB（commit `80d41c90`）：`crates/.code-review-graph/graph.db`、`desktop-client/ironclaw/.code-review-graph/graph.db`

---

Refs: #621 (step 2 / 3)
