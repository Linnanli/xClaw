//! Wire 类型 — Anthropic Messages 协议输入 / 输出 / streaming events。
//!
//! 设计原则：
//! - **纯数据**：所有类型都是 `serde` derive，不依赖任何 IO 或运行时副作用。
//! - **不计算成本**：`Usage` 只暴露 4 个原始 token 字段（input/output/cache_read/cache_creation）。
//!   成本计算是上层应用职责，本 crate 不耦合 pricing 表。
//! - **`#[serde(skip_serializing_if = ...)]`**：所有可选字段在 None 时不写入 JSON，
//!   避免向上游 Anthropic / OpenAI-compat API 发送多余字段触发 schema 校验失败。

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Anthropic Messages API 请求体。
///
/// 字段对齐 [Anthropic Messages API](https://docs.anthropic.com/en/api/messages)
/// 与 OpenAI Chat Completions 兼容层（temperature / top_p / penalties / stop / reasoning_effort
/// 在 OpenAI 系厂商生效；Anthropic 仅识别 temperature / top_p / stop_sequences，其它字段后端
/// 静默忽略）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MessageRequest {
    /// 模型名（可以是别名如 "opus" / "sonnet"，由 [`super::resolve_model_alias`] 解析为 canonical 名）。
    pub model: String,
    /// 最大输出 token 数。Anthropic 协议必填字段；OpenAI-compat 等价于 `max_tokens`。
    pub max_tokens: u32,
    /// 多轮消息历史（不含 system —— system 单独走 [`Self::system`] 字段）。
    pub messages: Vec<InputMessage>,
    /// System prompt。Anthropic 协议是顶层字段，OpenAI-compat 走第一条 `role=system` message
    /// （由 client 实现负责适配，wire 层统一暴露此字段）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    /// 工具定义。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
    /// 工具选择策略。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    /// 是否流式返回。SSE 流由 client 实现处理。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stream: bool,
    /// Sampling temperature（[0, 1]）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// Nucleus sampling top-p（[0, 1]）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    /// Frequency penalty（OpenAI-compat 特有，Anthropic 忽略）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f64>,
    /// Presence penalty（OpenAI-compat 特有，Anthropic 忽略）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f64>,
    /// Stop sequences。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    /// Reasoning effort：`"low"` / `"medium"` / `"high"`，OpenAI o-系列等 reasoning models 适用。
    /// 后端不支持时静默忽略。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
}

impl MessageRequest {
    /// 设置 `stream = true` 并返回自身，便于链式调用。
    #[must_use]
    pub fn with_streaming(mut self) -> Self {
        self.stream = true;
        self
    }
}

/// 单条对话消息。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputMessage {
    /// 角色：`"user"` / `"assistant"` / `"system"`。
    pub role: String,
    /// 消息内容块。允许混合文本 + tool_use + tool_result。
    pub content: Vec<InputContentBlock>,
}

impl InputMessage {
    /// 构造单段纯文本 user message。
    #[must_use]
    pub fn user_text(text: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: vec![InputContentBlock::Text { text: text.into() }],
        }
    }

    /// 构造 tool_result user message（Anthropic 协议要求 tool_result 必须挂在 role=user 上）。
    #[must_use]
    pub fn user_tool_result(
        tool_use_id: impl Into<String>,
        content: impl Into<String>,
        is_error: bool,
    ) -> Self {
        Self {
            role: "user".to_string(),
            content: vec![InputContentBlock::ToolResult {
                tool_use_id: tool_use_id.into(),
                content: vec![ToolResultContentBlock::Text {
                    text: content.into(),
                }],
                is_error,
            }],
        }
    }
}

/// 请求侧 content block。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputContentBlock {
    /// 纯文本。
    Text {
        /// 文本内容。
        text: String,
    },
    /// 工具调用（assistant 历史消息中的 tool_use）。
    ToolUse {
        /// 工具调用 ID（assistant 端生成，后续 tool_result 必须引用）。
        id: String,
        /// 工具名。
        name: String,
        /// 工具入参 JSON。
        input: Value,
    },
    /// 工具结果（必须挂在 role=user 上）。
    ToolResult {
        /// 引用对应 [`Self::ToolUse::id`]。
        tool_use_id: String,
        /// 结果内容块（通常单段 Text，复杂场景可多段）。
        content: Vec<ToolResultContentBlock>,
        /// 是否表示工具执行失败。
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        is_error: bool,
    },
}

/// 工具结果内嵌 content block。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolResultContentBlock {
    /// 文本结果。
    Text {
        /// 文本内容。
        text: String,
    },
    /// JSON 结果（Anthropic 协议扩展）。
    Json {
        /// JSON 值。
        value: Value,
    },
}

/// 工具定义。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// 工具名（必须满足 `^[a-zA-Z0-9_-]+$`，由 client 校验）。
    pub name: String,
    /// 工具描述（提示 LLM 何时使用）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema 定义入参。
    pub input_schema: Value,
}

/// 工具选择策略。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolChoice {
    /// LLM 自行决定是否调用工具。
    Auto,
    /// 必须调用某个工具（不限定哪一个）。
    Any,
    /// 必须调用指定工具。
    Tool {
        /// 强制调用的工具名。
        name: String,
    },
}

/// Anthropic Messages API 响应体。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageResponse {
    /// 消息 ID（Anthropic 端 `msg_xxx`）。
    pub id: String,
    /// 消息类型，固定 `"message"`。
    #[serde(rename = "type")]
    pub kind: String,
    /// 角色，固定 `"assistant"`。
    pub role: String,
    /// 输出 content blocks。
    pub content: Vec<OutputContentBlock>,
    /// 实际生成所用的模型 canonical 名。
    pub model: String,
    /// 终止原因：`"end_turn"` / `"max_tokens"` / `"stop_sequence"` / `"tool_use"` / `"refusal"`。
    #[serde(default)]
    pub stop_reason: Option<String>,
    /// 命中的 stop sequence（如有）。
    #[serde(default)]
    pub stop_sequence: Option<String>,
    /// Token 使用量。
    #[serde(default)]
    pub usage: Usage,
    /// 上游 trace ID（Anthropic 响应头 `request-id`，client 实现负责注入）。
    #[serde(default)]
    pub request_id: Option<String>,
}

impl MessageResponse {
    /// 总 token 数（input + output + cache_creation + cache_read）。
    #[must_use]
    pub const fn total_tokens(&self) -> u32 {
        self.usage.total_tokens()
    }
}

/// 响应侧 content block。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputContentBlock {
    /// 纯文本。
    Text {
        /// 文本内容。
        text: String,
    },
    /// 工具调用（assistant 想调用工具）。
    ToolUse {
        /// 工具调用 ID（assistant 端生成，后续 tool_result 必须引用此 ID）。
        id: String,
        /// 工具名。
        name: String,
        /// 工具入参 JSON。
        input: Value,
    },
    /// Extended thinking 输出（Claude Opus / Sonnet 4.x 专属）。
    Thinking {
        /// 思考过程文本。
        #[serde(default)]
        thinking: String,
        /// 签名（验签时需保留原样回传）。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
    /// Redacted thinking 输出（Anthropic 隐私策略下被遮蔽的思考块，data 是不透明 blob）。
    RedactedThinking {
        /// 不透明 redacted 数据，回传时必须原样保留。
        data: Value,
    },
}

/// Token 使用量原始字段。
///
/// **本 crate 不计算 USD 成本** —— 成本计算是上层应用职责（如 ironclaw 端
/// `costs::model_cost`）。本类型只暴露 4 个原始 token 计数。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    /// 输入 token 数。
    #[serde(default)]
    pub input_tokens: u32,
    /// 提示词缓存写入 token 数（首次写入 prefix cache 时计费）。
    #[serde(default)]
    pub cache_creation_input_tokens: u32,
    /// 提示词缓存命中 token 数（命中后续请求 prefix cache 时折扣）。
    #[serde(default)]
    pub cache_read_input_tokens: u32,
    /// 输出 token 数。
    #[serde(default)]
    pub output_tokens: u32,
}

impl Usage {
    /// 总 token 数（4 个字段相加）。
    #[must_use]
    pub const fn total_tokens(&self) -> u32 {
        self.input_tokens
            + self.output_tokens
            + self.cache_creation_input_tokens
            + self.cache_read_input_tokens
    }
}

// ---------- Streaming events ----------

/// `message_start` SSE 事件。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageStartEvent {
    /// 完整消息壳（content 此时为空，后续 ContentBlockStart/Delta/Stop 流式填充）。
    pub message: MessageResponse,
}

/// `message_delta` SSE 事件。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageDeltaEvent {
    /// 增量字段（stop_reason / stop_sequence）。
    pub delta: MessageDelta,
    /// 累计 token 使用量。
    #[serde(default)]
    pub usage: Usage,
}

/// `message_delta` 内嵌增量字段。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageDelta {
    /// 终止原因。
    #[serde(default)]
    pub stop_reason: Option<String>,
    /// 命中的 stop sequence。
    #[serde(default)]
    pub stop_sequence: Option<String>,
}

/// `content_block_start` SSE 事件。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentBlockStartEvent {
    /// content blocks 数组中的索引。
    pub index: u32,
    /// 起始 content block。
    pub content_block: OutputContentBlock,
}

/// `content_block_delta` SSE 事件。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentBlockDeltaEvent {
    /// content blocks 数组中的索引。
    pub index: u32,
    /// 增量内容。
    pub delta: ContentBlockDelta,
}

/// `content_block_delta` 内嵌增量类型。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockDelta {
    /// 文本增量。
    TextDelta {
        /// 增量文本。
        text: String,
    },
    /// Tool input JSON 增量（partial JSON，需上层拼接后整体反序列化）。
    InputJsonDelta {
        /// 部分 JSON 字符串。
        partial_json: String,
    },
    /// Thinking 文本增量。
    ThinkingDelta {
        /// 增量思考文本。
        thinking: String,
    },
    /// Thinking 签名（验签字段，Anthropic 必须原样回传）。
    SignatureDelta {
        /// 签名值。
        signature: String,
    },
}

/// `content_block_stop` SSE 事件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentBlockStopEvent {
    /// content blocks 数组中的索引。
    pub index: u32,
}

/// `message_stop` SSE 事件（无字段）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageStopEvent {}

/// SSE 流事件枚举（按 Anthropic streaming 协议 6 种事件类型）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    /// `message_start`。
    MessageStart(MessageStartEvent),
    /// `message_delta`。
    MessageDelta(MessageDeltaEvent),
    /// `content_block_start`。
    ContentBlockStart(ContentBlockStartEvent),
    /// `content_block_delta`。
    ContentBlockDelta(ContentBlockDeltaEvent),
    /// `content_block_stop`。
    ContentBlockStop(ContentBlockStopEvent),
    /// `message_stop`。
    MessageStop(MessageStopEvent),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_total_tokens_sums_all_four_fields() {
        let usage = Usage {
            input_tokens: 10,
            cache_creation_input_tokens: 2,
            cache_read_input_tokens: 3,
            output_tokens: 4,
        };
        assert_eq!(usage.total_tokens(), 19);
    }

    #[test]
    fn usage_default_is_all_zero() {
        let usage = Usage::default();
        assert_eq!(usage.total_tokens(), 0);
    }

    #[test]
    fn input_message_user_text_helper() {
        let msg = InputMessage::user_text("hello");
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content.len(), 1);
        match &msg.content[0] {
            InputContentBlock::Text { text } => assert_eq!(text, "hello"),
            other => unreachable!("expected Text variant, got {other:?}"),
        }
    }

    #[test]
    fn input_message_user_tool_result_helper() {
        let msg = InputMessage::user_tool_result("tool_42", "result body", true);
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content.len(), 1);
        match &msg.content[0] {
            InputContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                assert_eq!(tool_use_id, "tool_42");
                assert_eq!(content.len(), 1);
                assert!(is_error);
                match &content[0] {
                    ToolResultContentBlock::Text { text } => assert_eq!(text, "result body"),
                    other => unreachable!("expected Text variant, got {other:?}"),
                }
            }
            other => unreachable!("expected ToolResult variant, got {other:?}"),
        }
    }

    #[test]
    fn message_request_omits_none_fields_in_json() {
        let req = MessageRequest {
            model: "claude-opus-4-6".to_string(),
            max_tokens: 100,
            messages: vec![InputMessage::user_text("hi")],
            ..Default::default()
        };
        let json = serde_json::to_value(&req).expect("serialize");
        let obj = json.as_object().expect("object");
        // None 字段不应出现
        assert!(!obj.contains_key("system"));
        assert!(!obj.contains_key("tools"));
        assert!(!obj.contains_key("tool_choice"));
        assert!(!obj.contains_key("temperature"));
        assert!(!obj.contains_key("top_p"));
        assert!(!obj.contains_key("frequency_penalty"));
        assert!(!obj.contains_key("presence_penalty"));
        assert!(!obj.contains_key("stop"));
        assert!(!obj.contains_key("reasoning_effort"));
        // false stream 也省略
        assert!(!obj.contains_key("stream"));
        // 必填字段在
        assert_eq!(
            obj.get("model").and_then(|v| v.as_str()),
            Some("claude-opus-4-6")
        );
        assert_eq!(obj.get("max_tokens").and_then(|v| v.as_u64()), Some(100));
    }

    #[test]
    fn message_request_with_streaming_sets_stream_true() {
        let req = MessageRequest {
            model: "m".into(),
            max_tokens: 1,
            messages: vec![],
            ..Default::default()
        }
        .with_streaming();
        assert!(req.stream);
        let json = serde_json::to_value(&req).expect("serialize");
        assert_eq!(json.get("stream").and_then(|v| v.as_bool()), Some(true));
    }

    #[test]
    fn tool_choice_serializes_with_type_tag() {
        let auto = serde_json::to_value(ToolChoice::Auto).expect("serialize");
        assert_eq!(auto.get("type").and_then(|v| v.as_str()), Some("auto"));

        let any = serde_json::to_value(ToolChoice::Any).expect("serialize");
        assert_eq!(any.get("type").and_then(|v| v.as_str()), Some("any"));

        let tool =
            serde_json::to_value(ToolChoice::Tool { name: "foo".into() }).expect("serialize");
        assert_eq!(tool.get("type").and_then(|v| v.as_str()), Some("tool"));
        assert_eq!(tool.get("name").and_then(|v| v.as_str()), Some("foo"));
    }

    #[test]
    fn output_content_block_thinking_roundtrip() {
        let original = OutputContentBlock::Thinking {
            thinking: "step by step".into(),
            signature: Some("sig_abc".into()),
        };
        let json = serde_json::to_value(&original).expect("serialize");
        assert_eq!(json.get("type").and_then(|v| v.as_str()), Some("thinking"));
        let back: OutputContentBlock = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, original);
    }

    #[test]
    fn message_response_total_tokens_delegates_to_usage() {
        let resp = MessageResponse {
            id: "msg_1".into(),
            kind: "message".into(),
            role: "assistant".into(),
            content: vec![],
            model: "claude-opus-4-6".into(),
            stop_reason: Some("end_turn".into()),
            stop_sequence: None,
            usage: Usage {
                input_tokens: 5,
                cache_creation_input_tokens: 1,
                cache_read_input_tokens: 2,
                output_tokens: 7,
            },
            request_id: None,
        };
        assert_eq!(resp.total_tokens(), 15);
    }

    #[test]
    fn stream_event_message_stop_serializes_as_tagged_unit() {
        let ev = StreamEvent::MessageStop(MessageStopEvent {});
        let json = serde_json::to_value(&ev).expect("serialize");
        assert_eq!(
            json.get("type").and_then(|v| v.as_str()),
            Some("message_stop")
        );
    }
}
