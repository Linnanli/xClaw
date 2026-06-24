//! `ClawCodeLlmProvider` — LLM Provider 基于 `dasclaw_llm_provider` 的原生客户端。
//!
//! Phase 2 Step I 完成后，本模块是生产唯一 LLM 路径
//! （`GithubCopilot` / `CodexChatGpt` 由各自独立 provider 处理）。
//!
//! W6-C PR-A.4 完成后，底层 client 切换到主仓自实现的 [`dasclaw_llm_provider`]，
//! 不再依赖 `claw-code/rust/crates/api` 的 path-dep（claw-code 子仓只读化，详见
//! [ADR-118](../../../../docs/plans/architecture-refactor/adr-118-claw-code-readonly-and-self-impl.md)）。
//!
//! 关键收益：
//! - `OpenAiCompatClient` 已为各 OpenAI-compat 厂商（DashScope/Kimi/xAI/Ollama）
//!   提供 per-provider hook，消除"打 JSON 补丁"需要（参见
//!   [`docs/plans/architecture-refactor/04-phase2-claw-code-api.md`]）。
//! - 编译期裁掉大量历史 rig 适配层的异构类型体操。

use crate::{
    AnthropicClient, ApiError, AuthSource, ContentBlockDelta, InputContentBlock, InputMessage,
    MessageRequest, MessageResponse, OpenAiCompatClient, OpenAiCompatConfig, OutputContentBlock,
    ProviderClient, StreamEvent, SystemBlock, SystemPrompt, ToolChoice as ApiToolChoice,
    ToolDefinition as ApiToolDefinition, ToolResultContentBlock, Usage,
};
use async_trait::async_trait;
use rust_decimal::Decimal;
use secrecy::ExposeSecret;
use std::collections::BTreeMap;
use tokio::sync::mpsc;

use dasclaw_core::response_types::ResponseMetadata;

use crate::provider::config::{OAUTH_PLACEHOLDER, RegistryProviderConfig};
use crate::provider::error::LlmError;
use crate::provider::provider::{
    ChatMessage, CompletionRequest, CompletionResponse, ContentPart, FinishReason, LlmProvider,
    LlmProviderCapabilities, LlmStream, LlmStreamEvent, Role, ToolCall, ToolCompletionRequest,
    ToolCompletionResponse, ToolDefinition,
};
use crate::provider::registry::ProviderProtocol;

/// 使用 `claw-code-api` 作为底层 HTTP 客户端的 Provider。
#[derive(Debug)]
pub struct ClawCodeLlmProvider {
    client: ProviderClient,
    /// 配置时声明的模型名（可能是 alias，如 `"opus"`）。
    configured_model: String,
    /// `resolve_model_alias` 解析后的真实模型名（请求体用这个）。
    resolved_model: String,
    /// (input_rate, output_rate) — 每 token 成本；None 表示未知。
    cost_rates: Option<(Decimal, Decimal)>,
}

impl ClawCodeLlmProvider {
    /// 覆盖 token 单价（用于 `calculate_cost` / `cost_per_token`）。
    #[must_use]
    pub fn with_cost_rates(mut self, input: Decimal, output: Decimal) -> Self {
        self.cost_rates = Some((input, output));
        self
    }

    /// 从 ironclaw 的 [`RegistryProviderConfig`] 构造一个 Provider。
    ///
    /// 这是 **Step D** 的核心入口，负责把 x-claw 现有的 providers.json 配置
    /// 映射到 `dasclaw_llm_provider` 的具体 client：
    ///
    /// | `ProviderProtocol`  | 映射目标                                                    |
    /// |---------------------|-------------------------------------------------------------|
    /// | `Anthropic`         | `AnthropicClient::new(api_key)` 或 OAuth `AuthSource::BearerToken` |
    /// | `OpenAiCompletions` | `OpenAiCompatClient::new(api_key, cfg).with_base_url(...)`  |
    /// | `Ollama`            | 同上，空 api_key                                             |
    /// | `GithubCopilot`     | 不在本模块范围，由 `github_copilot::GithubCopilotProvider` 处理 |
    ///
    /// 不读取任何环境变量：凭据/URL 都来自显式配置，符合 x-claw 的 keychain 模型。
    pub fn from_registry_config(config: &RegistryProviderConfig) -> Result<Self, LlmError> {
        let configured_model = config.model.clone();
        let resolved_model = crate::resolve_model_alias(&configured_model).to_string();

        let client = match config.protocol {
            ProviderProtocol::Anthropic => build_anthropic_client(config)?,
            ProviderProtocol::OpenAiCompletions => build_openai_compat_client(config)?,
            ProviderProtocol::Ollama => build_ollama_client(config)?,
            ProviderProtocol::GithubCopilot => {
                return Err(LlmError::RequestFailed {
                    provider: config.provider_id.clone(),
                    reason: "github_copilot 不应由 ClawCodeLlmProvider 处理；请确认调用方先走 \
                             GithubCopilotProvider （见 mod.rs create_registry_provider）"
                        .to_string(),
                });
            }
        };

        Ok(Self {
            client,
            configured_model,
            resolved_model,
            cost_rates: None,
        })
    }

    /// 返回底层 `ProviderClient` 供测试注入使用（内部接口，不公开稳定）。
    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn client_for_test(&self) -> &ProviderClient {
        &self.client
    }
}

// ============================================================================
// 请求映射：ironclaw → dasclaw_llm_provider
// ============================================================================

/// 把 ironclaw 的 `CompletionRequest` 映射为 claw-code-api 的 `MessageRequest`。
/// **默认不含 tools**，工具场景走 [`build_tool_message_request`]。
pub(crate) fn build_chat_message_request(
    req: &CompletionRequest,
    default_model: &str,
) -> MessageRequest {
    let (system_text, messages) = split_system_and_messages(&req.messages);
    let model = req
        .model
        .clone()
        .unwrap_or_else(|| default_model.to_string());
    let system = build_system_prompt(system_text, &model);
    MessageRequest {
        model,
        max_tokens: req.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS),
        messages,
        system,
        tools: None,
        tool_choice: None,
        stream: false,
        temperature: req.temperature.map(f64::from),
        top_p: None,
        frequency_penalty: None,
        presence_penalty: None,
        stop: req.stop_sequences.clone(),
        reasoning_effort: None,
    }
}

/// 把 ironclaw 的 `ToolCompletionRequest` 映射为 claw-code-api 的 `MessageRequest`。
pub(crate) fn build_tool_message_request(
    req: &ToolCompletionRequest,
    default_model: &str,
) -> MessageRequest {
    let (system_text, messages) = split_system_and_messages(&req.messages);
    let tools = if req.tools.is_empty() {
        None
    } else {
        Some(req.tools.iter().map(map_tool_definition).collect())
    };
    let model = req
        .model
        .clone()
        .unwrap_or_else(|| default_model.to_string());
    let system = build_system_prompt(system_text, &model);
    MessageRequest {
        model,
        max_tokens: req.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS),
        messages,
        system,
        tools,
        tool_choice: req.tool_choice.as_deref().and_then(map_tool_choice),
        stream: false,
        temperature: req.temperature.map(f64::from),
        top_p: None,
        frequency_penalty: None,
        presence_penalty: None,
        stop: req.stop_sequences.clone(),
        reasoning_effort: None,
    }
}

const DEFAULT_MAX_TOKENS: u32 = 4096;

fn split_system_and_messages(messages: &[ChatMessage]) -> (Option<String>, Vec<InputMessage>) {
    let mut system_parts: Vec<String> = Vec::new();
    let mut out: Vec<InputMessage> = Vec::with_capacity(messages.len());

    for m in messages {
        match m.role {
            Role::System => {
                // 合并多条 system 消息（用两换行分隔，与 Anthropic 官方 pattern 一致）。
                if !m.content.is_empty() {
                    system_parts.push(m.content.clone());
                }
            }
            Role::User => out.push(map_user_message(m)),
            Role::Assistant => out.push(map_assistant_message(m)),
            Role::Tool => out.push(map_tool_result_message(m)),
        }
    }

    let system = if system_parts.is_empty() {
        None
    } else {
        Some(system_parts.join("\n\n"))
    };
    (system, out)
}

/// Anthropic prompt cache 边界标注（HTML comment 形态，由 [`crate::provider::prompt::LayeredPromptBuilder`] 注入）。
///
/// 与 [`dasclaw_core::PROMPT_CACHE_BOUNDARY`] 保持一致：包裹成 HTML 注释后，
/// 系统提示词中的边界标记不会被任何下游 markdown / 模型行为意外渲染。
const CACHE_BOUNDARY_COMMENT: &str = concat!("<!-- ", "__SYSTEM_PROMPT_DYNAMIC_BOUNDARY__", " -->");

/// 把合并好的 system 文本根据 protocol 投影到合适的 [`SystemPrompt`] 形态。
///
/// - **Anthropic 模型 + 含 [`dasclaw_core::PROMPT_CACHE_BOUNDARY`] 标记**：
///   切成 `[static_prefix(cache_control=ephemeral), dynamic_suffix]` 两个 [`SystemBlock`]，
///   让 Anthropic prompt cache 命中静态前缀（identity + tools + safety）；动态后缀
///   （skills + channel + runtime ctx）每轮变化不进缓存。
/// - **其它情形**：单段 `SystemPrompt::Text`（OpenAI-compat / 无边界标记 / 空文本）。
///
/// 边界检测在请求构造期完成，[`crate::provider::prompt::LayeredPromptBuilder`] 不感知 provider；
/// 这样 ADR-117 R-1 标记从 *惰性* 变 *实际生效*，并保持 builder 与 provider 解耦。
fn build_system_prompt(system_text: Option<String>, model: &str) -> Option<SystemPrompt> {
    let text = system_text?;
    if text.is_empty() {
        return None;
    }

    // 复用 reasoning.rs 中的 Anthropic 检测逻辑（model 名包含 "claude"）。
    let is_anthropic = model.contains("claude");
    if !is_anthropic {
        return Some(SystemPrompt::Text(text));
    }

    // 找到 HTML 注释形态的边界，整体剔除（不保留注释字面量）。
    match text.split_once(CACHE_BOUNDARY_COMMENT) {
        Some((prefix, suffix)) if !prefix.is_empty() => {
            let mut blocks = vec![SystemBlock::text(prefix).with_ephemeral_cache()];
            if !suffix.is_empty() {
                blocks.push(SystemBlock::text(suffix));
            }
            Some(SystemPrompt::Blocks(blocks))
        }
        // 边界不存在（非 layered 路径）或前缀为空（异常）：回退到单段文本。
        _ => Some(SystemPrompt::Text(text)),
    }
}

fn map_user_message(m: &ChatMessage) -> InputMessage {
    let mut content: Vec<InputContentBlock> = Vec::new();
    // 多模态优先；否则退回纯文本。
    if m.content_parts.is_empty() {
        if !m.content.is_empty() {
            content.push(InputContentBlock::Text {
                text: m.content.clone(),
            });
        }
    } else {
        for part in &m.content_parts {
            match part {
                ContentPart::Text { text } => {
                    content.push(InputContentBlock::Text { text: text.clone() })
                }
                ContentPart::ImageUrl { image_url: _ } => {
                    // dasclaw_llm_provider 的 InputContentBlock 目前不直接接受 OpenAI
                    // image_url，交由未来扩展。此处降级为占位文本以免丢失语义。
                    content.push(InputContentBlock::Text {
                        text: "[image]".to_string(),
                    });
                }
            }
        }
    }
    InputMessage {
        role: "user".to_string(),
        content,
    }
}

fn map_assistant_message(m: &ChatMessage) -> InputMessage {
    let mut content: Vec<InputContentBlock> = Vec::new();

    if !m.content.is_empty() {
        content.push(InputContentBlock::Text {
            text: m.content.clone(),
        });
    }

    if let Some(tool_calls) = &m.tool_calls {
        for tc in tool_calls {
            content.push(InputContentBlock::ToolUse {
                id: tc.id.clone(),
                name: tc.name.clone(),
                input: tc.arguments.clone(),
            });
        }
    }

    InputMessage {
        role: "assistant".to_string(),
        content,
    }
}

fn map_tool_result_message(m: &ChatMessage) -> InputMessage {
    // ironclaw 的 Role::Tool 消息要求带 tool_call_id；若缺失则降级为 "unknown" 以防硬崩溃，
    // 调用方应当在构造时就保证 tool_call_id 存在。
    let tool_use_id = m.tool_call_id.clone().unwrap_or_else(|| "unknown".into());
    InputMessage {
        role: "user".to_string(),
        content: vec![InputContentBlock::ToolResult {
            tool_use_id,
            content: vec![ToolResultContentBlock::Text {
                text: m.content.clone(),
            }],
            is_error: false,
        }],
    }
}

fn map_tool_definition(td: &ToolDefinition) -> ApiToolDefinition {
    let description = if td.description.is_empty() {
        None
    } else {
        Some(td.description.clone())
    };
    ApiToolDefinition {
        name: td.name.clone(),
        description,
        input_schema: td.parameters.clone(),
    }
}

fn map_tool_choice(choice: &str) -> Option<ApiToolChoice> {
    match choice {
        "auto" => Some(ApiToolChoice::Auto),
        "required" | "any" => Some(ApiToolChoice::Any),
        "none" => None,
        other => {
            // 特定工具名：`{"type":"tool","name":"..."}` 风格
            Some(ApiToolChoice::Tool {
                name: other.to_string(),
            })
        }
    }
}

// ============================================================================
// 响应映射：dasclaw_llm_provider → ironclaw
// ============================================================================

pub(crate) fn map_message_response(resp: MessageResponse) -> ToolCompletionResponse {
    let mut text = String::new();
    let mut reasoning = String::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();

    for block in resp.content {
        match block {
            OutputContentBlock::Text { text: t } => {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(&t);
            }
            OutputContentBlock::ToolUse { id, name, input } => {
                tool_calls.push(ToolCall {
                    id,
                    name,
                    arguments: input,
                    reasoning: None,
                });
            }
            OutputContentBlock::Thinking { thinking, .. } => {
                if !thinking.is_empty() {
                    if !reasoning.is_empty() {
                        reasoning.push('\n');
                    }
                    reasoning.push_str(&thinking);
                }
            }
            OutputContentBlock::RedactedThinking { .. } => {
                // redacted_thinking 是 Anthropic 的加密 payload，没有可读文本；
                // 保留 drop 策略（下游任何消费者都拿不到可读内容）。
            }
        }
    }

    let finish_reason = map_finish_reason(resp.stop_reason.as_deref());
    let cache_read = resp.usage.cache_read_input_tokens;
    let cache_creation = resp.usage.cache_creation_input_tokens;
    let actual_model = (!resp.model.is_empty()).then(|| resp.model.clone());

    ToolCompletionResponse {
        content: if text.is_empty() { None } else { Some(text) },
        reasoning: if reasoning.is_empty() {
            None
        } else {
            Some(reasoning)
        },
        tool_calls,
        input_tokens: resp.usage.input_tokens,
        output_tokens: resp.usage.output_tokens,
        finish_reason,
        metadata: ResponseMetadata {
            actual_model,
            ..ResponseMetadata::default()
        },
        cache_read_input_tokens: cache_read,
        cache_creation_input_tokens: cache_creation,
    }
}

fn map_finish_reason(reason: Option<&str>) -> FinishReason {
    match reason {
        Some("end_turn") | Some("stop") | Some("stop_sequence") => FinishReason::Stop,
        Some("max_tokens") | Some("length") => FinishReason::Length,
        Some("tool_use") | Some("tool_calls") => FinishReason::ToolUse,
        Some("content_filter") | Some("refusal") => FinishReason::ContentFilter,
        _ => FinishReason::Unknown,
    }
}

struct MessageStreamAccumulator {
    id: String,
    kind: String,
    role: String,
    model: String,
    stop_reason: Option<String>,
    stop_sequence: Option<String>,
    usage: Usage,
    blocks: BTreeMap<u32, OutputContentBlock>,
    active: BTreeMap<u32, ActiveOutputBlock>,
}

impl MessageStreamAccumulator {
    fn new(model: String) -> Self {
        Self {
            id: String::new(),
            kind: "message".to_string(),
            role: "assistant".to_string(),
            model,
            stop_reason: None,
            stop_sequence: None,
            usage: Usage::default(),
            blocks: BTreeMap::new(),
            active: BTreeMap::new(),
        }
    }

    fn ingest(&mut self, event: StreamEvent) -> Result<Vec<LlmStreamEvent>, LlmError> {
        Ok(match event {
            StreamEvent::MessageStart(event) => {
                self.id = event.message.id;
                self.kind = event.message.kind;
                self.role = event.message.role;
                self.model = event.message.model;
                self.usage = event.message.usage;
                self.stop_reason = event.message.stop_reason;
                self.stop_sequence = event.message.stop_sequence;
                for (index, block) in event.message.content.into_iter().enumerate() {
                    self.blocks.insert(index as u32, block);
                }
                Vec::new()
            }
            StreamEvent::MessageDelta(event) => {
                if event.delta.stop_reason.is_some() {
                    self.stop_reason = event.delta.stop_reason;
                }
                if event.delta.stop_sequence.is_some() {
                    self.stop_sequence = event.delta.stop_sequence;
                }
                self.usage = event.usage;
                Vec::new()
            }
            StreamEvent::ContentBlockStart(event) => {
                let deltas = match &event.content_block {
                    OutputContentBlock::Text { text } if !text.is_empty() => {
                        vec![LlmStreamEvent::TextDelta(text.clone())]
                    }
                    OutputContentBlock::Thinking { thinking, .. } if !thinking.is_empty() => {
                        vec![LlmStreamEvent::ReasoningSummaryDelta(thinking.clone())]
                    }
                    _ => Vec::new(),
                };
                self.active
                    .insert(event.index, ActiveOutputBlock::from(event.content_block));
                deltas
            }
            StreamEvent::ContentBlockDelta(event) => {
                let Some(active) = self.active.get_mut(&event.index) else {
                    return Ok(Vec::new());
                };
                match event.delta {
                    ContentBlockDelta::TextDelta { text } => {
                        active.push_text(&text);
                        vec![LlmStreamEvent::TextDelta(text)]
                    }
                    ContentBlockDelta::ThinkingDelta { thinking } => {
                        active.push_thinking(&thinking);
                        vec![LlmStreamEvent::ReasoningSummaryDelta(thinking)]
                    }
                    ContentBlockDelta::InputJsonDelta { partial_json } => {
                        active.push_input_json(&partial_json);
                        if let ActiveOutputBlock::ToolUse { id, name, .. } = active {
                            vec![LlmStreamEvent::ToolCallInputDelta {
                                id: id.clone(),
                                name: Some(name.clone()),
                                delta: partial_json,
                            }]
                        } else {
                            Vec::new()
                        }
                    }
                    ContentBlockDelta::SignatureDelta { signature } => {
                        active.set_signature(signature);
                        Vec::new()
                    }
                }
            }
            StreamEvent::ContentBlockStop(event) => {
                if let Some(active) = self.active.remove(&event.index) {
                    self.blocks
                        .insert(event.index, active.into_output_block(event.index)?);
                }
                Vec::new()
            }
            StreamEvent::MessageStop(_) => Vec::new(),
        })
    }

    fn finish(mut self) -> Result<MessageResponse, LlmError> {
        for (index, block) in std::mem::take(&mut self.active) {
            self.blocks.insert(index, block.into_output_block(index)?);
        }
        Ok(MessageResponse {
            id: self.id,
            kind: self.kind,
            role: self.role,
            content: self.blocks.into_values().collect(),
            model: self.model,
            stop_reason: self.stop_reason,
            stop_sequence: self.stop_sequence,
            usage: self.usage,
            request_id: None,
        })
    }
}

enum ActiveOutputBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input_json: String,
    },
    Thinking {
        thinking: String,
        signature: Option<String>,
    },
    RedactedThinking {
        data: serde_json::Value,
    },
}

impl From<OutputContentBlock> for ActiveOutputBlock {
    fn from(block: OutputContentBlock) -> Self {
        match block {
            OutputContentBlock::Text { text } => Self::Text { text },
            OutputContentBlock::ToolUse { id, name, input } => Self::ToolUse {
                id,
                name,
                input_json: if input == serde_json::json!({}) {
                    String::new()
                } else {
                    input.to_string()
                },
            },
            OutputContentBlock::Thinking {
                thinking,
                signature,
            } => Self::Thinking {
                thinking,
                signature,
            },
            OutputContentBlock::RedactedThinking { data } => Self::RedactedThinking { data },
        }
    }
}

impl ActiveOutputBlock {
    fn push_text(&mut self, delta: &str) {
        if let Self::Text { text } = self {
            text.push_str(delta);
        }
    }

    fn push_thinking(&mut self, delta: &str) {
        if let Self::Thinking { thinking, .. } = self {
            thinking.push_str(delta);
        }
    }

    fn push_input_json(&mut self, delta: &str) {
        if let Self::ToolUse { input_json, .. } = self {
            input_json.push_str(delta);
        }
    }

    fn set_signature(&mut self, value: String) {
        if let Self::Thinking { signature, .. } = self {
            *signature = Some(value);
        }
    }

    fn into_output_block(self, index: u32) -> Result<OutputContentBlock, LlmError> {
        match self {
            Self::Text { text } => Ok(OutputContentBlock::Text { text }),
            Self::ToolUse {
                id,
                name,
                input_json,
            } => {
                let input = if input_json.trim().is_empty() {
                    serde_json::json!({})
                } else {
                    serde_json::from_str(&input_json).map_err(|err| LlmError::InvalidResponse {
                        provider: "dasclaw_llm_provider".to_string(),
                        reason: format!(
                            "invalid streamed tool input JSON at content block {index}: {err}"
                        ),
                    })?
                };
                Ok(OutputContentBlock::ToolUse { id, name, input })
            }
            Self::Thinking {
                thinking,
                signature,
            } => Ok(OutputContentBlock::Thinking {
                thinking,
                signature,
            }),
            Self::RedactedThinking { data } => Ok(OutputContentBlock::RedactedThinking { data }),
        }
    }
}

fn map_api_error(err: ApiError) -> LlmError {
    LlmError::RequestFailed {
        provider: "dasclaw_llm_provider".to_string(),
        reason: err.to_string(),
    }
}

// ============================================================================
// Provider client builders（Step D — 配置迁移）
// ============================================================================

/// 从注册表配置读取 API key 明文；`OAUTH_PLACEHOLDER` 被视为"无 API key"。
fn registry_api_key(config: &RegistryProviderConfig) -> Option<String> {
    config
        .api_key
        .as_ref()
        .map(|k| k.expose_secret().to_string())
        .filter(|k| !k.is_empty() && k != OAUTH_PLACEHOLDER)
}

fn build_anthropic_client(config: &RegistryProviderConfig) -> Result<ProviderClient, LlmError> {
    // OAuth 优先：Anthropic Console 只发 session token 而不给 API key 的场景。
    let auth = match (
        registry_api_key(config),
        config
            .oauth_token
            .as_ref()
            .map(|t| t.expose_secret().to_string()),
    ) {
        (Some(api_key), Some(bearer_token)) => AuthSource::ApiKeyAndBearer {
            api_key,
            bearer_token,
        },
        (Some(api_key), None) => AuthSource::ApiKey(api_key),
        (None, Some(bearer_token)) => AuthSource::BearerToken(bearer_token),
        (None, None) => {
            return Err(LlmError::AuthFailed {
                provider: config.provider_id.clone(),
            });
        }
    };

    let client = AnthropicClient::with_auth(auth)
        .map_err(map_api_error)?
        .with_base_url(config.base_url.clone());
    Ok(ProviderClient::Anthropic(client))
}

/// 根据模型名自动选 DashScope / xAI / OpenAI 预设；然后 override base_url。
///
/// 仅基于模型名做静态映射，不读取环境（`EnvSnapshot::default()` 即可），
/// 因为 ironclaw 的鉴权一律来自 `RegistryProviderConfig` 的显式配置。
fn pick_openai_compat_config(model: &str) -> (OpenAiCompatConfig, ProviderVariant) {
    use crate::{EnvSnapshot, ProviderKind, detect_provider_kind, resolve_model_alias};
    let resolved = resolve_model_alias(model);
    match detect_provider_kind(&resolved, &EnvSnapshot::default()) {
        ProviderKind::Xai => (OpenAiCompatConfig::xai(), ProviderVariant::Xai),
        ProviderKind::OpenAi => {
            // 根据模型 metadata 进一步区分 DashScope vs 通用 OpenAI-compat
            // （Groq/Kimi/OpenRouter/Tinfoil 都用 openai() 预设 + 自定义 base_url）
            (OpenAiCompatConfig::openai(), ProviderVariant::OpenAi)
        }
        // Anthropic 不该走到这里；兜底 openai 预设以避免 panic，错误由上层抛出。
        ProviderKind::Anthropic => (OpenAiCompatConfig::openai(), ProviderVariant::OpenAi),
    }
}

/// 区分 `ProviderClient::Xai` vs `ProviderClient::OpenAi` 枚举分支。
enum ProviderVariant {
    Xai,
    OpenAi,
}

fn build_openai_compat_client(config: &RegistryProviderConfig) -> Result<ProviderClient, LlmError> {
    let api_key = registry_api_key(config).ok_or_else(|| LlmError::AuthFailed {
        provider: config.provider_id.clone(),
    })?;

    let (compat_config, variant) = pick_openai_compat_config(&config.model);
    let client = OpenAiCompatClient::new(api_key, compat_config)
        .map_err(map_api_error)?
        .with_base_url(config.base_url.clone());
    Ok(match variant {
        ProviderVariant::Xai => ProviderClient::Xai(client),
        ProviderVariant::OpenAi => ProviderClient::OpenAi(client),
    })
}

fn build_ollama_client(config: &RegistryProviderConfig) -> Result<ProviderClient, LlmError> {
    // Ollama 不需要 API key；如果用户填了 key（如反向代理鉴权），也传进去。
    let api_key = registry_api_key(config).unwrap_or_default();
    let client = OpenAiCompatClient::new(api_key, OpenAiCompatConfig::openai())
        .map_err(map_api_error)?
        .with_base_url(config.base_url.clone());
    Ok(ProviderClient::OpenAi(client))
}

// ============================================================================
// LlmProvider trait 实现
// ============================================================================

#[async_trait]
impl LlmProvider for ClawCodeLlmProvider {
    fn model_name(&self) -> &str {
        &self.configured_model
    }

    fn cost_per_token(&self) -> (Decimal, Decimal) {
        self.cost_rates.unwrap_or((Decimal::ZERO, Decimal::ZERO))
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        let msg_req = build_chat_message_request(&request, &self.resolved_model);
        let resp = self
            .client
            .send_message(&msg_req)
            .await
            .map_err(map_api_error)?;
        let tool_resp = map_message_response(resp);

        // complete() 不支持工具；如果 LLM 意外返回工具调用，记作 Unknown finish。
        let finish_reason = if tool_resp.tool_calls.is_empty() {
            tool_resp.finish_reason
        } else {
            FinishReason::Unknown
        };

        Ok(CompletionResponse {
            content: tool_resp.content.unwrap_or_default(),
            input_tokens: tool_resp.input_tokens,
            output_tokens: tool_resp.output_tokens,
            finish_reason,
            cache_read_input_tokens: tool_resp.cache_read_input_tokens,
            cache_creation_input_tokens: tool_resp.cache_creation_input_tokens,
        })
    }

    async fn complete_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse, LlmError> {
        let msg_req = build_tool_message_request(&request, &self.resolved_model);
        let resp = self
            .client
            .send_message(&msg_req)
            .await
            .map_err(map_api_error)?;
        Ok(map_message_response(resp))
    }

    async fn stream_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<LlmStream, LlmError> {
        let msg_req = build_tool_message_request(&request, &self.resolved_model);
        let mut provider_stream = self
            .client
            .stream_message(&msg_req)
            .await
            .map_err(map_api_error)?;
        let model = msg_req.model.clone();
        let (event_tx, event_rx) = mpsc::channel::<Result<LlmStreamEvent, LlmError>>(64);

        tokio::spawn(async move {
            let mut accumulator = MessageStreamAccumulator::new(model);
            loop {
                match provider_stream.next_event().await {
                    Ok(Some(event)) => match accumulator.ingest(event) {
                        Ok(events) => {
                            for event in events {
                                if event_tx.send(Ok(event)).await.is_err() {
                                    return;
                                }
                            }
                        }
                        Err(error) => {
                            let _ = event_tx.send(Err(error)).await;
                            return;
                        }
                    },
                    Ok(None) => {
                        match accumulator.finish() {
                            Ok(message) => {
                                let completed = map_message_response(message);
                                let _ = event_tx
                                    .send(Ok(LlmStreamEvent::Completed(completed)))
                                    .await;
                            }
                            Err(error) => {
                                let _ = event_tx.send(Err(error)).await;
                            }
                        }
                        return;
                    }
                    Err(error) => {
                        let _ = event_tx.send(Err(map_api_error(error))).await;
                        return;
                    }
                }
            }
        });

        Ok(LlmStream::new(event_rx))
    }

    fn capabilities(&self) -> LlmProviderCapabilities {
        LlmProviderCapabilities {
            native_streaming: true,
        }
    }

    async fn list_models(&self) -> Result<Vec<String>, LlmError> {
        // claw-code-api 暂未提供运行时列表接口；返回当前已配置模型作为最小保证。
        Ok(vec![self.configured_model.clone()])
    }

    fn effective_model_name(&self, requested_model: Option<&str>) -> String {
        requested_model
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| self.configured_model.clone())
    }

    fn active_model_name(&self) -> String {
        self.configured_model.clone()
    }
}

// ============================================================================
// 测试：消息映射 / 响应映射（纯函数，不需要网络）
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Usage;
    use serde_json::json;

    fn mk_user(text: &str) -> ChatMessage {
        ChatMessage::user(text)
    }

    fn mk_assistant(text: &str) -> ChatMessage {
        ChatMessage {
            role: Role::Assistant,
            content: text.to_string(),
            content_parts: Vec::new(),
            tool_call_id: None,
            name: None,
            tool_calls: None,
            tool_error: None,
            usage: None,
        }
    }

    // -------- split_system_and_messages --------

    #[test]
    fn test_split_system_single_system_message() {
        let msgs = vec![ChatMessage::system("you are helpful"), mk_user("hi")];
        let (sys, out) = split_system_and_messages(&msgs);
        assert_eq!(sys.as_deref(), Some("you are helpful"));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].role, "user");
    }

    #[test]
    fn test_split_system_multiple_system_messages_joined() {
        let msgs = vec![
            ChatMessage::system("rule A"),
            ChatMessage::system("rule B"),
            mk_user("hi"),
        ];
        let (sys, _) = split_system_and_messages(&msgs);
        assert_eq!(sys.as_deref(), Some("rule A\n\nrule B"));
    }

    #[test]
    fn test_split_system_empty_system_is_dropped() {
        let msgs = vec![ChatMessage::system(""), mk_user("hi")];
        let (sys, _) = split_system_and_messages(&msgs);
        assert!(sys.is_none());
    }

    #[test]
    fn test_split_preserves_conversation_order() {
        let msgs = vec![mk_user("q1"), mk_assistant("a1"), mk_user("q2")];
        let (_, out) = split_system_and_messages(&msgs);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].role, "user");
        assert_eq!(out[1].role, "assistant");
        assert_eq!(out[2].role, "user");
    }

    // -------- user message mapping --------

    #[test]
    fn test_user_plain_text() {
        let im = map_user_message(&mk_user("hello"));
        assert_eq!(im.role, "user");
        assert_eq!(im.content.len(), 1);
        match &im.content[0] {
            InputContentBlock::Text { text } => assert_eq!(text, "hello"),
            _ => panic!("expected text block"),
        }
    }

    #[test]
    fn test_user_multimodal_text_part() {
        let m = ChatMessage {
            role: Role::User,
            content: String::new(),
            content_parts: vec![
                ContentPart::Text { text: "a".into() },
                ContentPart::Text { text: "b".into() },
            ],
            tool_call_id: None,
            name: None,
            tool_calls: None,
            tool_error: None,
            usage: None,
        };
        let im = map_user_message(&m);
        assert_eq!(im.content.len(), 2);
    }

    // -------- assistant message with tool_calls --------

    #[test]
    fn test_assistant_with_tool_calls() {
        let m = ChatMessage {
            role: Role::Assistant,
            content: "thinking...".into(),
            content_parts: Vec::new(),
            tool_call_id: None,
            name: None,
            tool_calls: Some(vec![ToolCall {
                id: "tc-1".into(),
                name: "shell".into(),
                arguments: json!({"cmd": "ls"}),
                reasoning: None,
            }]),
            tool_error: None,
            usage: None,
        };
        let im = map_assistant_message(&m);
        assert_eq!(im.content.len(), 2);
        match &im.content[0] {
            InputContentBlock::Text { text } => assert_eq!(text, "thinking..."),
            _ => panic!("expected text first"),
        }
        match &im.content[1] {
            InputContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "tc-1");
                assert_eq!(name, "shell");
                assert_eq!(input["cmd"], "ls");
            }
            _ => panic!("expected tool_use"),
        }
    }

    #[test]
    fn test_assistant_tool_calls_only_no_text() {
        let m = ChatMessage {
            role: Role::Assistant,
            content: String::new(),
            content_parts: Vec::new(),
            tool_call_id: None,
            name: None,
            tool_calls: Some(vec![ToolCall {
                id: "tc-2".into(),
                name: "lookup".into(),
                arguments: json!({"q": "rust"}),
                reasoning: None,
            }]),
            tool_error: None,
            usage: None,
        };
        let im = map_assistant_message(&m);
        assert_eq!(im.content.len(), 1);
        assert!(matches!(im.content[0], InputContentBlock::ToolUse { .. }));
    }

    // -------- tool result mapping --------

    #[test]
    fn test_tool_result_wraps_into_user_message() {
        let m = ChatMessage::tool_result("tc-1", "shell", "ok");
        let im = map_tool_result_message(&m);
        assert_eq!(im.role, "user"); // Anthropic 协议：tool_result 是 user role
        match &im.content[0] {
            InputContentBlock::ToolResult {
                tool_use_id,
                is_error,
                ..
            } => {
                assert_eq!(tool_use_id, "tc-1");
                assert!(!is_error);
            }
            _ => panic!("expected tool_result"),
        }
    }

    // -------- tool definition --------

    #[test]
    fn test_tool_definition_mapping() {
        let td = ToolDefinition {
            name: "shell".into(),
            description: "run a shell command".into(),
            parameters: json!({"type": "object"}),
        };
        let api_td = map_tool_definition(&td);
        assert_eq!(api_td.name, "shell");
        assert_eq!(api_td.description.as_deref(), Some("run a shell command"));
        assert_eq!(api_td.input_schema, json!({"type": "object"}));
    }

    #[test]
    fn test_tool_definition_empty_description_becomes_none() {
        let td = ToolDefinition {
            name: "shell".into(),
            description: String::new(),
            parameters: json!({}),
        };
        let api_td = map_tool_definition(&td);
        assert!(api_td.description.is_none());
    }

    // -------- tool_choice mapping --------

    #[test]
    fn test_tool_choice_mapping() {
        assert!(matches!(map_tool_choice("auto"), Some(ApiToolChoice::Auto)));
        assert!(matches!(
            map_tool_choice("required"),
            Some(ApiToolChoice::Any)
        ));
        assert!(matches!(map_tool_choice("any"), Some(ApiToolChoice::Any)));
        assert!(map_tool_choice("none").is_none());
        match map_tool_choice("shell") {
            Some(ApiToolChoice::Tool { name }) => assert_eq!(name, "shell"),
            other => panic!("expected specific tool, got {other:?}"),
        }
    }

    // -------- finish reason --------

    #[test]
    fn test_finish_reason_mapping() {
        assert_eq!(map_finish_reason(Some("end_turn")), FinishReason::Stop);
        assert_eq!(map_finish_reason(Some("stop")), FinishReason::Stop);
        assert_eq!(map_finish_reason(Some("max_tokens")), FinishReason::Length);
        assert_eq!(map_finish_reason(Some("tool_use")), FinishReason::ToolUse);
        assert_eq!(map_finish_reason(Some("tool_calls")), FinishReason::ToolUse);
        assert_eq!(
            map_finish_reason(Some("content_filter")),
            FinishReason::ContentFilter
        );
        assert_eq!(map_finish_reason(None), FinishReason::Unknown);
        assert_eq!(map_finish_reason(Some("wat")), FinishReason::Unknown);
    }

    // -------- response mapping --------

    fn mk_resp(
        content: Vec<OutputContentBlock>,
        stop_reason: Option<&str>,
        usage: Usage,
    ) -> MessageResponse {
        MessageResponse {
            id: "m-test".into(),
            kind: "message".into(),
            role: "assistant".into(),
            content,
            model: "claude-sonnet".into(),
            stop_reason: stop_reason.map(|s| s.to_string()),
            stop_sequence: None,
            usage,
            request_id: None,
        }
    }

    #[test]
    fn test_response_text_only() {
        let resp = mk_resp(
            vec![OutputContentBlock::Text { text: "hi".into() }],
            Some("end_turn"),
            Usage {
                input_tokens: 5,
                output_tokens: 2,
                ..Usage::default()
            },
        );
        let out = map_message_response(resp);
        assert_eq!(out.content.as_deref(), Some("hi"));
        assert!(out.tool_calls.is_empty());
        assert_eq!(out.finish_reason, FinishReason::Stop);
        assert_eq!(out.input_tokens, 5);
        assert_eq!(out.output_tokens, 2);
    }

    #[test]
    fn test_response_tool_use_and_text_interleaved() {
        let resp = mk_resp(
            vec![
                OutputContentBlock::Text {
                    text: "I'll run it".into(),
                },
                OutputContentBlock::ToolUse {
                    id: "tc-x".into(),
                    name: "shell".into(),
                    input: json!({"cmd": "pwd"}),
                },
            ],
            Some("tool_use"),
            Usage::default(),
        );
        let out = map_message_response(resp);
        assert_eq!(out.content.as_deref(), Some("I'll run it"));
        assert_eq!(out.tool_calls.len(), 1);
        assert_eq!(out.tool_calls[0].id, "tc-x");
        assert_eq!(out.tool_calls[0].name, "shell");
        assert_eq!(out.tool_calls[0].arguments["cmd"], "pwd");
        assert_eq!(out.finish_reason, FinishReason::ToolUse);
    }

    #[test]
    fn test_stream_accumulator_rejects_malformed_tool_input_json() {
        let mut accumulator = MessageStreamAccumulator::new("claude-sonnet".to_string());
        accumulator
            .ingest(StreamEvent::ContentBlockStart(
                crate::ContentBlockStartEvent {
                    index: 0,
                    content_block: OutputContentBlock::ToolUse {
                        id: "tc-x".into(),
                        name: "shell".into(),
                        input: json!({}),
                    },
                },
            ))
            .expect("tool block start should ingest");
        let deltas = accumulator
            .ingest(StreamEvent::ContentBlockDelta(
                crate::ContentBlockDeltaEvent {
                    index: 0,
                    delta: ContentBlockDelta::InputJsonDelta {
                        partial_json: r#"{"cmd""#.into(),
                    },
                },
            ))
            .expect("partial JSON should still stream as delta");
        assert!(matches!(
            deltas.as_slice(),
            [LlmStreamEvent::ToolCallInputDelta { id, name: Some(name), delta }]
                if id == "tc-x" && name == "shell" && delta == r#"{"cmd""#
        ));

        let error = accumulator
            .ingest(StreamEvent::ContentBlockStop(
                crate::ContentBlockStopEvent { index: 0 },
            ))
            .expect_err("malformed streamed tool input JSON must fail closed");
        assert!(matches!(
            error,
            LlmError::InvalidResponse { provider, reason }
                if provider == "dasclaw_llm_provider"
                    && reason.contains("invalid streamed tool input JSON")
        ));
    }

    #[test]
    fn test_response_cache_tokens_propagated() {
        let resp = mk_resp(
            vec![OutputContentBlock::Text { text: "x".into() }],
            Some("end_turn"),
            Usage {
                input_tokens: 100,
                output_tokens: 10,
                cache_read_input_tokens: 80,
                cache_creation_input_tokens: 15,
            },
        );
        let out = map_message_response(resp);
        assert_eq!(out.cache_read_input_tokens, 80);
        assert_eq!(out.cache_creation_input_tokens, 15);
    }

    #[test]
    fn test_response_thinking_block_is_structured_outside_text() {
        let resp = mk_resp(
            vec![
                OutputContentBlock::Thinking {
                    thinking: "user wants weather; I should call the tool".into(),
                    signature: None,
                },
                OutputContentBlock::Text {
                    text: "Let me check the weather.".into(),
                },
            ],
            Some("end_turn"),
            Usage::default(),
        );
        let out = map_message_response(resp);
        assert_eq!(
            out.reasoning.as_deref(),
            Some("user wants weather; I should call the tool")
        );
        assert_eq!(out.content.as_deref(), Some("Let me check the weather."));
    }

    #[test]
    fn test_response_thinking_only_stays_out_of_content() {
        let resp = mk_resp(
            vec![OutputContentBlock::Thinking {
                thinking: "reasoning in progress".into(),
                signature: None,
            }],
            Some("end_turn"),
            Usage::default(),
        );
        let out = map_message_response(resp);
        assert_eq!(out.reasoning.as_deref(), Some("reasoning in progress"));
        assert_eq!(out.content.as_deref(), None);
    }

    /// 空 Thinking block 不应该生成空 `<think></think>`（会污染下游正则）。
    #[test]
    fn test_response_empty_thinking_does_not_pollute_content() {
        let resp = mk_resp(
            vec![
                OutputContentBlock::Thinking {
                    thinking: String::new(),
                    signature: None,
                },
                OutputContentBlock::Text {
                    text: "hello".into(),
                },
            ],
            Some("end_turn"),
            Usage::default(),
        );
        let out = map_message_response(resp);
        assert_eq!(out.content.as_deref(), Some("hello"));
    }

    // -------- chat request build --------

    #[test]
    fn test_chat_request_uses_override_model() {
        let req = CompletionRequest::new(vec![mk_user("hi")])
            .with_model("gpt-4o")
            .with_max_tokens(100)
            .with_temperature(0.2);
        let built = build_chat_message_request(&req, "sonnet");
        assert_eq!(built.model, "gpt-4o");
        assert_eq!(built.max_tokens, 100);
        // f32 → f64 会带精度误差，用 epsilon 比较
        let t = built.temperature.expect("temperature set");
        assert!((t - 0.2).abs() < 1e-6, "temperature = {t}");
        assert!(built.tools.is_none());
        assert_eq!(built.messages.len(), 1);
    }

    #[test]
    fn test_chat_request_falls_back_to_default_model() {
        let req = CompletionRequest::new(vec![mk_user("hi")]);
        let built = build_chat_message_request(&req, "sonnet");
        assert_eq!(built.model, "sonnet");
        assert_eq!(built.max_tokens, DEFAULT_MAX_TOKENS);
    }

    // -------- tool request build --------

    #[test]
    fn test_tool_request_includes_tools_and_choice() {
        let req = ToolCompletionRequest::new(
            vec![mk_user("go")],
            vec![ToolDefinition {
                name: "shell".into(),
                description: "".into(),
                parameters: json!({}),
            }],
        )
        .with_tool_choice("required")
        .with_model("qwen-plus");
        let built = build_tool_message_request(&req, "sonnet");
        assert_eq!(built.model, "qwen-plus");
        assert_eq!(built.tools.as_ref().unwrap().len(), 1);
        assert!(matches!(built.tool_choice, Some(ApiToolChoice::Any)));
    }

    #[test]
    fn test_tool_request_empty_tools_becomes_none() {
        let req = ToolCompletionRequest::new(vec![mk_user("hi")], vec![]);
        let built = build_tool_message_request(&req, "sonnet");
        assert!(built.tools.is_none());
    }

    // -------- full round-trip: build → response → map --------

    #[test]
    fn test_round_trip_tool_call_history() {
        // assistant 上一轮要求调 shell；当前轮把结果传回
        let msgs = vec![
            ChatMessage::system("be brief"),
            mk_user("list root"),
            ChatMessage {
                role: Role::Assistant,
                content: String::new(),
                content_parts: Vec::new(),
                tool_call_id: None,
                name: None,
                tool_calls: Some(vec![ToolCall {
                    id: "call-1".into(),
                    name: "shell".into(),
                    arguments: json!({"cmd": "ls /"}),
                    reasoning: None,
                }]),
                tool_error: None,
                usage: None,
            },
            ChatMessage::tool_result("call-1", "shell", "bin\netc\nusr"),
        ];
        let req = CompletionRequest::new(msgs);
        let built = build_chat_message_request(&req, "claude-sonnet-4-6");

        assert_eq!(
            built.system.as_ref().map(|s| s.as_text()).as_deref(),
            Some("be brief")
        );
        // user, assistant(tool_use), user(tool_result)
        assert_eq!(built.messages.len(), 3);
        assert_eq!(built.messages[0].role, "user");
        assert_eq!(built.messages[1].role, "assistant");
        assert!(matches!(
            built.messages[1].content[0],
            InputContentBlock::ToolUse { .. }
        ));
        assert_eq!(built.messages[2].role, "user");
        assert!(matches!(
            built.messages[2].content[0],
            InputContentBlock::ToolResult { .. }
        ));
    }

    // ========================================================================
    // Step D — 配置迁移测试
    //
    // 目标：验证 x-claw 现有 `RegistryProviderConfig` 能无损映射到
    // `ClawCodeLlmProvider`，覆盖每一种 `ProviderProtocol` 分支。
    // ========================================================================

    use crate::ProviderKind;
    use crate::provider::config::{CacheRetention, RegistryProviderConfig};
    use secrecy::SecretString;

    fn mk_config(
        protocol: ProviderProtocol,
        provider_id: &str,
        model: &str,
        base_url: &str,
        api_key: Option<&str>,
        oauth_token: Option<&str>,
    ) -> RegistryProviderConfig {
        RegistryProviderConfig {
            protocol,
            provider_id: provider_id.to_string(),
            api_key: api_key.map(|k| SecretString::from(k.to_string())),
            base_url: base_url.to_string(),
            model: model.to_string(),
            extra_headers: Vec::new(),
            oauth_token: oauth_token.map(|t| SecretString::from(t.to_string())),
            is_codex_chatgpt: false,
            refresh_token: None,
            auth_path: None,
            cache_retention: CacheRetention::None,
            unsupported_params: Vec::new(),
            strict_tools_schema: true,
        }
    }

    #[test]
    fn test_migrate_anthropic_api_key_config() {
        let cfg = mk_config(
            ProviderProtocol::Anthropic,
            "anthropic",
            "claude-sonnet-4-6",
            "https://api.anthropic.com",
            Some("sk-ant-real"),
            None,
        );
        let p = ClawCodeLlmProvider::from_registry_config(&cfg).expect("build");
        assert_eq!(p.model_name(), "claude-sonnet-4-6");
        assert_eq!(p.client_for_test().provider_kind(), ProviderKind::Anthropic);
    }

    #[test]
    fn test_migrate_anthropic_oauth_only_config() {
        // 与 `claude login` 场景对齐：只有 OAuth bearer token。
        let cfg = mk_config(
            ProviderProtocol::Anthropic,
            "anthropic-oauth",
            "claude-opus-4-6",
            "https://api.anthropic.com",
            Some(OAUTH_PLACEHOLDER),
            Some("oauth-bearer-token"),
        );
        let p = ClawCodeLlmProvider::from_registry_config(&cfg).expect("build");
        assert_eq!(p.resolved_model, "claude-opus-4-6");
        assert_eq!(p.client_for_test().provider_kind(), ProviderKind::Anthropic);
    }

    #[test]
    fn test_migrate_anthropic_rejects_missing_credentials() {
        let cfg = mk_config(
            ProviderProtocol::Anthropic,
            "anthropic-bad",
            "claude-sonnet-4-6",
            "https://api.anthropic.com",
            None,
            None,
        );
        let err = ClawCodeLlmProvider::from_registry_config(&cfg).unwrap_err();
        assert!(
            matches!(&err, LlmError::AuthFailed { provider } if provider == "anthropic-bad"),
            "unexpected err: {err:?}"
        );
    }

    #[test]
    fn test_migrate_openai_config() {
        let cfg = mk_config(
            ProviderProtocol::OpenAiCompletions,
            "openai",
            "gpt-4o",
            "https://api.openai.com/v1",
            Some("sk-openai"),
            None,
        );
        let p = ClawCodeLlmProvider::from_registry_config(&cfg).expect("build");
        assert_eq!(p.client_for_test().provider_kind(), ProviderKind::OpenAi);
    }

    #[test]
    fn test_migrate_dashscope_qwen_config() {
        // DashScope (qwen 系列) 走 OpenAI-compat 协议但 ProviderKind 仍是 OpenAi；
        // 关键是 base_url 能正确传进去。
        let cfg = mk_config(
            ProviderProtocol::OpenAiCompletions,
            "dashscope",
            "qwen-plus-2025-07-28",
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
            Some("sk-dashscope"),
            None,
        );
        let p = ClawCodeLlmProvider::from_registry_config(&cfg).expect("build");
        assert_eq!(p.client_for_test().provider_kind(), ProviderKind::OpenAi);
    }

    #[test]
    fn test_migrate_xai_config() {
        let cfg = mk_config(
            ProviderProtocol::OpenAiCompletions,
            "xai",
            "grok-3",
            "https://api.x.ai/v1",
            Some("xai-key"),
            None,
        );
        let p = ClawCodeLlmProvider::from_registry_config(&cfg).expect("build");
        assert_eq!(p.client_for_test().provider_kind(), ProviderKind::Xai);
    }

    #[test]
    fn test_migrate_kimi_generic_compat_config() {
        // Kimi 没有 detect_provider_kind 专门支持，走 OpenAI 通用分支；
        // 关键 base_url override 生效。
        let cfg = mk_config(
            ProviderProtocol::OpenAiCompletions,
            "kimi",
            "kimi-k1.5",
            "https://api.moonshot.cn/v1",
            Some("sk-kimi"),
            None,
        );
        let p = ClawCodeLlmProvider::from_registry_config(&cfg).expect("build");
        assert_eq!(p.client_for_test().provider_kind(), ProviderKind::OpenAi);
    }

    #[test]
    fn test_migrate_openai_compat_requires_api_key() {
        let cfg = mk_config(
            ProviderProtocol::OpenAiCompletions,
            "groq",
            "llama-3-70b",
            "https://api.groq.com/openai/v1",
            None,
            None,
        );
        let err = ClawCodeLlmProvider::from_registry_config(&cfg).unwrap_err();
        assert!(
            matches!(&err, LlmError::AuthFailed { provider } if provider == "groq"),
            "unexpected err: {err:?}"
        );
    }

    #[test]
    fn test_migrate_ollama_no_api_key_allowed() {
        let cfg = mk_config(
            ProviderProtocol::Ollama,
            "ollama",
            "llama-3",
            "http://localhost:11434/v1",
            None,
            None,
        );
        let p = ClawCodeLlmProvider::from_registry_config(&cfg).expect("build");
        assert_eq!(p.client_for_test().provider_kind(), ProviderKind::OpenAi);
    }

    #[test]
    fn test_migrate_github_copilot_rejected_with_guidance() {
        let cfg = mk_config(
            ProviderProtocol::GithubCopilot,
            "github",
            "gpt-4o",
            "https://api.githubcopilot.com",
            Some("ghp-token"),
            None,
        );
        let err = ClawCodeLlmProvider::from_registry_config(&cfg).unwrap_err();
        match err {
            LlmError::RequestFailed { provider, reason } => {
                assert_eq!(provider, "github");
                assert!(
                    reason.contains("GithubCopilotProvider"),
                    "error should redirect caller to GithubCopilotProvider: {reason}"
                );
            }
            other => panic!("unexpected err: {other:?}"),
        }
    }

    #[test]
    fn test_migrate_oauth_placeholder_not_treated_as_real_key() {
        // OAUTH_PLACEHOLDER 是虚假值，即便填进 api_key 也要被 registry_api_key 过滤掉。
        let cfg = mk_config(
            ProviderProtocol::Anthropic,
            "anthropic-placeholder-only",
            "claude-sonnet-4-6",
            "https://api.anthropic.com",
            Some(OAUTH_PLACEHOLDER),
            None,
        );
        let err = ClawCodeLlmProvider::from_registry_config(&cfg).unwrap_err();
        assert!(
            matches!(err, LlmError::AuthFailed { .. }),
            "placeholder-only config should fail auth: {err:?}"
        );
    }

    // ========================================================================
    // Step H — DLP/安全回归
    //
    // ClawCodeLlmProvider 是 LLM 传输层，SafetyLayer/Sanitizer/LeakDetector 挂
    // 在它上游（dispatcher/agent_loop/routine_engine）。Phase 2 Step I 完成后，
    // 本模块只要保证"已 sanitize 的 bytes 到 claw-code-api 请求体仍是同一
    // 份 bytes"即可；任何引入伪造前缀/改写内容的 bug 都会被这里的测试捕获。
    //
    // 原则（AGENTS.md 历史教训）：
    //   • 测试断言从业务目标出发（DLP 链路端到端不被破坏）
    //   • 失败路径优先（错误消息不含 key）
    //   • LeakDetector 作为"第二把锁"验证 mapper 无副作用
    // ========================================================================

    /// safety sanitizer 产生的 `[REDACTED:*]` 标记经 user message 映射后必须字节
    /// 不变——否则上游 DLP 策略会被下游悄悄抹掉。
    #[test]
    fn test_audit_sanitized_user_text_is_byte_identical_after_mapping() {
        let redacted = "login=bob password=[REDACTED:PASSWORD] token=[REDACTED:BEARER]";
        let im = map_user_message(&mk_user(redacted));
        assert_eq!(im.role, "user");
        match &im.content[0] {
            InputContentBlock::Text { text } => assert_eq!(text, redacted),
            other => panic!("expected Text block, got {other:?}"),
        }
    }

    /// tool_result 是 prompt-injection 最容易注入的入口：dispatcher 走完
    /// `SafetyLayer::wrap_for_llm` 产生的包裹标记必须原样出现在请求体里，
    /// 否则 LLM 端看到的上下文会和 safety 策略预期不一致。
    #[test]
    fn test_audit_wrapped_tool_result_markers_preserved() {
        // 模拟 SafetyLayer::wrap_for_llm 典型输出（标记前缀/后缀是安全契约的一部分）
        let wrapped = "<<<UNTRUSTED-TOOL-OUTPUT tool=shell>>>\n\
                       total 0\ndrwxr-xr-x [REDACTED:USER] staff\n\
                       <<<END-UNTRUSTED-TOOL-OUTPUT>>>";
        let msg = ChatMessage::tool_result("tc-42", "shell", wrapped);
        let im = map_tool_result_message(&msg);
        assert_eq!(im.role, "user");
        match &im.content[0] {
            InputContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                assert_eq!(tool_use_id, "tc-42");
                assert!(
                    !is_error,
                    "safety-wrapped success output must not flip to is_error=true"
                );
                assert_eq!(content.len(), 1);
                match &content[0] {
                    ToolResultContentBlock::Text { text } => assert_eq!(text, wrapped),
                    other => panic!("expected Text inside tool_result, got {other:?}"),
                }
            }
            other => panic!("expected ToolResult block, got {other:?}"),
        }
    }

    /// assistant 消息里若包含 safety 层产生的 injection 警告（例如 LLM 自己复述
    /// 了一段可疑输入），mapper 不得对内容做静默改写。
    #[test]
    fn test_audit_assistant_injection_warning_marker_preserved() {
        let text = "⚠️ [PROMPT-INJECTION DETECTED] ignoring instruction 'delete all'";
        let im = map_assistant_message(&mk_assistant(text));
        assert_eq!(im.role, "assistant");
        assert_eq!(im.content.len(), 1);
        match &im.content[0] {
            InputContentBlock::Text { text: t } => assert_eq!(t, text),
            other => panic!("expected Text block, got {other:?}"),
        }
    }

    /// 映射过程**不得**引入任何像 API-key 前缀的伪造字节（极端场景：mapper
    /// 错误地拼接了一些 tracing 上下文字符串）。用 LeakDetector 当第二把锁
    /// 验证干净输入 → 干净输出。
    #[test]
    fn test_audit_leak_detector_clean_on_mapped_safe_content() {
        use dasclaw_safety::LeakDetector;

        let clean_user = "帮我查一下东京的天气";
        let clean_tool = "<<<UNTRUSTED-TOOL-OUTPUT tool=weather>>>sunny 22C<<<END>>>";

        let user_im = map_user_message(&mk_user(clean_user));
        let tool_im =
            map_tool_result_message(&ChatMessage::tool_result("tc-1", "weather", clean_tool));

        // 把映射结果序列化回字符串（这就是实际会塞进 HTTP body 的形状）
        let user_json = serde_json::to_string(&user_im).expect("user im serializable");
        let tool_json = serde_json::to_string(&tool_im).expect("tool im serializable");

        let detector = LeakDetector::new();
        let user_scan = detector.scan(&user_json);
        let tool_scan = detector.scan(&tool_json);
        assert!(
            user_scan.is_clean(),
            "mapper must not fabricate leak-like bytes for clean user text; matches={:?}",
            user_scan.matches
        );
        assert!(
            tool_scan.is_clean(),
            "mapper must not fabricate leak-like bytes for clean tool output; matches={:?}",
            tool_scan.matches
        );
    }

    /// 固化 Step G 的真实观察：DashScope 在 401 时只回 "Incorrect API key
    /// provided"（不含原始 key）。把这段错误字符串喂给 LeakDetector 必须 clean，
    /// 否则说明上游把 key 回显进了错误消息——这是 Fail-Open 泄露。
    #[test]
    fn test_audit_observed_qwen_401_error_does_not_leak_key() {
        use dasclaw_safety::LeakDetector;

        // Step G 运行时真实抓到的错误字符串（见 test_qwen_invalid_api_key_returns_clear_error）
        let observed = "Provider claw-code-api request failed: api returned 401 Unauthorized \
                        (invalid_request_error) [trace 946cc7c5-8062-982f-8645-93a681ce631e]: \
                        Incorrect API key provided. For details, see: \
                        https://help.aliyun.com/zh/model-studio/error-code#apikey-error";

        let detector = LeakDetector::new();
        let scan = detector.scan(observed);
        assert!(
            scan.is_clean(),
            "observed 401 error must not contain any leak-like pattern; matches={:?}",
            scan.matches
        );
        // 双重保险：显式确认常见 key 前缀不出现
        assert!(
            !observed.contains("sk-"),
            "observed error must not echo sk-* keys"
        );
        assert!(
            !observed.contains("Bearer "),
            "observed error must not echo Bearer tokens"
        );
    }

    // -------- Anthropic prompt cache 边界切分 (issue #139, ADR-117 R-1) --------

    /// 边界常量必须与 dasclaw_core 源真理一致（HTML comment 包装版本）。
    #[test]
    fn cache_boundary_comment_matches_agent_constant() {
        let expected = format!("<!-- {} -->", dasclaw_core::PROMPT_CACHE_BOUNDARY);
        assert_eq!(CACHE_BOUNDARY_COMMENT, expected);
    }

    /// req_llm_anthropic_cache_001：Anthropic 模型 + system 含 prompt cache 边界标记
    /// → 切成 2 个 SystemBlock，静态前缀挂 `cache_control: {"type":"ephemeral"}`，
    ///   动态后缀不挂 cache_control。
    #[test]
    fn req_llm_anthropic_cache_001_boundary_splits_into_two_blocks() {
        let static_prefix = "You are an AI assistant.\n\nTools:\n- shell\n- read";
        let dynamic_suffix = "\n\nCurrent time: 2025-01-15T10:00:00Z";
        let system_text = format!(
            "{static_prefix}\n\n{boundary}\n\n{dynamic_suffix}",
            static_prefix = static_prefix,
            boundary = CACHE_BOUNDARY_COMMENT,
            dynamic_suffix = dynamic_suffix.trim_start(),
        );

        let req = CompletionRequest::new(vec![ChatMessage::system(&system_text), mk_user("hi")]);
        let built = build_chat_message_request(&req, "claude-sonnet-4-6");

        let blocks = match built.system.expect("system must be present") {
            SystemPrompt::Blocks(b) => b,
            other => panic!("expected Blocks form for Anthropic+boundary, got {other:?}"),
        };
        assert_eq!(blocks.len(), 2, "must produce exactly 2 system blocks");
        // 静态前缀块：必须挂 ephemeral cache_control
        assert_eq!(blocks[0].block_type, "text");
        assert!(
            blocks[0].text.contains("You are an AI assistant."),
            "static block must contain identity prefix; got: {:?}",
            blocks[0].text
        );
        let cc = blocks[0]
            .cache_control
            .as_ref()
            .expect("static block must carry cache_control");
        assert_eq!(cc.cache_type, "ephemeral");
        // 动态后缀块：不挂 cache_control
        assert_eq!(blocks[1].block_type, "text");
        assert!(
            blocks[1].text.contains("Current time"),
            "dynamic block must contain runtime context; got: {:?}",
            blocks[1].text
        );
        assert!(
            blocks[1].cache_control.is_none(),
            "dynamic block must NOT carry cache_control"
        );
        // 边界 HTML 注释必须被剔除（不在任何块的 text 中残留）
        for b in &blocks {
            assert!(
                !b.text.contains(CACHE_BOUNDARY_COMMENT),
                "boundary HTML comment must be stripped; leaked in: {:?}",
                b.text
            );
        }
    }

    /// req_llm_anthropic_cache_002：Anthropic 模型 + system 不含边界标记
    /// → 单段 `SystemPrompt::Text`，不引入任何 cache_control。
    ///   并验证：非 Anthropic 模型即便含边界标记也保持单段文本（cache_control 仅 Anthropic 适用）。
    #[test]
    fn req_llm_anthropic_cache_002_no_boundary_keeps_text_form() {
        // case 1：Anthropic 模型 + 无边界标记 → Text 形态
        let req_a = CompletionRequest::new(vec![
            ChatMessage::system("plain system prompt without boundary marker"),
            mk_user("hi"),
        ]);
        let built_a = build_chat_message_request(&req_a, "claude-sonnet-4-6");
        match built_a.system.expect("system present") {
            SystemPrompt::Text(s) => {
                assert_eq!(s, "plain system prompt without boundary marker");
            }
            other => panic!("expected Text form when no boundary marker; got {other:?}"),
        }

        // case 2：非 Anthropic 模型 + 含边界标记 → 仍是 Text 形态（cache_control 仅 Anthropic 识别）
        let with_marker = format!("static\n\n{CACHE_BOUNDARY_COMMENT}\n\ndynamic");
        let req_b = CompletionRequest::new(vec![ChatMessage::system(&with_marker), mk_user("hi")]);
        let built_b = build_chat_message_request(&req_b, "gpt-4o");
        match built_b.system.expect("system present") {
            SystemPrompt::Text(s) => {
                assert_eq!(s, with_marker, "non-Anthropic 模型应保留原始 system 文本");
            }
            other => panic!("expected Text form for non-Anthropic; got {other:?}"),
        }
    }

    /// req_llm_anthropic_cache_002b：边界切分后的 wire JSON 序列化形态匹配 Anthropic API
    /// （`system` 字段为数组，第一个元素含 `cache_control.type == "ephemeral"`）。
    #[test]
    fn req_llm_anthropic_cache_002b_wire_serialization_matches_anthropic_schema() {
        let system_text = format!("STATIC{boundary}DYNAMIC", boundary = CACHE_BOUNDARY_COMMENT,);
        let req = CompletionRequest::new(vec![ChatMessage::system(&system_text), mk_user("hi")]);
        let built = build_chat_message_request(&req, "claude-opus-4-6");
        let wire = serde_json::to_value(&built).expect("MessageRequest must serialize");
        let system = wire
            .get("system")
            .expect("system field present")
            .as_array()
            .expect("system must serialize as array under Blocks form");
        assert_eq!(system.len(), 2);
        assert_eq!(system[0]["type"], "text");
        assert_eq!(system[0]["text"], "STATIC");
        assert_eq!(system[0]["cache_control"]["type"], "ephemeral");
        assert_eq!(system[1]["type"], "text");
        assert_eq!(system[1]["text"], "DYNAMIC");
        assert!(system[1].get("cache_control").is_none());
    }
}
