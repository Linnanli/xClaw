# Dasclaw app-server 与 Codex app-server / AI SDK Transport 兼容计划

> 日期：2026-06-17
> 状态：待执行计划
> 目标：核对当前 `dasclaw-app-server` 协议是否与 `codex-cli-main` app-server 协议一致；若不一致，明确调整路径；把 provider 与现有 desktop-app 中的“事件映射经验”剥离为可执行的 `DasclawChatTransport` 计划。

## 0. 过程透明记录

本文件属于新增架构/协议对账文档，按仓库规则先完成 4 问与三层核验。

| 启动问题 | 结论 | 处理 |
|---|---|---|
| 是否新增模块 / crate / 文件？ | 是，新增计划文档 | 已先用 code-review-graph 语义搜索与 `rg` 查已有等价文档 |
| 结论是否包含“不一致 / 缺失 / 不能直接复用”等否定判断？ | 是 | 保留 Level 1 与 Level 3 证据，并标明 Level 2 限制 |
| 是否跨项目对账？ | 是，涉及 `crates/dasclaw_*`、`codex-cli-main`、`desktop-app`、第三方 npm provider | 用语义搜索、源码精确定位、npm tarball 代码核对交叉验证 |
| 是否写架构对账类文档？ | 是 | 本文区分“证据”和“推论”，不把事件名相似误判成协议完整一致 |

核验记录：

| 层级 | 工具 / 证据 | 结果 |
|---|---|---|
| Level 1 语义层 | `mcp__code_review_graph.semantic_search_nodes_tool` 查询 `app server JSON RPC thread start turn start notifications lifecycle protocol`、`streaming assistant message delta reasoning delta item completed turn completed app server notification` | Dasclaw 侧命中 `crates/dasclaw_protocol/src/protocol.rs` 的 agent message / reasoning delta 类型；Codex 侧 CRG 退化为 keyword fallback，未形成足够证据 |
| Level 1 跨 repo | `mcp__code_review_graph.cross_repo_search_tool` 查询 `app server protocol thread start turn start notification`、`AgentMessageDeltaNotification TurnStartParams ThreadStartResponse ServerNotification` | 命中 `crates/dasclaw_app_server_protocol`、`crates/dasclaw_app_server`、`desktop-client` 的 app-server / Vercel adapter 相关实现 |
| Level 2 符号层 | 本轮没有暴露可调用 `execute_lsp` / `lsp.*` 工具；`http://127.0.0.1:9527/mcp` 连接被拒绝 | 已记录限制；本文不伪装成 LSP 结论，用符号定义源码行号与 Level 3 精确检索补强 |
| Level 3 字面层 | `rg` 精确搜索 `ThreadStartParams`、`TurnStartParams`、`ServerNotification`、`item/agentMessage/delta`、`turn/completed`、`model/list`、`initialize` 等 | 定位到 Dasclaw 与 Codex 的请求、响应、notification 类型定义以及 desktop-app 当前事件聚合逻辑 |
| 第三方包核对 | `npm view ai-sdk-provider-codex-app-server version description dependencies --json` 与 `npm pack ai-sdk-provider-codex-app-server@latest` | 最新版为 `1.1.7`；dist 代码确认 provider 发送省略 `jsonrpc` 的 JSONL，请求 `thread/start` / `turn/start`，并期望 `threadResult.thread.id` / `turnResult.turn.id` |

已检查 `dasclaw codex app-server AI SDK compatibility plan` 是否已有，结论：已有 `dasclaw-app-server-protocol-v0.md`、`dasclaw-app-server-ownership-matrix.md`、`structured-reasoning-streaming-final-plan.md` 等相关文档，但未发现专门面向“Dasclaw 与 Codex app-server 协议逐项一致性 + AI SDK provider/Transport 事件映射剥离”的计划文档；本文补齐该决策与执行切片。

## 1. 结论

当前 `dasclaw-app-server` 和完整 `codex-cli-main` app-server 协议不一致。

更准确地说：Dasclaw 当前暴露一套 native app-server protocol；已实现的 chat-session surface 被塑造成 truthful Codex app-server v2-shaped subset，而不是完整 Codex app-server v2 的协议同构实现。`codex_app_server_v2_chat_session_subset` profile 只是这套 native surface 的描述性 metadata，不是第二套 wire layer，也不提供历史 smoke 名称。这个结论有直接源码证据：`CompatibilityProfile::codex_app_server_v2_chat_session_subset()` 的 `scope` 是 `ChatSessionSubset`，只声明 `initialize`、`thread/start`、`thread/read`、`thread/list`、`thread/turns/list`、`turn/start`、`turn/interrupt`、`turn/read`、`model/list`，并显式 opt out 完整 tool / approval / sandbox / MCP 等能力。

For AI SDK integration, consume the Dasclaw app-server native protocol as a Codex app-server v2-shaped chat-session subset. Do not rely on historical smoke names or response-level `threadId` / `turnId`; read `thread.id` and `turn.id`.

但这不等于不能接 AI SDK。当前 Dasclaw 已经具备可剥离的事件映射基础：

1. `item/agentMessage/delta`、`item/reasoning/summaryTextDelta`、`item/reasoning/summaryPartAdded`、`item/reasoning/textDelta` 这些 delta 事件名和字段与 Codex v2 的核心文本 / reasoning 流很接近。
2. `desktop-app/src/renderer/src/lib/appServerTurnTracker.ts` 已经把这些 Dasclaw notifications 聚合成 text / reasoning part。
3. `ai-sdk-provider-codex-app-server@1.1.7` 的 `NotificationRouter` 和 `StreamEmitter` 可作为 UIMessageChunk 映射经验来源，但不能直接成为 Dasclaw 后端协议的事实来源。

推荐方向：

1. 短期先做 `DasclawChatTransport`：消费当前 native v2-shaped Dasclaw app-server notification，转换为 AI SDK / assistant-ui 需要的 UI stream chunk。
2. 中期再补缺失能力：只在明确要让未修改的 `ai-sdk-provider-codex-app-server` 或完整 Codex-compatible client 直接连 Dasclaw 时，才补齐 wire envelope、initialize shape、tools / approval / sandbox / MCP 等尚未实现的能力。
3. 不建议为了接 assistant-ui 把 Dasclaw 后端协议直接改成 AI SDK wire protocol。AI SDK 属于 UI transport adapter 边界，app-server 仍应保留自有 typed protocol。

## 2. 协议差异矩阵

| 面向 | Codex app-server / provider 期望 | Dasclaw 当前实现 | 兼容判断 | 需要调整 |
|---|---|---|---|---|
| wire envelope | Codex README 明确 JSON-RPC 2.0 但 wire 上省略 `"jsonrpc":"2.0"`；provider `sendRequest()` 发送 `{ id, method, params }` | Dasclaw `JsonRpcRequest` / `JsonRpcResponse` / `ServerNotification` 都包含 `jsonrpc`，`ServerNotification::new()` 总是写入 `"2.0"` | 不一致 | 若要未改 provider 直连，需要 stdio/client 层接受省略 `jsonrpc` 的请求，并可选择发送省略 header 的响应 / notification |
| initialize params | Codex `InitializeParams { clientInfo, capabilities }`；连接初始化后客户端还会发 `initialized` notification | Dasclaw `InitializeParams { client, protocolVersion, workspace, requestedCapabilities, modelProvider }`；`codex_app_server_v2_chat_session_subset` 只描述当前 native chat-session subset | 不一致 | AI SDK adapter 使用 Dasclaw native initialize；只有 full Codex-compatible client 才需要新增 Codex initialize 反序列化入口 |
| initialize response | Codex 返回 `userAgent`、`codexHome`、`platformFamily`、`platformOs` | Dasclaw 返回 `server`、`lifecycle`、`capabilities`、`compatibilityProfiles`、`unavailableRequestedCapabilities` | 不一致 | Codex profile 下返回 Codex shape，或在外层 bridge/provider 适配 |
| compatibility 描述 | Codex 走 capabilities 与 schema generation，不返回 Dasclaw-style profile | Dasclaw 返回 `codex_app_server_v2_chat_session_subset` compatibility profile，且 scope 是 `chat_session_subset` | 有意扩展 | 保留 Dasclaw profile，但不要把它当作完整 Codex v2 等价证明，也不要把它当作第二套 wire layer |
| `thread/start` params | Codex `ThreadStartParams` 支持 model、cwd、approval、sandbox、instructions、dynamic tools、ephemeral 等 | Dasclaw `thread/start` 当前 truthful 子集只支持 optional `cwd` | 部分兼容 | AI SDK adapter 只发送支持字段；完整 Codex client 所需字段仍按缺口处理 |
| `thread/start` response | Codex `ThreadStartResponse { thread: Thread, model, modelProvider, cwd, ... }`；provider 直接读取 `threadResult.thread.id` | Dasclaw 返回 nested `thread`、`model`、`modelProvider`、`cwd` | 子集兼容 | 读取 `thread.id`，不要使用 response-level `threadId` |
| `turn/start` params | Codex `TurnStartParams { threadId, input: Vec<UserInput>, cwd, approvalPolicy, sandboxPolicy, model, effort, outputSchema, ... }` | Dasclaw `TurnStartParams { threadId, input, cwd, model, summary }`，当前支持 text-only `input` 并拼接成 prompt；runtime bridge 仍只接收 prompt/modelProvider | 部分兼容 | 短期维持 text-only；明确 reject/ignore 非 text input、attachments、tool directives、schema 等 |
| `turn/start` response | Codex `TurnStartResponse { turn: Turn }`；provider 直接读取 `turnResult.turn.id` | Dasclaw 返回 nested `turn`，状态为 `inProgress` | 子集兼容 | 读取 `turn.id`，不要使用 response-level `turnId` |
| `model/list` | Codex protocol 有 `model/list`，返回 `ModelListResponse { data, nextCursor }` | Dasclaw app-server 暴露 `modelProvider/selectForNextTurn`；desktop-app 还通过 renderer bridge 调 `modelProvider/list` | 不一致 | 若接 provider/direct Codex client，补 `model/list` 到 app-server protocol；内部可复用现有 model provider config |
| text delta notification | Codex `item/agentMessage/delta { threadId, turnId, itemId, delta }` | Dasclaw 同名事件字段一致 | 可复用 | `DasclawChatTransport` 可直接映射为 AI SDK text chunk；不要依赖 legacy `turn/delta` |
| reasoning delta notification | Codex `item/reasoning/summaryTextDelta`、`summaryPartAdded`、`textDelta` | Dasclaw 同名事件字段基本一致 | 可复用 | mapper 保留 summary 与 raw reasoning 的分流；`summaryPartAdded` 作为 reasoning part boundary |
| `item/started` payload | Codex `ItemStartedNotification { item: ThreadItem, threadId, turnId }` | Dasclaw `ItemStartedEvent { item, threadId, turnId }`，当前 truthful 子集以 agent message 为主 | 子集兼容 | mapper-first 可暂不依赖该事件；完整 item taxonomy 仍是后续能力 |
| `item/completed` payload | Codex `ItemCompletedNotification { item: ThreadItem, threadId, turnId }`；provider 会访问 `p.item.type` | Dasclaw `ItemCompletedEvent { item, threadId, turnId }` | 子集兼容 | mapper-first 可把它当作 item end 信号；完整 item taxonomy 仍是后续能力 |
| `turn/completed` payload | Codex `TurnCompletedNotification { threadId, turn: Turn }`；provider 检查 `p.turn.id` 并用 `p.turn.status/error` finish | Dasclaw 同名事件使用 nested `turn`；失败和中断也通过 `turn.status` / `turn.error` 表达 | 子集兼容 | mapper 只消费 nested `turn`，不要监听 legacy `turn/failed` / `turn/cancelled` |
| status vocabulary | Codex `TurnStatus` 是 `completed`、`interrupted`、`failed`、`inProgress` | Dasclaw nested `CodexTurnStatus` 使用 `completed`、`interrupted`、`failed`、`inProgress` | 子集兼容 | 内部 runtime 状态可继续自有；public view 使用 Codex vocabulary |
| tools / approval / diff / MCP / sandbox | Codex v2 protocol 有丰富 item 与 approval/tool notifications | Dasclaw compatibility profile 明确 phase one opt-out | 有意缺口 | 不是 mapper-first 范围；只有做 full Codex-compatible client 时才逐项补 contract |

## 3. 对 `ai-sdk-provider-codex-app-server` 的直接影响

未修改的 `ai-sdk-provider-codex-app-server@1.1.7` 不能直接连当前 Dasclaw app-server，阻塞点不是单一事件名，而是启动握手、响应 shape 和完成事件 shape。

实测包代码中的 provider 行为：

1. 初始化发送 `{ id, method: "initialize", params: { clientInfo } }`，随后发送 `{ method: "initialized", params: {} }`，都不带 `jsonrpc`。
2. 启动线程后读取 `threadResult.thread.id`。
3. 启动 turn 后读取 `turnResult.turn.id`。
4. `item/completed` handler 会访问 `p.item.type`，用于 tool / agent message / reasoning fallback。
5. `turn/completed` handler 会访问 `p.turn.id`、`p.turn.status`、`p.turn.error`，然后发 AI SDK `finish` 并关闭 stream。

因此有两条可选路线：

| 路线 | 内容 | 成本 | 适用场景 |
|---|---|---:|---|
| A. 写 `DasclawChatTransport` | 直接消费当前 Dasclaw app-server bridge，把 notification 映射成 AI SDK UIMessageChunk | 低到中 | 目标是 desktop-app / assistant-ui 更顺地接收 text、reasoning、finish |
| B. 做 Codex-compatible app-server shim | 让 Dasclaw stdio / protocol 在 Codex profile 下模拟 Codex app-server v2 的 wire 与 shape | 中到高 | 目标是未改 provider、open-cowork 或其他 Codex-compatible client 直接连接 |
| C. fork/封装 provider | 复用 provider 的 AI SDK surface，但底层 client 改成 Dasclaw wire/shape | 中 | 目标是沿用 provider API，但不想大改 app-server |

推荐先走 A，再按真实 client 需求决定是否走 B 或 C。

## 4. 事件映射经验剥离计划

### 4.1 要剥离什么

剥离目标不是复制第三方 provider 的进程管理或 Codex schema，而是抽出三类经验：

1. **事件过滤**：按 `threadId`、`turnId` 过滤当前 turn 的 notification。
2. **part 生命周期**：第一次 text/reasoning delta 前发 start，后续发 delta，turn 结束时补 end/finish。
3. **reasoning 分流**：`summaryTextDelta` 与 `textDelta` 都进入 reasoning part，但 metadata 要保留 summary/raw 的区别，`summaryPartAdded` 形成新的 reasoning section。

现有 desktop-app 的 `createAppServerTurnTracker()` 已经完成一版事件聚合：

```text
item/agentMessage/delta              -> { type: "text", text: delta }
item/reasoning/summaryTextDelta      -> { type: "reasoning", text: delta }
item/reasoning/textDelta             -> { type: "reasoning", text: delta }
item/reasoning/summaryPartAdded      -> 下一段 reasoning 强制新 part
turn/completed                       -> resolve / reject pending turn from nested turn.status/error
```

新的 `DasclawChatTransport` 应该把这个 tracker 的“聚合成 assistant-ui ThreadMessage”的职责下沉为“输出 UI stream chunk”。这样 UI 可以改走 `@assistant-ui/react-ai-sdk` 或 AI SDK `ChatTransport`，而 app-server protocol 不必被迫改成 AI SDK wire。

### 4.2 推荐模块边界

```text
desktop-app renderer
  hooks/useDasclawAssistantRuntime.ts
    现状：useExternalStoreRuntime + 手写 messages state
    目标：可切到 react-ai-sdk runtime

  lib/dasclawAppServerEvents.ts
    纯类型守卫和 notification normalizer

  lib/dasclawUiMessageChunkMapper.ts
    Dasclaw notification -> UIMessageChunk
    不访问 React，不访问 window.desktopAppServer

  lib/DasclawChatTransport.ts
    实现 AI SDK ChatTransport
    负责 thread/start、turn/start、interrupt、subscribe notification、abort cleanup
```

原则：

1. `dasclawUiMessageChunkMapper` 是纯函数/小状态机，单测优先。
2. `DasclawChatTransport` 是副作用层，集成测试覆盖 bridge 请求顺序、notification 顺序、abort、late notification。
3. 不从 `ai-sdk-provider-codex-app-server` import 内部 dist。第三方包只作为参考，不作为运行时依赖。
4. 当前 `desktop-app/package.json` 只看到 `@assistant-ui/react`，没有 `ai` / `@ai-sdk/*` / `@assistant-ui/react-ai-sdk`。真正迁移 runtime 时需要单独引入并锁版本，避免类型名随 AI SDK 版本漂移。

### 4.3 UIMessageChunk 映射草案

具体字段以引入的 AI SDK 版本类型为准；这里固定语义，不把字段名写成后端协议承诺。

| Dasclaw notification | Mapper 行为 |
|---|---|
| `turn/started` | 记录 active turn，可选择发 stream start / response metadata |
| `item/agentMessage/delta` | 若 text part 未开始，先发 text start；随后发 text delta |
| `item/reasoning/summaryTextDelta` | 若 reasoning part 未开始，先发 reasoning start；发 reasoning delta，metadata 标记 `summary=true` |
| `item/reasoning/textDelta` | 若 reasoning part 未开始，先发 reasoning start；发 reasoning delta，metadata 标记 `summary=false` |
| `item/reasoning/summaryPartAdded` | 结束当前 reasoning section 或设置下一段 reasoning boundary |
| `item/completed` | 若当前 item 有打开的 text/reasoning part，补 end；不要依赖 Dasclaw 当前 payload 的 `item` |
| `turn/completed` | 补齐所有 open part end，发 finish，关闭 stream |
| `turn/completed` with `turn.status = "failed"` / `error` | 发 error chunk 或 reject stream；补 cleanup |
| `turn/completed` with `turn.status = "interrupted"` | 映射为 abort/interrupted finish，关闭 stream |

### 4.4 必补测试

Mapper 单测：

1. text delta 之前自动补 text start。
2. 多个 text delta 合并在同一 text part。
3. summary reasoning 与 raw reasoning 都不会落进 assistant text。
4. `summaryPartAdded` 会形成新的 reasoning boundary。
5. `turn/completed` 在没有显式 `item/completed` 时也会补齐 end/finish。
6. failed `turn/completed` 会关闭 open part 并输出 error。
7. notification 早于 `turn/start` response 到达时，transport 能缓存或重放，不能丢 delta。

Transport 集成测试：

1. `thread/start` 只在首轮或明确新会话时调用。
2. `turn/start` 发送 text-only `input`，保持当前 Dasclaw 支持范围。
3. abort 调 `turn/interrupt`，失败时本地 stream 也要收敛。
4. notification listener 在发送 `turn/start` 前安装，避免 startTurn response 前的 early delta 被漏掉。
5. app-server bridge unavailable 时给出可恢复错误。

## 5. Codex-compatible shim 计划

如果目标是让未修改的 `ai-sdk-provider-codex-app-server` 直接连 Dasclaw，则需要补的是完整 Codex client 仍要求、但 Dasclaw native subset 尚未支持的能力和 wire envelope 差异。不要新增 legacy/native 双栈输出；已有 chat-session response/notification shape 应继续作为单一 native v2-shaped surface。

### Phase B0：合同夹具

1. 在 `crates/dasclaw_app_server_protocol` 增加 Codex v2 minimal fixture：
   - initialize request/response
   - thread/start request/response
   - turn/start request/response
   - item delta notifications
   - item completed notification
   - turn completed notification
2. fixture 必须覆盖 Dasclaw native v2-shaped shape，防止 response-level `threadId` / `turnId` 或 legacy terminal events 回流。
3. 增加 provider smoke fixture：用 `ai-sdk-provider-codex-app-server@1.1.7` 期待的最小 shape 作为测试输入/输出参考，但不要把 npm dist vendoring 到源码。

### Phase B1：wire envelope 兼容

1. stdio server 接受省略 `jsonrpc` 的 JSON-RPC request / notification。
2. response 是否省略 `jsonrpc` 按 connection profile 决定，或先保持带 header 并验证 provider 是否忽略。
3. 增加 `initialized` notification handler，至少 no-op。
4. 对 unknown notification 不回 error，避免客户端初始化 ack 失败。

### Phase B2：request/response shape 兼容

1. `initialize` 支持 Codex `clientInfo/capabilities`。
2. `thread/start` 支持 Codex params 子集，返回 `thread` object。
3. `turn/start` 返回 `turn` object，并将 `pending` 映射成 `inProgress`。
4. `model/list` 返回最小 `ModelListResponse`，复用现有 model provider config。
5. 明确非 text input、attachments、tool hints、approval/sandbox override 的处理策略：unsupported warning、ignored metadata，或 JSON-RPC invalid params。不要静默假装支持。

### Phase B3：notification shape 兼容

1. `item/started` / `item/completed` 继续带完整 `item: ThreadItem`。
2. 成功、失败和中断都发 `turn/completed { threadId, turn }`，其中 `turn.status` 区分 `completed` / `failed` / `interrupted`。
3. 历史 smoke 事件名 `turn/failed`、`turn/delta`、`turn/cancelled` 不应作为 public native chat-session surface 回流；缺失能力通过显式 gap 和 capability opt-out 表达。
4. 将 queue overflow、runtime degraded 等 Dasclaw-specific error 映射到 Codex `error` notification 或 JSON-RPC error。

### Phase B4：端到端验证

1. 用 in-process app-server client 跑 Codex shape golden test。
2. 用 line-delimited stdio client 跑省略 `jsonrpc` 的初始化、thread、turn、stream、finish。
3. 用一个最小 JS smoke 模拟 provider 的读取路径：
   - `initialize`
   - `initialized`
   - `thread/start` 后访问 `result.thread.id`
   - `turn/start` 后访问 `result.turn.id`
   - 收到 `item/agentMessage/delta`
   - 收到 `turn/completed` 后访问 `params.turn.id/status/error`

## 6. 推荐执行顺序

1. **先做 Track A：DasclawChatTransport mapper-first。**
   - 价值最快，改动集中在 `desktop-app`。
   - 不要求 Dasclaw app-server 立刻伪装成完整 Codex。
   - 可直接解决 assistant-ui / AI SDK runtime 的接入问题。
2. **并行补一份协议 parity test matrix。**
   - 用本文第 2 节矩阵转成 fixtures。
   - 防止后续又把“事件名相同”当成“协议一致”。
3. **确认是否真的需要未修改 provider 直连。**
   - 如果只是 desktop-app 使用 AI SDK UI runtime，Track A 足够。
   - 如果要让 community provider、open-cowork、Codex Electron-like client 无改动接入，则进入 Track B。
4. **进入 Track B 时只做 Codex profile 双栈。**
   - 不删除 Dasclaw v0 shape。
   - 不把 `modelProvider/selectForNextTurn` 等 Dasclaw-native method 硬改名。
   - 用 capability/profile 控制事件输出。

## 7. 验收标准

Track A 完成标准：

1. `DasclawChatTransport` 能用当前 app-server bridge 完成一轮 text-only turn。
2. assistant 正文不包含 reasoning 内容。
3. reasoning summary/raw reasoning 能作为独立 UI part 被消费。
4. completion、failure、cancel 都能关闭 stream，不留下 pending promise。
5. 单测覆盖 early notification、completion without item completed、failure cleanup。

Track B 完成标准：

1. 省略 `jsonrpc` 的 Codex-style client 能完成 initialize/thread/turn。
2. `thread/start` 与 `turn/start` response 满足 provider 的 `thread.id` / `turn.id` 读取路径。
3. `item/completed` 和 `turn/completed` payload 满足 Codex v2 shape。
4. text/reasoning delta 与 Dasclaw legacy notification 不互相污染。
5. 不支持的 Codex v2 能力有明确 opt-out、warning 或 invalid params，不静默成功。

## 8. 风险与非目标

风险：

1. AI SDK `UIMessageChunk` 类型随版本变化，计划只固定语义，落代码时必须以锁定版本类型为准。
2. provider 的 `NotificationRouter` 订阅时机在 `turn/start` 返回后，若 app-server 在 response 前已经发 delta，未修改 provider 可能漏事件；Dasclaw 自己写 transport 时应先装 listener 再发 `turn/start`。
3. Dasclaw runtime bridge 当前是 prompt-only，不能因为接了 Codex `input` array 就宣称支持 multimodal/rich input。
4. 补 Codex shim 时容易把 Dasclaw lifecycle/capability model 混进 Codex response，需要保持 profile 边界清晰。

非目标：

1. 本计划不要求立刻实现 tools、approval、diff、MCP、sandbox 的完整 Codex v2 能力。
2. 本计划不把 app-server protocol 改成 AI SDK UI stream protocol。
3. 本计划不 vendoring 第三方 npm provider 源码。
4. 本计划不删除现有 `useExternalStoreRuntime` 路径；迁移到 `@assistant-ui/react-ai-sdk` 应作为后续可回滚切片。
