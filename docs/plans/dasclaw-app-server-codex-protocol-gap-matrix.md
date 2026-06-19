# Dasclaw app-server 与 Codex app-server 协议缺口对照表

> 日期：2026-06-18
> 状态：能力补齐参考（删除线表示本轮 P0-P3 已完成；未划掉项仍按后续计划处理）
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
| Level 3 字面层 | Codex 生成协议 union：`codex-cli-main/codex-rs/app-server-protocol/schema/typescript/{ClientRequest,ServerNotification,ServerRequest,ClientNotification}.ts`；Dasclaw 常量、能力矩阵、router：`crates/dasclaw_app_server_protocol/src/lib.rs`、`crates/dasclaw_app_server/src/lib.rs` | Codex：74 个 `ClientRequest`、61 个 `ServerNotification`、9 个 `ServerRequest`、1 个 `ClientNotification`。Dasclaw 当前：18 个 method、22 个 event，并新增 JSON-RPC `ServerRequest` response path；router 只路由这些 method |

已检查 `dasclaw-app-server / codex app-server protocol gap matrix` 是否已有，结论：已有 `docs/plans/dasclaw-app-server-protocol-v0.md`、`docs/plans/dasclaw-app-server-ownership-matrix.md`、`docs/plans/dasclaw-codex-app-server-ai-sdk-compat-plan.md`，但未发现“Codex 全量 ClientRequest / ServerNotification / ServerRequest / ClientNotification 与 Dasclaw 当前协议逐域缺口”的对照表；本文补齐该空白。

子 agent 只读交叉检查结论与主线一致：`codex-cli-main/codex-rs/app-server-protocol/src/schema.rs` 在当前仓库不存在，实际应以 `src/protocol/v1.rs` 和 `schema/typescript/*.ts` 作为协议证据，其中 TypeScript 生成文件最适合逐项枚举。

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
| `crates/dasclaw_app_server_protocol/src/lib.rs` | Dasclaw 当前常量定义 18 个 method、22 个 event；`model/list` 与 `approval/respond` 已进入 protocol schema |
| `crates/dasclaw_app_server_protocol/src/lib.rs` | Dasclaw Phase 1 实现 `protocol`、`lifecycle`、`health`、`session`、`model_provider`；P3 feature-gated runtime 下可启用 `approval` / `tools` / `sandbox` 子集；`dlp_policy`、`jobs`、`skills`、`mcp`、`logs` 仍是 declared future |
| `crates/dasclaw_app_server_protocol/src/lib.rs` | `codex_app_server_v2_chat_session_subset` profile 明确是 `ChatSessionSubset`，只列 `initialize`、`thread/start`、`thread/read`、`thread/list`、`thread/turns/list`、`turn/start`、`turn/interrupt`、`turn/read`、`model/list` 并 opt out 完整 tool / approval / sandbox 等 Codex 能力 |
| `crates/dasclaw_app_server/src/lib.rs` | Dasclaw router 只实际路由当前 18 个 method，其他 method 会落到 `method_not_found` |
| `crates/dasclaw_app_server/src/lib.rs` | 运行时健康状态按 bridge feature gate 呈现 tools / sandbox；jobs / skills / mcp disabled，DLP policy unavailable fail-safe |

结论分三层：

| 层级 | 判断 |
|---|---|
| 本轮已完成 | `model/list`；`thread/start` / `turn/start` 返回 nested Codex `thread` / `turn` view；`turn/interrupt` 使用 Codex params/empty response；`thread/started`、`turn/started`、`turn/completed` 使用 v2-shaped nested view；失败和中断都通过 `turn/completed.turn.status` 表达；`item/started`、`item/agentMessage/delta`、`item/reasoning/summaryTextDelta`、`item/completed` 已接 v2-shaped producer；P3 command approval server-request loop、`approval/respond`、tool output/result notification、timeout/interrupt/shutdown fail-safe；`desktop-app` 已消费单一 native v2-shaped surface |
| 协议同名/近似可用 | `initialize`、`thread/start`、`thread/read`、`thread/list`、`turn/start`、`turn/interrupt`、若干 item/turn streaming notification |
| 协议缺口但可通过 compatibility view 补 | `thread/read` / `thread/list` 的完整 Codex `Thread` view、更多 item/turn 细粒度 payload shape |
| agent 框架 `crates` 已有底座但 app-server 未完全接线 | approval / tool execution / sandbox primitives 已在 `crates` 下的 runtime/tool/sandbox 相关 crate 中存在；P3 已补 command approval request/response、tool lifecycle notification、service health 与 capability gating 子集；缺的是 standalone command/fs service、dynamic tool registry/call、permission/file-change approval producer、MCP elicitation 和更完整的审计面 |
| 已有分散底座但 app-server 尚未成为 owner | MCP / skills / jobs / logs / filesystem / git / search / config 等在 `crates`、历史参考实现或 `desktop-app` manager 中有不同程度的实现；缺的是 app-server service owner、协议接线、能力健康状态和安全边界 |
| 产品/服务能力尚未定义或未迁移 | account / plugin marketplace / app list / device key / external agent import / Codex review 等仍偏 Codex 产品域或需要先定义 Dasclaw 产品语义 |

## 2. Codex app-server 协议清单

> 标记说明：删除线表示 Dasclaw 本轮 P0-P3 已补齐、已接入 Codex-compatible view，或已有受测的协议/桥接子集；同一行里未划掉的协议仍按后续计划处理。

### 2.1 ClientRequest：74 个

来源：`codex-cli-main/codex-rs/app-server-protocol/schema/typescript/ClientRequest.ts:79`。

| 域 | Codex method |
|---|---|
| 初始化 | `initialize` |
| Thread lifecycle / history | ~~`thread/start`~~、`thread/resume`、`thread/fork`、`thread/archive`、`thread/unsubscribe`、`thread/name/set`、`thread/metadata/update`、`thread/unarchive`、`thread/compact/start`、`thread/shellCommand`、`thread/approveGuardianDeniedAction`、`thread/rollback`、`thread/list`、`thread/loaded/list`、`thread/read`、`thread/turns/list`、`thread/inject_items` |
| Turn lifecycle | ~~`turn/start`~~、`turn/steer`、~~`turn/interrupt`~~ |
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
| Item streaming | ~~`item/started`~~、~~`item/completed`~~、`rawResponseItem/completed`、~~`item/agentMessage/delta`~~、`item/plan/delta`、~~`item/reasoning/summaryTextDelta`~~、`item/reasoning/summaryPartAdded`、`item/reasoning/textDelta` |
| Approval / command / file change | `item/autoApprovalReview/started`、`item/autoApprovalReview/completed`、`command/exec/outputDelta`、~~`item/commandExecution/outputDelta`~~、~~`item/commandExecution/terminalInteraction`~~、`item/fileChange/outputDelta`、`item/fileChange/patchUpdated`、~~`serverRequest/resolved`~~ |
| MCP | `item/mcpToolCall/progress`、`mcpServer/oauthLogin/completed`、`mcpServer/startupStatus/updated` |
| Account / app / skills | `account/updated`、`account/rateLimits/updated`、`account/login/completed`、`app/list/updated`、`skills/changed` |
| External / fs / model / fuzzy / hooks / Windows | `externalAgentConfig/import/completed`、`fs/changed`、`model/rerouted`、`model/verification`、`fuzzyFileSearch/sessionUpdated`、`fuzzyFileSearch/sessionCompleted`、`hook/started`、`hook/completed`、`windows/worldWritableWarning`、`windowsSandbox/setupCompleted` |

### 2.3 ServerRequest：9 个

来源：`codex-cli-main/codex-rs/app-server-protocol/schema/typescript/ServerRequest.ts:18`。

| 域 | Codex server-initiated request |
|---|---|
| Approval | ~~`item/commandExecution/requestApproval`~~、`item/fileChange/requestApproval`、`item/permissions/requestApproval`、`applyPatchApproval`、`execCommandApproval` |
| Tool / user input | `item/tool/requestUserInput`、`item/tool/call` |
| MCP elicitation | `mcpServer/elicitation/request` |
| Account token | `account/chatgptAuthTokens/refresh` |

### 2.4 ClientNotification：1 个

来源：`codex-cli-main/codex-rs/app-server-protocol/schema/typescript/ClientNotification.ts:5`。

| 域 | Codex client notification |
|---|---|
| Connection lifecycle | `initialized` |

## 3. Dasclaw 当前协议清单

### 3.1 Method：18 个

来源：`crates/dasclaw_app_server_protocol/src/lib.rs` 与 `phase_one_methods()`。

| 能力域 | Dasclaw method |
|---|---|
| Protocol | `initialize`、`protocol/schema` |
| Health | `health/check`、`capabilities/list` |
| Lifecycle | `lifecycle/status`、`shutdown` |
| Session / thread | `thread/start`、`thread/list`、`thread/read`、`thread/turns/list` |
| Session / turn | `turn/start`、`turn/interrupt`、`turn/read` |
| Model provider | ~~`model/list`~~、`modelProvider/selectForNextTurn` |
| Approval | ~~`approval/respond`~~ |

### 3.2 Event：22 个

来源：`crates/dasclaw_app_server_protocol/src/lib.rs:36-56` 与 `phase_one_events()`。

| 能力域 | Dasclaw event |
|---|---|
| Protocol | `notifications/initialized` |
| Lifecycle / health / logs | `lifecycle/changed`、`health/changed`、`capabilities/changed`、`log/entry` |
| Thread / turn | ~~`thread/started`~~、~~`turn/started`~~、~~`turn/completed`~~ |
| Item streaming | ~~`item/started`~~、~~`item/agentMessage/delta`~~、~~`item/reasoning/summaryTextDelta`~~、`item/reasoning/summaryPartAdded`、`item/reasoning/textDelta`、~~`item/completed`~~ |
| Approval / tool | ~~`serverRequest/resolved`~~、~~`item/commandExecution/outputDelta`~~、~~`item/commandExecution/terminalInteraction`~~ |
| Error | `error` |

### 3.3 当前 desktop-app 消费面

`desktop-app` 只消费 Dasclaw 当前子集：

| 证据 | 说明 |
|---|---|
| `desktop-app/src/main/appServerManager.ts` | 初始化发送 native capability 请求，并附带 model provider config；chat-session wire shape 本身已经是 v2-shaped，不再通过 alias profile 切换 |
| `desktop-app/src/main/appServerManager.ts` | ~~`modelProvider/list`~~ 仍是客户端 renderer alias，但内部已转发到 app-server ~~`model/list`~~；`modelProvider/selectForNextTurn` 仍由 manager 侧特殊处理 |
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
| Approval | `crates/dasclaw_runtime/src/approval.rs` 已有 `ApprovalPolicy`、`ApprovalRequest`、`ApprovalDecision`、`ApprovalInbox`、`Approver`；`Agent::respond_to_approval` 可回填 GUI 决策 | ~~P3 已把 `AgentEvent::ApprovalNeeded` 接成 `item/commandExecution/requestApproval` -> `approval/respond` -> `Agent::respond_to_approval`，并补 `serverRequest/resolved`、timeout/cancel/shutdown fail-safe~~；file-change / permission approval service 仍未作为 app-server producer / owner 暴露 |
| Tool execution | `crates/dasclaw_runtime/src/agent.rs` 有 `ToolExecutor`；`AgentBuilder::tool_executor*` 可接 executor；`tool_dispatch.rs` 有 approval gate -> egress -> executor -> sanitizer 的顺序 pipeline | ~~P3 已接 `ToolCallStart` / `ToolResult` 到 `item/commandExecution/outputDelta` 与 `item/commandExecution/terminalInteraction`~~；仍没有 tool registry/list/call、dynamic `item/tool/call` response |
| Sandbox | `crates/dasclaw_shell_tools`、`crates/dasclaw_sandbox*`、`crates/dasclaw_workspace_cap` 已有 shell sandbox、OS sandbox policy、Linux/Windows sandbox 相关实现 | ~~P3 已把 sandbox readiness 放到 runtime bridge feature gate / service health / capability matrix~~；仍没有 standalone command/fs/tool 沙箱执行入口、policy negotiation 或 workspace-bound sandbox adapter |
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
| Thread 核心 | ~~`thread/start`~~、`thread/read`、`thread/list`、`thread/turns/list` | `thread/start` 返回嵌套 `thread` object 与真实 `model` / `modelProvider` / `cwd` metadata；`thread/read`、`thread/list`、`thread/turns/list` 仍是 chat-session subset，需要后续补完整 Codex `Thread` history / lifecycle 字段 | A/B | `thread/start` 已完成最小 v2-shaped view；后续补 read/list 的完整状态字段、turn/item 容器 |
| Thread 扩展 | `thread/resume`、`thread/fork`、`thread/archive`、`thread/unarchive`、`thread/unsubscribe`、`thread/name/set`、`thread/metadata/update`、`thread/compact/start`、`thread/shellCommand`、`thread/approveGuardianDeniedAction`、`thread/rollback`、`thread/loaded/list`、`thread/inject_items` | 当前 public router 无这些 method；历史 `thread/create` smoke 名称不属于 native contract | C/D1/D2 | shell/approval/job 等底座部分存在，但还缺 app-server thread persistence、归档/恢复、压缩、inject item 等 owner |
| Turn 核心 | ~~`turn/start`~~、~~`turn/interrupt`~~、`turn/read` | `turn/start` 返回嵌套 `turn` object，接受 text-only `input: UserInput[]`；`turn/interrupt` 接收 `threadId` / `turnId` 并返回空对象；terminal 结果统一通过 `turn/completed` 的 nested `turn.status` 表达 | A/B | `turn/start` 已完成 text input 子集与 `Turn` object response；`turn/interrupt` 已完成当前 interrupt 子集，后续只需随更完整 terminal 状态语义继续校准 |
| Turn steer | `turn/steer` | `crates/dasclaw_protocol` 有 `ActiveTurnNotSteerable` / `NonSteerableTurnKind` 这类 steer 错误语义；app-server 无 `turn/steer` method | C/D1/D2 | 需要把运行中 turn steer 控制能力接成 app-server owner，不能只补空 handler |
| Model | ~~`model/list`~~ | ~~已由 app-server 路由并返回 Codex `ModelListResponse`；`desktop-app` 的 `modelProvider/list` 仅保留 renderer alias~~ | A | 已完成；后续若需要再补 pagination/hidden model 等更完整语义 |
| Skills | `skills/list`、`skills/config/write` | `crates` 有 skill 词汇，历史客户端有参考实现但不是当前 `desktop-app` 目标；app-server capability matrix 标 `skills` declared future，service health disabled | D1/D2 | 需要把 native skills registry/service 明确迁到 app-server；不能直接依赖已废弃 `desktop-client` |
| Plugin / marketplace / app | `plugin/list`、`plugin/read`、`plugin/install`、`plugin/uninstall`、`marketplace/add`、`marketplace/remove`、`marketplace/upgrade`、`app/list` | 无对应 Dasclaw app-server 能力 | E/D2 | Codex 产品扩展/市场域；除非 Dasclaw 要做插件市场，否则不建议照搬 |
| Filesystem | `fs/readFile`、`fs/writeFile`、`fs/createDirectory`、`fs/getMetadata`、`fs/readDirectory`、`fs/remove`、`fs/copy`、`fs/watch`、`fs/unwatch` | FS read/write/search 工具存在；app-server 无 `fs/*` method，sandbox disabled | D1/D2 | 已有文件工具底座，但 app-server 还缺 workspace root、权限、审计、watch/unwatch、`fs/changed`、sandbox/approval 边界 |
| Command exec | `command/exec`、`command/exec/write`、`command/exec/terminate`、`command/exec/resize` | shell tool 和 sandbox executor 存在；app-server 仍无 command service，tools/sandbox disabled | D1 | 需要把 PTY/进程生命周期、approval、sandbox、output streaming 接成 app-server control-plane |
| MCP | `mcpServer/oauth/login`、`config/mcpServer/reload`、`mcpServerStatus/list`、`mcpServer/resource/read`、`mcpServer/tool/call` | `crates/dasclaw_mcp` 有 config/auth/session/transport/client/executor；app-server `mcp` declared future，service health disabled | D1 | 需要把现有 MCP registry、OAuth、resource read、tool call、progress/event bridge 接入 app-server |
| Approval / guardian | `thread/approveGuardianDeniedAction`，以及 ServerRequest 里的 approval 系列 | ~~runtime command approval 已接成 fail-safe app-server request/response loop~~；guardian denied action、file-change approval、permission approval service 仍未接 | D1 | 下一步应补 guardian/file/permission 语义，而不是再补空 handler |
| Sandbox | `windowsSandbox/setupStart` | sandbox crates / shell sandbox 存在；P3 runtime bridge 可把 app-server `sandbox` capability / service health 切到 implemented / ready，但仍无 Codex Windows setup method | D1/E | Windows 特定 setup 可不照搬；但 Dasclaw 若要 command/fs/tool 能力，应继续把现有平台 sandbox 抽象接入 app-server |
| Account/auth/rate limit | `account/login/start`、`account/login/cancel`、`account/logout`、`account/rateLimits/read`、`account/sendAddCreditsNudgeEmail`、`account/read`、`getAuthStatus` | 无 | E | Codex/OpenAI 产品账户域，不属于 Dasclaw native app-server 必需能力 |
| Config / experimental / feedback / external agent | `config/read`、`config/value/write`、`config/batchWrite`、`configRequirements/read`、`experimentalFeature/list`、`experimentalFeature/enablement/set`、`feedback/upload`、`externalAgentConfig/detect`、`externalAgentConfig/import` | config 数据类型和 provider config 底座存在；app-server 无等价 service surface；external agent import/feedback/experiment 仍偏产品域 | C/D1/E | config 可从现有类型与 desktop-app manager 上收；实验/反馈/external-agent import 需先定义 Dasclaw 产品 owner |
| Device key | `device/key/create`、`device/key/public`、`device/key/sign` | 无 | E/C | 若 Dasclaw 需要本地设备身份，可另设安全设计；不建议直接借 Codex 名称 |
| Review / git / summary / fuzzy search | `review/start`、`gitDiffToRemote`、`getConversationSummary`、`fuzzyFileSearch` | git/search 工具存在；未发现 Codex-style review/start、remote diff、conversation summary、fuzzy search session app-server service | C/D1/D2/E | `gitDiffToRemote` 可复用 git 工具底座，`fuzzyFileSearch` 可借搜索工具但不是同构；review/summary 依赖 repo service、model summary 或 Codex 产品逻辑 |

## 6. Codex ServerNotification 对 Dasclaw 缺口表

| Codex 域 | Codex notification | Dasclaw 当前状态 | 分类 | 补齐含义 |
|---|---|---|---|---|
| Error | `error` | 有同名 | A | 需核对 payload shape |
| Thread 核心 | ~~`thread/started`~~ | 有同名，payload 带 nested `thread` view；历史 `thread/created` smoke event 不属于 public native chat-session surface | A | 已完成 v2-shaped `ThreadStartedNotification` 子集 |
| Thread 状态/历史 | `thread/status/changed`、`thread/archived`、`thread/unarchived`、`thread/closed`、`thread/name/updated`、`thread/goal/updated`、`thread/goal/cleared`、`thread/tokenUsage/updated`、`thread/compacted` | 无 | C/D | 需要 thread lifecycle、goal、usage、compaction 状态 |
| Turn 核心 | ~~`turn/started`~~、~~`turn/completed`~~ | 有同名，payload 带 nested `turn` view；成功、失败、中断都聚合到 `turn/completed`，由 `turn.status` 和 `turn.error` 区分 | A/B | 已完成 started/completed 的 v2-shaped 子集；历史 `turn/failed` / `turn/cancelled` 不再作为 public terminal event |
| Turn plan/diff | `turn/diff/updated`、`turn/plan/updated` | `crates/dasclaw_protocol` 有 `PlanDeltaEvent`；app-server 无 Codex turn-level plan/diff notification | D1/D2 | 需要 diff/plan producer 与流式更新，并区分 item-level `PlanDelta` 与 turn-level plan/diff snapshot |
| Item text/reasoning | ~~`item/started`~~、~~`item/agentMessage/delta`~~、~~`item/reasoning/summaryTextDelta`~~、`item/reasoning/summaryPartAdded`、`item/reasoning/textDelta`、~~`item/completed`~~ | ~~P1 已把 item started / agent delta / reasoning summary text delta / item completed 接到 Codex v2 profile producer~~；summary part added 与 reasoning text delta 仍只是 schema/profile 面，尚未看到 app-server producer | A/B | 核心文本流已完成受测子集；后续补 reasoning part/text producer 与更完整 item payload shape |
| Item plan/raw/tool/file/command | `rawResponseItem/completed`、`item/plan/delta`、~~`item/commandExecution/outputDelta`~~、~~`item/commandExecution/terminalInteraction`~~、`item/fileChange/outputDelta`、`item/fileChange/patchUpdated`、`command/exec/outputDelta` | ~~P3 已补 runtime tool output/result 到 commandExecution notification 的桥接~~；raw response、plan、file change、standalone command exec 仍未接 | D1/D2 | 后续需要 raw response、plan、command、file change 能力和安全边界 |
| Approval review | `item/autoApprovalReview/started`、`item/autoApprovalReview/completed`、~~`serverRequest/resolved`~~ | ~~P3 已补 server-request loop 的 resolved 结果~~；auto approval review started/completed 仍未接 | D1 | 下一步依赖 auto-approval policy/review producer |
| MCP | `item/mcpToolCall/progress`、`mcpServer/oauthLogin/completed`、`mcpServer/startupStatus/updated` | MCP crate 底座存在；app-server mcp disabled 且无这些 notification | D1 | 需要把 MCP registry / startup / progress event 接到 app-server notification bus |
| Account/app/skills | `account/updated`、`account/rateLimits/updated`、`account/login/completed`、`app/list/updated`、`skills/changed` | account/app 偏 Codex 产品域；skills 有协议/legacy 参考但 app-server disabled | E/D1/D2 | skills 需要 Dasclaw native registry；account/app list 不应默认照搬 |
| External/fs/model/fuzzy/hooks | `externalAgentConfig/import/completed`、`fs/changed`、`model/rerouted`、`model/verification`、`fuzzyFileSearch/sessionUpdated`、`fuzzyFileSearch/sessionCompleted`、`hook/started`、`hook/completed` | fs/model/search/hooks 底座分散存在；external agent import 与 Codex fuzzy session 未同构 | C/D1/D2/E | 需根据 Dasclaw 产品目标拆分：fs/model/hooks 可接线，fuzzy session/external agent 需另定协议 |
| Realtime / Windows | `thread/realtime/*`、`windows/worldWritableWarning`、`windowsSandbox/setupCompleted` | 无 | E | Codex 特定 realtime/audio/Windows sandbox surface，不建议作为 Dasclaw 能力补齐第一阶段 |
| Warning | `warning`、`guardianWarning`、`deprecationNotice`、`configWarning` | Dasclaw 有 health/lifecycle/error，但无这些具体 notification | B/C | 可补通用 warning channel；guardian/config warning 取决于 policy/config 能力 |

## 7. Codex ServerRequest 对 Dasclaw 缺口表

Codex 的 9 个 `ServerRequest` 不是普通 notification，而是服务端主动向客户端要决策/输入/令牌，因此必须有 request tracking、timeout、fail-safe、UI 决策回传和审计。P3 已完成 command execution approval 的 server-request loop；其余 request 仍不能只补协议名。

| Codex ServerRequest | Dasclaw 当前状态 | 分类 | 为什么不能只补协议 |
|---|---|---|---|
| ~~`item/commandExecution/requestApproval`~~ | ~~P3 已接 runtime approval needed -> JSON-RPC server request -> `approval/respond` / client response -> runtime decision；并覆盖 timeout、cancel、shutdown、malformed/error response fail-safe~~ | A/D1 | 后续仍需补审计与更完整 command/fs owner，但 request loop 已有 |
| `item/fileChange/requestApproval` | 文件工具与 approval primitive 分散存在，但 app-server 无 diff/approval request | D1/D2 | 需要 file change detector、diff/patch model、approval |
| `item/permissions/requestApproval` | 协议常量与 desktop-app 转发路径已存在；未进入 implemented capability，权限模型仍未作为 app-server service producer 暴露 | D1/D2 | 需要权限模型和拒绝/允许后的执行路径 |
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
| `thread/create` | method | Legacy Dasclaw smoke-surface alias，当前 public native contract intentionally unsupported；公开名称是 `thread/start` |
| `turn/cancel` | method | Legacy Dasclaw smoke-surface alias，当前 public native contract intentionally unsupported；公开名称是 `turn/interrupt` |
| `turn/list` | method | Legacy Dasclaw turn-level list alias，当前 public native contract intentionally unsupported；公开名称是 `thread/turns/list` |
| `turn/read` | method | Dasclaw native turn-level read；当前仍保留为已支持的 turn detail 读取入口 |
| ~~`approval/respond`~~ | method | P3 approval server-request 的客户端决策回传入口 |
| `modelProvider/selectForNextTurn` | method | desktop-app / renderer-mediated model provider selection；Codex 有 `model/list` 但没有这个同名选择入口 |
| `notifications/initialized` | event | Dasclaw 服务端通知；Codex 的 `initialized` 是 ClientNotification |
| `lifecycle/changed`、`health/changed`、`capabilities/changed` | event | Native control-plane state |
| `log/entry` | event | 已声明但 Phase 1 未接线的 logs event |
| `thread/created` | event | Legacy Dasclaw smoke-surface alias，当前 public native contract intentionally unsupported；公开名称是 `thread/started` |
| `turn/delta` | event | Legacy/smoke delta，当前 public native contract intentionally unsupported；Codex 核心文本流是 `item/agentMessage/delta` |
| `turn/failed`、`turn/cancelled` | event | Legacy terminal variants，当前 public native contract intentionally unsupported；失败/中断都聚合在 `turn/completed` 的 `turn.status` 与 `turn.error` |
| ~~`serverRequest/resolved`~~、~~`item/commandExecution/outputDelta`~~、~~`item/commandExecution/terminalInteraction`~~ | event | P3 新增的 approval/tool bridge 事件；名字与 Codex 对齐但仍受 runtime feature gate 控制 |

## 9. 能力补齐优先级

如果目标是“补齐 dasclaw-app-server 能力”，建议按能力依赖顺序补，而不是按 Codex method 字母顺序补。

| 优先级 | 目标 | 包含 | 原因 |
|---|---|---|---|
| P0 | 诚实的协议边界 | `codex_app_server_v2_chat_session_subset` 继续标成 chat-session subset；P3-P6 opt-out 保持显式 | 已由 capability/profile tests 约束 |
| P1 | Chat-session compatibility view | ~~`thread/start`~~、~~`turn/start`~~、~~`turn/interrupt`~~、~~`thread/started`~~、~~`turn/started`~~、~~`turn/completed`~~、~~`item/started`~~、~~`item/agentMessage/delta`~~、~~`item/reasoning/summaryTextDelta`~~、~~`item/completed`~~ 使用单一 v2-shaped native view / producer | desktop-app 直接消费 nested thread/turn/item shape，不再通过 response-level fallback normalize |
| P2 | Model catalog/service | ~~`model/list`~~ 由 app-server 返回 Codex `ModelListResponse`；~~`modelProvider/list`~~ 仅作为 desktop-app renderer alias | app-server 成为模型列表 owner，主进程不再直接暴露 provider secrets |
| P3 | Approval + tool + sandbox 三件套 | ~~ServerRequest request tracking、approval decision、tool lifecycle notification、sandbox capability gate、fail-safe timeout~~；standalone `fs/*` / `command/exec*`、dynamic `item/tool/call`、file-change approval 仍留给 P5 或后续专项 | Codex 大量协议依赖这组能力；本轮先补安全控制闭环，避免后续 command/fs 绕过 approval/sandbox |
| P4 | MCP / skills / logs / jobs | MCP registry/OAuth/resource/tool call；skills registry；logs source；job host | MCP、logs、jobs、skills 已有不同程度底座或 legacy 参考；重点是上收 app-server owner 与协议接线 |
| P5 | Filesystem / command exec | `fs/*`、`command/exec*`、`fs/changed`、command output | 必须在 P3 安全边界之后做，否则风险大 |
| P6 | Product-specific Codex domains | account、plugin、marketplace、app list、feedback、external agent import、Windows sandbox、realtime audio | 只有当 Dasclaw 明确要兼容未改 Codex client 或复刻相关产品能力时再做 |

## 10. 决策建议

1. 不建议把 Dasclaw native protocol 改名伪装成完整 Codex app-server。当前证据显示它只覆盖 chat-session subset，硬伪装会让客户端在 tools/MCP/approval/fs/command/account 等域踩到 app-server 接线缺口或产品语义缺口。
2. 可以保留 Codex-compatible profile 描述，但它只是 capability/profile metadata；wire shape 仍是 Dasclaw native v2-shaped contract，未实现域要明确 unsupported，而不是静默 no-op。
3. 最短可交付路线是先补 P1：让同名 thread/turn/item streaming 返回 nested Codex-style objects。这样既能服务 `desktop-app`/AI SDK transport，又不会承诺完整 Codex 产品控制面。
4. P3 已先补 approval、tool lifecycle notification、sandbox gate 这组安全边界；下一步做 `fs/*`、`command/exec*`、`item/tool/call` 时仍不能绕过这条 request tracking / fail-safe / capability gating 路径。
