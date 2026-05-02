//! `dasclaw_llm_provider` — 主仓自实现的 LLM provider crate
//!
//! 本 crate 在 [ADR-118] 定义下，替代主仓对 `claw-code-api` 的 path-dep。
//! claw-code 子仓视为只读参考库，本 crate **重新设计** Anthropic Messages /
//! OpenAI Chat Completions 兼容层 wire 类型与 client 抽象，不直接消费子仓代码。
//!
//! 当前阶段：**PR-A.0 骨架**
//! - ✅ wire 类型（`types`）—— Anthropic Messages 协议输入/输出/streaming events
//! - ✅ 强类型错误（`error::ApiError`）—— 拆分 `RateLimited` / `AuthFailed` /
//!   `ContextWindowExceeded` / `Http` / `Json` / `Io` / `Provider`，便于上层精确映射
//! - ✅ `ProviderKind` + 模型别名表 + `metadata_for_model` + `max_tokens_for_model`
//! - 🟡 `AnthropicClient` / `OpenAiCompatClient`：留给 PR-A.2，本骨架暂未实现
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
pub mod providers;
pub mod types;

pub use error::ApiError;
pub use providers::{
    detect_provider_kind, max_tokens_for_model, max_tokens_for_model_with_override,
    metadata_for_model, model_token_limit, resolve_model_alias, EnvSnapshot, ModelTokenLimit,
    ProviderKind, ProviderMetadata,
};
pub use types::{
    ContentBlockDelta, ContentBlockDeltaEvent, ContentBlockStartEvent, ContentBlockStopEvent,
    InputContentBlock, InputMessage, MessageDelta, MessageDeltaEvent, MessageRequest,
    MessageResponse, MessageStartEvent, MessageStopEvent, OutputContentBlock, StreamEvent,
    ToolChoice, ToolDefinition, ToolResultContentBlock, Usage,
};
