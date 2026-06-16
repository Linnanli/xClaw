# Headless Agent Invoke / Stream API Refactor Plan

> 日期：2026-06-15
> 状态：待执行方案
> 目标：参考 `codex-cli-main` 的分层，把无头 agent 框架的 `invoke` / `stream` API 定义清楚，去掉 provider streaming 自动检测，并在未发布首版前直接替换 `Agent::run(...)` / `Agent::run_streaming(...)`，不保留兼容别名。

## 0. 过程透明记录

本文件是新增架构方案文档，且参考 `codex-cli-main` 做跨项目对账。按仓库规则先完成 4 问与三层核验。

| 启动问题 | 结论 | 处理 |
|---|---|---|
| 是否新增模块 / crate / 文件？ | 是，新增方案文档 | 先查是否已有等价方案 |
| 是否包含否定性结论？ | 是，会判断哪些 fallback / 兼容 API 不保留 | 保留证据与边界 |
| 是否跨项目对账？ | 是，参考 `codex-cli-main` | 做语义层、符号层、字面层核验 |
| 是否写架构对账类文档？ | 是 | 记录 evidence / inference，不靠印象下结论 |

核验记录：

| 层级 | 工具 / 证据 | 结果 |
|---|---|---|
| Level 1 语义层 | `mcp__code_review_graph.semantic_search_nodes_tool` 查询 `headless agent stream invoke API design provider call mode event stream`、`agent run streaming LLM call mode provider native streaming frontend event stream` | 未发现专门定义 headless agent `invoke/stream` API 的方案文档；命中当前 provider stream 入口、`ProviderClient::stream_message`、旧 `desktop-client` 参考实现等相关面 |
| Level 2 符号层 | 本地 `lsp-mcp execute_lsp document_symbols`：`crates/dasclaw_runtime/src/agent.rs`、`crates/dasclaw_core/src/agentic_loop.rs`、`crates/dasclaw_llm_provider/src/provider/provider.rs` | 确认当前 `Agent` 暴露 `run` / `run_streaming`；core loop 以 `event_tx: Option<_>` 决定 `respond` / `respond_streaming`；`LlmProvider` 暴露 `complete_with_tools`、`supports_streaming`、`complete_with_tools_structured_stream` |
| Level 3 字面层 | `rg` 精确搜索 `run_streaming`、`supports_streaming`、`complete_with_tools_structured_stream`、`client_session.stream`、`stream: true`、`AgentMessageDelta`、`ReasoningSummaryTextDelta` | 确认当前 runtime 自动检测 provider streaming；`codex-cli-main` 模型侧用原生 stream，core 转成内部事件，app-server 再转通知 |

已检查 `headless agent invoke stream API refactor plan` 是否已有，结论：已有 `structured-reasoning-streaming-final-plan.md` 解决 reasoning/text 分流，有 `dasclaw-app-server-architecture-plan.md` 说明 app-server 分层，但没有专门面向无头 agent `invoke` / `stream` API 的重构方案；本文件补齐该 API 决策。

## 1. 一句话结论

采用 `codex-cli-main` 的分层思路：**对外暴露稳定的 `Stream` 消费体验，内部用 channel-backed wrapper 做桥接和收口**，避免一个 `stream` 同时表示三件事：

```text
provider.stream(...) -> LlmStreamEvent       // 模型原生流的归一化封装
agent.stream(...)    -> AgentEvent           // agent 生命周期事件流
app-server notify    -> ServerNotification   // UI / 协议通知流
```

`Agent::stream(...)` 返回的是 agent event stream，不返回 OpenAI / Anthropic / provider 原始 SSE 或 WebSocket event。模型原生 stream 只存在于 provider 层。

首版未发布，因此直接删除 / 重命名现有 `Agent::run(...)` 和 `Agent::run_streaming(...)`，不保留 deprecated alias。

实现上优先采用和 `codex-cli-main` `ResponseStream` 相同的形状：

```text
provider / runtime producer task
  -> tokio::sync::mpsc::Receiver<Result<AgentEvent, AgentError>>
  -> AgentRunStream implements futures::Stream
```

也就是说，`AgentRunStream` 是真正的 `Stream` API，但底层可以继续使用 `tokio` channel。要学的是“桥接和收口”的架构，不是把 `mpsc::Sender` 继续暴露给调用方。

## 2. Codex 参考结论

`codex-cli-main` 不是把 provider 原始 SSE 直接暴露给前端，而是分层转译：

```text
Responses API stream / WebSocket stream
  -> core ResponseEvent
  -> core EventMsg
  -> app-server ServerNotification
  -> UI state machine
```

证据：

- `codex-cli-main/codex-rs/core/src/client.rs:879` 构造 `ResponsesApiRequest`，`stream: true`。
- `codex-cli-main/codex-rs/core/src/client.rs:1157` 的 `stream_responses_api` 发起 HTTP streaming request。
- `codex-cli-main/codex-rs/core/src/client.rs:1497` 先尝试 Responses WebSocket stream，fallback 到 HTTP SSE 仍是 stream。
- `codex-cli-main/codex-rs/core/src/session/turn.rs:1884` 调用 `client_session.stream(...)`。
- `codex-cli-main/codex-rs/core/src/session/turn.rs:1921` 通过 `stream.next()` 消费模型事件。
- `codex-cli-main/codex-rs/core/src/session/turn.rs:2152` 把 `ResponseEvent::OutputTextDelta` 转成 assistant text path。
- `codex-cli-main/codex-rs/core/src/session/turn.rs:2181` 消费 tool-call input delta。
- `codex-cli-main/codex-rs/core/src/session/turn.rs:2199` 把 reasoning summary delta 转成 reasoning event。
- `codex-cli-main/codex-rs/app-server/src/bespoke_event_handling.rs:1373` 把 `EventMsg::AgentMessageContentDelta` 转成 `ServerNotification::AgentMessageDelta`。
- `codex-cli-main/codex-rs/app-server/src/bespoke_event_handling.rs:1420` 把 `EventMsg::ReasoningContentDelta` 转成 `ServerNotification::ReasoningSummaryTextDelta`。
- `codex-cli-main/codex-rs/core/src/client_common.rs:176` 定义 `ResponseStream { rx_event: mpsc::Receiver<Result<ResponseEvent>> }`。
- `codex-cli-main/codex-rs/core/src/client_common.rs:180` 为 `ResponseStream` 实现 `futures::Stream`，`poll_next` 委托给 `rx_event.poll_recv(cx)`。
- `codex-cli-main/codex-rs/core/src/client.rs:1624` 的 `map_response_stream(...)` 用 `tokio::spawn` 消费底层 API stream，统一错误、telemetry、last response 副产物，再通过 channel 返回上层 `ResponseStream`。

推论：我们应参考的是“provider 原生 stream 在下层、agent/app-server 暴露语义事件流”的分层，而不是把 provider 原始协议泄漏到无头 agent API。

进一步推论：`Agent::stream(...)` 第一版应采用 channel-backed stream wrapper，而不是直接让 app-server / 调用方持有 `mpsc::Sender`。channel 是内部桥接手段，`Stream` 是公开消费契约。

需要明确区分两类 fallback：

- 允许 provider / transport 层内部在“仍然是 native stream”的前提下选择 WebSocket、HTTP SSE、fixture 等不同 transport。
- 不允许 `ModelCallMode::Stream` 静默 fallback 到 `ModelCallMode::Invoke`，否则调用方无法证明自己正在测试 native stream path。

## 3. 当前问题

当前实现把两个决策耦合在一起：

1. 调用方是否要边跑边收到 agent 事件。
2. provider 是否用模型原生 stream API。

当前路径：

- `Agent::run(...)` 不传 `event_tx`，core loop 调 `AgentResponder::respond(...)`。
- `Agent::run_streaming(...)` 传 `event_tx`，core loop 调 `AgentResponder::respond_streaming(...)`。
- `LlmProviderResponder::respond_streaming(...)` 内部用 `provider.supports_streaming()` 自动决定走 `complete_with_tools(...)` 还是 `complete_with_tools_structured_stream(...)`。

这个形状的问题：

- `run_streaming` 名字让人误以为一定使用 provider 原生 stream，实际可能 fallback 成完整响应后一次性发事件。
- `supports_streaming()` 同时承担 capability 与 routing policy，调用方无法显式要求 invoke 或 stream。
- 静默 fallback 会掩盖配置错误：用户以为在测 native stream，实际可能走 invoke 后一次性 delta。
- `event_tx` 作为 API 参数不够 Rust 生态常见；调用方更习惯拿到 `Stream<Item = ...>`。

## 4. 目标 API

### 4.1 Agent 层

Agent 层只表达调用方消费方式：

```rust
use std::sync::Arc;

impl Agent {
    pub async fn invoke(
        &self,
        prompt: &str,
        options: AgentRunOptions,
    ) -> Result<AgentRunOutput, AgentError>;

    pub fn stream(
        self: Arc<Self>,
        prompt: impl Into<String>,
        options: AgentRunOptions,
    ) -> AgentRunStream;
}
```

语义：

- `invoke`：请求 / 响应式 API，调用方只拿最终 `AgentRunOutput`。
- `stream`：事件式 API，调用方消费 `AgentEvent`，最终事件携带 `AgentRunOutput`。
- 二者都可通过 `AgentRunOptions.model_call_mode` 指定 provider 调用方式。
- `stream` 的事件是 agent 语义事件，不是 raw model event。

建议类型：

```rust
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::Stream;
use tokio::sync::mpsc;

pub struct AgentRunOptions {
    pub model_call_mode: ModelCallMode,
}

pub enum ModelCallMode {
    Invoke,
    Stream,
}

pub struct AgentRunOutput {
    pub text: String,
}

pub struct AgentRunStream {
    rx_event: mpsc::Receiver<Result<AgentEvent, AgentError>>,
}

impl Stream for AgentRunStream {
    type Item = Result<AgentEvent, AgentError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.rx_event.poll_recv(cx)
    }
}

pub enum AgentEvent {
    ReasoningSummaryDelta(String),
    TextDelta(String),
    ToolCallStarted { name: String, arguments: serde_json::Value },
    ToolCallCompleted { name: String, content: String, is_error: bool },
    FinishReason(FinishReason),
    Completed(AgentRunOutput),
}
```

依赖说明：

- 公开 trait 只需要 `futures-core` 的 `Stream`，避免引入完整 `futures` 依赖面。
- 内部继续用现有 `tokio::sync::mpsc`；如后续想减少手写 wrapper，可评估 `tokio-stream::wrappers::ReceiverStream`，但第一版不需要为了这一点新增 `tokio-stream`。

命名说明：

- 用 `invoke` 替代 `run`，强调请求 / 响应式调用。
- 用 `stream` 替代 `run_streaming`，强调返回 `Stream`。
- 用 `ModelCallMode` 而不是 `StreamMode`，避免和 agent event stream 混淆。
- 不使用 `auto` 作为默认模式；第一版 API 必须显式选择，或由配置解析出确定值。
- `Agent::stream(...)` 使用 `self: Arc<Self>`，不使用 `&self`。stream API 会立刻返回
  `AgentRunStream`，后台 producer task 仍会继续跑一个 turn；`tokio::spawn` 要求被
  spawn 的 future 持有 `Send + 'static` 状态，短生命周期的 `&Agent` 不能被安全移入
  该 task。`Arc<Agent>` 把“stream 还活着时 agent 状态必须还活着”的所有权事实写进
  类型签名。

`Agent::stream(...)` 的内部流程：

```text
Arc<Agent>::stream(prompt, options)
  -> create bounded mpsc channel
  -> move Arc<Agent> + owned prompt + options into one producer task
  -> producer task drives run_turn_in_context(...)
  -> task sends AgentEvent deltas and final Completed(output)
  -> return AgentRunStream { rx_event }
```

调用方提前 drop `AgentRunStream` 时，producer task 应尽快观察到 channel closed，并停止继续发送 UI 事件。LLM/provider 是否立即取消由现有 cancellation token 语义承接；本方案不在 stream wrapper 上新增第二套 cancellation API。

所有权决策：

- `invoke(&self, ...)` 保持借用式 API，因为调用方会 await 到最终结果，`&self` 的生命周期
  能覆盖整个调用。
- `stream(self: Arc<Self>, ...)` 使用共享所有权 API，因为函数返回后后台 producer task 仍在
  运行，不能把短生命周期 `&self` 放进 `tokio::spawn`。
- 不给 `Agent` 第一版公开承诺 `Clone`。当前字段虽然大多是 `Arc` 或可共享 handle，但
  `Clone` 容易被误读为复制一个独立 agent；实际需要共享 `ApprovalInbox`、cancellation
  token、hooks 与 provider handle。使用 `Arc<Agent>` 能避免“clone 是深拷贝还是共享句柄”的
  语义歧义，也不会让未来新增不可 clone 字段时破坏公开合同。
- app-server / CLI / tests 如需调用 stream，应在构造后持有 `Arc<Agent>`：

```rust
let agent = Arc::new(Agent::builder().build()?);
let mut stream = Arc::clone(&agent).stream(prompt, options);
```

如果某个调用方只有 owned `Agent` 且只打算单次使用，可直接 `Arc::new(agent).stream(...)`。
这不是额外 runtime 成本，而是把后台任务生命周期显式交给 `Arc` 管理。

### 4.2 Provider 层

Provider 层只表达模型调用方式：

```rust
pub trait LlmProvider: Send + Sync {
    async fn invoke_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse, LlmError>;

    async fn stream_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<LlmStream, LlmError>;

    fn capabilities(&self) -> LlmProviderCapabilities;
}
```

建议类型：

```rust
pub struct LlmProviderCapabilities {
    pub native_streaming: bool,
}

pub struct LlmStream {
    rx_event: tokio::sync::mpsc::Receiver<Result<LlmStreamEvent, LlmError>>,
}

pub enum LlmStreamEvent {
    TextDelta(String),
    ReasoningSummaryDelta(String),
    ToolCallInputDelta { call_id: String, delta: String },
    Completed(ToolCompletionResponse),
}
```

建流错误语义：

- `stream_with_tools(...)` 必须是 `async` 并返回 `Result<LlmStream, LlmError>`。现有
  `ProviderClient::stream_message(...)` 在发起 HTTP / WebSocket / fixture stream 时就是
  async，且鉴权失败、网络失败、provider 拒绝 stream、响应头不合法等错误都发生在“拿到
  stream 对象之前”。
- 这一层应显式对齐 `codex-cli-main` 的两层错误模型：
  `client_session.stream(...).await -> Result<ResponseStream>` 负责“能不能建好流”，
  `ResponseStream: Stream<Item = Result<ResponseEvent>>` 负责“流建立成功后每个事件是否正常”。
  我们的 provider 层应采用同构形状：
  `stream_with_tools(...).await -> Result<LlmStream, LlmError>` 加
  `LlmStream: Stream<Item = Result<LlmStreamEvent, LlmError>>`。
- 第一版不采用 `fn stream_with_tools(...) -> LlmStream` 加“第一项 event 表达建流错误”的形状。
  那会让建流失败和流中途失败混在同一个通道里，调用方也更难在 turn start 阶段 fail-fast。
- `LlmStream` 本身仍可在消费过程中产生 `Result<LlmStreamEvent, LlmError>`，用于表达已经建流成功
  之后的 SSE 解析错误、连接中断、provider 中途错误或 tool-input delta 重组失败。
- 换句话说，外层 `Result` 表示 “stream start”，内层 `Result<Item>` 表示
  “stream receive / decode / map event”。

迁移语义：

- `complete_with_tools(...)` 重命名为 `invoke_with_tools(...)`。
- `complete_with_tools_structured_stream(...)` 重命名并收敛为 `stream_with_tools(...)`，异步返回 provider 层归一化事件流，不暴露厂商原始 event。
- `supports_streaming()` 不再参与自动路由；要么删除，要么只作为 `capabilities().native_streaming` 的兼容内部实现细节。
- 当 `ModelCallMode::Stream` 遇到不支持 native streaming 的 provider，返回显式错误，不静默 fallback。
- provider wrapper（retry / circuit breaker / failover / response cache / smart routing / token refreshing）必须显式转发 `capabilities()`、`invoke_with_tools(...)`、`stream_with_tools(...)`。其中 failover 的 stream 语义要保持当前约束：streaming 只委托给选定 provider，不能在已发 delta 后切换 provider。

错误建议：

```rust
LlmError::UnsupportedCallMode {
    provider: String,
    requested: ModelCallMode,
}
```

### 4.3 Runtime / responder 层

Runtime adapter 负责把 `ModelCallMode` 从 agent options 传到 provider：

```text
Agent::invoke(options.model_call_mode)
  -> AgenticLoop
  -> AgentResponder::respond(ctx, CallPolicy)
  -> LlmProviderResponder
  -> provider.invoke_with_tools(...) 或 provider.stream_with_tools(...)

Arc<Agent>::stream(options.model_call_mode)
  -> AgenticLoop with event sink
  -> same provider call decision
  -> AgentEvent stream
```

关键约束：

- `Arc<Agent>::stream(..., ModelCallMode::Invoke)` 是合法组合：agent 对调用方仍然返回事件流，但模型结果只会在完整响应后形成一次或少数几次 `TextDelta` / `ReasoningSummaryDelta`。
- `Agent::invoke(..., ModelCallMode::Stream)` 也是合法组合：内部可使用 provider native stream 汇总结果，但调用方只拿最终 `AgentRunOutput`。
- 因此“调用方消费方式”和“模型调用方式”必须正交。

这要求 core loop 不能继续只用 `event_tx: Option<_>` 决定 `respond` / `respond_streaming`。需要新增中性的 call policy，例如：

```rust
pub struct AgentCallPolicy {
    pub model_call_mode: ModelCallMode,
    pub event_sink: Option<mpsc::Sender<AgentEvent>>,
}
```

核心约束：

- `event_sink` 只表示调用方是否消费 agent events。
- `model_call_mode` 只表示 provider 用 invoke 还是 native stream。
- `AgentResponder` / `LlmProviderResponder` 根据 `model_call_mode` 选择 provider path，再根据 `event_sink` 决定是否转发中间事件。
- `Completed(AgentRunOutput)` 只由 `AgentRunStream` producer task 发送；app-server 不再同时依赖 stream event 和 `run_streaming` 返回值发完成通知，避免双完成。

## 5. 配置设计

配置字段建议命名为 `model_call_mode`：

```toml
[agent]
model_call_mode = "stream" # "invoke" | "stream"
```

app-server 初始化或 desktop-app model provider config 中如需透传，应使用同名语义：

```json
{
  "runtime": {
    "modelCallMode": "stream"
  }
}
```

规则：

- CLI / app-server / tests 必须能直接构造 `AgentRunOptions { model_call_mode: ... }`，不要只依赖全局配置。
- 配置解析失败应 fail-fast。
- provider 不支持配置指定的 call mode 时，turn start 失败并给出明确错误。
- 不做“stream 失败后自动 invoke”的隐式降级；如以后要降级，必须是显式策略字段，例如 `fallback_on_stream_error = true`，本方案不包含该能力。

## 6. app-server 与 desktop-app 边界

app-server 对 desktop-app 仍使用事件通知协议，不拆成两套 turn API：

```text
desktop-app <-> app-server: 始终 ServerNotification / notification stream
app-server -> runtime: Arc<Agent>::stream(..., AgentRunOptions)
runtime -> provider: 按 ModelCallMode 选择 invoke 或 stream
```

原因：

- desktop-app 已经需要 reasoning、text、tool、approval、completion 等多类通知。
- 即使 provider 走 invoke，app-server 也可以在完整响应回来后发出 typed notifications。
- UI 不应该关心 provider 是 native stream 还是 invoke；它只关心事件语义。

app-server 第一版推荐固定调用 `Arc<Agent>::stream(...)`，但由配置控制 `ModelCallMode`：

- `model_call_mode = "stream"`：token / reasoning 原生增量尽快转成通知。
- `model_call_mode = "invoke"`：完整响应回来后转成通知。

## 7. 执行切片

### Slice 1：类型与 API 命名

目标：建立新公共 API，并移除未发布旧 API。

工作项：

1. 新增 `AgentRunOptions`、`AgentRunOutput`、`ModelCallMode`。
2. 新增 `AgentRunStream` channel-backed wrapper，形状参考 `codex-cli-main` `ResponseStream`。
3. 将 `Agent::run(...)` 替换为 `Agent::invoke(...)`。
4. 将 `Agent::run_streaming(...)` 替换为 `Agent::stream(self: Arc<Self>, ...) -> AgentRunStream`。
5. 将 `Session::run(...)` / `Session::run_streaming(...)` 同步迁移为 `Session::invoke(...)` / `Session::stream(...)`，避免公共 API 半旧半新。
6. 删除 `run_in_context_streaming` 这类对外 streaming 命名，内部可保留 `run_in_context_inner` 或改为更中性的 `run_turn_in_context`。
7. 更新 `dasclaw_runtime::README.md`、`dasclaw_session` README / tests 和 re-export。

验收：

- 对外文档中不再出现 `Agent::run(...)` / `Agent::run_streaming(...)` 作为 API。
- `agent.stream()` 返回事件流对象，而不是要求调用方传 `mpsc::Sender`。
- 需要后台 producer task 的调用方持有 `Arc<Agent>`，并通过 `Arc::clone(&agent).stream(...)` 启动。
- `AgentRunStream` 的调用方式与 `codex-cli-main` 一致：`while let Some(event) = stream.next().await`。

### Slice 2：provider call mode 显式化

目标：去掉 `supports_streaming()` 自动路由。

工作项：

1. 在 responder / adapter 层传入 `AgentCallPolicy` 或等价结构。
2. `ModelCallMode::Invoke` 固定调用 provider invoke path。
3. `ModelCallMode::Stream` 固定调用 provider native stream path。
4. provider capability 只用于校验，不用于自动切换到 invoke。
5. stream mode 遇到不支持 native stream 的 provider 时 fail-fast。
6. `stream_with_tools(...).await` 的外层错误只表达建流前失败；建流后的事件错误放进 `LlmStream` 的 `Item = Result<...>`。
7. 同步更新 `dasclaw_core::llm::LlmProviderFacade` 合同文档，移除“调用方用 `supports_streaming()` 判定路径”的旧语义。

验收：

- 单元测试能证明 `ModelCallMode::Invoke` 即使 provider 支持 stream 也不会调用 stream path。
- 单元测试能证明 `ModelCallMode::Stream` 不会 fallback 到 invoke path。
- 单元测试能证明建流前失败直接由 `stream_with_tools(...).await` 返回，不会伪装成第一项 stream event。
- 单元测试能证明建流成功后，中途流错误通过 `LlmStream` 的 `Err(...)` 冒泡。
- 不再有 `if !provider.supports_streaming() { complete_with_tools(...) } else { stream(...) }` 这种自动检测式路由。

### Slice 3：Agent event stream 收口

目标：让 `Agent::stream` 成为主流 Rust 调用风格。

工作项：

1. 定义 `AgentRunStream` 类型，内部由 `mpsc` + spawned task 实现。
2. stream 错误通过 `Stream<Item = Result<AgentEvent, AgentError>>` 表达，不再额外设计 `AgentEvent::Failed`。
3. 最终成功必须发 `Completed(AgentRunOutput)`。
4. tool / approval / finish reason 事件保持结构化。
5. producer task 负责把 loop 返回值转成 `Completed`，并把 loop/provider error 转成 `Err(AgentError)`。
6. receiver drop 后 producer 不再继续发送 agent events；是否取消底层 LLM 请求由 cancellation token 策略处理。

推荐第一版不要使用裸 type alias，而使用具名 wrapper：

```rust
pub struct AgentRunStream {
    rx_event: tokio::sync::mpsc::Receiver<Result<AgentEvent, AgentError>>,
}
```

这样既保留 `Stream` 消费体验，又能把 channel 作为内部实现细节。错误走 `Result`，业务事件不需要塞 `Failed` 分支；app-server 可把错误映射成 turn failed notification。

验收：

- 调用方能用 `while let Some(event) = stream.next().await` 消费。
- 最后一个成功事件是 `AgentEvent::Completed(...)`。
- tool call、tool result、approval needed 的顺序与现有 `AgentEvent` 测试一致。

### Slice 4：app-server 接线

目标：app-server 固定使用 agent event stream，provider call mode 来自配置。

工作项：

1. app-server runtime bridge 构造 `AgentRunOptions`。
2. 当前 `run_streaming(&prompt, event_tx)` 改为 `Arc::clone(&agent).stream(prompt, options)`。
3. 事件 pump 从手写 channel receiver 改成消费 `AgentRunStream`。
4. `AgentEvent::TextDelta` 映射为 `AgentMessageDeltaEvent`。
5. `AgentEvent::ReasoningSummaryDelta` 映射为 `ReasoningSummaryTextDeltaEvent`。
6. `AgentEvent::Completed` 映射为 turn complete。
7. `Err(AgentError)` 映射为 turn failed notification。

验收：

- desktop-app 侧不需要知道 `model_call_mode`。
- invoke 模式下仍能收到完整 text / reasoning notifications。
- stream 模式下能收到增量 text / reasoning notifications。

### Slice 5：测试与文档

目标：锁住分层语义，避免测试掩盖客户端 bug。

必须测试：

| 层 | 用例 | 断言 |
|---|---|---|
| runtime unit | `Agent::invoke(..., Invoke)` | 只调用 provider invoke path，返回最终 output |
| runtime unit | `Agent::invoke(..., Stream)` | 调用 provider stream path，但调用方只拿最终 output |
| runtime unit | `Agent::stream(..., Invoke)` | 返回 agent event stream，text/reasoning 一次性或少量事件发出 |
| runtime unit | `Agent::stream(..., Stream)` | 返回 agent event stream，增量事件顺序正确 |
| provider adapter | stream mode + unsupported provider | 返回 `UnsupportedCallMode`，不 fallback |
| provider adapter | stream 建流前失败 | `stream_with_tools(...).await` 直接返回错误，不产生空 stream |
| provider adapter | stream 建流后中途失败 | `LlmStream` 产生 `Err(LlmError)`，调用方可区分于建流前错误 |
| app-server contract | `modelCallMode=invoke` | notifications 类型正确，UI 不做 text 清洗 |
| app-server contract | `modelCallMode=stream` | delta notifications 类型正确，turn complete 在最后 |

禁止的测试形态：

- 为了通过 UI 测试而从 assistant text 里 strip reasoning 标签。
- 用 snapshot 固化错误的 `<think>` 泄漏。
- 测试只断言“最后文字看起来对”，不检查 reasoning/text/tool 事件类型。

## 8. 迁移影响面

主要改动文件预计包括：

| 区域 | 预计文件 |
|---|---|
| Agent API | `crates/dasclaw_runtime/src/agent.rs`、`crates/dasclaw_runtime/src/lib.rs` |
| Session API | `crates/dasclaw_session/src/lib.rs`、`crates/dasclaw_session/tests/session_streaming.rs` |
| Core loop trait | `crates/dasclaw_core/src/agentic_loop.rs` |
| Core LLM facade docs | `crates/dasclaw_core/src/llm.rs` |
| Provider trait | `crates/dasclaw_llm_provider/src/provider/provider.rs` 及 wrapper providers |
| Runtime adapter | `crates/dasclaw_runtime/src/llm_adapter.rs` |
| App-server bridge | `crates/dasclaw_app_server/src/lib.rs` |
| Config / protocol | `crates/dasclaw_app_server_protocol/src/lib.rs`、`desktop-app/src/main/appServerManager.ts` 如需透传 |
| Tests | runtime streaming tests、provider wrapper tests、app-server contract tests |
| Docs | `crates/dasclaw_runtime/README.md` |

首版未发布，因此不需要兼容：

- 不保留 `Agent::run(...)`。
- 不保留 `Agent::run_streaming(...)`。
- 不保留旧 `event_tx` 参数式公开 streaming API。
- 不提供 deprecated alias。

## 9. 推荐执行顺序

1. 先新增 `ModelCallMode`、`AgentRunOptions`、`AgentRunOutput`、`AgentRunStream`，用 channel-backed wrapper 建立 `Stream` 消费形状。
2. 再改 core loop / responder policy，让 `event_sink` 与 `model_call_mode` 正交。
3. 再改 provider trait 和 wrapper providers，把 `supports_streaming()` 从自动路由逻辑中移除，并把 provider stream 建流错误表达为 `stream_with_tools(...).await -> Result<LlmStream, LlmError>`。
4. 再迁移 `Agent` / `Session` 公共 API 命名。
5. 再接 app-server bridge 与配置，让 app-server 消费 `AgentRunStream` 并由 `Completed` / `Err` 决定 turn 终态。
6. 最后更新 desktop-app 透传配置与文档。

这样排序的原因：

- 先建立 `Stream` wrapper，后续迁移仍可复用现有 `tokio::mpsc` 事件发送路径。
- 先让 core policy 正交，才能正确实现 `invoke(..., Stream)` 与 `stream(..., Invoke)` 两个交叉组合。
- provider call mode 显式化后，测试能直接证明没有 stream-to-invoke 自动 fallback。
- app-server 始终消费 agent events，desktop-app 风险最小。

## 10. 非目标

- 不把 provider 原始 SSE / WebSocket event 暴露给 `Agent::stream` 调用方。
- 不设计自动 fallback 策略。
- 不新增 UI 末端清洗逻辑。
- 不保留首版前的旧 API 兼容层。
- 不把 `@assistant-ui` / Vercel UI stream protocol 上移为 runtime 或 provider 的领域模型。

## 11. 成功标准

完成后应能用下面四种组合清晰表达行为：

| 调用方式 | 模型调用方式 | 语义 |
|---|---|---|
| `agent.invoke(..., Invoke)` | provider invoke | 最简单的请求 / 响应 |
| `agent.invoke(..., Stream)` | provider native stream | 内部流式汇总，调用方只拿最终结果 |
| `agent.stream(..., Invoke)` | provider invoke | 调用方消费 agent event stream，但模型结果一次性到达 |
| `agent.stream(..., Stream)` | provider native stream | 调用方消费 agent event stream，模型 delta 增量到达 |

程序员不需要猜：

- `invoke` 表示调用方等最终结果。
- `stream` 表示调用方消费 agent 事件。
- `ModelCallMode` 表示 provider 怎么请求模型。
- provider capability 只校验，不自动改路。
