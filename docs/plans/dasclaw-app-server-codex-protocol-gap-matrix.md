# Dasclaw app-server 与 Codex app-server 协议缺口对照表

> 日期：2026-06-17
> 状态：能力补齐参考
> 目标：列出 `codex-cli-main` app-server 的协议面，和当前 `dasclaw-app-server` 做逐域对照，区分“只差协议 shape”和“底层能力未接入”。

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
| Level 3 字面层 | Codex 生成协议 union：`codex-cli-main/codex-rs/app-server-protocol/schema/typescript/{ClientRequest,ServerNotification,ServerRequest,ClientNotification}.ts`；Dasclaw 常量、能力矩阵、router：`crates/dasclaw_app_server_protocol/src/lib.rs`、`crates/dasclaw_app_server/src/lib.rs` | Codex：74 个 `ClientRequest`、61 个 `ServerNotification`、9 个 `ServerRequest`、1 个 `ClientNotification`。Dasclaw：16 个 method、19 个 event，且 router 只路由这些 method |

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
| `crates/dasclaw_app_server_protocol/src/lib.rs:17-56` | Dasclaw 当前常量定义 16 个 method、19 个 event |
| `crates/dasclaw_app_server_protocol/src/lib.rs:417-475` | Dasclaw Phase 1 只实现 `protocol`、`lifecycle`、`health`、`session`、`model_provider`；`approval`、`dlp_policy`、`tools`、`jobs`、`skills`、`mcp`、`sandbox`、`logs` 是 declared future |
| `crates/dasclaw_app_server_protocol/src/lib.rs:529-575` | `codex_app_server_v2` profile 明确是 `ChatSessionSubset`，只列 `initialize`、`thread/start`、`thread/read`、`turn/start`、`turn/interrupt` 并 opt out 多个 Codex 能力 |
| `crates/dasclaw_app_server/src/lib.rs:820-892` | Dasclaw router 只实际路由当前 16 个 method，其他 method 会落到 `method_not_found` |
| `crates/dasclaw_app_server/src/lib.rs:897-923` | 运行时健康状态明确显示 tools / sandbox / jobs / skills / mcp disabled，DLP policy unavailable fail-safe |

结论分三层：

| 层级 | 判断 |
|---|---|
| 协议同名/近似可用 | `initialize`、`thread/start`、`thread/read`、`thread/list`、`turn/start`、`turn/interrupt`、若干 item/turn streaming notification |
| 协议缺口但可通过 compatibility view 补 | Codex response shape、`model/list`、Codex-style `thread` / `turn` object、`item/completed` / `turn/completed` payload shape |
| 不能只靠补协议 | approval / tool execution / sandbox / MCP / skills / jobs / filesystem watch/write / command exec / account / plugin / marketplace 等，需要底层服务、权限、安全边界和产品状态先落地 |

## 2. Codex app-server 协议清单

### 2.1 ClientRequest：74 个

来源：`codex-cli-main/codex-rs/app-server-protocol/schema/typescript/ClientRequest.ts:79`。

| 域 | Codex method |
|---|---|
| 初始化 | `initialize` |
| Thread lifecycle / history | `thread/start`、`thread/resume`、`thread/fork`、`thread/archive`、`thread/unsubscribe`、`thread/name/set`、`thread/metadata/update`、`thread/unarchive`、`thread/compact/start`、`thread/shellCommand`、`thread/approveGuardianDeniedAction`、`thread/rollback`、`thread/list`、`thread/loaded/list`、`thread/read`、`thread/turns/list`、`thread/inject_items` |
| Turn lifecycle | `turn/start`、`turn/steer`、`turn/interrupt` |
| Skills / plugins / marketplace / apps | `skills/list`、`skills/config/write`、`plugin/list`、`plugin/read`、`plugin/install`、`plugin/uninstall`、`marketplace/add`、`marketplace/remove`、`marketplace/upgrade`、`app/list` |
| Device key | `device/key/create`、`device/key/public`、`device/key/sign` |
| Filesystem | `fs/readFile`、`fs/writeFile`、`fs/createDirectory`、`fs/getMetadata`、`fs/readDirectory`、`fs/remove`、`fs/copy`、`fs/watch`、`fs/unwatch` |
| Review / model / experiment | `review/start`、`model/list`、`experimentalFeature/list`、`experimentalFeature/enablement/set` |
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
| Thread | `thread/started`、`thread/status/changed`、`thread/archived`、`thread/unarchived`、`thread/closed`、`thread/name/updated`、`thread/goal/updated`、`thread/goal/cleared`、`thread/tokenUsage/updated`、`thread/compacted` |
| Realtime | `thread/realtime/started`、`thread/realtime/itemAdded`、`thread/realtime/transcript/delta`、`thread/realtime/transcript/done`、`thread/realtime/outputAudio/delta`、`thread/realtime/sdp`、`thread/realtime/error`、`thread/realtime/closed` |
| Turn | `turn/started`、`turn/completed`、`turn/diff/updated`、`turn/plan/updated` |
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

### 3.1 Method：16 个

来源：`crates/dasclaw_app_server_protocol/src/lib.rs:17-34` 与 `phase_one_methods()`。

| 能力域 | Dasclaw method |
|---|---|
| Protocol | `initialize`、`protocol/schema` |
| Health | `health/check`、`capabilities/list` |
| Lifecycle | `lifecycle/status`、`shutdown` |
| Session / thread | `thread/create`、`thread/start`、`thread/list`、`thread/read` |
| Session / turn | `turn/start`、`turn/cancel`、`turn/interrupt`、`turn/list`、`turn/read` |
| Model provider | `modelProvider/selectForNextTurn` |

### 3.2 Event：19 个

来源：`crates/dasclaw_app_server_protocol/src/lib.rs:36-56` 与 `phase_one_events()`。

| 能力域 | Dasclaw event |
|---|---|
| Protocol | `notifications/initialized` |
| Lifecycle / health / logs | `lifecycle/changed`、`health/changed`、`capabilities/changed`、`log/entry` |
| Thread / turn | `thread/created`、`thread/started`、`turn/started`、`turn/delta`、`turn/completed`、`turn/failed`、`turn/cancelled` |
| Item streaming | `item/started`、`item/agentMessage/delta`、`item/reasoning/summaryTextDelta`、`item/reasoning/summaryPartAdded`、`item/reasoning/textDelta`、`item/completed` |
| Error | `error` |

### 3.3 当前 desktop-app 消费面

`desktop-app` 只消费 Dasclaw 当前子集：

| 证据 | 说明 |
|---|---|
| `desktop-app/src/main/appServerManager.ts:150-161` | 初始化发送 `requestedCapabilities: ["protocol", "lifecycle", "health", "session", "codex_app_server_v2"]` |
| `desktop-app/src/main/appServerManager.ts:187-196` | `modelProvider/list` 在 desktop-app manager 本地处理，`modelProvider/selectForNextTurn` 也由 manager 侧特殊处理 |
| `desktop-app/src/renderer/src/lib/appServerTurnTracker.ts:65-101` | renderer 只聚合 content / reasoning delta，并等待 `turn/completed` 或 `turn/failed` |

这说明当前消费者不要求 Codex 全量协议，但如果目标改成“让 Dasclaw app-server 具备接近 Codex app-server 的完整能力”，需要补的是服务能力，不只是 provider/transport 适配。

## 4. 缺口分类标准

| 标记 | 含义 |
|---|---|
| A：已支持/近似支持 | Dasclaw 有同名或等价 method/event，但 payload shape 可能不同 |
| B：协议 shape 缺口 | 底层能力方向存在，主要需要补 Codex-compatible request/response/notification view |
| C：app-server 暴露缺口 | 能力可能存在于 desktop-app/manager/runtime 侧，但 app-server 没有一等接口 |
| D：底层能力缺口 | Dasclaw capability matrix / service health 明确 declared future、disabled 或 unavailable，不能只靠补 method 名称 |
| E：Codex 产品专属或暂不建议补 | 和 OpenAI/Codex 账户、插件市场、Windows 特定沙箱、Codex cloud/product 体验绑定；除非产品目标明确要求，否则不纳入 Dasclaw native core |

## 5. Codex ClientRequest 对 Dasclaw 缺口表

| Codex 域 | Codex method | Dasclaw 当前状态 | 分类 | 补齐含义 |
|---|---|---|---|---|
| 初始化 | `initialize` | 有同名，但 params/response 与 Codex 不同；Dasclaw 要求 `protocolVersion`、`requestedCapabilities`、可带 `modelProvider` | A/B | 若要 Codex client 直连，需要 Codex initialize view；若只服务 desktop-app，保持 native shape 更清楚 |
| Thread 核心 | `thread/start`、`thread/read`、`thread/list` | 有同名或同义 method，但 response shape 不是 Codex `Thread` object | A/B | 补最小 `Thread` view、状态字段、turn/item 容器 |
| Thread 扩展 | `thread/resume`、`thread/fork`、`thread/archive`、`thread/unarchive`、`thread/unsubscribe`、`thread/name/set`、`thread/metadata/update`、`thread/compact/start`、`thread/shellCommand`、`thread/approveGuardianDeniedAction`、`thread/rollback`、`thread/loaded/list`、`thread/turns/list`、`thread/inject_items` | 当前 router 无这些 method；Dasclaw 只有 `thread/create/start/list/read` | C/D | 需要线程持久化、归档/恢复、压缩、shell command、guardian denied action、inject item 等真实 session 管理能力 |
| Turn 核心 | `turn/start`、`turn/interrupt` | 有同名；`turn/start` 当前主要 text prompt，`turn/interrupt` 复用 cancel params | A/B | 补 Codex text input 子集、`Turn` object response、状态映射 `pending -> inProgress`、`cancelled -> interrupted` |
| Turn steer | `turn/steer` | 无 | D | 需要运行中 turn steer 控制能力，不能只补空 handler |
| Model | `model/list` | 无；Dasclaw 有 `modelProvider/selectForNextTurn`，desktop-app 本地处理 `modelProvider/list` | C | 可先把 model catalog/selection 抬进 app-server，返回最小 `ModelListResponse` |
| Skills | `skills/list`、`skills/config/write` | capability matrix 标 `skills` declared future；service health disabled | D | 需要 app-server skills registry/service |
| Plugin / marketplace / app | `plugin/list`、`plugin/read`、`plugin/install`、`plugin/uninstall`、`marketplace/add`、`marketplace/remove`、`marketplace/upgrade`、`app/list` | 无对应 Dasclaw app-server 能力 | E/D | Codex 产品扩展/市场域；除非 Dasclaw 要做插件市场，否则不建议照搬 |
| Filesystem | `fs/readFile`、`fs/writeFile`、`fs/createDirectory`、`fs/getMetadata`、`fs/readDirectory`、`fs/remove`、`fs/copy`、`fs/watch`、`fs/unwatch` | 无 app-server method；sandbox disabled | D | 需要 workspace root、权限、审计、watch、sandbox/approval 边界一起设计 |
| Command exec | `command/exec`、`command/exec/write`、`command/exec/terminate`、`command/exec/resize` | 无；tools/sandbox disabled | D | 需要 PTY/进程生命周期、approval、sandbox、output streaming |
| MCP | `mcpServer/oauth/login`、`config/mcpServer/reload`、`mcpServerStatus/list`、`mcpServer/resource/read`、`mcpServer/tool/call` | `mcp` declared future；service health disabled | D | 需要 MCP registry、OAuth、resource read、tool call、progress/event bridge |
| Approval / guardian | `thread/approveGuardianDeniedAction`，以及 ServerRequest 里的 approval 系列 | `approval` declared future；DLP/policy unavailable fail-safe | D | 需要 fail-safe approval orchestration、policy/DLP、client decision loop |
| Sandbox | `windowsSandbox/setupStart` | `sandbox` declared future；service health disabled | D/E | Windows 特定 setup 可不照搬；但 Dasclaw 若要 command/fs/tool 能力，仍要有平台 sandbox 抽象 |
| Account/auth/rate limit | `account/login/start`、`account/login/cancel`、`account/logout`、`account/rateLimits/read`、`account/sendAddCreditsNudgeEmail`、`account/read`、`getAuthStatus` | 无 | E | Codex/OpenAI 产品账户域，不属于 Dasclaw native app-server 必需能力 |
| Config / experimental / feedback / external agent | `config/read`、`config/value/write`、`config/batchWrite`、`configRequirements/read`、`experimentalFeature/list`、`experimentalFeature/enablement/set`、`feedback/upload`、`externalAgentConfig/detect`、`externalAgentConfig/import` | 无等价 app-server surface | E/C | 需要先决定 Dasclaw 产品配置、实验开关、反馈、外部 agent import 是否由 app-server 拥有 |
| Device key | `device/key/create`、`device/key/public`、`device/key/sign` | 无 | E/C | 若 Dasclaw 需要本地设备身份，可另设安全设计；不建议直接借 Codex 名称 |
| Review / git / summary / fuzzy search | `review/start`、`gitDiffToRemote`、`getConversationSummary`、`fuzzyFileSearch` | 无 | C/D/E | `fuzzyFileSearch` 可作为 UX 辅助能力补；`review/git/summary` 依赖 repo service、model summary 或 Codex 产品逻辑 |

## 6. Codex ServerNotification 对 Dasclaw 缺口表

| Codex 域 | Codex notification | Dasclaw 当前状态 | 分类 | 补齐含义 |
|---|---|---|---|---|
| Error | `error` | 有同名 | A | 需核对 payload shape |
| Thread 核心 | `thread/started` | 有同名；另有 Dasclaw-only `thread/created` | A/B | 若走 Codex profile，需要 Codex `ThreadStartedNotification` shape |
| Thread 状态/历史 | `thread/status/changed`、`thread/archived`、`thread/unarchived`、`thread/closed`、`thread/name/updated`、`thread/goal/updated`、`thread/goal/cleared`、`thread/tokenUsage/updated`、`thread/compacted` | 无 | C/D | 需要 thread lifecycle、goal、usage、compaction 状态 |
| Turn 核心 | `turn/started`、`turn/completed` | 有同名；Dasclaw 另有 `turn/failed`、`turn/cancelled` | A/B | Codex 把成功/失败/中断聚合到 `turn/completed { turn.status }`；Dasclaw 当前 terminal event 更分散 |
| Turn plan/diff | `turn/diff/updated`、`turn/plan/updated` | 无 | D | 需要 diff/plan producer 与流式更新 |
| Item text/reasoning | `item/started`、`item/agentMessage/delta`、`item/reasoning/summaryTextDelta`、`item/reasoning/summaryPartAdded`、`item/reasoning/textDelta`、`item/completed` | 有同名或近似事件；payload shape 不完全一致 | A/B | 这是最适合先补 Codex-compatible view 的核心 streaming 面 |
| Item plan/raw/tool/file/command | `rawResponseItem/completed`、`item/plan/delta`、`item/commandExecution/outputDelta`、`item/commandExecution/terminalInteraction`、`item/fileChange/outputDelta`、`item/fileChange/patchUpdated`、`command/exec/outputDelta` | 无 | D | 需要 raw response、plan、command、file change 能力和安全边界 |
| Approval review | `item/autoApprovalReview/started`、`item/autoApprovalReview/completed`、`serverRequest/resolved` | 无 | D | 依赖 approval server-request loop |
| MCP | `item/mcpToolCall/progress`、`mcpServer/oauthLogin/completed`、`mcpServer/startupStatus/updated` | 无；mcp disabled | D | 需要 MCP registry / progress event |
| Account/app/skills | `account/updated`、`account/rateLimits/updated`、`account/login/completed`、`app/list/updated`、`skills/changed` | 无 | E/D | Product-specific；skills 需要 Dasclaw registry |
| External/fs/model/fuzzy/hooks | `externalAgentConfig/import/completed`、`fs/changed`、`model/rerouted`、`model/verification`、`fuzzyFileSearch/sessionUpdated`、`fuzzyFileSearch/sessionCompleted`、`hook/started`、`hook/completed` | 无 | C/D/E | 需根据 Dasclaw 产品目标拆分：model/fuzzy 可能有价值，external agent/hook 需单独设计 |
| Realtime / Windows | `thread/realtime/*`、`windows/worldWritableWarning`、`windowsSandbox/setupCompleted` | 无 | E | Codex 特定 realtime/audio/Windows sandbox surface，不建议作为 Dasclaw 能力补齐第一阶段 |
| Warning | `warning`、`guardianWarning`、`deprecationNotice`、`configWarning` | Dasclaw 有 health/lifecycle/error，但无这些具体 notification | B/C | 可补通用 warning channel；guardian/config warning 取决于 policy/config 能力 |

## 7. Codex ServerRequest 对 Dasclaw 缺口表

Codex 的 9 个 `ServerRequest` 在 Dasclaw 当前 app-server 中都没有同构实现。它们不是普通 notification，而是服务端主动向客户端要决策/输入/令牌，因此必须有 request tracking、timeout、fail-safe、UI 决策回传和审计。

| Codex ServerRequest | Dasclaw 当前状态 | 分类 | 为什么不能只补协议 |
|---|---|---|---|
| `item/commandExecution/requestApproval` | 无 | D | 需要 command execution、approval policy、client decision loop |
| `item/fileChange/requestApproval` | 无 | D | 需要 file change detector、diff/patch model、approval |
| `item/permissions/requestApproval` | 无 | D | 需要权限模型和拒绝/允许后的执行路径 |
| `item/tool/requestUserInput` | 无 | D | 需要 tool/user-input suspension 和 resume |
| `item/tool/call` | 无 | D | 需要 dynamic tool registry、tool call execution、result streaming |
| `mcpServer/elicitation/request` | 无 | D | 需要 MCP elicitation support 和 client UI contract |
| `account/chatgptAuthTokens/refresh` | 无 | E | Codex/OpenAI account token 域 |
| `applyPatchApproval` | 无 | D/E | Codex legacy approval；若 Dasclaw 做 patch approval，应基于自有 file change/approval 设计 |
| `execCommandApproval` | 无 | D/E | Codex legacy exec approval；应和 command execution/sandbox 一起设计 |

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
| `turn/failed`、`turn/cancelled` | event | Native terminal variants；Codex 聚合在 `turn/completed` 的 `turn.status` |

## 9. 能力补齐优先级

如果目标是“补齐 dasclaw-app-server 能力”，建议按能力依赖顺序补，而不是按 Codex method 字母顺序补。

| 优先级 | 目标 | 包含 | 原因 |
|---|---|---|---|
| P0 | 诚实的协议边界 | 继续把 `codex_app_server_v2` 标成 subset；文档和 schema 不宣称 full Codex app-server | 防止客户端误以为 74/61/9/1 全部可用 |
| P1 | Chat-session compatibility view | Codex-style `initialize`、`thread/start/read/list`、`turn/start/interrupt` response shape；`item/*`、`turn/completed` payload shape | 这是当前 desktop-app/assistant-ui 最直接受益的最小闭环 |
| P2 | Model catalog/service | `model/list`，统一 `modelProvider/list` 与 `modelProvider/selectForNextTurn` 的 owner | 当前 model 能力散在 desktop-app manager，适合上收为 app-server 控制面 |
| P3 | Approval + tool + sandbox 三件套 | ServerRequest request tracking、approval decision、tool registry、sandbox adapter、fail-safe timeout | Codex 大量协议依赖这组能力，不能分开虚补 |
| P4 | MCP / skills / logs / jobs | MCP registry/OAuth/resource/tool call；skills registry；logs source；job host | Dasclaw capability matrix 已预留这些域，适合按产品优先级补 |
| P5 | Filesystem / command exec | `fs/*`、`command/exec*`、`fs/changed`、command output | 必须在 P3 安全边界之后做，否则风险大 |
| P6 | Product-specific Codex domains | account、plugin、marketplace、app list、feedback、external agent import、Windows sandbox、realtime audio | 只有当 Dasclaw 明确要兼容未改 Codex client 或复刻相关产品能力时再做 |

## 10. 决策建议

1. 不建议把 Dasclaw native protocol 改名伪装成完整 Codex app-server。当前证据显示它只覆盖 chat-session subset，硬伪装会让客户端在 tools/MCP/approval/fs/command/account 等域踩到 runtime 缺口。
2. 可以新增 Codex-compatible profile/view，但要按 capability gating 输出，未实现域要明确 unsupported，而不是静默 no-op。
3. 最短可交付路线是先补 P1：让同名 thread/turn/item streaming 在 Codex profile 下返回 Codex shape。这样既能服务 `desktop-app`/AI SDK transport，又不会承诺完整 Codex 产品控制面。
4. 真正的能力补齐应从 P3 开始进入重活：approval、tool execution、sandbox 是一组安全边界，任何 `fs/*`、`command/exec*`、`item/tool/call` 都不应该绕过它们单独开放。
