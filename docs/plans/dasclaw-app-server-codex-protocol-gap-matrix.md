# Dasclaw app-server 与 Codex app-server 协议缺口对照表

> 日期：2026-06-17
> 状态：能力补齐参考（删除线表示本轮 P0-P2 已完成）
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
| Level 1 语义层 | `semantic_search_nodes_tool` 查询 Codex app-server protocol / Dasclaw capability matrix / compatibility profile 等语义 | 对全仓查询曾退化为 keyword 且 0 命中；不把它作为否定结论的唯一依据。追加查询可命中 graph 但偏向旧 `desktop-client/ironclaw` capability 符号，不能替代精确 schema 对账 |
| Level 2 符号层 | 当前 Codex App 工具面未暴露可调用的 `execute_lsp`；`tool_search` 未返回 LSP 执行工具 | 本文不伪装成 LSP 结论，改用生成 TypeScript union、Rust 常量、Rust router 行号和 `rg` 精确检索补强 |
| Level 3 字面层 | Codex 生成协议 union：`codex-cli-main/codex-rs/app-server-protocol/schema/typescript/{ClientRequest,ServerNotification,ServerRequest,ClientNotification}.ts`；Dasclaw 常量、能力矩阵、router：`crates/dasclaw_app_server_protocol/src/lib.rs`、`crates/dasclaw_app_server/src/lib.rs` | Codex：74 个 `ClientRequest`、61 个 `ServerNotification`、9 个 `ServerRequest`、1 个 `ClientNotification`。Dasclaw 当前：17 个 method、19 个 event，且 router 只路由这些 method |

已检查 `dasclaw-app-server / codex app-server protocol gap matrix` 是否已有，结论：已有 `docs/plans/dasclaw-app-server-protocol-v0.md`、`docs/plans/dasclaw-app-server-ownership-matrix.md`、`docs/plans/dasclaw-codex-app-server-ai-sdk-compat-plan.md`，但未发现“Codex 全量 ClientRequest / ServerNotification / ServerRequest / ClientNotification 与 Dasclaw 当前协议逐域缺口”的对照表；本文补齐该空白。

子 agent 只读交叉检查结论与主线一致：`codex-cli-main/codex-rs/app-server-protocol/src/schema.rs` 在当前仓库不存在，实际应以 `src/protocol/v1.rs` 和 `schema/typescript/*.ts` 作为协议证据，其中 TypeScript 生成文件最适合逐项枚举。

## 1. 总结

当前 `dasclaw-app-server` 不是完整 Codex app-server v2 实现，而是一个 Dasclaw native app-server 加上 `codex_app_server_v2` 的 chat-session subset compatibility profile。

直接证据：

| 证据 | 说明 |
|---|---|
| `ClientRequest.ts:79` | Codex 客户端请求 union 一行列出 74 个 method |
| `ServerNotification.ts:69` | Codex 服务端通知 union 一行列出 61 个 notification |
| `ServerRequest.ts:18` | Codex 服务端发起请求 union 一行列出 9 个 request |
| `ClientNotification.ts:5` | Codex 客户端 notification 只有 `initialized` |
| `crates/dasclaw_app_server_protocol/src/lib.rs` | Dasclaw 当前常量定义 17 个 method、19 个 event；`model/list` 已进入 protocol schema |
| `crates/dasclaw_app_server_protocol/src/lib.rs` | Dasclaw Phase 1 实现 `protocol`、`lifecycle`、`health`、`session`、`model_provider`；`approval`、`dlp_policy`、`tools`、`jobs`、`skills`、`mcp`、`sandbox`、`logs` 是 declared future |
| `crates/dasclaw_app_server_protocol/src/lib.rs` | `codex_app_server_v2` profile 明确是 `ChatSessionSubset`，只列 `initialize`、`thread/start`、`thread/read`、`thread/list`、`turn/start`、`turn/interrupt`、`model/list` 并 opt out 多个 Codex 能力 |
| `crates/dasclaw_app_server/src/lib.rs` | Dasclaw router 只实际路由当前 17 个 method，其他 method 会落到 `method_not_found` |
| `crates/dasclaw_app_server/src/lib.rs:897-923` | 运行时健康状态明确显示 tools / sandbox / jobs / skills / mcp disabled，DLP policy unavailable fail-safe |

结论分三层：

| 层级 | 判断 |
|---|---|
| 本轮已完成 | `model/list`；`thread/start` / `turn/start` 附带 Codex `thread` / `turn` view；`thread/started`、`turn/started`、`turn/completed`、`turn/failed` 附带 Codex-compatible view；`desktop-app` 已兼容 native 与 Codex shape |
| 协议同名/近似可用 | `initialize`、`thread/start`、`thread/read`、`thread/list`、`turn/start`、`turn/interrupt`、若干 item/turn streaming notification |
| 协议缺口但可通过 compatibility view 补 | `item/completed` payload shape、`thread/read` / `thread/list` 的完整 Codex `Thread` view、更多 item/turn 细粒度 payload shape |
| agent 框架 `crates` 已有底座但 app-server 未接线 | approval / tool execution / sandbox primitives 已在 `crates` 下的 runtime/tool/sandbox 相关 crate 中存在；缺的是 app-server 暴露、订阅事件、决策回传、service health、capability gating 和 Codex-style request/notification view |
| 已有分散底座但 app-server 尚未成为 owner | MCP / skills / jobs / logs / filesystem / git / search / config 等在 `crates`、历史参考实现或 `desktop-app` manager 中有不同程度的实现；缺的是 app-server service owner、协议接线、能力健康状态和安全边界 |
| 产品/服务能力尚未定义或未迁移 | account / plugin marketplace / app list / device key / external agent import / Codex review 等仍偏 Codex 产品域或需要先定义 Dasclaw 产品语义 |

## 2. Codex app-server 协议清单

> 标记说明：删除线表示 Dasclaw 本轮 P0-P2 已补齐或已接入 Codex-compatible view；同一行里未划掉的协议仍按后续计划处理。

### 2.1 ClientRequest：74 个

来源：`codex-cli-main/codex-rs/app-server-protocol/schema/typescript/ClientRequest.ts:79`。

| 域 | Codex method |
|---|---|
| 初始化 | `initialize` |
| Thread lifecycle / history | ~~`thread/start`~~、`thread/resume`、`thread/fork`、`thread/archive`、`thread/unsubscribe`、`thread/name/set`、`thread/metadata/update`、`thread/unarchive`、`thread/compact/start`、`thread/shellCommand`、`thread/approveGuardianDeniedAction`、`thread/rollback`、`thread/list`、`thread/loaded/list`、`thread/read`、`thread/turns/list`、`thread/inject_items` |
| Turn lifecycle | ~~`turn/start`~~、`turn/steer`、`turn/interrupt` |
| Skills / plugins / marketplace / apps | `skills/list`、`skills/config/write`、`plugin/list`、`plugin/read`、`plugin/install`、`plugin/uninstall`、`marketplace/add`、`marketplace/remove`、`marketplace/upgrade`、`app/list` |
| Device key | `device/key/create`、`device/key/public`、`device/key/sign` |
| Filesystem | `fs/readFile`、`fs/writeFile`、`fs/createDirectory`、`fs/getMetadata`、`fs/readDirectory`、`fs/remove`、`fs/copy`、`fs/watch`、`fs/unwatch` |
| Review / model / experiment | `review/start`、~~`model/list`~~、`experimentalFeature/list`、`experimentalFeature/enablement/set` |
| MCP | `mcpServer/oauth/login`、`config/mcpServer/reload`、`mcpServerStatus/list`、`mcpServer/resource/read`、`mcpServer/tool/call` |
| Sandbox / account / feedback | `windowsSandbox/setupStart`、`account/login/start`、`account/login/cancel`、`account/logout`、`account/rateLimits/read`、`account/sendAddCreditsNudgeEmail`、`account/read`、`feedback/upload` |
| Local command | `command/exec`、`command/exec/write`、`command/exec/terminate`、`command/exec/resize` |
| Config / external agent | `config/read`、`config/value/write`、`config/batchWrite`、`configRequirements/read`、`externalAgentConfig/detect`、`externalAgentConfig/import` |
| Misc | `getConversationSummary`、`gitDiffToRemote`、`getAuthStatus`、`fuzzyFileSearch` |

### 2.2 ServerNotification：61 个

来源：`codex-cli-main/codex-rs/app-server-protocol/schema/typescript/ServerNotification.ts:69`。

| 域 | Codex notification |
|---|---|
| Error / warnings | `error`、`warning`、`guardianWarning`、`deprecationNotice`、`configWarning` |
| Thread | ~~`thread/started`~~、`thread/status/changed`、`thread/archived`、`thread/unarchived`、`thread/closed`、`thread/name/updated`、`thread/goal/updated`、`thread/goal/cleared`、`thread/tokenUsage/updated`、`thread/compacted` |
| Realtime | `thread/realtime/started`、`thread/realtime/itemAdded`、`thread/realtime/transcript/delta`、`thread/realtime/transcript/done`、`thread/realtime/outputAudio/delta`、`thread/realtime/sdp`、`thread/realtime/error`、`thread/realtime/closed` |
| Turn | ~~`turn/started`~~、~~`turn/completed`~~、`turn/diff/updated`、`turn/plan/updated` |
| Item streaming | `item/started`、`item/completed`、`rawResponseItem/completed`、`item/agentMessage/delta`、`item/plan/delta`、`item/reasoning/summaryTextDelta`、`item/reasoning/summaryPartAdded`、`item/reasoning/textDelta` |
| Approval / command / file change | `item/autoApprovalReview/started`、`item/autoApprovalReview/completed`、`command/exec/outputDelta`、`item/commandExecution/outputDelta`、`item/commandExecution/terminalInteraction`、`item/fileChange/outputDelta`、`item/fileChange/patchUpdated`、`serverRequest/resolved` |
| MCP | `item/mcpToolCall/progress`、`mcpServer/oauthLogin/completed`、`mcpServer/startupStatus/updated` |
| Account / app / skills | `account/updated`、`account/rateLimits/updated`、`account/login/completed`、`app/list/updated`、`skills/changed` |
| External / fs / model / fuzzy / hooks / Windows | `externalAgentConfig/import/completed`、`fs/changed`、`model/rerouted`、`model/verification`、`fuzzyFileSearch/sessionUpdated`、`fuzzyFileSearch/sessionCompleted`、`hook/started`、`hook/completed`、`windows/worldWritableWarning`、`windowsSandbox/setupCompleted` |

### 2.3 ServerRequest：9 个

来源：`codex-cli-main/codex-rs/app-server-protocol/schema/typescript/ServerRequest.ts:18`。

| 域 | Codex server-initiated request |
|---|---|
| Approval | `item/commandExecution/requestApproval`、`item/fileChange/requestApproval`、`item/permissions/requestApproval`、`applyPatchApproval`、`execCommandApproval` |
| Tool / user input | `item/tool/requestUserInput`、`item/tool/call` |
| MCP elicitation | `mcpServer/elicitation/request` |
| Account token | `account/chatgptAuthTokens/refresh` |

### 2.4 ClientNotification：1 个

来源：`codex-cli-main/codex-rs/app-server-protocol/schema/typescript/ClientNotification.ts:5`。

| 域 | Codex client notification |
|---|---|
| Connection lifecycle | `initialized` |

## 3. Dasclaw 当前协议清单

### 3.1 Method：17 个

来源：`crates/dasclaw_app_server_protocol/src/lib.rs` 与 `phase_one_methods()`。

| 能力域 | Dasclaw method |
|---|---|
| Protocol | `initialize`、`protocol/schema` |
| Health | `health/check`、`capabilities/list` |
| Lifecycle | `lifecycle/status`、`shutdown` |
| Session / thread | `thread/create`、`thread/start`、`thread/list`、`thread/read` |
| Session / turn | `turn/start`、`turn/cancel`、`turn/interrupt`、`turn/list`、`turn/read` |
| Model provider | ~~`model/list`~~、`modelProvider/selectForNextTurn` |

### 3.2 Event：19 个

来源：`crates/dasclaw_app_server_protocol/src/lib.rs:36-56` 与 `phase_one_events()`。

| 能力域 | Dasclaw event |
|---|---|
| Protocol | `notifications/initialized` |
| Lifecycle / health / logs | `lifecycle/changed`、`health/changed`、`capabilities/changed`、`log/entry` |
| Thread / turn | `thread/created`、~~`thread/started`~~、~~`turn/started`~~、`turn/delta`、~~`turn/completed`~~、~~`turn/failed`~~、`turn/cancelled` |
| Item streaming | `item/started`、`item/agentMessage/delta`、`item/reasoning/summaryTextDelta`、`item/reasoning/summaryPartAdded`、`item/reasoning/textDelta`、`item/completed` |
| Error | `error` |

### 3.3 当前 desktop-app 消费面

`desktop-app` 只消费 Dasclaw 当前子集：

| 证据 | 说明 |
|---|---|
| `desktop-app/src/main/appServerManager.ts` | 初始化发送 `requestedCapabilities: ["protocol", "lifecycle", "health", "session", "codex_app_server_v2"]` |
| `desktop-app/src/main/appServerManager.ts` | ~~`modelProvider/list`~~ 仍是客户端 renderer alias，但内部已转发到 app-server ~~`model/list`~~；`modelProvider/selectForNextTurn` 仍由 manager 侧特殊处理 |
| `desktop-app/src/renderer/src/lib/appServerTurnTracker.ts` | renderer 聚合 content / reasoning delta，并已兼容 ~~`turn/completed`~~ / ~~`turn/failed`~~ 的 native 与 Codex nested `turn` shape |

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
| Approval | `crates/dasclaw_runtime/src/approval.rs` 已有 `ApprovalPolicy`、`ApprovalRequest`、`ApprovalDecision`、`ApprovalInbox`、`Approver`；`Agent::respond_to_approval` 可回填 GUI 决策 | `dasclaw_app_server_protocol` 仍把 `approval` 标成 declared future；app-server 没有 Codex `ServerRequest` 风格的 approval request/resolve loop |
| Tool execution | `crates/dasclaw_runtime/src/agent.rs` 有 `ToolExecutor`；`AgentBuilder::tool_executor*` 可接 executor；`tool_dispatch.rs` 有 approval gate -> egress -> executor -> sanitizer 的顺序 pipeline | app-server service health 仍显示 `Tools` disabled；协议没有 tool registry/list/call、tool lifecycle notification、dynamic tool call response |
| Sandbox | `crates/dasclaw_shell_tools`、`crates/dasclaw_sandbox*`、`crates/dasclaw_workspace_cap` 已有 shell sandbox、OS sandbox policy、Linux/Windows sandbox 相关实现 | app-server service health 仍显示 `Sandbox` disabled；协议没有 workspace-bound sandbox adapter、policy negotiation、command/fs/tool 沙箱执行入口 |
| MCP | `crates/dasclaw_mcp` 已有 MCP config/auth/session/transport/client/factory，并且 `McpToolExecutor` 可把 MCP tools 接到 `ToolExecutor` | app-server service health 仍显示 `Mcp` disabled；协议没有 `mcpServer/*` method、OAuth UI flow、resource read、server status、tool-call progress notification |
| Filesystem / search | `crates/dasclaw_fs_tools` 已有 `ReadFileTool`、grep/glob search、path policy、file guard 等工具 | app-server 没有 Codex `fs/*` method，也没有 watch/unwatch、`fs/changed` notification 或 app-server-owned workspace permission model |
| Git | `crates/dasclaw_git_tools` 已有 `git_diff`、`git_status`、`git_commit` 等工具 | app-server 没有 Codex `gitDiffToRemote` method，也没有 review/diff notification owner |
| Jobs | `crates/dasclaw_runtime/src/job.rs` 和 `job_context.rs` 已有 job state / core context vocabulary | app-server service health 仍显示 `Jobs` disabled；没有 job host、job list、job lifecycle protocol |
| Logs / observability | `crates/dasclaw_observability` 有 `LogObserver` 和 observer events/metrics | app-server `logs` 仍 declared future，`log/entry` event 没有 source wiring |
| Model / config | `crates/dasclaw_llm_provider` 有 provider model fetching；`crates/dasclaw_protocol/src/config_types.rs` 有 sandbox/model/config 数据类型；`desktop-app` manager 保留 renderer alias | ~~`model/list`~~ 已由 app-server 接管；`config/read`、`config/value/write` 等 app-server service 仍未接线 |
| Skills | `crates/dasclaw_protocol` 有 `ListSkills` / `SkillMetadata` 等协议词汇，历史 `desktop-client/ironclaw` 有 skill registry 参考但不是当前客户端目标 | 当前目标是 `desktop-app` + `dasclaw_app_server`，app-server service health 仍显示 `Skills` disabled；没有 native skills registry/list/config-write service |
| Turn steer / plan delta | `crates/dasclaw_protocol` 有 `ActiveTurnNotSteerable` / `NonSteerableTurnKind` 和 `PlanDeltaEvent` 等协议词汇 | app-server 无 `turn/steer` method，也没有 Codex `turn/plan/updated` / `turn/diff/updated` notification producer |

## 5. Codex ClientRequest 对 Dasclaw 缺口表

| Codex 域 | Codex method | Dasclaw 当前状态 | 分类 | 补齐含义 |
|---|---|---|---|---|
| 初始化 | `initialize` | 有同名，但 params/response 与 Codex 不同；Dasclaw 要求 `protocolVersion`、`requestedCapabilities`、可带 `modelProvider` | A/B | 若要 Codex client 直连，需要 Codex initialize view；若只服务 desktop-app，保持 native shape 更清楚 |
| Thread 核心 | ~~`thread/start`~~、`thread/read`、`thread/list` | ~~`thread/start` 已保留 native `threadId` 并附带 Codex `thread` view~~；`thread/read` / `thread/list` 仍需要完整 Codex `Thread` view 对齐 | A/B | `thread/start` 已完成最小 view；后续补 read/list 的完整状态字段、turn/item 容器 |
| Thread 扩展 | `thread/resume`、`thread/fork`、`thread/archive`、`thread/unarchive`、`thread/unsubscribe`、`thread/name/set`、`thread/metadata/update`、`thread/compact/start`、`thread/shellCommand`、`thread/approveGuardianDeniedAction`、`thread/rollback`、`thread/loaded/list`、`thread/turns/list`、`thread/inject_items` | 当前 router 无这些 method；Dasclaw 只有 `thread/create/start/list/read` | C/D1/D2 | shell/approval/job 等底座部分存在，但还缺 app-server thread persistence、归档/恢复、压缩、inject item 等 owner |
| Turn 核心 | ~~`turn/start`~~、`turn/interrupt` | ~~`turn/start` 已保留 native `turnId` 并附带 Codex `turn` view~~；`turn/interrupt` 仍复用 cancel params | A/B | `turn/start` 已完成 Codex text input 子集与 `Turn` object response；后续继续核对 interrupt terminal 状态映射 |
| Turn steer | `turn/steer` | `crates/dasclaw_protocol` 有 `ActiveTurnNotSteerable` / `NonSteerableTurnKind` 这类 steer 错误语义；app-server 无 `turn/steer` method | C/D1/D2 | 需要把运行中 turn steer 控制能力接成 app-server owner，不能只补空 handler |
| Model | ~~`model/list`~~ | ~~已由 app-server 路由并返回 Codex `ModelListResponse`；`desktop-app` 的 `modelProvider/list` 仅保留 renderer alias~~ | A | 已完成；后续若需要再补 pagination/hidden model 等更完整语义 |
| Skills | `skills/list`、`skills/config/write` | `crates` 有 skill 词汇，历史客户端有参考实现但不是当前 `desktop-app` 目标；app-server capability matrix 标 `skills` declared future，service health disabled | D1/D2 | 需要把 native skills registry/service 明确迁到 app-server；不能直接依赖已废弃 `desktop-client` |
| Plugin / marketplace / app | `plugin/list`、`plugin/read`、`plugin/install`、`plugin/uninstall`、`marketplace/add`、`marketplace/remove`、`marketplace/upgrade`、`app/list` | 无对应 Dasclaw app-server 能力 | E/D2 | Codex 产品扩展/市场域；除非 Dasclaw 要做插件市场，否则不建议照搬 |
| Filesystem | `fs/readFile`、`fs/writeFile`、`fs/createDirectory`、`fs/getMetadata`、`fs/readDirectory`、`fs/remove`、`fs/copy`、`fs/watch`、`fs/unwatch` | FS read/write/search 工具存在；app-server 无 `fs/*` method，sandbox disabled | D1/D2 | 已有文件工具底座，但 app-server 还缺 workspace root、权限、审计、watch/unwatch、`fs/changed`、sandbox/approval 边界 |
| Command exec | `command/exec`、`command/exec/write`、`command/exec/terminate`、`command/exec/resize` | shell tool 和 sandbox executor 存在；app-server 仍无 command service，tools/sandbox disabled | D1 | 需要把 PTY/进程生命周期、approval、sandbox、output streaming 接成 app-server control-plane |
| MCP | `mcpServer/oauth/login`、`config/mcpServer/reload`、`mcpServerStatus/list`、`mcpServer/resource/read`、`mcpServer/tool/call` | `crates/dasclaw_mcp` 有 config/auth/session/transport/client/executor；app-server `mcp` declared future，service health disabled | D1 | 需要把现有 MCP registry、OAuth、resource read、tool call、progress/event bridge 接入 app-server |
| Approval / guardian | `thread/approveGuardianDeniedAction`，以及 ServerRequest 里的 approval 系列 | runtime approval primitive 存在；app-server `approval` declared future，DLP/policy unavailable fail-safe | D1 | 需要把 `AgentEvent::ApprovalNeeded` / `respond_to_approval` 接成 fail-safe app-server request/response loop |
| Sandbox | `windowsSandbox/setupStart` | sandbox crates / shell sandbox 存在；app-server `sandbox` declared future 且 service health disabled | D1/E | Windows 特定 setup 可不照搬；但 Dasclaw 若要 command/fs/tool 能力，应把现有平台 sandbox 抽象接入 app-server |
| Account/auth/rate limit | `account/login/start`、`account/login/cancel`、`account/logout`、`account/rateLimits/read`、`account/sendAddCreditsNudgeEmail`、`account/read`、`getAuthStatus` | 无 | E | Codex/OpenAI 产品账户域，不属于 Dasclaw native app-server 必需能力 |
| Config / experimental / feedback / external agent | `config/read`、`config/value/write`、`config/batchWrite`、`configRequirements/read`、`experimentalFeature/list`、`experimentalFeature/enablement/set`、`feedback/upload`、`externalAgentConfig/detect`、`externalAgentConfig/import` | config 数据类型和 provider config 底座存在；app-server 无等价 service surface；external agent import/feedback/experiment 仍偏产品域 | C/D1/E | config 可从现有类型与 desktop-app manager 上收；实验/反馈/external-agent import 需先定义 Dasclaw 产品 owner |
| Device key | `device/key/create`、`device/key/public`、`device/key/sign` | 无 | E/C | 若 Dasclaw 需要本地设备身份，可另设安全设计；不建议直接借 Codex 名称 |
| Review / git / summary / fuzzy search | `review/start`、`gitDiffToRemote`、`getConversationSummary`、`fuzzyFileSearch` | git/search 工具存在；未发现 Codex-style review/start、remote diff、conversation summary、fuzzy search session app-server service | C/D1/D2/E | `gitDiffToRemote` 可复用 git 工具底座，`fuzzyFileSearch` 可借搜索工具但不是同构；review/summary 依赖 repo service、model summary 或 Codex 产品逻辑 |

## 6. Codex ServerNotification 对 Dasclaw 缺口表

| Codex 域 | Codex notification | Dasclaw 当前状态 | 分类 | 补齐含义 |
|---|---|---|---|---|
| Error | `error` | 有同名 | A | 需核对 payload shape |
| Thread 核心 | ~~`thread/started`~~ | ~~有同名；Codex profile 下已附带 Codex `thread` view~~；另有 Dasclaw-only `thread/created` | A | 已完成 Codex-compatible `ThreadStartedNotification` 子集 |
| Thread 状态/历史 | `thread/status/changed`、`thread/archived`、`thread/unarchived`、`thread/closed`、`thread/name/updated`、`thread/goal/updated`、`thread/goal/cleared`、`thread/tokenUsage/updated`、`thread/compacted` | 无 | C/D | 需要 thread lifecycle、goal、usage、compaction 状态 |
| Turn 核心 | ~~`turn/started`~~、~~`turn/completed`~~ | ~~有同名；Codex profile 下已附带 Codex `turn` view~~；Dasclaw 另有 ~~`turn/failed`~~、`turn/cancelled` | A/B | 已完成 started/completed 的 Codex-compatible 子集，且 native `turn/failed` 已附带 failed `turn` view；`turn/cancelled` 和 Codex 聚合 terminal 语义仍待后续核对 |
| Turn plan/diff | `turn/diff/updated`、`turn/plan/updated` | `crates/dasclaw_protocol` 有 `PlanDeltaEvent`；app-server 无 Codex turn-level plan/diff notification | D1/D2 | 需要 diff/plan producer 与流式更新，并区分 item-level `PlanDelta` 与 turn-level plan/diff snapshot |
| Item text/reasoning | `item/started`、`item/agentMessage/delta`、`item/reasoning/summaryTextDelta`、`item/reasoning/summaryPartAdded`、`item/reasoning/textDelta`、`item/completed` | 有同名或近似事件；payload shape 不完全一致 | A/B | 这是最适合先补 Codex-compatible view 的核心 streaming 面 |
| Item plan/raw/tool/file/command | `rawResponseItem/completed`、`item/plan/delta`、`item/commandExecution/outputDelta`、`item/commandExecution/terminalInteraction`、`item/fileChange/outputDelta`、`item/fileChange/patchUpdated`、`command/exec/outputDelta` | tool/command/file primitives 分散存在，但 app-server 无这些通知面 | D1/D2 | 需要 raw response、plan、command、file change 能力和安全边界 |
| Approval review | `item/autoApprovalReview/started`、`item/autoApprovalReview/completed`、`serverRequest/resolved` | runtime approval primitive 存在；app-server 无 server-request loop | D1 | 依赖 approval server-request loop |
| MCP | `item/mcpToolCall/progress`、`mcpServer/oauthLogin/completed`、`mcpServer/startupStatus/updated` | MCP crate 底座存在；app-server mcp disabled 且无这些 notification | D1 | 需要把 MCP registry / startup / progress event 接到 app-server notification bus |
| Account/app/skills | `account/updated`、`account/rateLimits/updated`、`account/login/completed`、`app/list/updated`、`skills/changed` | account/app 偏 Codex 产品域；skills 有协议/legacy 参考但 app-server disabled | E/D1/D2 | skills 需要 Dasclaw native registry；account/app list 不应默认照搬 |
| External/fs/model/fuzzy/hooks | `externalAgentConfig/import/completed`、`fs/changed`、`model/rerouted`、`model/verification`、`fuzzyFileSearch/sessionUpdated`、`fuzzyFileSearch/sessionCompleted`、`hook/started`、`hook/completed` | fs/model/search/hooks 底座分散存在；external agent import 与 Codex fuzzy session 未同构 | C/D1/D2/E | 需根据 Dasclaw 产品目标拆分：fs/model/hooks 可接线，fuzzy session/external agent 需另定协议 |
| Realtime / Windows | `thread/realtime/*`、`windows/worldWritableWarning`、`windowsSandbox/setupCompleted` | 无 | E | Codex 特定 realtime/audio/Windows sandbox surface，不建议作为 Dasclaw 能力补齐第一阶段 |
| Warning | `warning`、`guardianWarning`、`deprecationNotice`、`configWarning` | Dasclaw 有 health/lifecycle/error，但无这些具体 notification | B/C | 可补通用 warning channel；guardian/config warning 取决于 policy/config 能力 |

## 7. Codex ServerRequest 对 Dasclaw 缺口表

Codex 的 9 个 `ServerRequest` 在 Dasclaw 当前 app-server 中都没有同构实现。它们不是普通 notification，而是服务端主动向客户端要决策/输入/令牌，因此必须有 request tracking、timeout、fail-safe、UI 决策回传和审计。

| Codex ServerRequest | Dasclaw 当前状态 | 分类 | 为什么不能只补协议 |
|---|---|---|---|
| `item/commandExecution/requestApproval` | approval / command / sandbox primitive 存在，但 app-server 无 request loop | D1 | 需要把 command execution、approval policy、client decision loop 接成 app-server 协议 |
| `item/fileChange/requestApproval` | 文件工具与 approval primitive 分散存在，但 app-server 无 diff/approval request | D1/D2 | 需要 file change detector、diff/patch model、approval |
| `item/permissions/requestApproval` | approval primitive 存在，权限模型未作为 app-server service 暴露 | D1/D2 | 需要权限模型和拒绝/允许后的执行路径 |
| `item/tool/requestUserInput` | tool execution primitive 存在，user-input suspension 未作为 app-server 协议暴露 | D1/D2 | 需要 tool/user-input suspension 和 resume |
| `item/tool/call` | `ToolExecutor` / dispatcher 存在，但 app-server 无 dynamic tool call service | D1 | 需要 dynamic tool registry、tool call execution、result streaming |
| `mcpServer/elicitation/request` | MCP crate 底座存在；未见 app-server elicitation request loop | D1/D2 | 需要 MCP elicitation support 和 client UI contract |
| `account/chatgptAuthTokens/refresh` | 无 | E | Codex/OpenAI account token 域 |
| `applyPatchApproval` | approval primitive 存在，但 Codex legacy patch approval 面未接 | D1/E | Codex legacy approval；若 Dasclaw 做 patch approval，应基于自有 file change/approval 设计 |
| `execCommandApproval` | approval / shell sandbox primitive 存在，但 Codex legacy exec approval 面未接 | D1/E | Codex legacy exec approval；应和 command execution/sandbox 一起设计 |

## 8. Dasclaw 有而 Codex app-server 协议没有的面

这些不是 Codex 的缺陷，而是 Dasclaw app-server 作为本地 service control plane 的自有设计。

| Dasclaw-only surface | 类型 | 意义 |
|---|---|---|
| `protocol/schema` | method | Dasclaw native 协议发现；Codex 依赖生成 schema，不走 runtime schema 方法 |
| `health/check`、`capabilities/list` | method | 明确让 GUI 按 capability matrix/health gating，而不是猜后端能力 |
| `lifecycle/status`、`shutdown` | method | 本地 sidecar lifecycle 控制面 |
| `thread/create` | method | Legacy smoke / native alias；compat profile 已声明可映射到 `thread/start` |
| `turn/cancel` | method | Native alias；compat profile 映射到 `turn/interrupt` |
| `turn/list`、`turn/read` | method | Dasclaw native turn-level read/list；Codex 用 `thread/turns/list` 等线程视角 |
| `modelProvider/selectForNextTurn` | method | desktop-app / renderer-mediated model provider selection；Codex 有 `model/list` 但没有这个同名选择入口 |
| `notifications/initialized` | event | Dasclaw 服务端通知；Codex 的 `initialized` 是 ClientNotification |
| `lifecycle/changed`、`health/changed`、`capabilities/changed` | event | Native control-plane state |
| `log/entry` | event | 已声明但 Phase 1 未接线的 logs event |
| `thread/created` | event | Native thread creation alias |
| `turn/delta` | event | Legacy/smoke delta；Codex 核心文本流是 `item/agentMessage/delta` |
| ~~`turn/failed`~~、`turn/cancelled` | event | ~~`turn/failed` 已补 Codex-compatible failed `turn` view~~；`turn/cancelled` 仍是 native terminal variant；Codex 聚合在 `turn/completed` 的 `turn.status` |

## 9. 能力补齐优先级

如果目标是“补齐 dasclaw-app-server 能力”，建议按能力依赖顺序补，而不是按 Codex method 字母顺序补。

| 优先级 | 目标 | 包含 | 原因 |
|---|---|---|---|
| P0 | 诚实的协议边界 | `codex_app_server_v2` 继续标成 chat-session subset；P3-P6 opt-out 保持显式 | 已由 capability/profile tests 约束 |
| P1 | Chat-session compatibility view | ~~`thread/start`~~、~~`turn/start`~~、~~`thread/started`~~、~~`turn/started`~~、~~`turn/completed`~~ 保留 native fields 并附带 Codex `thread` / `turn` view | desktop-app 通过 normalization 同时兼容 native 与 Codex view |
| P2 | Model catalog/service | ~~`model/list`~~ 由 app-server 返回 Codex `ModelListResponse`；~~`modelProvider/list`~~ 仅作为 desktop-app renderer alias | app-server 成为模型列表 owner，主进程不再直接暴露 provider secrets |
| P3 | Approval + tool + sandbox 三件套 | ServerRequest request tracking、approval decision、tool registry、sandbox adapter、fail-safe timeout | Codex 大量协议依赖这组能力，不能分开虚补 |
| P4 | MCP / skills / logs / jobs | MCP registry/OAuth/resource/tool call；skills registry；logs source；job host | MCP、logs、jobs、skills 已有不同程度底座或 legacy 参考；重点是上收 app-server owner 与协议接线 |
| P5 | Filesystem / command exec | `fs/*`、`command/exec*`、`fs/changed`、command output | 必须在 P3 安全边界之后做，否则风险大 |
| P6 | Product-specific Codex domains | account、plugin、marketplace、app list、feedback、external agent import、Windows sandbox、realtime audio | 只有当 Dasclaw 明确要兼容未改 Codex client 或复刻相关产品能力时再做 |

## 10. 决策建议

1. 不建议把 Dasclaw native protocol 改名伪装成完整 Codex app-server。当前证据显示它只覆盖 chat-session subset，硬伪装会让客户端在 tools/MCP/approval/fs/command/account 等域踩到 app-server 接线缺口或产品语义缺口。
2. 可以新增 Codex-compatible profile/view，但要按 capability gating 输出，未实现域要明确 unsupported，而不是静默 no-op。
3. 最短可交付路线是先补 P1：让同名 thread/turn/item streaming 在 Codex profile 下返回 Codex shape。这样既能服务 `desktop-app`/AI SDK transport，又不会承诺完整 Codex 产品控制面。
4. 真正的能力补齐应从 P3 开始进入重活：approval、tool execution、sandbox 是一组安全边界，任何 `fs/*`、`command/exec*`、`item/tool/call` 都不应该绕过它们单独开放。
