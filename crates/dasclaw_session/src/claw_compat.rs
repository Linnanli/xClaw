//! Bidirectional conversion between `dasclaw_core`'s OpenAI-shaped
//! [`ChatMessage`] and `claw-code`'s `ConversationMessage` /
//! `ContentBlock` shape used in its on-disk `session.jsonl`.
//!
//! ## Why a parallel type set
//!
//! Each side reflects a different upstream API contract:
//!
//! - `ChatMessage` mirrors OpenAI Chat Completions
//!   (`role` + `content` string + `tool_calls` array).
//! - claw-code stores Anthropic-style structured content blocks
//!   (`role` + `blocks: [{ type: "text" | "tool_use" | "tool_result", ... }]`).
//!
//! Reusing `ChatMessage` for the claw-code wire would force its
//! serde shape to fork, breaking every dasclaw provider. So we keep
//! `ChatMessage` as-is and provide a `ClawMessage` whose serde shape
//! exactly matches what `claw-code/rust/crates/runtime/src/session.rs`
//! reads and writes.
//!
//! ## Lossy edges
//!
//! - `ChatMessage::content_parts` (multimodal `ImageUrl`) has no
//!   counterpart in claw-code's `ContentBlock`. Conversion preserves
//!   the text content only; images are dropped. This is documented
//!   by `req_dasclaw_session_d1_content_parts_images_drop_to_text_only`.
//! - `ToolCall::arguments` is a `serde_json::Value` on the dasclaw
//!   side and a JSON-stringified field on the claw-code side. We
//!   serialize to a JSON string on conversion and parse it back on
//!   the return trip. Non-JSON-object arguments (rare in practice)
//!   are serialized as their literal JSON form.

use dasclaw_core::messages::{ChatMessage, Role, ToolCall};
use serde::{Deserialize, Serialize};

/// Claw-code's message-level role tag. Serializes as lowercase to
/// match `claw-code`'s on-disk format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClawRole {
    System,
    User,
    Assistant,
    Tool,
}

/// Claw-code's structured content block. The `type` field is the
/// serde tag — exactly the shape `claw-code`'s `ContentBlock::to_json`
/// emits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClawContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: String,
    },
    ToolResult {
        tool_use_id: String,
        tool_name: String,
        output: String,
        is_error: bool,
    },
}

/// Claw-code-shaped persisted message. Mirrors
/// `claw-code::runtime::session::ConversationMessage` exactly on the
/// wire, minus `usage` (token-usage fields land in
/// [`crate::SessionSnapshot`] when we wire them up in a later PR).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClawMessage {
    pub role: ClawRole,
    pub blocks: Vec<ClawContentBlock>,
}

/// Convert from the OpenAI-shaped [`ChatMessage`] to a claw-code-shaped
/// [`ClawMessage`].
///
/// Mapping:
/// - System / User text-only → single `Text` block carrying `content`
/// - Assistant text-only → single `Text` block
/// - Assistant with `tool_calls`: optional leading `Text` block (if
///   `content` is non-empty) followed by one `ToolUse` block per
///   call, with `input` = JSON-stringified `arguments`
/// - Tool result → single `ToolResult` block reading `tool_call_id`,
///   `name`, and `content`; `is_error` is recorded as `false` (the
///   dasclaw `ChatMessage::tool_result` constructor does not yet
///   surface an error flag — adding one is a forward-compatible
///   change because `is_error` is required by the claw wire schema)
#[must_use]
pub fn chat_message_to_claw(message: &ChatMessage) -> ClawMessage {
    match message.role {
        Role::System => ClawMessage {
            role: ClawRole::System,
            blocks: vec![ClawContentBlock::Text {
                text: message.content.clone(),
            }],
        },
        Role::User => ClawMessage {
            role: ClawRole::User,
            blocks: vec![ClawContentBlock::Text {
                text: message.content.clone(),
            }],
        },
        Role::Assistant => {
            let mut blocks: Vec<ClawContentBlock> = Vec::new();
            if !message.content.is_empty() {
                blocks.push(ClawContentBlock::Text {
                    text: message.content.clone(),
                });
            }
            if let Some(calls) = &message.tool_calls {
                for call in calls {
                    blocks.push(ClawContentBlock::ToolUse {
                        id: call.id.clone(),
                        name: call.name.clone(),
                        input: call.arguments.to_string(),
                    });
                }
            }
            ClawMessage {
                role: ClawRole::Assistant,
                blocks,
            }
        }
        Role::Tool => ClawMessage {
            role: ClawRole::Tool,
            blocks: vec![ClawContentBlock::ToolResult {
                tool_use_id: message.tool_call_id.clone().unwrap_or_default(),
                tool_name: message.name.clone().unwrap_or_default(),
                output: message.content.clone(),
                is_error: false,
            }],
        },
    }
}

/// Convert from a claw-code-shaped [`ClawMessage`] back to the
/// OpenAI-shaped [`ChatMessage`].
///
/// Mapping:
/// - All `Text` blocks are concatenated into `content` (newline-
///   separated). The most common case has a single block, so this
///   collapses to the original text.
/// - `ToolUse` blocks become entries in `tool_calls`; `input` is
///   parsed as JSON (falling back to a JSON string if not parseable).
/// - A single `ToolResult` block on a Tool-role message becomes a
///   `tool_result` `ChatMessage`.
#[must_use]
pub fn claw_to_chat_message(message: &ClawMessage) -> ChatMessage {
    let role: Role = match message.role {
        ClawRole::System => Role::System,
        ClawRole::User => Role::User,
        ClawRole::Assistant => Role::Assistant,
        ClawRole::Tool => Role::Tool,
    };

    // Handle Tool role with a single ToolResult block first — that's
    // the only legal layout claw-code emits.
    if role == Role::Tool
        && let Some(ClawContentBlock::ToolResult {
            tool_use_id,
            tool_name,
            output,
            is_error: _,
        }) = message.blocks.first()
    {
        return ChatMessage::tool_result(tool_use_id, tool_name, output);
    }

    let mut text_parts: Vec<&str> = Vec::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();
    for block in &message.blocks {
        match block {
            ClawContentBlock::Text { text } => text_parts.push(text),
            ClawContentBlock::ToolUse { id, name, input } => {
                let arguments: serde_json::Value = serde_json::from_str(input)
                    .unwrap_or_else(|_| serde_json::Value::String(input.clone()));
                tool_calls.push(ToolCall {
                    id: id.clone(),
                    name: name.clone(),
                    arguments,
                    reasoning: None,
                });
            }
            ClawContentBlock::ToolResult { .. } => {
                // tool_result on a non-Tool role is malformed claw input;
                // drop the block silently (Fail-Safe over Fail-Open).
            }
        }
    }

    let content = text_parts.join("\n");

    let mut msg = ChatMessage {
        role,
        content,
        content_parts: Vec::new(),
        tool_call_id: None,
        name: None,
        tool_calls: None,
        tool_error: None,
        usage: None,
    };
    if !tool_calls.is_empty() {
        msg.tool_calls = Some(tool_calls);
    }
    msg
}
