# Desktop App Reasoning Summary Streaming Modification Plan

> 日期：2026-06-16
> 状态：改造执行文档
> 目标：让 `desktop-app` 发起对话时稳定显示结构化 reasoning summary，并把用户可见正文与 reasoning summary 从 provider 到 UI 全链路分离。

## 0. 过程透明记录

本文件是新增改造文档，且包含 `codex-cli-main` / `dasclaw-app-server` / `desktop-app` 的链路对账。按仓库规则先完成 4 问与核验。

| 启动问题 | 结论 | 处理 |
|---|---|---|
| 是否新增模块 / crate / 文件？ | 是，新增方案文档 | 先查 `docs/plans` 是否已有等价文档 |
| 是否包含否定性结论？ | 是，会判断剩余缺口 | 用源码行号和搜索记录标注证据边界 |
| 是否跨项目对账？ | 是，参考 `codex-cli-main` 并对照当前 crates / desktop-app | 使用 code-review-graph repo 列表、`rg`、源码行号核验 |
| 是否写架构对账类文档？ | 是 | 区分已做事实、推论、待验证项 |

核验记录：

| 层级 | 工具 / 证据 | 结果 |
|---|---|---|
| Level 1 语义层 | 本轮 `code-review-graph` CLI 仅暴露 `repos/build/update/detect-changes/serve` 等命令，未暴露 direct semantic-search 子命令；已执行 `code-review-graph repos` 确认可用 repo 包含 `codex-cli`、`xclaw-core`、`desktop-client-ui` 等 | 不能在本轮用 CLI 直接跑 `semantic_search_nodes_tool`；参考既有 `structured-reasoning-streaming-final-plan.md` 的语义核验记录，并用源码证据复核 |
| Level 2 符号层 | 本轮会话经 tool discovery 未暴露可直接调用的 `execute_lsp` | 记录限制；以精确源码行号和现有测试替代符号级证据 |
| Level 3 字面层 | `rg` 搜索 `reasoning_summary_text`、`summaryTextDelta`、`ReasoningSummaryChunk`、`ReasoningSummaryDelta`、`item/reasoning`、`AgentMessageDelta` | 证实 `codex-cli-main`、`dasclaw_app_server_protocol`、`dasclaw_app_server_client`、`dasclaw_app_server`、`desktop-app` 均已有部分结构化 reasoning 链路 |

已检查是否已有等价方案，结论：已有 `docs/plans/structured-reasoning-streaming-final-plan.md` 和 `docs/plans/headless-agent-invoke-stream-api-refactor-plan.md`，但它们偏总体方案 / API 重构；本文件补一份面向当前代码现状的文件级改造执行文档。

## 1. 当前结论

这不是一个纯 UI 问题。当前代码已经具备一条主要结构化链路：

```text
provider native reasoning / legacy <think>
  -> LlmStreamEvent::ReasoningSummaryDelta
  -> AgentEvent::ReasoningSummaryChunk
  -> RuntimeTurnOutcome::ReasoningSummaryDelta
  -> item/reasoning/summaryTextDelta
  -> desktop-app reasoning part
```

因此改造重点不是“从零实现 reasoning summary”，而是把链路补齐并验实：

1. 保证实际 desktop-app 启动的 sidecar/runtime 路径走的是这条链路。
2. 补齐 Codex app-server v2 的 reasoning 事件契约差异。
3. 避免 UI 末端通过 strip `<think>` 掩盖后端错误。
4. 用端到端测试证明 `item/reasoning/summaryTextDelta` 在真实发起对话时出现。

## 2. Codex CLI 参考链路

`codex-cli-main` 的做法是结构化事件一路传递，而不是在 UI 解析正文标签。

| 层 | 证据 | 说明 |
|---|---|---|
| 请求侧 | `codex-cli-main/codex-rs/core/src/client.rs:829`、`:841`、`:886`、`:888` | Responses request 带 `reasoning`，并启用 `stream: true` |
| SSE 解析 | `codex-cli-main/codex-rs/codex-api/src/sse/responses.rs:311`、`:319`、`:403` | `response.reasoning_summary_text.delta` / `response.reasoning_text.delta` / `response.reasoning_summary_part.added` 分别映射为内部事件 |
| turn 层 | `codex-cli-main/codex-rs/core/src/session/turn.rs:2032`、`:2199`、`:2217`、`:2229` | delta 绑定当前 active reasoning item，并进入不同 `EventMsg` |
| app-server | `codex-cli-main/codex-rs/app-server/src/bespoke_event_handling.rs:1420`、`:1434`、`:1447` | core event 转成 app-server notification |
| 协议 | `codex-cli-main/codex-rs/app-server-protocol/src/protocol/common.rs:1080`、`:1081`、`:1082` | 暴露 `summaryTextDelta`、`summaryPartAdded`、`textDelta` 三类 reasoning method |

对当前项目的直接启发：

- `item/agentMessage/delta` 只承载用户可见正文。
- `item/reasoning/summaryTextDelta` 承载可展示 reasoning summary。
- `item/reasoning/textDelta` 承载 raw reasoning text，默认可隐藏。
- `summaryPartAdded` 是 summary 分段边界，不能和普通 text delta 混用。

## 3. 当前 x-claw 链路事实

### 3.1 provider / runtime 已具备 reasoning 事件

- `crates/dasclaw_llm_provider/src/provider/provider.rs:39` 定义 `LlmStreamEvent::TextDelta` 与 `ReasoningSummaryDelta`。
- `crates/dasclaw_llm_provider/src/provider/claw_code_provider.rs:461` 将 `OutputContentBlock::Text` 转为 `TextDelta`。
- `crates/dasclaw_llm_provider/src/provider/claw_code_provider.rs:466` 将 `OutputContentBlock::Thinking` 转为 `ReasoningSummaryDelta`。
- `crates/dasclaw_runtime/src/llm_adapter.rs:141` 消费 provider stream。
- `crates/dasclaw_runtime/src/llm_adapter.rs:152` 将 native `ReasoningSummaryDelta` 转为 `AgentEvent::ReasoningSummaryChunk`。
- `crates/dasclaw_runtime/src/llm_adapter.rs:240` 的 `LegacyReasoningStream` 处理 legacy `<think>` / `<final>` 分段。
- `crates/dasclaw_core/src/agentic_loop.rs:151` 定义 `AgentEvent::TextChunk` 与 `ReasoningSummaryChunk`。

### 3.2 app-server 已能发送 summaryTextDelta

- `crates/dasclaw_app_server_protocol/src/lib.rs:50` 定义 `item/agentMessage/delta`。
- `crates/dasclaw_app_server_protocol/src/lib.rs:51` 定义 `item/reasoning/summaryTextDelta`。
- `crates/dasclaw_app_server_protocol/src/lib.rs:1280` 定义 `AgentMessageDeltaEvent`。
- `crates/dasclaw_app_server_protocol/src/lib.rs:1289` 定义 `ReasoningSummaryTextDeltaEvent`。
- `crates/dasclaw_app_server/src/lib.rs:1561` 通过 `Agent::stream(...)` 消费 runtime 事件。
- `crates/dasclaw_app_server/src/lib.rs:1576` 将 `AgentEvent::ReasoningSummaryChunk` 写入 runtime update。
- `crates/dasclaw_app_server/src/lib.rs:1131` drain runtime updates。
- `crates/dasclaw_app_server/src/lib.rs:1152` 处理 `RuntimeTurnOutcome::ReasoningSummaryDelta`。
- `crates/dasclaw_app_server/src/lib.rs:1054` 发出 `ReasoningSummaryTextDeltaEvent`。

### 3.3 client / desktop-app 已能消费 summaryTextDelta

- `crates/dasclaw_app_server_client/src/lib.rs:63` 的 typed notification 包含 `ReasoningSummaryTextDelta`。
- `crates/dasclaw_app_server_client/src/lib.rs:118` 能将 method 解码为 typed notification。
- `crates/dasclaw_app_server_client/src/lib.rs:1433` 有 line-delimited transport 解码 reasoning notification 的测试。
- `desktop-app/src/renderer/src/lib/appServerTurnTracker.ts:137` 同时识别 `item/agentMessage/delta` 与 `item/reasoning/summaryTextDelta`。
- `desktop-app/src/renderer/src/lib/appServerTurnTracker.ts:151` 将 `summaryTextDelta` 映射成 `{ type: "reasoning" }`。
- `desktop-app/src/renderer/src/lib/appServerTurnTracker.test.ts:27` 覆盖 reasoning 与正文分离。
- `desktop-app/src/renderer/src/lib/appServerTurnTracker.test.ts:73` 明确要求 renderer 保留后端错误发来的 `<think>` 文本，不做静默清洗。

## 4. 剩余改造点

| 优先级 | 改造点 | 当前证据 | 目标 |
|---|---|---|---|
| P0 | 真实 desktop-app 对话路径验证 | 单元 / contract 证据较完整，但需要证明实际 sidecar 发起 turn 时收到 `summaryTextDelta` | 增加真实 stdio / Electron manager 层测试，覆盖 runtime bridge 到 renderer tracker |
| P0 | 不要把 reasoning item id 固化成伪 id | `crates/dasclaw_app_server/src/lib.rs:1067` 当前使用 `format!("{turn_id}:reasoning")` | 若 runtime 能提供 item id，则透传真实 id；短期保留伪 id 时文档化为兼容策略 |
| P1 | Codex v2 reasoning event 契约补齐 | 当前 protocol 只有 `summaryTextDelta`，`rg` 未找到 `summaryPartAdded` / `item/reasoning/textDelta` 的 dasclaw 实现 | 增加 `summaryPartAdded`，评估是否增加 raw `textDelta` |
| P1 | summaryIndex 分段 | app-server bridge 当前把 `summary_index` 固定为 `0` | provider/runtime 具备多段信息时透传；没有时继续 `0` 并用测试锁定 |
| P1 | capabilities / schema 宣告 | protocol fixture 有 reasoning notification，但需核对 advertised emitted events 是否包含新增 method | capabilities 中声明所有会发出的 Codex v2 event |
| P2 | UI grouping 细化 | desktop tracker 只按相邻 `type` 合并 | 后续可按 `itemId + summaryIndex` 聚合，避免多 reasoning section 被合并 |

## 5. 修改方案

### Slice A：端到端验实当前链路

目标：证明实际 desktop-app 发起对话时，不只是最终 completion 可用，而是中途收到 `item/reasoning/summaryTextDelta`。

建议修改 / 新增测试：

1. `crates/dasclaw_app_server/src/lib.rs`
   - 扩展已有测试 `codex_v2_runtime_bridge_keeps_reasoning_out_of_agent_text` 附近的覆盖。
   - 验证通知顺序至少包含：
     - `item/reasoning/summaryTextDelta`
     - `item/agentMessage/delta`
     - `turn/completed`
   - 断言 `item/agentMessage/delta` 不包含 `<think>`、`</think>`、reasoning 文本。

2. `desktop-app/src/main/appServerManager.test.ts`
   - 用 fake sidecar notification 模拟真实 manager 收到 `summaryTextDelta`。
   - 断言 renderer listener 能收到该 notification。

3. `desktop-app/src/renderer/src/lib/assistantMessages.test.ts`
   - 保持现有 reasoning part 测试。
   - 增加“先 reasoning 后 text、穿插多段 delta”场景。

验收命令：

```bash
cargo check -p dasclaw_app_server --tests
cargo nextest run -p dasclaw_app_server
cargo check -p dasclaw_app_server_client --tests
cargo nextest run -p dasclaw_app_server_client
pnpm --dir desktop-app test -- appServerTurnTracker assistantMessages appServerManager
```

### Slice B：补齐 Codex-compatible reasoning methods

目标：协议面接近 Codex app-server v2，不让后续 UI 或 app-server client 在新增 reasoning section / raw reasoning 时再次改协议。

建议改动：

1. `crates/dasclaw_app_server_protocol/src/lib.rs`
   - 增加：
     - `event::ITEM_REASONING_SUMMARY_PART_ADDED = "item/reasoning/summaryPartAdded"`
     - `event::ITEM_REASONING_TEXT_DELTA = "item/reasoning/textDelta"`
   - 增加：
     - `ReasoningSummaryPartAddedEvent { thread_id, turn_id, item_id, summary_index }`
     - `ReasoningTextDeltaEvent { thread_id, turn_id, item_id, content_index, delta }`
   - 给 `ServerNotification` 增加构造函数与 fixture。

2. `crates/dasclaw_app_server_client/src/lib.rs`
   - typed enum 增加：
     - `ReasoningSummaryPartAdded`
     - `ReasoningTextDelta`
   - `TryFrom<ServerNotification>` 增加 method decode。
   - 增加 line-delimited transport 解码测试。

3. `crates/dasclaw_app_server/src/lib.rs`
   - capabilities / profile emitted event 列表加入新增 method。
   - 暂不发 raw `textDelta`，除非 runtime/provider 明确提供 raw reasoning。

验收：

- `codex_app_server_v2` profile 宣告的 event 与实际 emitted notification 一致。
- 未实现 raw reasoning 时，protocol 可以解析但 app-server 不主动发。

### Slice C：改善 reasoning item id 与 summaryIndex

目标：降低 UI 聚合歧义。

当前 `crates/dasclaw_app_server/src/lib.rs:1067` 使用 `format!("{turn_id}:reasoning")`。这能让 UI 稳定收敛，但与 Codex “绑定 active reasoning item id” 不完全一致。

改造路径：

1. 短期：
   - 保留 `{turn_id}:reasoning`，把它明确为 dasclaw app-server 兼容 id。
   - 在测试中断言同一 turn 的 reasoning delta 使用稳定 item id。

2. 中期：
   - 在 `AgentEvent::ReasoningSummaryChunk` 或 runtime update 中携带 `item_id: Option<String>` / `summary_index`。
   - provider 没有原生 item id 时仍 fallback 到 `{turn_id}:reasoning`。

3. 长期：
   - 如果 provider upstream 能返回 response item id，则透传真实 id。

验收：

- UI 按 `itemId + summaryIndex` 可聚合。
- 当前没有真实 item id 的 provider 不阻塞 reasoning 显示。

### Slice D：UI 只做映射，不做清洗

目标：让 UI adapter 继续保持当前正确方向：根据 app-server method 构造 part，不解析 `<think>`。

建议改动：

1. `desktop-app/src/renderer/src/lib/appServerTurnTracker.ts`
   - 支持未来新增 `item/reasoning/textDelta`，可映射为 `reasoning` 或独立 `rawReasoning`，默认先隐藏 raw reasoning 也可以。
   - 如果支持 `summaryPartAdded`，不要生成可见 text part，只更新 section 边界状态。

2. `desktop-app/src/renderer/src/lib/appServerTurnTracker.test.ts`
   - 保留 `keeps backend text deltas literal instead of stripping think tags`。
   - 增加 `summaryPartAdded` 不污染 text 的测试。

验收：

- renderer 中不新增 `stripThinkTags` / `sanitizeThinking` 类型 helper。
- `item/agentMessage/delta` 若包含 `<think>`，测试仍应暴露为后端 bug，而不是静默吞掉。

## 6. 排查顺序

如果 desktop-app 发起对话时 reasoning summary 仍未显示，按这个顺序查：

1. provider 是否发 `LlmStreamEvent::ReasoningSummaryDelta`
   - 查 `crates/dasclaw_llm_provider/src/provider/claw_code_provider.rs:466`
   - 或 legacy `<think>` 是否被 `LegacyReasoningStream` 解析

2. runtime 是否发 `AgentEvent::ReasoningSummaryChunk`
   - 查 `crates/dasclaw_runtime/src/llm_adapter.rs:152`

3. app-server bridge 是否收到并写 update
   - 查 `crates/dasclaw_app_server/src/lib.rs:1576`

4. app-server 是否 emit notification
   - 查 `crates/dasclaw_app_server/src/lib.rs:1152`
   - 查 `crates/dasclaw_app_server/src/lib.rs:1054`

5. desktop main process 是否把 notification 转给 renderer
   - 查 `desktop-app/src/main/appServerManager.ts`

6. renderer 是否映射成 reasoning part
   - 查 `desktop-app/src/renderer/src/lib/appServerTurnTracker.ts:137`

## 7. 非目标

- 不在 UI 层清理 `<think>` 标签作为主要修复。
- 不把 Vercel UI stream 作为后端协议事实来源。
- 不默认展示 raw chain-of-thought；本方案只处理可展示 reasoning summary。
- 不在本次改造中重写 provider / runtime 架构。

## 8. 完成标准

本改造完成时应满足：

1. 真实 app-server runtime bridge 测试能观测到 `item/reasoning/summaryTextDelta`。
2. desktop-app renderer 能把 reasoning notification 变成 reasoning part。
3. `item/agentMessage/delta` 不包含 `<think>` 或 reasoning 内容。
4. 协议 / client 对新增 reasoning method 有 contract test。
5. `codex_app_server_v2` profile 宣告与实际可发事件一致。

推荐最终验证：

```bash
cargo check -p dasclaw_app_server_protocol --tests
cargo check -p dasclaw_app_server_client --tests
cargo check -p dasclaw_app_server --tests
cargo nextest run -p dasclaw_app_server_protocol -p dasclaw_app_server_client -p dasclaw_app_server
pnpm --dir desktop-app test -- appServerTurnTracker assistantMessages appServerManager
cargo fmt --all
python3.12 scripts/check_no_panics.py --base origin/xClaw
```

已检查 `desktop-app reasoning summary streaming modification plan` 是否已有，结论：已有总方案但没有这份面向当前实现状态的改造执行文档；本文件记录当前已具备链路、剩余缺口与文件级执行步骤。
