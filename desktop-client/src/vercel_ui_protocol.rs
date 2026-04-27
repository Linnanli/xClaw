//! Vercel AI SDK UI Stream Protocol types.
//!
//! Extracted from [lazy-hq/aisdk](https://github.com/lazy-hq/aisdk) (MIT license)
//! `src/integrations/vercel_aisdk_ui.rs`.
//!
//! Only the protocol serialization types are included — no LLM provider code,
//! no axum integration, no `LanguageModelStreamChunkType` mapping.
//!
//! Our own `StatusUpdate → VercelUIStream` mapping lives in `tauri_channel.rs`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Rust → Frontend: stream event types
// ---------------------------------------------------------------------------

/// Vercel AI SDK UI message chunk types.
///
/// These represent the JSON chunks emitted over IPC to the frontend,
/// compatible with `@assistant-ui/react-ai-sdk`'s `useChatRuntime`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum VercelUIStream {
    // === Text ===
    /// Start of text message.
    #[serde(rename = "text-start")]
    TextStart {
        /// Message ID.
        id: String,
        #[serde(rename = "providerMetadata")]
        #[serde(skip_serializing_if = "Option::is_none")]
        provider_metadata: Option<Value>,
    },
    /// Delta of text message.
    #[serde(rename = "text-delta")]
    TextDelta {
        /// Message ID.
        id: String,
        /// Text delta.
        delta: String,
        #[serde(rename = "providerMetadata")]
        #[serde(skip_serializing_if = "Option::is_none")]
        provider_metadata: Option<Value>,
    },
    /// End of text message.
    #[serde(rename = "text-end")]
    TextEnd {
        /// Message ID.
        id: String,
        #[serde(rename = "providerMetadata")]
        #[serde(skip_serializing_if = "Option::is_none")]
        provider_metadata: Option<Value>,
    },

    // === Reasoning ===
    /// Start of reasoning message.
    #[serde(rename = "reasoning-start")]
    ReasoningStart {
        id: String,
        #[serde(rename = "providerMetadata")]
        #[serde(skip_serializing_if = "Option::is_none")]
        provider_metadata: Option<Value>,
    },
    /// Delta of reasoning message.
    #[serde(rename = "reasoning-delta")]
    ReasoningDelta {
        id: String,
        delta: String,
        #[serde(rename = "providerMetadata")]
        #[serde(skip_serializing_if = "Option::is_none")]
        provider_metadata: Option<Value>,
    },
    /// End of reasoning message.
    #[serde(rename = "reasoning-end")]
    ReasoningEnd {
        id: String,
        #[serde(rename = "providerMetadata")]
        #[serde(skip_serializing_if = "Option::is_none")]
        provider_metadata: Option<Value>,
    },

    // === Tool Input (args) ===
    /// Tool call started — tool name and call ID.
    #[serde(rename = "tool-input-start")]
    ToolInputStart {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "toolName")]
        tool_name: String,
        /// Whether the tool was already executed on the server side.
        #[serde(rename = "providerExecuted")]
        #[serde(skip_serializing_if = "Option::is_none")]
        provider_executed: Option<bool>,
        #[serde(rename = "providerMetadata")]
        #[serde(skip_serializing_if = "Option::is_none")]
        provider_metadata: Option<Value>,
    },
    /// Incremental tool input JSON fragment.
    #[serde(rename = "tool-input-delta")]
    ToolInputDelta {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "inputTextDelta")]
        input_text_delta: String,
    },
    /// Tool input fully available (complete args).
    #[serde(rename = "tool-input-available")]
    ToolInputAvailable {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "toolName")]
        tool_name: String,
        /// Parsed tool input (args).
        input: Value,
        #[serde(rename = "providerExecuted")]
        #[serde(skip_serializing_if = "Option::is_none")]
        provider_executed: Option<bool>,
        #[serde(rename = "providerMetadata")]
        #[serde(skip_serializing_if = "Option::is_none")]
        provider_metadata: Option<Value>,
    },
    /// Tool input parsing failed.
    #[serde(rename = "tool-input-error")]
    ToolInputError {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "toolName")]
        tool_name: String,
        input: Value,
        #[serde(rename = "errorText")]
        error_text: String,
    },

    // === Tool Output (result) ===
    /// Tool output available.
    #[serde(rename = "tool-output-available")]
    ToolOutputAvailable {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        output: Value,
        #[serde(rename = "providerExecuted")]
        #[serde(skip_serializing_if = "Option::is_none")]
        provider_executed: Option<bool>,
    },
    /// Tool output failed.
    #[serde(rename = "tool-output-error")]
    ToolOutputError {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "errorText")]
        error_text: String,
        #[serde(rename = "providerExecuted")]
        #[serde(skip_serializing_if = "Option::is_none")]
        provider_executed: Option<bool>,
    },

    // === Error ===
    /// Error chunk.
    #[serde(rename = "error")]
    Error {
        #[serde(rename = "errorText")]
        error_text: String,
    },

    // === Stream lifecycle ===
    /// Signals the end of the stream.
    #[serde(rename = "finish")]
    Finish { id: String },

    // === Custom (non-Vercel) extensions ===
    /// Custom data part (for approval_needed, connection_status, etc.)
    #[serde(rename = "data-custom")]
    DataCustom {
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        data: Value,
    },
}

/// Configuration for Vercel UI message stream.
#[derive(Default)]
pub struct StreamOptions {
    /// Whether to emit reasoning-start/delta/end events.
    pub send_reasoning: bool,
    /// Whether to emit text-start events.
    pub send_start: bool,
    /// Whether to emit text-end / finish events.
    pub send_finish: bool,
}

// ---------------------------------------------------------------------------
// Frontend → Rust: message deserialization
// ---------------------------------------------------------------------------

/// A part of a UI message from Vercel's useChat hook.
#[derive(Deserialize, Debug)]
pub struct VercelUIMessagePart {
    /// The text content of the part.
    #[serde(default)]
    pub text: Option<String>,
    /// The type of the part (e.g., "text", "tool-get_weather").
    #[serde(rename = "type")]
    pub part_type: String,
    /// Tool call identifier for tool parts.
    #[serde(rename = "toolCallId")]
    #[serde(default)]
    pub tool_call_id: Option<String>,
    /// Tool execution state for tool parts.
    #[serde(default)]
    pub state: Option<String>,
    /// Tool input payload.
    #[serde(default)]
    pub input: Option<Value>,
    /// Tool output payload.
    #[serde(default)]
    pub output: Option<Value>,
    /// Raw tool input for error cases.
    #[serde(rename = "rawInput")]
    #[serde(default)]
    pub raw_input: Option<Value>,
    /// Tool error text for error states.
    #[serde(rename = "errorText")]
    #[serde(default)]
    pub error_text: Option<String>,
    /// Tool name for dynamic tool parts.
    #[serde(rename = "toolName")]
    #[serde(default)]
    pub tool_name: Option<String>,
}

/// A UI message from Vercel's useChat hook.
#[derive(Deserialize, Debug)]
pub struct VercelUIMessage {
    /// Unique identifier for the message.
    pub id: String,
    /// Role of the message sender ("user", "assistant", "system").
    pub role: String,
    /// Array of message parts.
    pub parts: Vec<VercelUIMessagePart>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_tool_input_start_serialization() {
        let event = VercelUIStream::ToolInputStart {
            tool_call_id: "call_1".into(),
            tool_name: "read_file".into(),
            provider_executed: Some(true),
            provider_metadata: None,
        };
        let json = serde_json::to_value(&event).expect("should serialize");
        assert_eq!(json["type"], "tool-input-start");
        assert_eq!(json["toolCallId"], "call_1");
        assert_eq!(json["toolName"], "read_file");
        assert_eq!(json["providerExecuted"], true);
        assert!(json.get("providerMetadata").is_none());
    }

    #[test]
    fn test_tool_input_available_serialization() {
        let event = VercelUIStream::ToolInputAvailable {
            tool_call_id: "call_1".into(),
            tool_name: "read_file".into(),
            input: json!({"path": "/foo/bar.rs"}),
            provider_executed: Some(true),
            provider_metadata: None,
        };
        let json = serde_json::to_value(&event).expect("should serialize");
        assert_eq!(json["type"], "tool-input-available");
        assert_eq!(json["toolCallId"], "call_1");
        assert_eq!(json["input"]["path"], "/foo/bar.rs");
    }

    #[test]
    fn test_tool_output_available_serialization() {
        let event = VercelUIStream::ToolOutputAvailable {
            tool_call_id: "call_1".into(),
            output: json!("file content here"),
            provider_executed: Some(true),
        };
        let json = serde_json::to_value(&event).expect("should serialize");
        assert_eq!(json["type"], "tool-output-available");
        assert_eq!(json["toolCallId"], "call_1");
        assert_eq!(json["output"], "file content here");
    }

    #[test]
    fn test_tool_output_error_serialization() {
        let event = VercelUIStream::ToolOutputError {
            tool_call_id: "call_2".into(),
            error_text: "file not found".into(),
            provider_executed: Some(true),
        };
        let json = serde_json::to_value(&event).expect("should serialize");
        assert_eq!(json["type"], "tool-output-error");
        assert_eq!(json["errorText"], "file not found");
    }

    #[test]
    fn test_text_delta_serialization() {
        let event = VercelUIStream::TextDelta {
            id: "msg_1".into(),
            delta: "Hello ".into(),
            provider_metadata: None,
        };
        let json = serde_json::to_value(&event).expect("should serialize");
        assert_eq!(json["type"], "text-delta");
        assert_eq!(json["id"], "msg_1");
        assert_eq!(json["delta"], "Hello ");
    }

    #[test]
    fn test_reasoning_delta_serialization() {
        let event = VercelUIStream::ReasoningDelta {
            id: "msg_1".into(),
            delta: "thinking...".into(),
            provider_metadata: None,
        };
        let json = serde_json::to_value(&event).expect("should serialize");
        assert_eq!(json["type"], "reasoning-delta");
        assert_eq!(json["delta"], "thinking...");
    }

    #[test]
    fn test_error_serialization() {
        let event = VercelUIStream::Error {
            error_text: "something went wrong".into(),
        };
        let json = serde_json::to_value(&event).expect("should serialize");
        assert_eq!(json["type"], "error");
        assert_eq!(json["errorText"], "something went wrong");
    }

    #[test]
    fn test_data_custom_serialization() {
        let event = VercelUIStream::DataCustom {
            id: Some("approval_1".into()),
            data: json!({
                "kind": "approval_needed",
                "request_id": "r-1",
                "tool_name": "rm"
            }),
        };
        let json = serde_json::to_value(&event).expect("should serialize");
        assert_eq!(json["type"], "data-custom");
        assert_eq!(json["id"], "approval_1");
        assert_eq!(json["data"]["kind"], "approval_needed");
    }

    #[test]
    fn test_finish_serialization() {
        let event = VercelUIStream::Finish { id: "msg_1".into() };
        let json = serde_json::to_value(&event).expect("should serialize");
        assert_eq!(json["type"], "finish");
        assert_eq!(json["id"], "msg_1");
    }

    #[test]
    fn test_deserialization_roundtrip() {
        let event = VercelUIStream::ToolInputStart {
            tool_call_id: "call_1".into(),
            tool_name: "shell".into(),
            provider_executed: Some(true),
            provider_metadata: None,
        };
        let json_str = serde_json::to_string(&event).expect("serialize");
        let deserialized: VercelUIStream = serde_json::from_str(&json_str).expect("deserialize");
        let json_again = serde_json::to_string(&deserialized).expect("re-serialize");
        assert_eq!(json_str, json_again);
    }

    #[test]
    fn test_vercel_ui_message_deserialization() {
        let msg: VercelUIMessage = serde_json::from_value(json!({
            "id": "msg_1",
            "role": "assistant",
            "parts": [
                {
                    "type": "text",
                    "text": "Hello!"
                },
                {
                    "type": "tool-read_file",
                    "toolCallId": "call_1",
                    "toolName": "read_file",
                    "state": "output-available",
                    "input": {"path": "/foo"},
                    "output": "file content"
                }
            ]
        }))
        .expect("should deserialize");

        assert_eq!(msg.id, "msg_1");
        assert_eq!(msg.role, "assistant");
        assert_eq!(msg.parts.len(), 2);
        assert_eq!(msg.parts[0].part_type, "text");
        assert_eq!(msg.parts[0].text.as_deref(), Some("Hello!"));
        assert_eq!(msg.parts[1].part_type, "tool-read_file");
        assert_eq!(msg.parts[1].tool_call_id.as_deref(), Some("call_1"));
        assert_eq!(msg.parts[1].state.as_deref(), Some("output-available"));
    }
}
