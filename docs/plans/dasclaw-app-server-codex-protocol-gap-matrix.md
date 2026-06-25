# Dasclaw app-server 与 Codex app-server 协议缺口对照表

> 日期：2026-06-18
> 状态：能力补齐参考（删除线表示 P0-P4 honest subset、P5a filesystem / command exec 与 P5 sandbox protocol slice 中已完成的受测子集；未划掉项与 §9.1 剩余优先级仍按后续计划处理）
> 运行态限定：P4 删除线表示 app-server 的 `AppServerServices::real()` 服务集合与默认 sidecar 入口已经接入 honest subset；显式 `DASCLAW_APP_SERVER_RUNTIME=noop` 仍保留禁用/declared 语义。
> 目标：列出 `codex-cli-main` app-server 的协议面，和当前 `dasclaw-app-server` 做逐域对照，区分“只差协议 shape”、“agent 框架 `crates` 已有底座但 app-server 未接线”和“产品能力本身未定义”。

## 0. 过程透明记录

本文件是新增架构/协议对账文档，并且包含“缺失”“无法仅靠协议补齐”这类否定性结论，因此按仓库规则先完成 4 问与三层核验。

| 启动问题 | 结论 | 本轮处理 |
|---|---|---|
| 是否新增模块 / crate / 文件？ | 是，新增本对照表文档 | 先查已有 app-server 计划/协议文档，确认没有全量 Codex method/notification 对照表 |
| 结论是否包含否定语？ | 是，会判断 Dasclaw 缺哪些协议、哪些不能只靠协议补 | 用生成 schema、Rust 常量、路由表、能力矩阵交叉核验 |
| 是否跨项目对账？ | 是，涉及 `codex-cli-main`、`crates/dasclaw_app_server*`、`desktop-app` | `desktop-app` 只作为当前消费者参考，不把已废弃 `desktop-client` 当目标 |
| 是否写架构对账类文档？ | 是 | 本文区分证据、推论和未知，不把名称相似当成协议等价 |

三层核验记录：

| 层级 | 证据 | 结果 / 限制 |
|---|---|---|
| Level 1 语义层 | 刷新 `crates` / `desktop-app` 图谱后用 `semantic_search_nodes_tool` 查询 Codex app-server protocol / Dasclaw compatibility profile / desktop-app v2-shaped consumer / P4 MCP tool-resource-progress 等语义 | 查询结果仍偏噪声；只作为“已查语义层”的弱证据，不把它作为完成/缺失判断的唯一依据 |
| Level 2 符号层 | `lsp-mcp execute_lsp` 查询 `ThreadListResponse`、`TurnReadResponse`、`ReasoningTextDeltaEvent` 等 workspace symbols；本轮追加 `crates/dasclaw_app_server/src/mcp_service.rs` `document_symbols` | 符号层确认 thread list / turn read / reasoning event schema 落在 `dasclaw_app_server_protocol`；追加确认 `AppServerMcpService` 真实包含 `list_status`、`reload`、`call_tool`、`read_resource`、`drain_tool_call_progress_events`、`availability`，且 `oauth_login` 仍是单独 fail-safe 方法；再结合 router 与 producer 精确检索区分“已接线”和“仅 schema” |
| Level 3 字面层 | Codex 生成协议 union：`codex-cli-main/codex-rs/app-server-protocol/schema/typescript/{ClientRequest,ServerNotification,ServerRequest,ClientNotification}.ts`；Dasclaw 常量、能力矩阵、router：`crates/dasclaw_app_server_protocol/src/lib.rs`、`crates/dasclaw_app_server/src/lib.rs`；`rg` 精确核验 router / producer / tests | Codex：74 个 `ClientRequest`、61 个 `ServerNotification`、9 个 `ServerRequest`、1 个 `ClientNotification`。Dasclaw 当前以 `phase_one_methods()` / `phase_one_events()` 和 router 为准；P4 已新增 jobs / skills / logs / MCP honest subset |

已检查 `dasclaw-app-server / codex app-server protocol gap matrix` 是否已有，结论：已有 `docs/plans/dasclaw-app-server-protocol-v0.md`、`docs/plans/dasclaw-app-server-ownership-matrix.md`、`docs/plans/dasclaw-codex-app-server-ai-sdk-compat-plan.md`，但未发现“Codex 全量 ClientRequest / ServerNotification / ServerRequest / ClientNotification 与 Dasclaw 当前协议逐域缺口”的对照表；本文补齐该空白。

子 agent 只读交叉检查结论与主线一致：`codex-cli-main/codex-rs/app-server-protocol/src/schema.rs` 在当前仓库不存在，实际应以 `src/protocol/v1.rs` 和 `schema/typescript/*.ts` 作为协议证据，其中 TypeScript 生成文件最适合逐项枚举。

2026-06-22 剩余优先级复核：再次用 `semantic_search_nodes_tool` 查询 app-server remaining protocol gaps / thread lifecycle / approval tool call / MCP OAuth / sandbox PTY / config git fuzzy search 等语义；LSP `workspace_symbols` 查询 `thread_archive`、`turn_steer`、`ITEM_TOOL_CALL`、`ITEM_PERMISSIONS_REQUEST_APPROVAL`、`mcp_server_oauth_login`、`config_requirements_read`；最后用 `rg` 精确核验 router、producer、capability matrix 与文档。结论：`configRequirements/read` 已是 sandbox requirements 子集；MCP OAuth 有 fail-safe route 但真实 OAuth owner 未接；`ITEM_TOOL_CALL` 与 `ITEM_PERMISSIONS_REQUEST_APPROVAL` 仍只停在协议/客户端转发层，未发现 app-server service producer；thread archive/fork/resume 等扩展生命周期未发现 app-server owner。

2026-06-23 R4 thread lifecycle owner 落地复核：实现计划见 `docs/superpowers/plans/2026-06-22-dasclaw-app-server-thread-lifecycle-persistence-owner.md`。本轮新增 `ThreadLifecycleHost` JSON snapshot owner，并把 `thread/resume`、`thread/fork`、`thread/archive`、`thread/unarchive`、`thread/unsubscribe`、`thread/name/set`、`thread/metadata/update`、`thread/rollback`、`thread/loaded/list`、`thread/inject_items`、`thread/goal/set|get|clear` 接到 router / supported methods / schema / capability profile；`thread/tokenUsage/updated` 已由真实 runtime bridge 的 completed usage 推送并由 app-server 累加持久化。`thread/compact/start` 现在有 fail-safe route：默认返回 `CAPABILITY_UNAVAILABLE`，只有 runtime bridge 显式声明 `thread_compact` feature 才会记录 `compactedTurnId` 并发 `thread/compacted`；真实 Dasclaw runtime compact operation 仍未接入。验证命令：`CARGO_TARGET_DIR=target/codex-r4-verify RUSTC_WRAPPER= cargo check -p dasclaw_app_server --tests`；`CARGO_TARGET_DIR=target/codex-r4-verify RUSTC_WRAPPER= cargo nextest run -p dasclaw_app_server -p dasclaw_app_server_protocol -E 'test(thread_lifecycle_) | test(thread_archive_) | test(thread_resume) | test(thread_fork) | test(thread_unsubscribe) | test(thread_rollback) | test(thread_loaded_list) | test(thread_inject_items) | test(thread_goal_) | test(thread_token_usage_) | test(thread_compact_) | test(dasclaw_runtime_bridge_forwards_nonzero_completed_usage_before_completion) | test(dasclaw_runtime_bridge_uses_snapshot_model_call_mode) | test(implemented_capability_methods_are_routable) | test(phase_one_schema_methods_match_implemented_capability_methods) | test(compatibility_profile_is_descriptive_subset_without_aliases)'`。

2026-06-23 R4 review 修正：独立 code-review / architect lane 复核后，补齐 `thread/unarchive` 返回 nested `thread`、`thread/name/set` 空响应、`thread/name/updated.threadName` 以及 `thread/inject_items` 结构化 append 语义，避免测试侧适配错误 shape。R4 删除线仍只代表 app-server owner / protocol shape / fail-safe 子集完成；`thread/compact/start` 与 `thread/compacted` 继续不划掉，metadata 目前仅 `gitInfo` patch，goal 的 `timeUsedSeconds` 仍无真实计时 owner。

2026-06-22 R2 runtime request owner 复核：基于用户提供的 R2 runtime request owners 执行计划执行。Level 1 语义搜索确认 `dasclaw_protocol` 已有 request-user-input / dynamic-tool / permissions / approval DTO；Level 2 LSP workspace symbols 确认 app-server bridge、protocol request structs 与 runtime approval-only response 面；Level 3 `rg` 确认 `dasclaw_app_server_protocol` 已有四类 `ServerRequest` 常量，且本轮前 `DasclawAgentRuntimeBridge::resolve_server_request` 对 dynamic tool 只返回成功、user-input/file-change 未接 runtime waiter、permissions 只粗略映射 approval。实现后验证：`RUSTC_WRAPPER= cargo check -p dasclaw_app_server --tests`；`RUSTC_WRAPPER= cargo nextest run -p dasclaw_app_server -E 'test(default_runtime_bridge_advertises_r2_request_owner_capabilities) | test(dynamic_tool_json_rpc_response_resumes_runtime_waiter_once) | test(user_input_json_rpc_response_resumes_runtime_waiter) | test(permissions_json_rpc_denial_resumes_runtime_waiter_fail_safe) | test(file_change_json_rpc_denial_resumes_runtime_waiter_without_executor) | test(malformed_dynamic_tool_json_rpc_response_unblocks_runtime_waiter_fail_safe) | test(timed_out_dynamic_tool_json_rpc_request_unblocks_runtime_waiter_fail_safe) | test(shutdown_cancels_pending_dynamic_tool_json_rpc_request) | test(non_command_permissions_denial_rejects_legacy_approval_waiter) | test(agent_runtime_bridge_does_not_emit_dynamic_request_from_tool_start) | test(dynamic_tool_request_waits_for_renderer_response_and_cleans_pending) | test(user_input_request_returns_renderer_answers) | test(permissions_denial_is_fail_safe) | test(file_change_denial_prevents_inner_executor) | test(stale_runtime_request_response_is_rejected)'`。结论：dynamic tool、user-input、permissions denial、file-change denial、stale/duplicate/malformed response、timeout、shutdown cancel 的 runtime 挂起-响应 owner 已有受测闭环；renderer 真实 UI、完整 patch apply engine、MCP elicitation 与 sandbox enforcement 不属于本轮完成范围。

2026-06-25 R6 producer 复核：执行计划见 `docs/superpowers/plans/2026-06-25-dasclaw-app-server-r6-producer-completion.md`。本轮把 R6 readiness 从“delivery 存在”改为“真实 producer 存在”：`hook/started` / `hook/completed` 已接真实 `HookRegistry` observer，并覆盖 tool `preToolUse`、turn inbound `userPromptSubmit`、turn outbound `postToolUse`，同时保留 hook modified content 语义；`warning` 仅在 Linux 上由实际 resolved sandbox policy 调用 `system_bwrap_warning` 后广告和发送，非 Linux 不广告 generic warning；`configWarning` / `deprecationNotice` 继续来自真实 config/startup path；`guardianWarning` 来自真实 `AutoApprovalReviewCompleted` runtime update，并由 `auto_approval_review` feature gate 控制 readiness。`model/rerouted` 和 `model/verification` 仍不广告：前者缺可信 reroute reason producer，后者缺真实 provider metadata parser 证据。验证：`cargo nextest run -p dasclaw_app_server -E 'test(turn_start_runs_inbound_hook) | test(inbound_hook_rejection) | test(inbound_hook_modification) | test(completed_turn_runs_outbound_hook) | test(outbound_hook_rejection) | test(outbound_hook_modification) | test(auto_approval_review_completed) | test(guardian_warning) | test(real_r6_services_advertise_only_wired_notification_producers) | test(capabilities_advertise_ready_r6_warning_producers_only) | test(manually_queued_warning_delivery_does_not_flip_producer_readiness) | test(turn_start_warning_conversion_error) | test(converts_workspace_policy_to_codex_policy_explicitly) | test(codex_policy_conversion_rejects_relative_workspace_roots) | test(turn_start_external_sandbox_policy_does_not_emit_generic_warning) | test(warning_service_events_drain_to_json_rpc_notifications)'`；`cargo check -p dasclaw_app_server --tests`；`cargo fmt --all --check`。

2026-06-25 P0/P1 remaining 复核：按 `docs/superpowers/plans/2026-06-25-dasclaw-app-server-p0-p1-remaining.md` 补齐后，`thread/shellCommand` 与 `thread/approveGuardianDeniedAction` 已有真实 route owner、synthetic turn 持久化与通知链路；`command/exec` 的 PTY streaming 现已接受 `read-only`/`workspace-write` sandbox override、保持 `danger-full-access` fail-closed，并补齐 non-PTY split stdout/stderr streaming 与 streaming control audit 覆盖。验证：`cargo nextest run -p dasclaw_app_server -E 'test(command_service_tty_streaming_accepts_read_only_sandbox_policy) | test(command_service_tty_streaming_rejects_danger_full_access_even_after_streaming_support) | test(command_service_streaming_control_audit_redacts_payload_but_records_outcome) | test(json_rpc_thread_shell_command_returns_empty_object_and_emits_turn_notifications) | test(json_rpc_thread_approve_guardian_denied_action_replays_stashed_command_action)'`；`cargo nextest run -p dasclaw_app_server_protocol -E 'test(protocol_declares_thread_shell_command_and_guardian_replay_methods) | test(thread_shell_command_params_serialize_in_codex_shape) | test(guardian_replay_params_accept_serialized_guardian_event)'`。

## 1. 总结

当前 `dasclaw-app-server` 不是完整 Codex app-server v2 实现，而是一个 Dasclaw native app-server，其已实现的 chat-session surface 使用 Codex app-server v2-shaped method/event/payload。`codex_app_server_v2_chat_session_subset` compatibility profile 是该子集的描述性 metadata，不是第二套 wire layer，也不增加 alias。

Protocol-shape consolidation belongs before capability expansion. The first implementation phase should remove dev-stage aliases and make the existing native chat-session surface v2-shaped. Missing Codex app-server capabilities remain explicit gaps; they are not unlocked by a translation layer.

直接证据：

| 证据 | 说明 |
|---|---|
| `ClientRequest.ts:79` | Codex 客户端请求 union 一行列出 74 个 method |
| `ServerNotification.ts:69` | Codex 服务端通知 union 一行列出 61 个 notification |
| `ServerRequest.ts:18` | Codex 服务端发起请求 union 一行列出 9 个 request |
| `ClientNotification.ts:5` | Codex 客户端 notification 只有 `initialized` |
| `crates/dasclaw_app_server_protocol/src/lib.rs` | Dasclaw 当前常量与 `phase_one_methods()` / `phase_one_events()` 已包含 P4 jobs / skills / logs / MCP schema；implemented capability 仍按真实 service availability 过滤 |
| `crates/dasclaw_app_server_protocol/src/lib.rs` | Dasclaw Phase 1 实现 `protocol`、`lifecycle`、`health`、`session`、`model_provider`；P3 feature-gated runtime 下可启用 `approval` / `tools` / `sandbox` 子集；P4 增加 `jobs`、`skills`、`logs` 与 MCP honest subset，并把 MCP capability 拆成 method/event 粒度，避免广告未接线的 OAuth |
| `crates/dasclaw_app_server_protocol/src/lib.rs` | `codex_app_server_v2_chat_session_subset` profile 明确是 `ChatSessionSubset`，列出当前真实可用的 chat-session、thread lifecycle、thread goal、R5 plan / diff / raw producer 与 model 方法；`turn/steer` route 保持 schema/routable，但 capability 只在 runtime bridge 声明 `turn_steer` 后广告；仍 opt out 完整 tool / approval / sandbox / compact / rich input 等未完整实现的 Codex 能力，不再 opt out 已接线的 `codex.diff` / `codex.plan` |
| `crates/dasclaw_app_server/src/lib.rs` | Dasclaw router 只实际路由 native schema 中的 method，其他 method 会落到 `method_not_found`；P4 MCP status/reload/tool/resource/progress 通过真实 service owner 接线，OAuth route 保持 fail-safe 且不广告 implemented capability |
| `crates/dasclaw_app_server/src/lib.rs` | 运行时健康状态按 bridge feature gate 呈现 tools / sandbox；app services 按真实 service readiness 汇报 jobs / skills / logs / MCP honest subset；R5 raw reasoning / raw response item producer 与 plan/diff source-event forwarding 已从 provider -> `AgentEvent` -> app-server notification 接通；`turn/steer` route 已接真实 `DasclawAgentRuntimeBridge::steer_turn`，通过 `Agent::inject_user_message` 在 running turn 下一轮 LLM 调用前按序注入 queued user messages；DLP policy unavailable fail-safe |
| `crates/dasclaw_app_server/src/main.rs` | 默认 sidecar 入口通过 `AppServerServices::real()` 注入 logs / jobs / skills / MCP honest subset；显式 `DASCLAW_APP_SERVER_RUNTIME=noop` 仍保留禁用/declared 行为 |

结论分三层：

| 层级 | 判断 |
|---|---|
| 本轮已完成 | `model/list`；`initialize` native shape；`thread/start` / `thread/read` / `thread/list` / `thread/turns/list` 返回当前 chat-session subset 的 Codex-shaped view；R4 thread lifecycle owner 已接 `thread/resume` / `fork` / `archive` / `unarchive` / `unsubscribe` / `name/set` / `metadata/update` / `rollback` / `loaded/list` / `inject_items` / `goal/*` 与 `thread/status/changed` / archived / unarchived / name / goal / token usage events；`thread/shellCommand` 与 `thread/approveGuardianDeniedAction` 已接 thread-scoped owner、synthetic turn 持久化与通知链路；`thread/compact/start` 已有 fail-safe feature gate 但默认 runtime 不支持真实 compact；`turn/start` / `turn/interrupt` / `turn/read` 返回 nested Codex `turn` view 或 empty response；R5 `turn/steer` 已由真实 `DasclawAgentRuntimeBridge` 注入 running turn，`turn/plan/updated`、`turn/diff/updated` 使用 v2-shaped view/notification；失败和中断都通过 `turn/completed.turn.status` 表达；`item/started`、`item/agentMessage/delta`、`item/reasoning/summaryTextDelta`、`item/reasoning/summaryPartAdded`、`item/reasoning/textDelta`、`item/plan/delta`、`rawResponseItem/completed`、`item/completed` 已接 v2-shaped producer；P3 command approval server-request loop、`approval/respond`、tool output/result notification、timeout/interrupt/shutdown fail-safe；P4 `jobs/list` / `jobs/read`、`skills/list` / `skills/config/write` / `skills/changed`、`log/entry`、MCP `mcpServerStatus/list` / `config/mcpServer/reload` / `mcpServer/resource/read` / `mcpServer/tool/call` / `item/mcpToolCall/progress` / startup status event 已通过 `AppServerServices::real()` 接入默认 sidecar 与受测真实 service owner；`command/exec` streaming path 已补齐 sandbox-aware PTY、non-PTY split stdout/stderr 与 control audit redaction coverage；`desktop-app` 已消费单一 native v2-shaped surface |
| 协议同名/近似可用 | ~~`initialize`~~、~~`thread/start`~~、~~`thread/read`~~、~~`thread/list`~~、~~`turn/start`~~、~~`turn/interrupt`~~、若干 item/turn streaming notification |
| 协议缺口但可通过 compatibility view 补 | 完整 Codex `Thread` history / lifecycle 字段、更多 item/turn 细粒度 payload shape |
| agent 框架 `crates` 已有底座但 app-server 未完全接线 | approval / tool execution / sandbox primitives 已在 `crates` 下的 runtime/tool/sandbox 相关 crate 中存在；P3 已补 command approval request/response、tool lifecycle notification、service health 与 capability gating 子集；缺的是 standalone command/fs service、dynamic tool registry/call、permission/file-change approval producer、MCP elicitation 和更完整的审计面 |
| 已有分散底座但 app-server 尚未成为 owner | P4 已把 logs / jobs / skills 与 MCP status/reload/tool-call/resource-read/progress honest subset 收到 app-server service owner；MCP OAuth、filesystem / git / search / config 等仍在 `crates`、历史参考实现或 `desktop-app` manager 中分散存在，缺的是协议接线、能力健康状态和安全边界 |
| 产品/服务能力尚未定义或未迁移 | account / plugin marketplace / app list / device key / external agent import / Codex review 等仍偏 Codex 产品域或需要先定义 Dasclaw 产品语义 |

## 2. Codex app-server 协议清单

> 标记说明：删除线表示 Dasclaw 本轮 P0-P4 honest subset 已补齐、已接入 Codex-compatible view，或已有受测的协议/桥接子集；同一行里未划掉的协议仍按后续计划处理。P4 jobs / skills / logs / MCP 的删除线已覆盖默认 sidecar 的 `AppServerServices::real()` 装配路径；显式 noop 模式仍是禁用路径。

### 2.1 ClientRequest：74 个

来源：`codex-cli-main/codex-rs/app-server-protocol/schema/typescript/ClientRequest.ts:79`。

| 域 | Codex method |
|---|---|
| 初始化 | ~~`initialize`~~ |
| Thread lifecycle / history | ~~`thread/start`~~、~~`thread/resume`~~、~~`thread/fork`~~、~~`thread/archive`~~、~~`thread/unsubscribe`~~、~~`thread/name/set`~~、~~`thread/metadata/update`~~、~~`thread/unarchive`~~、`thread/compact/start`、~~`thread/shellCommand`~~、~~`thread/approveGuardianDeniedAction`~~、~~`thread/rollback`~~、~~`thread/list`~~、~~`thread/loaded/list`~~、~~`thread/read`~~、~~`thread/turns/list`~~、~~`thread/inject_items`~~ |
| Turn lifecycle | ~~`turn/start`~~、~~`turn/steer`~~、~~`turn/interrupt`~~ |
| Skills / plugins / marketplace / apps | ~~`skills/list`~~、~~`skills/config/write`~~、`plugin/list`、`plugin/read`、`plugin/install`、`plugin/uninstall`、`marketplace/add`、`marketplace/remove`、`marketplace/upgrade`、`app/list` |
| Device key | `device/key/create`、`device/key/public`、`device/key/sign` |
| Filesystem | ~~`fs/readFile`~~、~~`fs/writeFile`~~、~~`fs/createDirectory`~~、~~`fs/getMetadata`~~、~~`fs/readDirectory`~~、~~`fs/remove`~~、~~`fs/copy`~~、~~`fs/watch`~~、~~`fs/unwatch`~~ |
| Review / model / experiment | ~~`review/start`~~、~~`model/list`~~、`experimentalFeature/list`、`experimentalFeature/enablement/set` |
| MCP | `mcpServer/oauth/login`、~~`config/mcpServer/reload`~~、~~`mcpServerStatus/list`~~、~~`mcpServer/resource/read`~~、~~`mcpServer/tool/call`~~ |
| Sandbox / account / feedback | `windowsSandbox/setupStart`、`account/login/start`、`account/login/cancel`、`account/logout`、`account/rateLimits/read`、`account/sendAddCreditsNudgeEmail`、`account/read`、`feedback/upload` |
| Local command | ~~`command/exec`~~、~~`command/exec/write`~~、~~`command/exec/terminate`~~、~~`command/exec/resize`~~ |
| Config / external agent | ~~`config/read`~~、~~`config/value/write`~~、~~`config/batchWrite`~~、~~`configRequirements/read`~~、`externalAgentConfig/detect`、`externalAgentConfig/import` |
| Misc | ~~`getConversationSummary`~~、~~`gitDiffToRemote`~~、`getAuthStatus`、~~`fuzzyFileSearch`~~ |

### 2.2 ServerNotification：61 个

来源：`codex-cli-main/codex-rs/app-server-protocol/schema/typescript/ServerNotification.ts:69`。

| 域 | Codex notification |
|---|---|
| Error / warnings | `error`、~~`warning`~~、~~`guardianWarning`~~、~~`deprecationNotice`~~、~~`configWarning`~~ |
| Thread | ~~`thread/started`~~、~~`thread/status/changed`~~、~~`thread/archived`~~、~~`thread/unarchived`~~、`thread/closed`、~~`thread/name/updated`~~、~~`thread/goal/updated`~~、~~`thread/goal/cleared`~~、~~`thread/tokenUsage/updated`~~、`thread/compacted` |
| Realtime | `thread/realtime/started`、`thread/realtime/itemAdded`、`thread/realtime/transcript/delta`、`thread/realtime/transcript/done`、`thread/realtime/outputAudio/delta`、`thread/realtime/sdp`、`thread/realtime/error`、`thread/realtime/closed` |
| Turn | ~~`turn/started`~~、~~`turn/completed`~~、~~`turn/diff/updated`~~、~~`turn/plan/updated`~~ |
| Item streaming | ~~`item/started`~~、~~`item/completed`~~、~~`rawResponseItem/completed`~~、~~`item/agentMessage/delta`~~、~~`item/plan/delta`~~、~~`item/reasoning/summaryTextDelta`~~、~~`item/reasoning/summaryPartAdded`~~、~~`item/reasoning/textDelta`~~ |
| Approval / command / file change | ~~`item/autoApprovalReview/started`~~、~~`item/autoApprovalReview/completed`~~、~~`command/exec/outputDelta`~~、~~`item/commandExecution/outputDelta`~~、~~`item/commandExecution/terminalInteraction`~~、`item/fileChange/outputDelta`、`item/fileChange/patchUpdated`、~~`serverRequest/resolved`~~ |
| MCP | ~~`item/mcpToolCall/progress`~~、`mcpServer/oauthLogin/completed`、~~`mcpServer/startupStatus/updated`~~ |
| Account / app / skills | `account/updated`、`account/rateLimits/updated`、`account/login/completed`、`app/list/updated`、~~`skills/changed`~~ |
| External / fs / model / fuzzy / hooks / Windows | `externalAgentConfig/import/completed`、~~`fs/changed`~~、`model/rerouted`、`model/verification`、~~`fuzzyFileSearch/sessionUpdated`~~、~~`fuzzyFileSearch/sessionCompleted`~~、~~`hook/started`~~、~~`hook/completed`~~、`windows/worldWritableWarning`、`windowsSandbox/setupCompleted` |

### 2.3 ServerRequest：9 个

来源：`codex-cli-main/codex-rs/app-server-protocol/schema/typescript/ServerRequest.ts:18`。

| 域 | Codex server-initiated request |
|---|---|
| Approval | ~~`item/commandExecution/requestApproval`~~、~~`item/fileChange/requestApproval`~~、~~`item/permissions/requestApproval`~~、`applyPatchApproval`、`execCommandApproval` |
| Tool / user input | ~~`item/tool/requestUserInput`~~、~~`item/tool/call`~~ |
| MCP elicitation | `mcpServer/elicitation/request` |
| Account token | `account/chatgptAuthTokens/refresh` |

### 2.4 ClientNotification：1 个

来源：`codex-cli-main/codex-rs/app-server-protocol/schema/typescript/ClientNotification.ts:5`。

| 域 | Codex client notification |
|---|---|
| Connection lifecycle | `initialized` |

## 3. Dasclaw 当前协议清单

### 3.1 Method：当前 phase_one schema

来源：`crates/dasclaw_app_server_protocol/src/lib.rs` 与 `phase_one_methods()`。

| 能力域 | Dasclaw method |
|---|---|
| Protocol | ~~`initialize`~~、~~`protocol/schema`~~ |
| Health | ~~`health/check`~~、~~`capabilities/list`~~ |
| Lifecycle | ~~`lifecycle/status`~~、~~`shutdown`~~ |
| Session / thread | ~~`thread/start`~~、~~`thread/list`~~、~~`thread/read`~~、~~`thread/turns/list`~~ |
| Session / turn | ~~`turn/start`~~、~~`turn/steer`~~、~~`turn/interrupt`~~、~~`turn/read`~~ |
| Model provider | ~~`model/list`~~、~~`modelProvider/selectForNextTurn`~~ |
| Approval | ~~`approval/respond`~~ |
| Jobs | ~~`jobs/list`~~、~~`jobs/read`~~ |
| Skills | ~~`skills/list`~~、~~`skills/config/write`~~ |
| MCP | `mcpServer/oauth/login`、~~`config/mcpServer/reload`~~、~~`mcpServerStatus/list`~~、~~`mcpServer/resource/read`~~、~~`mcpServer/tool/call`~~ |
| Filesystem | ~~`fs/readFile`~~、~~`fs/writeFile`~~、~~`fs/createDirectory`~~、~~`fs/getMetadata`~~、~~`fs/readDirectory`~~、~~`fs/remove`~~、~~`fs/copy`~~、~~`fs/watch`~~、~~`fs/unwatch`~~ |
| Local command | ~~`command/exec`~~、~~`command/exec/write`~~、~~`command/exec/terminate`~~、~~`command/exec/resize`~~ |

### 3.2 Event：当前 phase_one schema

来源：`crates/dasclaw_app_server_protocol/src/lib.rs` 与 `phase_one_events()`。

| 能力域 | Dasclaw event |
|---|---|
| Protocol | ~~`notifications/initialized`~~ |
| Lifecycle / health / logs | ~~`lifecycle/changed`~~、~~`health/changed`~~、~~`capabilities/changed`~~、~~`log/entry`~~ |
| Thread / turn | ~~`thread/started`~~、~~`thread/status/changed`~~、~~`thread/archived`~~、~~`thread/unarchived`~~、~~`thread/name/updated`~~、~~`thread/goal/updated`~~、~~`thread/goal/cleared`~~、~~`thread/tokenUsage/updated`~~、~~`turn/started`~~、~~`turn/completed`~~、~~`turn/plan/updated`~~、~~`turn/diff/updated`~~ |
| Item streaming | ~~`item/started`~~、~~`item/agentMessage/delta`~~、~~`item/reasoning/summaryTextDelta`~~、~~`item/reasoning/summaryPartAdded`~~、~~`item/reasoning/textDelta`~~、~~`item/plan/delta`~~、~~`rawResponseItem/completed`~~、~~`item/completed`~~ |
| Approval / tool | ~~`serverRequest/resolved`~~、~~`item/commandExecution/outputDelta`~~、~~`item/commandExecution/terminalInteraction`~~ |
| Skills / MCP | ~~`skills/changed`~~、~~`item/mcpToolCall/progress`~~、`mcpServer/oauthLogin/completed`、~~`mcpServer/startupStatus/updated`~~ |
| Filesystem / command | ~~`fs/changed`~~、~~`command/exec/outputDelta`~~ |
| Error | `error` |

### 3.3 当前 desktop-app 消费面

`desktop-app` 只消费 Dasclaw 当前子集：

| 证据 | 说明 |
|---|---|
| `desktop-app/src/main/appServerManager.ts` | 初始化发送 native capability 请求，并附带 model provider config；chat-session wire shape 本身已经是 v2-shaped，不再通过 alias profile 切换 |
| `desktop-app/src/main/appServerManager.ts` | ~~`modelProvider/list`~~ 仍是客户端 renderer alias，但内部已转发到 app-server ~~`model/list`~~；~~`modelProvider/selectForNextTurn`~~ 由 manager 校验 renderer 参数后转发给 app-server route |
| `desktop-app/src/renderer/src/lib/appServerTurnTracker.ts` | renderer 聚合 item content / reasoning delta，并只从 nested ~~`turn/completed`~~ 读取 terminal state |

这说明当前消费者不要求 Codex 全量协议，但如果目标改成“让 Dasclaw app-server 具备接近 Codex app-server 的完整能力”，需要补的是服务能力，不只是 provider/transport 适配。

## 4. 缺口分类标准

| 标记 | 含义 |
|---|---|
| A：已支持/近似支持 | Dasclaw 有同名或等价 method/event，但 payload shape 可能不同 |
| B：协议 shape 缺口 | 底层能力方向存在，主要需要补 Codex-compatible request/response/notification view |
| C：app-server 暴露缺口 | 能力可能存在于 desktop-app/manager/runtime 侧，但 app-server 没有一等接口 |
| D1：已有底座但 app-server 未接线 | agent 框架 `crates` 或 `desktop-app` manager 有 primitive，但 Dasclaw app-server capability matrix / service health 仍是 declared future、disabled、unavailable 或没有对应 method |
| D2：产品/服务能力缺口 | 当前未看到可直接作为 app-server 服务的 owner，需要先定义服务、权限、状态和持久化 |
| E：Codex 产品专属或暂不建议补 | 和 OpenAI/Codex 账户、插件市场、Windows 特定沙箱、Codex cloud/product 体验绑定；除非产品目标明确要求，否则不纳入 Dasclaw native core |

### 4.1 本轮误分类排查：已有底座但未接入 app-server 的关键项

| 能力 | 已有底座证据 | app-server 当前缺口 |
|---|---|---|
| Approval | `crates/dasclaw_runtime/src/approval.rs` 已有 `ApprovalPolicy`、`ApprovalRequest`、`ApprovalDecision`、`ApprovalInbox`、`Approver`；`Agent::respond_to_approval` 可回填 GUI 决策 | ~~P3 已把 `AgentEvent::ApprovalNeeded` 接成 `item/commandExecution/requestApproval` -> `approval/respond` -> `Agent::respond_to_approval`，并补 `serverRequest/resolved`、timeout/cancel/shutdown fail-safe~~；~~R2 已把 file-change / permission approval 接成 runtime-owned pending waiter 与 JSON-RPC response roundtrip~~；guardian denied action 已接 thread-scoped replay owner；长期 permission grant policy、完整 patch apply engine / diff UI 仍未完成 |
| Tool execution | `crates/dasclaw_runtime/src/agent.rs` 有 `ToolExecutor`；`AgentBuilder::tool_executor*` 可接 executor；`tool_dispatch.rs` 有 approval gate -> egress -> executor -> sanitizer 的顺序 pipeline | ~~P3 已接 `ToolCallStart` / `ToolResult` 到 `item/commandExecution/outputDelta` 与 `item/commandExecution/terminalInteraction`~~；~~R2 已接 `item/tool/call` 与 `item/tool/requestUserInput` 的 runtime-owned pending waiter / JSON-RPC response roundtrip~~；仍没有 dynamic tool registry/list、MCP elicitation、复杂 renderer 表单与客户端工具执行器产品闭环 |
| Sandbox | `crates/dasclaw_shell_tools`、`crates/dasclaw_sandbox*`、`crates/dasclaw_workspace_cap` 已有 shell sandbox、OS sandbox policy、Linux/Windows sandbox 相关实现 | ~~P3 已把 sandbox readiness 放到 runtime bridge feature gate / service health / capability matrix~~；P5 sandbox protocol slice 已接入 buffered `command/exec.sandboxPolicy`（read-only / workspace-write 等策略 override）与 `command/exec.permissionProfile` 建模，并把 `thread/start.sandbox`、`thread/start.permissionProfile`、`turn/start.sandboxPolicy`、`turn/start.permissionProfile` 纳入协议参数面，解析为 `RuntimeSandboxContext` 后传给 `RuntimeTurnStartRequest`；`configRequirements/read.allowedSandboxModes` 已暴露 `read-only` / `workspace-write`。streaming `command/exec` 现已接受 sandbox-aware PTY override，并保持 `danger-full-access` fail-closed；仍没有 standalone tool 沙箱执行入口、workspace-bound sandbox adapter、完整 sandbox manager/API 或 Windows sandbox setup；真实 `DasclawAgentRuntimeBridge` 在尚不能 enforce sandbox context 时对非空 thread/turn context fail-closed，不能写成 runtime 已执行 sandbox |
| MCP | `crates/dasclaw_mcp` 已有 MCP config/auth/session/transport/client/factory，并且 `McpToolExecutor` 可把 MCP tools 接到 `ToolExecutor` | P4 已接 MCP `mcpServerStatus/list`、`config/mcpServer/reload`、真实 `tool/call`、真实 `resource/read` 与可定位 `item/mcpToolCall/progress`；OAuth owner 尚未接线，因此不广告 `mcpServer/oauth/login` / `mcpServer/oauthLogin/completed` |
| Filesystem / search | `crates/dasclaw_fs_tools` 已有 `ReadFileTool`、grep/glob search、path policy、file guard 等工具；P5a 新增 `crates/dasclaw_app_server/src/fs_service.rs` 的 `AppServerFsService` | ~~P5a 已接 app-server-owned `fs/*` route、root containment、polling watch/unwatch 和 Dasclaw-native `fs/changed` notification~~；Codex `changedPaths[]` shape、Codex 默认 `recursive/force` 语义、fuzzy search、file-change approval UI、审计/权限产品语义仍未接 |
| Git | `crates/dasclaw_git_tools` 已有 `git_diff`、`git_status`、`git_commit` 等工具 | ~~R6 已接 `gitDiffToRemote` 的 app-server service owner 与测试~~；review/diff notification owner 仍未扩展到更宽的 Codex review 域 |
| Jobs | `crates/dasclaw_runtime/src/job.rs` 和 `job_context.rs` 已有 job state / core context vocabulary | P4 已接 native job host 的 `jobs/list` / `jobs/read`，并复用 service 生命周期内的 job runtime worker；完整 job lifecycle mutation / subscription 仍未定义 |
| Logs / observability | `crates/dasclaw_observability` 有 `LogObserver` 和 observer events/metrics | P4 已把 observability bridge 接到 `log/entry`，并在 NotificationBus 前增加有界缓冲；日志持久化、过滤和订阅策略仍未定义 |
| Model / config | `crates/dasclaw_llm_provider` 有 provider model fetching；`crates/dasclaw_protocol/src/config_types.rs` 有 sandbox/model/config 数据类型；`desktop-app` manager 保留 renderer alias | ~~`model/list`~~、~~`config/read`~~、~~`config/value/write`~~、~~`config/batchWrite`~~、~~`configRequirements/read`~~ 已由 app-server 接管；更宽的实验/产品域 config surface 仍未定义 |
| Skills | `crates/dasclaw_protocol` 有 `ListSkills` / `SkillMetadata` 等协议词汇，历史 `desktop-client/ironclaw` 有 skill registry 参考但不是当前客户端目标 | P4 已接 native `skills/list` / `skills/config/write` / `skills/changed`；path 写入不依赖先 list，name-only 写入仍只使用已知唯一目标以避免模糊全盘匹配 |
| Turn steer / plan delta | `crates/dasclaw_protocol` 有 `ActiveTurnNotSteerable` / `NonSteerableTurnKind` 和 `PlanDeltaEvent` 等协议词汇；R5 已在 app-server 接入 `turn/steer` route、`turn/plan/updated`、`turn/diff/updated`、`item/plan/delta` 与 raw/reasoning producer | `turn/steer` 仍受 `RuntimeBridgeFeatures::turn_steer` 控制，但真实 `DasclawAgentRuntimeBridge` 默认声明可 steer，并通过 `Agent::inject_user_message` 在下一轮 agentic-loop 调用前按序注入 queued user messages；plan/diff 完成口径是“上游提供 source event 时转发”，不是从 Codex Responses SSE 普通文本或缺失事件中合成 |

## 5. Codex ClientRequest 对 Dasclaw 缺口表

| Codex 域 | Codex method | Dasclaw 当前状态 | 分类 | 补齐含义 |
|---|---|---|---|---|
| 初始化 | ~~`initialize`~~ | 有同名 native initialize，已服务当前 `desktop-app`；params/response 与 Codex 不同，Dasclaw 要求 `protocolVersion`、`requestedCapabilities`、可带 `modelProvider` | A/B | Native initialize 已完成；若要 Codex client 直连，仍需要 Codex initialize view |
| Thread 核心 | ~~`thread/start`~~、~~`thread/read`~~、~~`thread/list`~~、~~`thread/turns/list`~~ | `thread/start` 返回嵌套 `thread` object 与真实 `model` / `modelProvider` / `cwd` metadata；`thread/read`、`thread/list`、`thread/turns/list` 已返回 current chat-session subset 与 Codex-shaped read/list page | A/B | 当前 v2-shaped chat-session subset 已完成；完整 Codex `Thread` history / lifecycle 字段仍是后续扩展 |
| Thread 扩展 | ~~`thread/resume`~~、~~`thread/fork`~~、~~`thread/archive`~~、~~`thread/unarchive`~~、~~`thread/unsubscribe`~~、~~`thread/name/set`~~、~~`thread/metadata/update`~~、`thread/compact/start`、~~`thread/shellCommand`~~、~~`thread/approveGuardianDeniedAction`~~、~~`thread/rollback`~~、~~`thread/loaded/list`~~、~~`thread/inject_items`~~ | R4 已把 lifecycle / persistence / subscription / metadata / rollback / loaded list / inject-items append / goal / usage owner 收到 app-server；新增 `ThreadActionService` 后，`thread/shellCommand` 与 `thread/approveGuardianDeniedAction` 已由 thread-scoped owner 接管，复用现有 command exec owner 与 synthetic turn 持久化；`thread/compact/start` 现在有 fail-safe route 与 feature-gated success，但默认 runtime 尚无真实 compact owner；历史 `thread/create` smoke 名称不属于 native contract | A/C/D1 | 真实 runtime compact 仍需后续 owner；`thread/inject_items` 不执行注入项也不是 runtime replay；当前 compact 不应从 compatibility opt-out 移除 |
| Turn 核心 | ~~`turn/start`~~、~~`turn/interrupt`~~、~~`turn/read`~~ | `turn/start` 返回嵌套 `turn` object，接受 text-only `input: UserInput[]`；`turn/interrupt` 接收 `threadId` / `turnId` 并返回空对象；`turn/read` 返回 nested turn detail；terminal 结果统一通过 `turn/completed` 的 nested `turn.status` 表达 | A/B | 当前 text input、interrupt、read 子集已完成；后续只需随更完整 terminal / item 状态语义继续校准 |
| Turn steer | ~~`turn/steer`~~ | R5 已接 app-server route、active turn precondition、expected turn id 校验、feature-disabled fail-safe、真实 `DasclawAgentRuntimeBridge::steer_turn` 与 `Agent::inject_user_message` running-turn 注入测试；默认运行时已广告 steer 能力 | A/D1 | 已完成；steer 在下一次 agentic-loop signal check 注入，不打断正在进行中的 provider call |
| Model | ~~`model/list`~~ | ~~已由 app-server 路由并返回 Codex `ModelListResponse`；`desktop-app` 的 `modelProvider/list` 仅保留 renderer alias~~ | A | 已完成；后续若需要再补 pagination/hidden model 等更完整语义 |
| Skills | ~~`skills/list`~~、~~`skills/config/write`~~ | P4 已由 app-server native skills registry 接管，并通过 `skills/changed` 通知配置写入；当前实现只解析 repo/extra roots 下的 `SKILL.md` frontmatter，不依赖已废弃 `desktop-client` | A/D2 | 已完成最小 registry/list/config-write owner；后续若要 marketplace/plugin 语义需另定产品边界 |
| Plugin / marketplace / app | `plugin/list`、`plugin/read`、`plugin/install`、`plugin/uninstall`、`marketplace/add`、`marketplace/remove`、`marketplace/upgrade`、`app/list` | 无对应 Dasclaw app-server 能力 | E/D2 | Codex 产品扩展/市场域；除非 Dasclaw 要做插件市场，否则不建议照搬 |
| Filesystem | ~~`fs/readFile`~~、~~`fs/writeFile`~~、~~`fs/createDirectory`~~、~~`fs/getMetadata`~~、~~`fs/readDirectory`~~、~~`fs/remove`~~、~~`fs/copy`~~、~~`fs/watch`~~、~~`fs/unwatch`~~ | P5a 已由 `AppServerFsService` 接管 Dasclaw-native subset，并通过 real JSON-RPC route tests 覆盖 read/write/list/stat/copy/remove、containment 和 `fs/changed` drain | A/D2 | 基础 app-server filesystem owner 已完成；Codex shape/default parity、fuzzy search、file-change approval UI、审计/权限产品语义仍是后续专项 |
| Command exec | ~~`command/exec`~~、~~`command/exec/write`~~、~~`command/exec/terminate`~~、~~`command/exec/resize`~~ | P5a 已由 `AppServerCommandExecService` 接管 buffered `command/exec`，经 `SandboxedShellExecutor` 执行；PTY-backed streaming path 已实现并测试 `command/exec/outputDelta`、write/terminate/resize；P5 sandbox protocol slice 已支持 buffered `command/exec.sandboxPolicy`（含 read-only / workspace-write override）与 `command/exec.permissionProfile`；streaming path 现已支持 read-only/workspace-write sandbox override、non-PTY split stdout/stderr 与 control audit redaction；`danger-full-access` 继续由 executor 双 opt-in fail-closed | A/D1 | 已完成 buffered command exec control-plane、buffered sandbox/permission 参数面、sandbox-aware streaming lifecycle/control 与 non-PTY split；仍需 runtime sandbox context enforce、更加完整的 command/filesystem 审计面与更完整 Codex parity |
| MCP | `mcpServer/oauth/login`、~~`config/mcpServer/reload`~~、~~`mcpServerStatus/list`~~、~~`mcpServer/resource/read`~~、~~`mcpServer/tool/call`~~ | P4 已由 app-server 接管 MCP status/reload/tool-call/resource-read，并只在 capability matrix 中广告真实可用子集；`mcpServer/oauth/login` 仍是 fail-safe route，不作为 implemented capability 暴露 | A/D1 | 下一步只剩 MCP OAuth orchestration / callback / token storage 需要真实接线；不能靠“拼出 URL”或测试 fake 冒充完成 |
| Approval / guardian | ~~`thread/approveGuardianDeniedAction`~~，以及 ServerRequest 里的 approval 系列 | ~~runtime command approval 已接成 fail-safe app-server request/response loop~~；~~R2 已接 file-change / permission approval 的 runtime request owner 与 fail-safe response loop~~；guardian denied action 已由 `ThreadActionService` 接管 replay owner，优先回放 stash 中的 typed guardian action，缺 stash 时回退到请求载荷中的 typed event；完整 file-change 产品语义、长期 permission policy 仍未接 | D1 | 下一步应补 file-change 产品语义和 permission policy，并在 runtime review update 能携带完整 action payload 后再收紧 guardian replay 来源 |
| Sandbox | `windowsSandbox/setupStart` | sandbox crates / shell sandbox 存在；P3 runtime bridge 可把 app-server `sandbox` capability / service health 切到 implemented / ready；P5 sandbox protocol slice 已把 `thread/start.sandbox`、`thread/start.permissionProfile`、`turn/start.sandboxPolicy`、`turn/start.permissionProfile`、buffered `command/exec.sandboxPolicy`、buffered `command/exec.permissionProfile` 接入参数面；streaming command path 现也支持 read-only/workspace-write sandbox override，并继续对 `danger-full-access` fail-closed。但仍无 Codex Windows setup method；thread/turn context 只解析并传递，真实 `DasclawAgentRuntimeBridge` 对非空 context fail-closed；完整 sandbox manager/API 仍未完成 | D1/E | Windows 特定 setup 可不照搬；但 Dasclaw 若要 command/fs/tool 能力，应继续把现有平台 sandbox 抽象接入 app-server，并把 thread/turn 的 sandbox context 与 buffered/streaming command override 从协议参数面推进到真实 enforce；`danger-full-access` 不应在没有双 opt-in 的路径上放开 |
| Account/auth/rate limit | `account/login/start`、`account/login/cancel`、`account/logout`、`account/rateLimits/read`、`account/sendAddCreditsNudgeEmail`、`account/read`、`getAuthStatus` | 无 | E | Codex/OpenAI 产品账户域，不属于 Dasclaw native app-server 必需能力 |
| Config / experimental / feedback / external agent | ~~`config/read`~~、~~`config/value/write`~~、~~`config/batchWrite`~~、~~`configRequirements/read`~~、`experimentalFeature/list`、`experimentalFeature/enablement/set`、`feedback/upload`、`externalAgentConfig/detect`、`externalAgentConfig/import` | R6 已接 config allowlist owner，P5 sandbox protocol slice 已实现 `configRequirements/read.allowedSandboxModes`，只 advertised `read-only` / `workspace-write`，故意不 advertised `danger-full-access`，且 thread/turn runtime context 对非空 context fail-closed；experimental / feedback / external-agent 仍偏产品域 | A/D1/E | 当前 config owner 已完成并受测；后续若要兼容更完整 Codex config/experiment 面，仍需明确产品 owner 与 policy limits |
| Device key | `device/key/create`、`device/key/public`、`device/key/sign` | 无 | E/C | 若 Dasclaw 需要本地设备身份，可另设安全设计；不建议直接借 Codex 名称 |
| Review / git / summary / fuzzy search | ~~`review/start`~~、~~`gitDiffToRemote`~~、~~`getConversationSummary`~~、~~`fuzzyFileSearch`~~ | R6 已接 app-server review-start routing、read-only repo diff service、deterministic conversation summary 与 fuzzy search session notifications | A/D2 | 当前 owner 已完成并受测；更宽的 Codex review 产品逻辑或额外 diff/review notifications 仍可后续扩展 |

## 6. Codex ServerNotification 对 Dasclaw 缺口表

| Codex 域 | Codex notification | Dasclaw 当前状态 | 分类 | 补齐含义 |
|---|---|---|---|---|
| Error | `error` | 有同名 | A | 需核对 payload shape |
| Thread 核心 | ~~`thread/started`~~ | 有同名，payload 带 nested `thread` view；历史 `thread/created` smoke event 不属于 public native chat-session surface | A | 已完成 v2-shaped `ThreadStartedNotification` 子集 |
| Thread 状态/历史 | ~~`thread/status/changed`~~、~~`thread/archived`~~、~~`thread/unarchived`~~、`thread/closed`、~~`thread/name/updated`~~、~~`thread/goal/updated`~~、~~`thread/goal/cleared`~~、~~`thread/tokenUsage/updated`~~、`thread/compacted` | R4 已接 lifecycle / goal / usage 状态事件；`thread/compacted` 仅在 runtime bridge 声明 compact support 后发出，默认 runtime 仍不支持真实 compact | A/C/D | 仍需 thread closed 语义和真实 runtime compaction owner |
| Turn 核心 | ~~`turn/started`~~、~~`turn/completed`~~ | 有同名，payload 带 nested `turn` view；成功、失败、中断都聚合到 `turn/completed`，由 `turn.status` 和 `turn.error` 区分 | A/B | 已完成 started/completed 的 v2-shaped 子集；历史 `turn/failed` / `turn/cancelled` 不再作为 public terminal event |
| Turn plan/diff | ~~`turn/diff/updated`~~、~~`turn/plan/updated`~~ | R5 已把 provider/runtime turn-level plan/diff snapshot 接成 Codex notification producer，并加入 capability/profile/schema registry | A/D2 | 已完成 plan/diff producer；后续只随更完整 plan-mode 产品 UI 与 diff renderer 扩展 |
| Item text/reasoning | ~~`item/started`~~、~~`item/agentMessage/delta`~~、~~`item/reasoning/summaryTextDelta`~~、~~`item/reasoning/summaryPartAdded`~~、~~`item/reasoning/textDelta`~~、~~`item/completed`~~ | P1 已把 item started / agent delta / reasoning summary text delta / item completed 接到 Codex v2 profile producer；R5 已把 summary part 与 raw reasoning text delta 从 provider/runtime bridge 接到 app-server notification producer | A/B | 核心文本与 reasoning 流已完成受测子集；后续只随更完整 item payload shape 与 policy 展示策略扩展 |
| Item plan/raw/tool/file/command | ~~`rawResponseItem/completed`~~、~~`item/plan/delta`~~、~~`item/commandExecution/outputDelta`~~、~~`item/commandExecution/terminalInteraction`~~、`item/fileChange/outputDelta`、`item/fileChange/patchUpdated`、~~`command/exec/outputDelta`~~ | P3 已补 runtime tool output/result 到 commandExecution notification 的桥接；P5a 已接 buffered standalone `command/exec`；PTY-backed standalone command streaming / `command/exec/outputDelta` 已接并受测；R5 已接 raw response item completed 与 item-level plan delta；file change 与非 PTY split command streaming 仍未接 | A/D1/D2 | 后续需要 file change、非 PTY command streaming 能力和安全边界 |
| Approval review | ~~`item/autoApprovalReview/started`~~、~~`item/autoApprovalReview/completed`~~、~~`serverRequest/resolved`~~ | ~~P3 已补 server-request loop 的 resolved 结果~~；R6 已把 auto approval review started/completed producer 与 guardian warning readiness 接到真实 runtime update | A/D1 | 剩余是长期 policy 与更完整审计面，不是 event producer 缺口 |
| MCP | ~~`item/mcpToolCall/progress`~~、`mcpServer/oauthLogin/completed`、~~`mcpServer/startupStatus/updated`~~ | P4 已接 reload 触发的 startup status updated；tool-call progress 在客户端传入 `threadId + turnId + itemId` 时发可定位事件，缺少 `turnId/itemId` 时不伪造；OAuth completed 仅作为 fail-safe completion 通知存在，真实 OAuth owner 未接线且不广告 | A/D1 | 下一步需要补 MCP OAuth owner / callback / token storage；progress 不应退回不可定位或测试 fake |
| Account/app/skills | `account/updated`、`account/rateLimits/updated`、`account/login/completed`、`app/list/updated`、~~`skills/changed`~~ | account/app 偏 Codex 产品域；P4 已接 native skills config change notification | E/A/D2 | account/app list 不应默认照搬；skills 后续只需随 registry/watch 能力扩展 |
| External/fs/model/fuzzy/hooks | `externalAgentConfig/import/completed`、~~`fs/changed`~~、`model/rerouted`、`model/verification`、`fuzzyFileSearch/sessionUpdated`、`fuzzyFileSearch/sessionCompleted`、~~`hook/started`~~、~~`hook/completed`~~ | P5a 已接 Dasclaw-native `fs/changed` polling watcher；R6 已把 HookRegistry observer 接入 tool `preToolUse`、turn inbound `userPromptSubmit`、turn outbound `postToolUse`，并保留 modified content / fail-safe rejection 语义；model/search 底座仍分散存在，external agent import 与 Codex fuzzy session 未同构 | C/D1/D2/E | `fs/changed` native subset 与当前 hook lifecycle producer 已完成；`model/rerouted` / `model/verification` 仍需真实 provider/runtime producer；fuzzy session/external agent 需另定协议 |
| Realtime / Windows | `thread/realtime/*`、`windows/worldWritableWarning`、`windowsSandbox/setupCompleted` | 无 | E | Codex 特定 realtime/audio/Windows sandbox surface，不建议作为 Dasclaw 能力补齐第一阶段 |
| Warning | ~~`warning`~~、~~`guardianWarning`~~、~~`deprecationNotice`~~、~~`configWarning`~~ | R6 已接真实 config/deprecation producer；generic `warning` 仅在 Linux resolved sandbox policy 能触发 `system_bwrap_warning` 时广告/发送，非 Linux 不广告；`guardianWarning` 来自 auto-approval review runtime update 且受 feature gate 控制 | A/B/C | 当前完成的是真实 producer 与 readiness gate；没有 startup-wide sandbox policy 时不伪造 startup warning，后续若新增 startup-wide policy source 需单独接线和测试 |

## 7. Codex ServerRequest 对 Dasclaw 缺口表

Codex 的 9 个 `ServerRequest` 不是普通 notification，而是服务端主动向客户端要决策/输入/令牌，因此必须有 request tracking、timeout、fail-safe、UI 决策回传和审计。P3 已完成 command execution approval 的 server-request loop；其余 request 仍不能只补协议名。

| Codex ServerRequest | Dasclaw 当前状态 | 分类 | 为什么不能只补协议 |
|---|---|---|---|
| ~~`item/commandExecution/requestApproval`~~ | ~~P3 已接 runtime approval needed -> JSON-RPC server request -> `approval/respond` / client response -> runtime decision；并覆盖 timeout、cancel、shutdown、malformed/error response fail-safe~~ | A/D1 | 后续仍需补审计与更完整 command/fs owner，但 request loop 已有 |
| ~~`item/fileChange/requestApproval`~~ | R2 已把 file-change approval 接到 runtime-owned pending waiter；拒绝响应会恢复 tool execution 并阻止 fallback file-changing executor 运行 | D1/D2 | 完整 patch apply engine、diff UI 与 session grant policy 仍属后续产品/工具层工作 |
| ~~`item/permissions/requestApproval`~~ | R2 已把 permissions request 接到 runtime-owned pending waiter；denial/null permissions 以 fail-safe error tool result 回填执行路径 | D1/D2 | 权限 profile 的长期 grant/session policy 与真实 sandbox enforcement 仍需后续专项 |
| ~~`item/tool/requestUserInput`~~ | R2 已把 `request_user_input` tool call 接到 runtime-owned pending waiter；renderer answers 会回填为 tool result 并恢复 turn | D1/D2 | renderer 表单体验与复杂输入类型仍按产品 UI 计划推进 |
| ~~`item/tool/call`~~ | R2 已把 client dynamic tool call 接到 runtime-owned pending waiter；renderer success/failure response 会回填为 tool result，stale response 明确拒绝 | D1 | dynamic tool registry/list、MCP elicitation 与更完整 result streaming 仍需后续专项 |
| `mcpServer/elicitation/request` | MCP crate 底座存在；未见 app-server elicitation request loop | D1/D2 | 需要 MCP elicitation support 和 client UI contract |
| `account/chatgptAuthTokens/refresh` | 无 | E | Codex/OpenAI account token 域 |
| `applyPatchApproval` | approval primitive 存在，但 Codex legacy patch approval 面未接 | D1/E | Codex legacy approval；若 Dasclaw 做 patch approval，应基于自有 file change/approval 设计 |
| `execCommandApproval` | approval / shell sandbox primitive 存在，但 Codex legacy exec approval 面未接 | D1/E | Codex legacy exec approval；应和 command execution/sandbox 一起设计 |

## 8. Dasclaw 有而 Codex app-server 协议没有的面

这些不是 Codex 的缺陷，而是 Dasclaw app-server 作为本地 service control plane 的自有设计。

| Dasclaw-only surface | 类型 | 意义 |
|---|---|---|
| ~~`protocol/schema`~~ | method | Dasclaw native 协议发现；Codex 依赖生成 schema，不走 runtime schema 方法 |
| ~~`health/check`~~、~~`capabilities/list`~~ | method | 明确让 GUI 按 capability matrix/health gating，而不是猜后端能力 |
| ~~`lifecycle/status`~~、~~`shutdown`~~ | method | 本地 sidecar lifecycle 控制面 |
| `thread/create` | method | Legacy Dasclaw smoke-surface alias，当前 public native contract intentionally unsupported；公开名称是 `thread/start` |
| `turn/cancel` | method | Legacy Dasclaw smoke-surface alias，当前 public native contract intentionally unsupported；公开名称是 `turn/interrupt` |
| `turn/list` | method | Legacy Dasclaw turn-level list alias，当前 public native contract intentionally unsupported；公开名称是 `thread/turns/list` |
| ~~`turn/read`~~ | method | Dasclaw native turn-level read；当前仍保留为已支持的 turn detail 读取入口 |
| ~~`approval/respond`~~ | method | P3 approval server-request 的客户端决策回传入口 |
| ~~`modelProvider/selectForNextTurn`~~ | method | desktop-app / renderer-mediated model provider selection；Codex 有 `model/list` 但没有这个同名选择入口 |
| ~~`notifications/initialized`~~ | event | Dasclaw 服务端通知；Codex 的 `initialized` 是 ClientNotification |
| ~~`lifecycle/changed`~~、~~`health/changed`~~、~~`capabilities/changed`~~ | event | Native control-plane state |
| ~~`log/entry`~~ | event | P4 已由 observability bridge 接入，并在进入 NotificationBus 前使用有界队列 |
| `thread/created` | event | Legacy Dasclaw smoke-surface alias，当前 public native contract intentionally unsupported；公开名称是 `thread/started` |
| `turn/delta` | event | Legacy/smoke delta，当前 public native contract intentionally unsupported；Codex 核心文本流是 `item/agentMessage/delta` |
| `turn/failed`、`turn/cancelled` | event | Legacy terminal variants，当前 public native contract intentionally unsupported；失败/中断都聚合在 `turn/completed` 的 `turn.status` 与 `turn.error` |
| ~~`serverRequest/resolved`~~、~~`item/commandExecution/outputDelta`~~、~~`item/commandExecution/terminalInteraction`~~ | event | P3 新增的 approval/tool bridge 事件；名字与 Codex 对齐但仍受 runtime feature gate 控制 |

## 9. 能力补齐优先级

如果目标是“补齐 dasclaw-app-server 能力”，建议按能力依赖顺序补，而不是按 Codex method 字母顺序补。

| 优先级 | 目标 | 包含 | 原因 |
|---|---|---|---|
| P0 | 诚实的协议边界 | ~~`codex_app_server_v2_chat_session_subset` 继续标成 chat-session subset；P3-P6 opt-out 保持显式~~ | 已由 capability/profile tests 约束 |
| P1 | Chat-session compatibility view | ~~`thread/start`~~、~~`thread/read`~~、~~`thread/list`~~、~~`thread/turns/list`~~、~~`turn/start`~~、~~`turn/interrupt`~~、~~`turn/read`~~、~~`thread/started`~~、~~`turn/started`~~、~~`turn/completed`~~、~~`item/started`~~、~~`item/agentMessage/delta`~~、~~`item/reasoning/summaryTextDelta`~~、~~`item/completed`~~ 使用单一 v2-shaped native view / producer | desktop-app 直接消费 nested thread/turn/item shape，不再通过 response-level fallback normalize |
| P2 | Model catalog/service | ~~`model/list`~~ 由 app-server 返回 Codex `ModelListResponse`；~~`modelProvider/list`~~ 仅作为 desktop-app renderer alias | app-server 成为模型列表 owner，主进程不再直接暴露 provider secrets |
| P3 | Approval + tool + sandbox 三件套 | ~~ServerRequest request tracking、approval decision、tool lifecycle notification、sandbox capability gate、fail-safe timeout~~；~~P5a standalone `fs/*` / buffered `command/exec` 已接~~；~~thread/turn sandbox context 与 buffered command sandbox/permission 参数面已接入协议/解析层~~；~~R2 已接 dynamic `item/tool/call`、`item/tool/requestUserInput`、file-change approval、permissions approval 的 runtime request owner~~；command PTY sandbox follow-up、runtime sandbox enforce 仍留给后续专项 | Codex 大量协议依赖这组能力；安全控制闭环先于 command/fs owner，后续继续补 tool/file/PTY 边界；sandbox readiness 与协议参数面只能说明可表达/可 fail-closed，不能替代真实 runtime sandbox enforce 或完整 sandbox manager/API |
| P4 | MCP / skills / logs / jobs | ~~MCP status/reload/resource/tool call/progress~~、MCP OAuth、~~skills registry~~、~~logs source~~、~~job host~~ | P4 honest subset 已通过 `AppServerServices::real()` 接入默认 sidecar，覆盖 logs/jobs/skills 与 MCP status/reload/tool/resource/progress 的 app-server owner；MCP OAuth 仍作为后续专项，不在 capability matrix 中冒充 implemented |
| P5 | Filesystem / command exec | ~~Dasclaw-native `fs/*` subset~~、~~buffered `command/exec`~~、~~Dasclaw-native `fs/changed`~~、~~PTY-backed `command/exec/outputDelta`~~、~~PTY-backed `command/exec/write`~~、~~PTY-backed `command/exec/terminate`~~、~~PTY-backed `command/exec/resize`~~、~~buffered `command/exec.sandboxPolicy` read-only/workspace-write override~~、~~buffered `command/exec.permissionProfile` 建模~~、~~streaming `command/exec` sandbox-aware PTY override~~、~~non-PTY split stdout/stderr streaming~~；buffered `danger-full-access` 执行、runtime sandbox context enforce 与更完整安全审计仍未完成 | P5a 已在 P3 安全边界之后接入 app-server-owned filesystem、buffered command exec、buffered sandbox protocol slice 与 PTY-backed streaming command controls；当前新增了 sandbox-aware PTY 与 non-PTY split streaming，但 Codex FS parity、runtime sandbox enforce、完整 sandbox manager/API 与更完整安全审计仍不冒充完成 |
| P6 | Product-specific Codex domains | account、plugin、marketplace、app list、feedback、external agent import、Windows sandbox、realtime audio | 只有当 Dasclaw 明确要兼容未改 Codex client 或复刻相关产品能力时再做 |

### 9.1 当前剩余优先级（2026-06-22）

下面按“安全/owner 依赖优先、协议 shape 其次、产品域最后”排序。P0-P5 已划掉的受测子集不再重复列入。

| 剩余优先级 | 范围 | 仍未完成的协议/能力 | 为什么排这里 | 完成判定 |
|---|---|---|---|---|
| R1 | Runtime tool / approval owner | ~~`item/tool/call` client `open_url` E2E~~、~~renderer-visible `item/tool/requestUserInput`~~、~~renderer-visible `item/permissions/requestApproval`~~、~~renderer-visible `item/fileChange/requestApproval`~~、~~renderer-visible file-change / auto-review notifications~~；runtime-owned file-change apply/user-input suspension 仍保持 explicit unsupported，直到底层 runtime producer 不再依赖测试桥 | app-server 已能把 client dynamic tool response 送回 runtime；desktop renderer 不再用自动 reject / 空 answers 冒充 UI；file-change/user-input 已有用户交互和 fail-safe response，但真实 runtime owner 仍需单独任务接入 | R1 本轮完成判定：client `open_url` 能从 LLM tool call 到 renderer 执行再回到 runtime tool result；renderer 对 user-input/file-change/permissions 给出显式用户交互；auto-review/file-change notification 可见；测试明确区分真实 runtime path 与 test bridge path |
| R2 | Sandbox enforcement / command safety follow-up | runtime sandbox context enforce、安全审计；buffered `danger-full-access` 仍保持双 opt-in fail-closed | P5 已把 sandbox 参数面接入协议/解析层，streaming `command/exec` 也已支持 read-only/workspace-write sandbox override 与 non-PTY split stdout/stderr；但真实 `DasclawAgentRuntimeBridge` 对非空 sandbox context 仍 fail-closed，不能把“可表达”误写成“已执行沙箱” | 非空 thread/turn sandbox context 能被 runtime 实际 enforce；streaming/buffered command 路径继续保持一致沙箱语义；`danger-full-access` 仍只在显式双 opt-in 路径放行；敏感参数不进日志/事件 |
| R3 | MCP OAuth owner | `mcpServer/oauth/login`、`mcpServer/oauthLogin/completed` 的真实成功路径、callback listener、state 校验、token storage / redaction | MCP status/reload/tool/resource/progress 已完成；OAuth 现在只有 fail-safe route 和失败 completion notification，不应在 capability matrix 中冒充 implemented | 成功路径能生成授权 URL、接收 callback、校验 state、落安全 token store，并只在 readiness 成立时广告 OAuth capability；失败路径不泄露 code/token/state |
| R4 | Thread lifecycle / persistence owner | ~~`thread/resume`~~、~~`thread/fork`~~、~~`thread/archive`~~、~~`thread/unarchive`~~、~~`thread/unsubscribe`~~、~~`thread/name/set`~~、~~`thread/metadata/update`~~、`thread/compact/start`、~~`thread/rollback`~~、~~`thread/loaded/list`~~、~~`thread/inject_items`~~；对应 ~~`thread/status/changed`~~、~~`thread/archived`~~、~~`thread/unarchived`~~、~~`thread/name/updated`~~、~~`thread/goal/updated`~~、~~`thread/goal/cleared`~~、~~`thread/tokenUsage/updated`~~；`thread/compacted` 仍受 runtime compact feature gate 限制 | R4 已由 `docs/superpowers/plans/2026-06-22-dasclaw-app-server-thread-lifecycle-persistence-owner.md` 接入持久化 thread store、订阅、归档、命名、metadata、rollback、loaded list、inject-items 结构化 append、goal 和真实 usage 累加；`thread/compact/start` 目前只保证 fail-safe route 和 compact-capable bridge 成功路径，不表示默认 runtime 已实现 compact | app-server 有 thread store / subscription owner；lifecycle/goal/usage 方法和 events 在 router、schema、capability、tests 中一致；`thread/inject_items` 不执行注入项且不是 runtime replay；`thread/metadata/update` 当前只建模 `gitInfo` patch；旧 smoke alias 不回流到 public native contract；compact 从剩余项移除前必须接入真实 runtime compact operation |
| R5a | Protocol/router/adapter receive path | ~~`turn/plan/updated`~~、~~`turn/diff/updated`~~、~~`item/plan/delta`~~、~~`rawResponseItem/completed`~~、~~`item/reasoning/summaryPartAdded` producer~~、~~`item/reasoning/textDelta` producer~~；~~`turn/steer` route/fail-safe~~ | R5a 已完成 schema、notification constructors、runtime adapter forwarding、app-server notification producer 与 feature-gated `turn/steer` route；scripted provider / synthetic bridge 测试只证明接收路径 | schema、route、adapter forwarding、app-server notification tests 通过 |
| R5b | Real provider producer | ~~raw reasoning / raw response item producer；plan-diff source-event forwarding~~ | 当前完成范围是 provider fixture/parser 可真实消费的事件：Codex raw reasoning / raw response item、ClawCode raw reasoning，以及存在 `turn.plan.updated` / `turn.diff.updated` source event 时的 plan/diff 转发；ordinary text 不会被伪造成 plan/diff。该项不是 Codex Responses SSE 原生 plan/diff parity，也不表示 ClawCode 已有正向 plan/diff source event | provider fixture/parser tests 通过后可标记对应 producer 子项；不能从 provider 拿到的事件保持 explicit unsupported / no-synthesis 语义 |
| R5c | Real turn steer injection | ~~`turn/steer` real runtime injection~~ | 已新增 `Agent::inject_user_message` 运行时注入 API；真实 `DasclawAgentRuntimeBridge::steer_turn` 查找 active agent 并注入 prompt，默认声明 `turn_steer: true`；测试不用 `RecordingRuntimeBridge::with_turn_steer()` 作为完成证据 | `agent_inject_user_message_reaches_next_loop_iteration`、`agent_inject_user_message_drains_multiple_messages_before_next_loop_iteration` 与 `dasclaw_runtime_bridge_steer_turn_injects_input_into_running_turn` 通过；steer 注入下一轮 LLM 调用，不中断当前 provider call，多条 queued steer 按序注入 |
| R6 | Config / repo tools / search owner | ~~broad `config/read`、`config/value/write`、`config/batchWrite`~~；~~`gitDiffToRemote`~~；~~`fuzzyFileSearch` session~~；~~`getConversationSummary`~~；~~`review/start`~~；model reroute producer / model verification producer；~~HookRegistry `preToolUse` / `userPromptSubmit` / `postToolUse` events~~；~~config/deprecation warning producer~~；~~Linux resolved-sandbox generic warning producer~~；~~guardian warning producer from auto-approval review updates~~ | R6 app-server owner 已完成 config allowlist policy、read-only repo diff service、path fuzzy search、deterministic conversation summary、review-start routing、HookRegistry observer events（tool `preToolUse`、turn inbound `userPromptSubmit`、turn outbound `postToolUse`，含 modified content 与 rejection fail-safe），config/deprecation warning producers，Linux resolved sandbox policy -> `system_bwrap_warning` generic warning producer，以及 auto-approval review completed -> `guardianWarning` producer。`model/rerouted` 目前只有 runtime delivery，provider `actual_model` 不再被误标成 `highRiskCyberActivity`；`model/verification` 仍缺真实 provider metadata source。Dasclaw 既有 git/search/hooks/core/config/sandboxing 底座被复用；Codex product-only 的 external-agent / feedback / experiment 等域仍留在 R7，不混入 R6。 | R6 producer slice 现在完成非模型事件族；`model/rerouted` 从剩余项移除前必须有真实 reroute reason source，`model/verification` 从剩余项移除前必须有真实 provider metadata parser 证据。非 Linux 不广告 generic `warning`；没有 startup-wide sandbox policy 时不伪造 startup warning |
| R7 | Codex product / platform domains | account/auth/rate limit、plugin marketplace、app list、device key、feedback、external agent import、realtime/audio、Windows sandbox setup、world-writable warning | 这些与 Codex/OpenAI 产品或平台体验强绑定；除非目标变成兼容未改 Codex client 或复刻相关产品能力，否则不应压过 Dasclaw native core | 先有产品决策和安全设计，再进入 protocol matrix；否则保持 explicit unsupported / opt-out |

## 9.2 当前未完成清单（2026-06-25 复核）

以下清单只保留本轮复核后仍未完成、或仅完成 fail-safe / feature-gated 子集的项，并按建议执行优先级排序：先补安全与真实执行语义，再补 owner 闭环，再补事件细节，最后才是产品域和平台域。

### P0 安全与 enforce 闭环

优先原因：
这些项直接决定 command / sandbox 能力是不是“真的安全可执行”，优先级高于新增协议名或产品域扩展。

- thread/turn sandbox context 从“协议参数面”推进到更完整真实 enforce
- 更完整 command / filesystem 安全审计面

### P1 真实 owner / 成功链路

优先原因：
这些项已经有协议入口或 fail-safe 子集，但还没有完整 owner；补完后能显著减少“看起来支持、实际上半接线”的状态。

- `mcpServer/oauth/login`
- `mcpServer/oauthLogin/completed`
- `thread/compact/start`
- `thread/compacted`

说明：
`mcpServer/oauth/*` 当前只有 fail-safe route / failure completion notification，缺真实 success path、callback listener、state 校验、token storage / redaction。
`thread/compact/*` 当前已有 fail-safe route，且 compact-capable runtime bridge 可走成功路径；默认 runtime 仍无真实 compact owner。

### P2 事件 / 状态补齐

优先原因：
这些项主要影响 renderer 可见性、状态可解释性和 richer UX；重要，但不应排在安全边界和 owner 闭环之前。

- `item/fileChange/outputDelta`
- `item/fileChange/patchUpdated`
- `model/rerouted`
- `model/verification`

说明：
`model/rerouted` 仍缺可信 reroute reason source；`model/verification` 仍缺真实 provider metadata parser / source。

### P3 Codex 产品域尚未迁移 / 未定义

优先原因：
这些能力与 Codex/OpenAI 产品面强绑定，除非 Dasclaw 明确要复刻对应产品语义，否则不应压过 native core 补齐。

- `plugin/list`、`plugin/read`、`plugin/install`、`plugin/uninstall`
- `marketplace/add`、`marketplace/remove`、`marketplace/upgrade`
- `app/list`、`app/list/updated`
- `device/key/create`、`device/key/public`、`device/key/sign`
- `account/login/start`、`account/login/cancel`、`account/logout`、`account/rateLimits/read`、`account/sendAddCreditsNudgeEmail`、`account/read`
- `account/updated`、`account/rateLimits/updated`、`account/login/completed`、`getAuthStatus`
- `feedback/upload`
- `experimentalFeature/list`、`experimentalFeature/enablement/set`
- `externalAgentConfig/detect`、`externalAgentConfig/import`、`externalAgentConfig/import/completed`

### P4 平台特定 / 非当前优先级

优先原因：
这些项要么平台特定，要么属于更远的体验扩展；在当前 honest subset 路线下不应先于 P0-P3。

- `windowsSandbox/setupStart`
- `windowsSandbox/setupCompleted`
- `windows/worldWritableWarning`
- `thread/realtime/*`
- `thread/closed`

## 10. 决策建议

1. 不建议把 Dasclaw native protocol 改名伪装成完整 Codex app-server。当前证据显示它只覆盖 chat-session subset，硬伪装会让客户端在 tools/MCP/approval/fs/command/account 等域踩到 app-server 接线缺口或产品语义缺口。
2. 可以保留 Codex-compatible profile 描述，但它只是 capability/profile metadata；wire shape 仍是 Dasclaw native v2-shaped contract，未实现域要明确 unsupported，而不是静默 no-op。
3. ~~最短可交付路线是先补 P1：让同名 thread/turn/item streaming 返回 nested Codex-style objects。~~ P1 已完成；当前路线应继续保持 chat-session subset 边界，后续扩能力时不要承诺完整 Codex 产品控制面。
4. P3 已先补 approval、tool lifecycle notification、sandbox gate 这组安全边界；下一步做 `fs/*`、`command/exec*`、`item/tool/call` 时仍不能绕过这条 request tracking / fail-safe / capability gating 路径。
