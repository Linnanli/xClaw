//! Tests for `claw_compat`: bidirectional conversion between
//! `dasclaw_core::messages::ChatMessage` (OpenAI Chat Completions
//! shape) and the `ClawMessage` / `ClawContentBlock` shape that
//! `claw-code`'s `session.jsonl` writes.
//!
//! Roundtrip rules:
//! - text-only system / user / assistant messages survive verbatim
//! - assistant + tool_calls survives: tool_calls become `tool_use`
//!   blocks (input serialized as JSON string), content stays as a
//!   leading `text` block when non-empty
//! - tool result survives: stored as `ToolResult { tool_use_id,
//!   tool_name, output, is_error: false }`
//! - `content_parts` (images) currently lossy: claw-code's
//!   `ContentBlock` has no image variant, so the conversion keeps
//!   the text content only. Documented behaviour, explicitly tested.

use dasclaw_core::messages::{ChatMessage, ContentPart, ImageUrl, Role, ToolCall};
use dasclaw_session::claw_compat::{
    ClawContentBlock, ClawMessage, ClawRole, chat_message_to_claw, claw_to_chat_message,
};

#[test]
fn req_dasclaw_session_d1_system_text_round_trip() {
    let original = ChatMessage::system("be concise");
    let claw = chat_message_to_claw(&original);
    assert_eq!(claw.role, ClawRole::System);
    assert_eq!(
        claw.blocks,
        vec![ClawContentBlock::Text {
            text: "be concise".to_string()
        }]
    );

    let back = claw_to_chat_message(&claw);
    assert_eq!(back.role, Role::System);
    assert_eq!(back.content, "be concise");
    assert!(back.tool_calls.is_none());
    assert!(back.tool_call_id.is_none());
}

#[test]
fn req_dasclaw_session_d1_user_and_assistant_text_round_trip() {
    let user = ChatMessage::user("hello");
    let assistant = ChatMessage::assistant("hi back");

    let user_claw = chat_message_to_claw(&user);
    let asst_claw = chat_message_to_claw(&assistant);
    assert_eq!(user_claw.role, ClawRole::User);
    assert_eq!(asst_claw.role, ClawRole::Assistant);

    let user_back = claw_to_chat_message(&user_claw);
    let asst_back = claw_to_chat_message(&asst_claw);
    assert_eq!(user_back.content, "hello");
    assert_eq!(asst_back.content, "hi back");
}

#[test]
fn req_dasclaw_session_d1_assistant_with_tool_calls_round_trip() {
    let calls = vec![
        ToolCall {
            id: "call_1".to_string(),
            name: "list_files".to_string(),
            arguments: serde_json::json!({"path": "/tmp"}),
            reasoning: None,
        },
        ToolCall {
            id: "call_2".to_string(),
            name: "read_file".to_string(),
            arguments: serde_json::json!({"path": "/tmp/x", "limit": 100}),
            reasoning: None,
        },
    ];
    let original = ChatMessage::assistant_with_tool_calls(Some("thinking...".to_string()), calls);

    let claw = chat_message_to_claw(&original);
    assert_eq!(claw.role, ClawRole::Assistant);
    // expect: [Text(thinking...), ToolUse(call_1), ToolUse(call_2)]
    assert_eq!(claw.blocks.len(), 3);
    matches!(claw.blocks[0], ClawContentBlock::Text { .. });
    match &claw.blocks[1] {
        ClawContentBlock::ToolUse { id, name, input } => {
            assert_eq!(id, "call_1");
            assert_eq!(name, "list_files");
            // input must be a JSON string of the arguments
            let parsed: serde_json::Value = serde_json::from_str(input).expect("input is JSON");
            assert_eq!(parsed, serde_json::json!({"path": "/tmp"}));
        }
        other => panic!("expected ToolUse, got {other:?}"),
    }

    let back = claw_to_chat_message(&claw);
    assert_eq!(back.role, Role::Assistant);
    assert_eq!(back.content, "thinking...");
    let back_calls = back.tool_calls.as_ref().expect("tool_calls preserved");
    assert_eq!(back_calls.len(), 2);
    assert_eq!(back_calls[0].id, "call_1");
    assert_eq!(back_calls[0].name, "list_files");
    assert_eq!(back_calls[0].arguments, serde_json::json!({"path": "/tmp"}));
    assert_eq!(back_calls[1].id, "call_2");
}

#[test]
fn req_dasclaw_session_d1_assistant_tool_calls_only_no_text_round_trip() {
    // assistant with empty content but tool_calls — leading text block
    // must NOT appear in claw output (empty text is meaningless).
    let calls = vec![ToolCall {
        id: "call_x".to_string(),
        name: "noop".to_string(),
        arguments: serde_json::json!({}),
        reasoning: None,
    }];
    let original = ChatMessage::assistant_with_tool_calls(None, calls);
    assert_eq!(original.content, ""); // sanity

    let claw = chat_message_to_claw(&original);
    assert_eq!(claw.blocks.len(), 1, "no leading empty Text block expected");
    matches!(claw.blocks[0], ClawContentBlock::ToolUse { .. });

    let back = claw_to_chat_message(&claw);
    assert!(back.tool_calls.is_some());
    assert_eq!(back.content, "");
}

#[test]
fn req_dasclaw_session_d1_tool_result_round_trip() {
    let original = ChatMessage::tool_result("call_1", "list_files", "a.txt\nb.txt");
    let claw = chat_message_to_claw(&original);
    assert_eq!(claw.role, ClawRole::Tool);
    match &claw.blocks[..] {
        [
            ClawContentBlock::ToolResult {
                tool_use_id,
                tool_name,
                output,
                is_error,
            },
        ] => {
            assert_eq!(tool_use_id, "call_1");
            assert_eq!(tool_name, "list_files");
            assert_eq!(output, "a.txt\nb.txt");
            assert!(!is_error);
        }
        other => panic!("expected single ToolResult, got {other:?}"),
    }

    let back = claw_to_chat_message(&claw);
    assert_eq!(back.role, Role::Tool);
    assert_eq!(back.tool_call_id.as_deref(), Some("call_1"));
    assert_eq!(back.name.as_deref(), Some("list_files"));
    assert_eq!(back.content, "a.txt\nb.txt");
}

#[test]
fn req_dasclaw_session_d1_content_parts_images_drop_to_text_only() {
    // claw-code's ContentBlock has no image variant; convert is lossy
    // on the image side but text content survives.
    let original = ChatMessage::user_with_parts(
        "describe this",
        vec![ContentPart::ImageUrl {
            image_url: ImageUrl {
                url: "https://example/x.png".to_string(),
                detail: None,
            },
        }],
    );
    let claw = chat_message_to_claw(&original);
    assert_eq!(claw.blocks.len(), 1);
    match &claw.blocks[0] {
        ClawContentBlock::Text { text } => assert_eq!(text, "describe this"),
        other => panic!("expected Text only, got {other:?}"),
    }
}

#[test]
fn req_dasclaw_session_d1_claw_message_serde_matches_claw_code_wire() {
    // Hand-crafted claw-code-shaped JSON must parse via ClawMessage.
    let raw = r#"{
        "role": "assistant",
        "blocks": [
            {"type": "text", "text": "let me check"},
            {"type": "tool_use", "id": "call_a", "name": "ls", "input": "{\"path\":\"/\"}"}
        ]
    }"#;
    let parsed: ClawMessage = serde_json::from_str(raw).expect("parse claw wire");
    assert_eq!(parsed.role, ClawRole::Assistant);
    assert_eq!(parsed.blocks.len(), 2);

    // And our re-serialize must produce a value claw-code can read
    // back (type tag preserved, field names preserved).
    let again = serde_json::to_value(&parsed).expect("ser");
    assert_eq!(
        again["blocks"][0]["type"].as_str(),
        Some("text"),
        "block type tag must be \"text\""
    );
    assert_eq!(
        again["blocks"][1]["type"].as_str(),
        Some("tool_use"),
        "block type tag must be \"tool_use\""
    );
    assert_eq!(again["role"].as_str(), Some("assistant"));
}

#[test]
fn req_dasclaw_session_d1_tool_result_claw_code_wire_parses() {
    let raw = r#"{
        "role": "tool",
        "blocks": [{
            "type": "tool_result",
            "tool_use_id": "call_a",
            "tool_name": "ls",
            "output": "x\ny",
            "is_error": false
        }]
    }"#;
    let parsed: ClawMessage = serde_json::from_str(raw).expect("parse claw tool result wire");
    assert_eq!(parsed.role, ClawRole::Tool);
    match &parsed.blocks[..] {
        [
            ClawContentBlock::ToolResult {
                tool_use_id,
                tool_name,
                output,
                is_error,
            },
        ] => {
            assert_eq!(tool_use_id, "call_a");
            assert_eq!(tool_name, "ls");
            assert_eq!(output, "x\ny");
            assert!(!is_error);
        }
        other => panic!("expected ToolResult, got {other:?}"),
    }
}
