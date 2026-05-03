//! `dasclaw_llm_provider` — 主仓自实现的 LLM provider crate
//!
//! 本 crate 在 [ADR-118] 定义下，替代主仓对 `claw-code-api` 的 path-dep。
//! claw-code 子仓视为只读参考库，本 crate **重新设计** Anthropic Messages /
//! OpenAI Chat Completions 兼容层 wire 类型与 client 抽象，不直接消费子仓代码。
//!
//! 当前阶段：**PR-A.3 — OpenAI Chat Completions 兼容客户端**
//! - ✅ wire 类型（`types`）—— Anthropic Messages 协议输入/输出/streaming events
//! - ✅ 强类型错误（`error::ApiError`）—— 拆分 `RateLimited` / `AuthFailed` /
//!   `ContextWindowExceeded` / `ServerError` / `Transport` / `MalformedSseFrame` 等，
//!   便于上层精确映射
//! - ✅ `ProviderKind` + 模型别名表 + `metadata_for_model` + `max_tokens_for_model`
//! - ✅ `http::ProxyConfig` + `http::build_http_client_with` —— `reqwest::Client`
//!   构造 + 代理（HTTP/HTTPS/统一 URL/no_proxy）支持，纯函数 + 显式 env 入口
//! - ✅ `sse::SseParser` + `sse::parse_frame` —— 增量 SSE 帧解析器，识别 ping/DONE/comment
//! - ✅ `retry::RetryPolicy` —— 指数退避 + 抖动 + `Retry-After` 头部尊重
//!   （**改进**：claw-code 完全忽略 `Retry-After`，本 crate 优先采纳上游建议）
//! - ✅ `providers::AnthropicClient` —— `complete()` / `stream()` + 状态码精确映射到
//!   `ApiError` 各变体；`AuthSource` 支持 `ApiKey` / `BearerToken` / 双 header
//! - ✅ `providers::OpenAiCompatClient` —— `complete()` + xAI / OpenAI / DashScope 三预设；
//!   复用 `RetryPolicy` 与 `ApiError` 强类型；inline error envelope 兜底；
//!   `gpt-5*` 模型自动改用 `max_completion_tokens`；reasoning_content 提升为 Thinking block
//! - 🟡 `OpenAiCompatClient::stream()`：留给 PR-A.3.1（ironclaw 当前 call site 仅用 `send_message`）
//! - 🟡 删除主仓 `claw-code-api` path-dep：留给 PR-A.4
//!
//! 与 `claw-code-api` 的设计差异（顺势处理架构债，详见 ADR-118 §8.5）：
//!
//! 1. **错误强类型化**：`ApiError` 拆变体，调用方可精确决策重试 / 重新鉴权 / 不重试。
//!    `claw-code-api::ApiError` 是 all-in-one，调用方靠字符串扫描判断。
//! 2. **无 env 副作用**：`detect_provider_kind` 接受显式 `EnvSnapshot`，而不是
//!    内部调 `std::env::var_os`。这让本 crate 完全 pure，便于测试与多租户。
//! 3. **无 pricing 耦合**：本 crate 只输出 `Usage { input_tokens / output_tokens / ... }`
//!    四个原始字段，**不计算 USD 成本**。成本计算是 ironclaw 应用层 `costs::model_cost`
//!    的职责（claw-code-api 反向依赖 `runtime::pricing_for_model` 是层间泄漏，本 crate 不复刻）。
//!
//! [ADR-118]: ../docs/plans/architecture-refactor/adr-118-claw-code-readonly-and-self-impl.md

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod http;
pub mod providers;
pub mod retry;
pub mod sse;
pub mod types;

pub use error::ApiError;
pub use http::{build_http_client, build_http_client_with, ProxyConfig};
pub use providers::{
    detect_provider_kind, max_tokens_for_model, max_tokens_for_model_with_override,
    metadata_for_model, model_token_limit, resolve_model_alias, AnthropicClient, AnthropicStream,
    AuthSource, EnvSnapshot, ModelTokenLimit, OpenAiCompatClient, OpenAiCompatConfig, ProviderKind,
    ProviderMetadata,
};
pub use retry::{parse_retry_after, RetryPolicy};
pub use sse::{parse_frame, SseParser};
pub use types::{
    ContentBlockDelta, ContentBlockDeltaEvent, ContentBlockStartEvent, ContentBlockStopEvent,
    InputContentBlock, InputMessage, MessageDelta, MessageDeltaEvent, MessageRequest,
    MessageResponse, MessageStartEvent, MessageStopEvent, OutputContentBlock, StreamEvent,
    ToolChoice, ToolDefinition, ToolResultContentBlock, Usage,
};
