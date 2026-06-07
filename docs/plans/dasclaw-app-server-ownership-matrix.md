# Dasclaw App Server Ownership Matrix

> 状态：Phase 0 草案
> 日期：2026-06-07
> 范围：`desktop-client` 后端能力到 `dasclaw_app_server` / `dasclaw_runtime` / Electron shell 的 ownership 拆分。

## 0. 过程透明记录

本文件是架构对账文档，并且为后续新增 `dasclaw_app_server_protocol` / `dasclaw_app_server` crate 做准备，因此按仓库规则先回答 4 问：

| 启动问题 | 结论 | 本轮处理 |
|---|---|---|
| 是否新增模块 / crate / 文件？ | 是，新增 ownership matrix 文档；后续可能新增 app-server crate | 先做图谱接入和等价实现核验，本轮不新增 Rust crate |
| 是否包含否定性结论？ | 是，会判断哪些能力不应搬入 app-server | 保留 Level 1/2/3 证据，避免主观边界 |
| 是否跨项目 / 跨层对账？ | 是，涉及 `crates`、`desktop-client/src`、`desktop-client/ironclaw`、`codex-cli-main` | 仅对目标 worktree 的必要子目录建图，不扫整仓 |
| 是否写架构对账类文档？ | 是 | 本文件记录证据和归属矩阵 |

### 0.1 图谱接入范围

已为目标 worktree `/Users/nallylin/Documents/code/x-claw-open-cowork-gui-poc` 构建并注册以下 code-review-graph 子图：

| Alias | Path | Build result |
|---|---|---|
| `xclaw-poc-crates` | `/Users/nallylin/Documents/code/x-claw-open-cowork-gui-poc/crates` | 597 files / 13,275 nodes / 86,316 edges |
| `xclaw-poc-desktop-src` | `/Users/nallylin/Documents/code/x-claw-open-cowork-gui-poc/desktop-client/src` | 72 files / 1,763 nodes / 14,499 edges |
| `xclaw-poc-ironclaw` | `/Users/nallylin/Documents/code/x-claw-open-cowork-gui-poc/desktop-client/ironclaw` | 441 files / 10,539 nodes / 105,669 edges |

`codex-cli-main` 仍使用既有主 worktree 图谱作为参考来源；本轮不在 `codex-cli-main` 修改代码。

### 0.2 三层核验证据

当前会话仍未暴露 `semantic_search` / `vscode_listCodeUsages` MCP 工具，因此本轮采用图谱接入后的可用替代路径，并明确限制：

| Level | 使用方式 | 结果 |
|---|---|---|
| Level 1 概念层 | code-review-graph 子图 + FTS 概念查询：`app server protocol initialize health capability`、`lifecycle initialize shutdown health`、`engine state policy dlp lifecycle`、`agent loop tool executor session thread approval` | 找到 `dasclaw_protocol`、`dasclaw_runtime::Agent` / `ToolExecutor` / tool lifecycle、`desktop-client` DLP/policy/engine/Tauri channel、IronClaw host agent；未发现可直接作为 Electron GUI control-plane 的 dasclaw app-server host |
| Level 2 符号层 | code-review-graph nodes/edges 定位关键类型和函数 | `crates/dasclaw_runtime/src/agent.rs` owns `ToolExecutor`、`ToolLifecycleEvent`、`AgentBuilder`；`desktop-client/src/dlp/*` owns DLP detector/integration；`desktop-client/ironclaw/src/agent/agent_loop.rs` owns legacy host `Agent` |
| Level 3 字面层 | `rg` 定位 `VercelUIStream`、`TauriChannel`、`ToolExecutor`、`AgenticLoop`、`app-server`、`health`、`capability` | 证实现有 Tauri transport 与 UI stream 在 `desktop-client/src`，agent loop/tool contract 在 `dasclaw_runtime`，Codex app-server 位于参考 repo |

已检查 `dasclaw app-server ownership matrix / protocol skeleton` 是否已有，结论：目标 worktree 中已有 app-server 架构计划和 desktop/runtime 边界文档，但未发现专门的 `desktop-client -> dasclaw_app_server` ownership matrix，也未发现可直接复用的 dasclaw-native app-server protocol skeleton 文档。

## 1. 一句话边界

`dasclaw_app_server` 是 local service host / GUI control plane，负责把现有 headless runtime 和本地产品服务稳定暴露给 Electron shell。

它不重新实现 agent loop，不重新定义 `ToolExecutor`，也不把 Tauri-specific state 原样复制成新的 God object。

```text
Electron / open-cowork shell
  -> dasclaw_app_server_protocol
  -> dasclaw_app_server
  -> dasclaw_runtime + dasclaw_core + capability crates
```

## 2. Phase 0/1 ownership matrix

| Capability | Current source | Future owner | Protocol surface | Phase 0/1 decision | Notes |
|---|---|---|---|---|---|
| Window/menu/tray/preload | Tauri UI / future Electron shell | Electron shell | none | 不迁入 app-server | shell 只负责进程 supervision 和 UI 交互 |
| Tauri command registration | `desktop-client/src/lib.rs::all_tauri_commands!()` | `desktop-client` legacy adapter | none | 保留 legacy | Electron 不继承 Tauri command 名称 |
| Tauri event transport | `desktop-client/src/tauri_channel.rs` | `desktop-client` legacy adapter | mapped events | 只作为语义来源 | 不复制 `TauriChannel` transport |
| Vercel UI stream shape | `desktop-client/src/vercel_ui_protocol.rs` | protocol 参考，最终由 `dasclaw_app_server_protocol` 定义稳定事件 | `event/*` | Phase 1 只纳入 lifecycle/log/health/capability | chat stream 留到后续切片 |
| Runtime bootstrap | `desktop-client/src/engine.rs` | `dasclaw_app_server` | `initialize`、`shutdown`、`health/check` | Phase 1 建最小 lifecycle skeleton | 不把完整 `engine.rs` 复制过去 |
| App host state | `desktop-client/src/state.rs` | 拆为 app-server state + legacy Tauri state | `lifecycle/status`、`capabilities/list` | Phase 1 只定义状态词汇 | 防止复制 `AppState` / `EngineState` |
| Agent loop | `crates/dasclaw_runtime` | `dasclaw_runtime` | none directly | 不迁移 | app-server 只托管和桥接 |
| Tool execution contract | `crates/dasclaw_runtime::ToolExecutor` | `dasclaw_runtime` | future `tool/*` events | 不重新定义 | app-server 组合 tool registry，不拥有 trait |
| Tool lifecycle event | `crates/dasclaw_runtime::ToolLifecycleEvent` | `dasclaw_runtime` emits, app-server adapts | future `tool/lifecycle` | Phase 1 不做全量 tool stream | 先保留 event slot |
| Session/thread vocabulary | `crates/dasclaw_core` + IronClaw host | `dasclaw_core` vocabulary, app-server host orchestration | future `thread/*` | Phase 1 声明 capability，暂不迁移 full session host | 后续要保证 `threadId` 强约束 |
| Approval primitive | `crates/dasclaw_runtime/src/approval.rs` | primitive in runtime, orchestration in app-server | future `approval/requested` / response method | Phase 1 声明 capability，暂不实现全链路 | GUI 关闭/IPC 失败必须 fail-safe |
| DLP detector / sanitizer | `desktop-client/src/dlp/*` | app-server local service, reusable core 后续再抽 | future `dlp_policy/*` | Phase 1 只声明 capability | 不在第一批迁移全量 DLP |
| Policy sync / managed policy | `desktop-client/src/policy_sync.rs`、`enterprise_policy_sync.rs`、`managed_policy.rs` | app-server local service | future `policy/status` | Phase 1 只声明 degraded/ready 状态 | admin-backend 仍是 source of truth |
| Model/provider config | `desktop-client/src/engine.rs` + provider crates | app-server service backed by provider crates | future `model/*` | Phase 1 只放入 capability matrix | health probe 不进入 agent loop |
| Channel manager | desktop/IronClaw channels | app-server notification bus + legacy adapters | future `event/*` | Phase 1 只定义 notification bus 边界 | 不把 web/telegram/slack 全量塞进第一批 |
| Jobs/routines | IronClaw host / routines crates | app-server orchestration, runtime/core vocabulary | future `job/*` | Phase 1 declared/stub | 第一批不迁移 |
| Skills/extensions | desktop/IronClaw | app-server registry service | future `skills/*` | Phase 1 declared/stub | 不做 marketplace |
| MCP tools | `crates/dasclaw_mcp` and desktop IPC wrappers | app-server tool registry composition | future `mcp/*` | Phase 1 declared/stub | 不阻塞 initialize/health |
| Sandbox status | `desktop-client/src/ipc/sandbox.rs` + sandbox crates | app-server platform adapter | future `sandbox/status` | Phase 1 declared/stub | health/status 不等于 agent turn |
| Logs/audit/reporting | desktop logger / data reporter | app-server local audit service | `log/entry` | Phase 1 可定义 event | 内容必须避免敏感数据泄露 |
| Codex app-server protocol | `codex-cli-main/codex-rs/app-server-protocol` | reference only | design reference | 不整体搬迁 | 借鉴 request/event/client 形态 |
| Codex app-server processor | `codex-cli-main/codex-rs/app-server` | none | none | 不搬入 Phase 0/1 | goal/thread processor 产品耦合重 |

## 3. Phase 0/1 最小切片

### Phase 0：文档与 contract 冻结

| Slice | Deliverable | Files |
|---|---|---|
| P0-1 | ownership matrix | `docs/plans/dasclaw-app-server-ownership-matrix.md` |
| P0-2 | protocol v0 method/event/capability schema | `docs/plans/dasclaw-app-server-protocol-v0.md` |
| P0-3 | app-server 架构计划引用上述两个文档 | `docs/plans/dasclaw-app-server-architecture-plan.md` |

### Phase 1：最小 Rust skeleton

Phase 1 才新增 crate，且新增前需要再跑一次三层核验并在 commit message 中写：

```text
已检查 dasclaw_app_server_protocol / dasclaw_app_server 是否已有，结论：目标 worktree 中已有 dasclaw_protocol/runtime/core 可复用组件，但没有专用 GUI control-plane app-server crate；本 PR 新增最小 protocol/lifecycle skeleton。
```

| Slice | Deliverable | Non-goal |
|---|---|---|
| P1-1 | `dasclaw_app_server_protocol` types for initialize/health/capabilities/lifecycle | 不迁移 chat stream |
| P1-2 | `dasclaw_app_server` router + lifecycle state machine skeleton | 不接真实 `Agent` |
| P1-3 | initialize returns capability matrix | 不虚报未实现 handler |
| P1-4 | health returns degraded/ready detail | 不做 DLP full sync |
| P1-5 | lifecycle changed event | 不做 Electron UI |

## 4. Guardrails

| Guardrail | Why |
|---|---|
| app-server 不 import Tauri types | 保持 Electron / CLI / future host 可复用 |
| app-server 不定义新的 `ToolExecutor` | runtime 已有 contract，重复定义会制造双真相 |
| app-server 不拥有 admin policy authoring | admin-backend 仍是 source of truth |
| app-server 不把 Codex app-server wholesale port 进来 | Codex processor 含产品假设，先借鉴协议结构 |
| capability matrix 只能声明真实 handler 或明确 `declared`/`stub` | 防止 GUI 猜能力或虚报能力 |
| lifecycle 是一等状态 | 启动失败、版本不兼容、policy degraded、DLP degraded 都必须可解释 |

## 5. Open questions

| Question | Recommended default |
|---|---|
| Phase 1 transport 先用什么？ | stdio JSON-RPC for sidecar PoC；保留 Unix socket / named pipe 作为后续 transport |
| protocol crate 是否复用 `crates/dasclaw_protocol`？ | 先新增 `dasclaw_app_server_protocol`，依赖/复用 `dasclaw_protocol` 的共享模型，避免把 OpenAI/Codex protocol 面直接暴露给 GUI |
| DLP/policy 什么时候迁移？ | initialize/health/capability skeleton 之后，先迁 local service 状态，再迁 enforcement |
| Codex app-server 代码是否搬迁？ | 不整体搬；可 selective port protocol/client helper，必须逐块说明产品耦合剥离 |
