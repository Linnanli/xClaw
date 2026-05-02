//! 集成测试：wire 类型 JSON round-trip。
//!
//! 这些测试把代表性 wire 数据走 `serde_json::to_value` → `from_value` 一轮，
//! 验证 `#[serde(tag = "type")]` 标签、`skip_serializing_if` 行为、字段命名都符合
//! Anthropic Messages 协议规范。

use dasclaw_llm_provider::{
    ContentBlockDelta, ContentBlockDeltaEvent, InputContentBlock, InputMessage, MessageRequest,
    MessageResponse, OutputContentBlock, StreamEvent, ToolChoice, ToolDefinition,
    ToolResultContentBlock, Usage,
};
use serde_json::json;

#[test]
fn input_message_text_roundtrip() {
    let msg = InputMessage::user_text("hello world");
    let json = serde_json::to_value(&msg).expect("serialize");
    let back: InputMessage = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, msg);
}

#[test]
fn input_message_tool_use_roundtrip() {
    let msg = InputMessage {
        role: "assistant".into(),
        content: vec![InputContentBlock::ToolUse {
            id: "toolu_abc".into(),
            name: "read_file".into(),
            input: json!({ "path": "/etc/hosts" }),
        }],
    };
    let json = serde_json::to_value(&msg).expect("serialize");
    assert_eq!(json["content"][0]["type"], "tool_use");
    let back: InputMessage = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, msg);
}

#[test]
fn input_message_tool_result_roundtrip() {
    let msg = InputMessage::user_tool_result("toolu_abc", "file contents", false);
    let json = serde_json::to_value(&msg).expect("serialize");
    assert_eq!(json["content"][0]["type"], "tool_result");
    // is_error: false 应被 skip_serializing_if 省略
    assert!(json["content"][0].get("is_error").is_none());
    let back: InputMessage = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, msg);
}

#[test]
fn tool_result_with_error_includes_is_error_field() {
    let msg = InputMessage::user_tool_result("toolu_abc", "denied", true);
    let json = serde_json::to_value(&msg).expect("serialize");
    assert_eq!(json["content"][0]["is_error"], true);
}

#[test]
fn tool_definition_roundtrip() {
    let tool = ToolDefinition {
        name: "read_file".into(),
        description: Some("Read a file".into()),
        input_schema: json!({
            "type": "object",
            "properties": { "path": { "type": "string" } },
            "required": ["path"]
        }),
    };
    let json = serde_json::to_value(&tool).expect("serialize");
    let back: ToolDefinition = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, tool);
}

#[test]
fn tool_choice_three_variants_roundtrip() {
    for tc in [
        ToolChoice::Auto,
        ToolChoice::Any,
        ToolChoice::Tool {
            name: "read_file".into(),
        },
    ] {
        let json = serde_json::to_value(&tc).expect("serialize");
        let back: ToolChoice = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, tc);
    }
}

#[test]
fn full_message_request_roundtrip() {
    let req = MessageRequest {
        model: "claude-opus-4-6".into(),
        max_tokens: 4096,
        messages: vec![InputMessage::user_text("hi")],
        system: Some("You are helpful.".into()),
        tools: Some(vec![ToolDefinition {
            name: "search".into(),
            description: None,
            input_schema: json!({"type": "object"}),
        }]),
        tool_choice: Some(ToolChoice::Auto),
        stream: true,
        temperature: Some(0.7),
        top_p: Some(0.9),
        frequency_penalty: None,
        presence_penalty: None,
        stop: Some(vec!["</end>".into()]),
        reasoning_effort: Some("high".into()),
    };
    let json = serde_json::to_value(&req).expect("serialize");
    let back: MessageRequest = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, req);
}

#[test]
fn message_response_with_thinking_block_roundtrip() {
    let resp = MessageResponse {
        id: "msg_xyz".into(),
        kind: "message".into(),
        role: "assistant".into(),
        content: vec![
            OutputContentBlock::Thinking {
                thinking: "Let me think...".into(),
                signature: Some("sig_abc".into()),
            },
            OutputContentBlock::Text {
                text: "Hello!".into(),
            },
            OutputContentBlock::ToolUse {
                id: "toolu_42".into(),
                name: "search".into(),
                input: json!({"q": "rust"}),
            },
        ],
        model: "claude-opus-4-6".into(),
        stop_reason: Some("tool_use".into()),
        stop_sequence: None,
        usage: Usage {
            input_tokens: 100,
            cache_creation_input_tokens: 10,
            cache_read_input_tokens: 50,
            output_tokens: 30,
        },
        request_id: Some("req_abc".into()),
    };
    let json = serde_json::to_value(&resp).expect("serialize");
    let back: MessageResponse = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, resp);
}

#[test]
fn redacted_thinking_preserves_opaque_blob() {
    let block = OutputContentBlock::RedactedThinking {
        data: json!({"opaque": [1, 2, 3], "version": "v1"}),
    };
    let json = serde_json::to_value(&block).expect("serialize");
    assert_eq!(json["type"], "redacted_thinking");
    let back: OutputContentBlock = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, block);
}

#[test]
fn tool_result_content_block_json_variant_roundtrip() {
    let block = ToolResultContentBlock::Json {
        value: json!({"result": "ok"}),
    };
    let json = serde_json::to_value(&block).expect("serialize");
    assert_eq!(json["type"], "json");
    let back: ToolResultContentBlock = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, block);
}

#[test]
fn stream_event_message_start_tagged_correctly() {
    let ev = StreamEvent::MessageStart(dasclaw_llm_provider::MessageStartEvent {
        message: MessageResponse {
            id: "msg_1".into(),
            kind: "message".into(),
            role: "assistant".into(),
            content: vec![],
            model: "claude-sonnet-4-6".into(),
            stop_reason: None,
            stop_sequence: None,
            usage: Usage::default(),
            request_id: None,
        },
    });
    let json = serde_json::to_value(&ev).expect("serialize");
    assert_eq!(json["type"], "message_start");
    let back: StreamEvent = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, ev);
}

#[test]
fn stream_event_content_block_delta_text_delta() {
    let ev = StreamEvent::ContentBlockDelta(ContentBlockDeltaEvent {
        index: 0,
        delta: ContentBlockDelta::TextDelta {
            text: "hello".into(),
        },
    });
    let json = serde_json::to_value(&ev).expect("serialize");
    assert_eq!(json["type"], "content_block_delta");
    assert_eq!(json["delta"]["type"], "text_delta");
    let back: StreamEvent = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, ev);
}

#[test]
fn stream_event_input_json_delta_for_partial_tool_input() {
    let ev = StreamEvent::ContentBlockDelta(ContentBlockDeltaEvent {
        index: 1,
        delta: ContentBlockDelta::InputJsonDelta {
            partial_json: r#"{"path":"#.into(),
        },
    });
    let json = serde_json::to_value(&ev).expect("serialize");
    assert_eq!(json["delta"]["type"], "input_json_delta");
    let back: StreamEvent = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, ev);
}

#[test]
fn message_request_default_omits_optional_fields() {
    let req = MessageRequest {
        model: "claude-sonnet-4-6".into(),
        max_tokens: 100,
        messages: vec![InputMessage::user_text("hi")],
        ..MessageRequest::default()
    };
    let json = serde_json::to_string(&req).expect("serialize");
    assert!(!json.contains("system"));
    assert!(!json.contains("tools"));
    assert!(!json.contains("tool_choice"));
    assert!(!json.contains("temperature"));
    assert!(!json.contains("stream"));
}
