//! PR2 GUI-only blind-spot: `dasclaw_channels` public-surface contract.
//!
//! Today the only in-tree consumer of [`dasclaw_channels`] is
//! `desktop-client/ironclaw` (e.g. `src/channels/mod.rs:45`). The headless
//! CLI never links the crate in production, so any silent breakage in
//! `StatusUpdate` / `IncomingMessage` / `OutgoingResponse` / the metadata
//! helpers escapes the ADR-153 §1 CLI test matrix.
//!
//! This file pins the public surface from a non-GUI consumer:
//!
//! 1. `StatusUpdate` variant inventory — pattern-exhaustive so a new
//!    variant either updates this test or breaks the build (catches a
//!    common "added one more variant, GUI updated, headless contract
//!    drifted" failure mode).
//! 2. `AttachmentKind::from_mime_type` dispatch (audio / image / fallback).
//! 3. `IncomingMessage::new` + builder chain default invariants
//!    (`is_internal == false`, `owner_id == user_id`, conversation scope
//!    falls back to `thread_id`).
//! 4. `tool_enriched_metadata` JSON-shape invariants (keys present,
//!    non-tool keys untouched).
//! 5. `routing_target_from_metadata` parses `target` from both strings
//!    and numbers; `OutgoingResponse::text` default field state.
//!
//! Note: these types deliberately do **not** derive `serde::Serialize`
//! (only `Debug, Clone`), so we can't pin them via
//! `serde_json::to_value`. Structural assertions are the available
//! contract surface today; promoting `StatusUpdate` to `Serialize` is
//! a separate decision.

use dasclaw_channels::{
    AttachmentKind, IncomingAttachment, IncomingMessage, OutgoingResponse, StatusUpdate,
    ToolDecision, routing_target_from_metadata, tool_enriched_metadata,
};
use serde_json::json;

/// `req_dasclaw_cli_channels_pr2_status_update_variants_exhaustive` —
/// pattern-match every public `StatusUpdate` variant. If a new variant
/// is added without updating this match, the build breaks; that is the
/// guard against silent surface drift.
#[test]
fn req_dasclaw_cli_channels_pr2_status_update_variants_exhaustive() {
    let samples = [
        StatusUpdate::Thinking("think".into()),
        StatusUpdate::ToolStarted {
            name: "shell".into(),
        },
        StatusUpdate::ToolCompleted {
            name: "shell".into(),
            success: true,
            error: None,
            parameters: None,
        },
        StatusUpdate::ToolResult {
            name: "shell".into(),
            preview: "ok".into(),
        },
        StatusUpdate::StreamChunk("chunk".into()),
        StatusUpdate::Status("status".into()),
        StatusUpdate::JobStarted {
            job_id: "j1".into(),
            title: "t".into(),
            browse_url: "/jobs/j1".into(),
        },
        StatusUpdate::ApprovalNeeded {
            request_id: "r1".into(),
            tool_name: "shell".into(),
            description: "rm -rf /".into(),
            parameters: json!({}),
            allow_always: false,
        },
        StatusUpdate::AuthRequired {
            extension_name: "gh".into(),
            instructions: None,
            auth_url: None,
            setup_url: None,
        },
        StatusUpdate::AuthCompleted {
            extension_name: "gh".into(),
            success: true,
            message: "ok".into(),
        },
        StatusUpdate::ImageGenerated {
            data_url: "data:image/png;base64,AAA".into(),
            path: None,
        },
        StatusUpdate::Suggestions {
            suggestions: vec!["a".into(), "b".into()],
        },
        StatusUpdate::ReasoningUpdate {
            narrative: "n".into(),
            decisions: vec![ToolDecision {
                tool_name: "shell".into(),
                rationale: "test".into(),
            }],
        },
        StatusUpdate::TurnCost {
            input_tokens: 1,
            output_tokens: 2,
            cost_usd: "0.001".into(),
        },
    ];
    // Exhaustive match — the build breaks if a new variant lands
    // upstream without extending this list.
    let mut counted = 0usize;
    for s in &samples {
        match s {
            StatusUpdate::Thinking(_)
            | StatusUpdate::ToolStarted { .. }
            | StatusUpdate::ToolCompleted { .. }
            | StatusUpdate::ToolResult { .. }
            | StatusUpdate::StreamChunk(_)
            | StatusUpdate::Status(_)
            | StatusUpdate::JobStarted { .. }
            | StatusUpdate::ApprovalNeeded { .. }
            | StatusUpdate::AuthRequired { .. }
            | StatusUpdate::AuthCompleted { .. }
            | StatusUpdate::ImageGenerated { .. }
            | StatusUpdate::Suggestions { .. }
            | StatusUpdate::ReasoningUpdate { .. }
            | StatusUpdate::TurnCost { .. } => counted += 1,
        }
    }
    assert_eq!(
        counted,
        samples.len(),
        "every sampled variant must be matched by the exhaustive arm"
    );
}

/// `req_dasclaw_cli_channels_pr2_attachment_kind_mime_dispatch` —
/// MIME-to-kind dispatch must not depend on parameters after `;` and
/// must fall through to `Document` for unknown bases.
#[test]
fn req_dasclaw_cli_channels_pr2_attachment_kind_mime_dispatch() {
    assert_eq!(
        AttachmentKind::from_mime_type("audio/ogg"),
        AttachmentKind::Audio
    );
    assert_eq!(
        AttachmentKind::from_mime_type("audio/mpeg; codecs=mp3"),
        AttachmentKind::Audio,
        "MIME params after `;` must be stripped before matching"
    );
    assert_eq!(
        AttachmentKind::from_mime_type("image/jpeg"),
        AttachmentKind::Image
    );
    assert_eq!(
        AttachmentKind::from_mime_type("application/pdf"),
        AttachmentKind::Document,
        "non audio/* and non image/* must fall through to Document"
    );
    assert_eq!(
        AttachmentKind::from_mime_type("text/plain"),
        AttachmentKind::Document
    );
}

/// `req_dasclaw_cli_channels_pr2_incoming_message_defaults_and_builder` —
/// the builder chain preserves invariants that other crates rely on:
/// internal flag defaults to `false`, owner+sender mirror `user_id`,
/// `with_thread` also seeds `conversation_scope_id`, and
/// `conversation_scope()` falls back to `thread_id` for legacy callers.
#[test]
fn req_dasclaw_cli_channels_pr2_incoming_message_defaults_and_builder() {
    let msg = IncomingMessage::new("cli", "alice", "hello");
    assert_eq!(msg.channel, "cli");
    assert_eq!(msg.user_id, "alice");
    assert_eq!(msg.owner_id, "alice", "owner_id defaults to user_id");
    assert_eq!(msg.sender_id, "alice", "sender_id defaults to user_id");
    assert!(!msg.is_internal, "is_internal must default to false");
    assert!(msg.attachments.is_empty());
    assert!(msg.thread_id.is_none());
    assert!(msg.conversation_scope_id.is_none());

    let threaded = msg.clone().with_thread("T1");
    assert_eq!(threaded.thread_id.as_deref(), Some("T1"));
    assert_eq!(
        threaded.conversation_scope_id.as_deref(),
        Some("T1"),
        "with_thread must also seed conversation_scope_id"
    );
    assert_eq!(threaded.conversation_scope(), Some("T1"));

    // Legacy fallback: when only thread_id is set, conversation_scope()
    // still resolves it. Use a fresh message to exercise the fallback
    // branch (the builder above sets both).
    let mut legacy = IncomingMessage::new("cli", "bob", "hi");
    legacy.thread_id = Some("L1".into());
    legacy.conversation_scope_id = None;
    assert_eq!(legacy.conversation_scope(), Some("L1"));

    // into_internal flips the internal flag without touching identity.
    let internal = IncomingMessage::new("cli", "sys", "x").into_internal();
    assert!(internal.is_internal);
    assert_eq!(internal.user_id, "sys");
}

/// `req_dasclaw_cli_channels_pr2_tool_enriched_metadata_preserves_base` —
/// the helper must (a) insert `_tool_call_id`, (b) insert
/// `_tool_arguments` only when supplied, and (c) leave caller-supplied
/// keys untouched.
#[test]
fn req_dasclaw_cli_channels_pr2_tool_enriched_metadata_preserves_base() {
    let base = json!({ "target": "chat-42", "user_label": "Alice" });
    let args = json!({"cmd": "ls"});

    let enriched = tool_enriched_metadata(&base, "call-1", Some(&args));
    let obj = enriched.as_object().expect("must be an object");
    assert_eq!(obj.get("_tool_call_id"), Some(&json!("call-1")));
    assert_eq!(obj.get("_tool_arguments"), Some(&args));
    assert_eq!(
        obj.get("target"),
        Some(&json!("chat-42")),
        "pre-existing keys must be preserved verbatim"
    );
    assert_eq!(obj.get("user_label"), Some(&json!("Alice")));

    // Without args: key must be absent (not null).
    let without = tool_enriched_metadata(&base, "call-2", None);
    let without_obj = without.as_object().expect("must be an object");
    assert_eq!(without_obj.get("_tool_call_id"), Some(&json!("call-2")));
    assert!(
        !without_obj.contains_key("_tool_arguments"),
        "arguments key must be omitted when None is supplied"
    );
}

/// `req_dasclaw_cli_channels_pr2_routing_target_parses_string_and_number`
/// — Telegram-shaped numeric chat ids and string targets must both
/// resolve through the helper.
#[test]
fn req_dasclaw_cli_channels_pr2_routing_target_parses_string_and_number() {
    assert_eq!(
        routing_target_from_metadata(&json!({"target": "chat-1"})),
        Some("chat-1".to_string())
    );
    assert_eq!(
        routing_target_from_metadata(&json!({"target": 9_876_543_210_u64})),
        Some("9876543210".to_string()),
        "numeric chat ids (Telegram-style) must serialize to string"
    );
    assert_eq!(routing_target_from_metadata(&json!({})), None);
    assert_eq!(routing_target_from_metadata(&serde_json::Value::Null), None);
}

/// `req_dasclaw_cli_channels_pr2_outgoing_response_text_defaults` —
/// `OutgoingResponse::text` produces an empty-thread, empty-attachments,
/// null-metadata response. Builder methods set fields in isolation.
#[test]
fn req_dasclaw_cli_channels_pr2_outgoing_response_text_defaults() {
    let r = OutgoingResponse::text("hi");
    assert_eq!(r.content, "hi");
    assert!(r.thread_id.is_none());
    assert!(r.attachments.is_empty());
    assert_eq!(r.metadata, serde_json::Value::Null);

    let chained = OutgoingResponse::text("hi")
        .in_thread("T")
        .with_attachments(vec!["a.png".into()]);
    assert_eq!(chained.thread_id.as_deref(), Some("T"));
    assert_eq!(chained.attachments, vec!["a.png".to_string()]);
}

/// `req_dasclaw_cli_channels_pr2_incoming_attachment_round_trip` —
/// `IncomingAttachment` accepts all optional fields as `None` and an
/// empty body. Pins that callers can construct a minimal attachment
/// without reaching for private constructors.
#[test]
fn req_dasclaw_cli_channels_pr2_incoming_attachment_round_trip() {
    let att = IncomingAttachment {
        id: "f1".into(),
        kind: AttachmentKind::Image,
        mime_type: "image/png".into(),
        filename: None,
        size_bytes: None,
        source_url: None,
        storage_key: None,
        extracted_text: None,
        data: Vec::new(),
        duration_secs: None,
    };
    assert_eq!(att.kind, AttachmentKind::Image);
    assert!(att.data.is_empty());
    assert!(att.filename.is_none());
}
