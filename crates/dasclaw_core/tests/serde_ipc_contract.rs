//! Issue #909 (GUI blocker B3): JSON round-trip + `insta` snapshot pins for
//! the IPC-critical types `TokenUsage`, `FinishReason`, `ToolResult`,
//! `RespondResult`, `RespondOutput`, and `LoopOutcome`.
//!
//! These types cross the Rust → JSON → JS/TS boundary in Tauri / Electron
//! GUIs. The snapshots pin the exact wire shape so a `#[serde(rename_all)]`,
//! a variant rename, or a new field fails the test instead of silently
//! breaking GUI consumers.
//!
//! Approach mirrors PR #902 / #903: real `serde_json::to_string` →
//! `serde_json::from_str` round trip for every constructable variant, then
//! one `insta::assert_json_snapshot!` per non-trivial enum variant.

use dasclaw_core::agentic_loop::LoopOutcome;
use dasclaw_core::messages::{FinishReason, ToolCall, ToolResult};
use dasclaw_core::response_types::{
    RespondOutput, RespondResult, ResponseAnomaly, ResponseMetadata, ResponseModelVerification,
    TokenUsage,
};
use serde_json::json;

// -------------------------------------------------------------------------
// Round-trip helpers
// -------------------------------------------------------------------------

fn roundtrip<T>(value: &T) -> T
where
    T: serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    let json = serde_json::to_string(value).expect("serialize");
    serde_json::from_str(&json).expect("deserialize")
}

// -------------------------------------------------------------------------
// req_dasclaw_core_909_token_usage_roundtrip
// -------------------------------------------------------------------------

#[test]
fn req_dasclaw_core_909_token_usage_roundtrip() {
    let sample = TokenUsage {
        input_tokens: 10,
        output_tokens: 20,
        cache_read_input_tokens: 3,
        cache_creation_input_tokens: 4,
    };
    let back = roundtrip(&sample);
    assert_eq!(back.input_tokens, sample.input_tokens);
    assert_eq!(back.output_tokens, sample.output_tokens);
    assert_eq!(back.cache_read_input_tokens, sample.cache_read_input_tokens);
    assert_eq!(
        back.cache_creation_input_tokens,
        sample.cache_creation_input_tokens
    );
}

// -------------------------------------------------------------------------
// req_dasclaw_core_909_finish_reason_roundtrip_all_variants
// -------------------------------------------------------------------------

#[test]
fn req_dasclaw_core_909_finish_reason_roundtrip_all_variants() {
    let variants = [
        FinishReason::Stop,
        FinishReason::Length,
        FinishReason::ToolUse,
        FinishReason::ContentFilter,
        FinishReason::Unknown,
    ];
    for v in variants {
        assert_eq!(roundtrip(&v), v);
    }
}

// -------------------------------------------------------------------------
// req_dasclaw_core_909_response_anomaly_roundtrip
// -------------------------------------------------------------------------

#[test]
fn req_dasclaw_core_909_response_anomaly_roundtrip() {
    for v in [
        ResponseAnomaly::EmptyToolCompletion,
        ResponseAnomaly::EmptyTextResponse,
    ] {
        assert_eq!(roundtrip(&v), v);
    }
}

// -------------------------------------------------------------------------
// req_dasclaw_core_909_response_metadata_roundtrip
// -------------------------------------------------------------------------

#[test]
fn req_dasclaw_core_909_response_metadata_roundtrip() {
    let with = ResponseMetadata {
        anomaly: Some(ResponseAnomaly::EmptyTextResponse),
        ..ResponseMetadata::default()
    };
    assert_eq!(roundtrip(&with), with);

    let without = ResponseMetadata::default();
    assert_eq!(roundtrip(&without), without);
}

#[test]
fn response_metadata_roundtrip() {
    let sample = ResponseMetadata {
        anomaly: Some(ResponseAnomaly::EmptyTextResponse),
        actual_model: Some("gpt-5.5-cyber".to_string()),
        model_verifications: vec![ResponseModelVerification::TrustedAccessForCyber],
    };

    let json = serde_json::to_value(&sample).expect("serialize response metadata");
    assert_eq!(json["actualModel"], "gpt-5.5-cyber");
    assert_eq!(
        json["modelVerifications"],
        serde_json::json!(["trustedAccessForCyber"])
    );
    assert!(json.get("actual_model").is_none());
    assert!(json.get("model_verifications").is_none());

    assert_eq!(roundtrip(&sample), sample);
}

#[test]
fn response_metadata_deserializes_legacy_anomaly_only_payload() {
    let metadata: ResponseMetadata =
        serde_json::from_value(json!({ "anomaly": null })).expect("deserialize legacy metadata");

    assert_eq!(metadata.anomaly, None);
    assert_eq!(metadata.actual_model, None);
    assert!(metadata.model_verifications.is_empty());
}

// -------------------------------------------------------------------------
// req_dasclaw_core_909_tool_result_roundtrip
// -------------------------------------------------------------------------

#[test]
fn req_dasclaw_core_909_tool_result_roundtrip() {
    let sample = ToolResult {
        tool_call_id: "call-1".into(),
        name: "shell".into(),
        content: "stdout".into(),
        is_error: false,
    };
    let back = roundtrip(&sample);
    assert_eq!(back.tool_call_id, sample.tool_call_id);
    assert_eq!(back.name, sample.name);
    assert_eq!(back.content, sample.content);
    assert_eq!(back.is_error, sample.is_error);
}

// -------------------------------------------------------------------------
// req_dasclaw_core_909_respond_result_text_roundtrip
// -------------------------------------------------------------------------

#[test]
fn req_dasclaw_core_909_respond_result_text_roundtrip() {
    let sample = RespondResult::Text("hello".into());
    let back = roundtrip(&sample);
    match back {
        RespondResult::Text(s) => assert_eq!(s, "hello"),
        other => panic!("unexpected variant: {other:?}"),
    }
}

// -------------------------------------------------------------------------
// req_dasclaw_core_909_respond_result_tool_calls_roundtrip
// -------------------------------------------------------------------------

#[test]
fn req_dasclaw_core_909_respond_result_tool_calls_roundtrip() {
    let sample = RespondResult::ToolCalls {
        tool_calls: vec![ToolCall {
            id: "abc123def".into(),
            name: "shell".into(),
            arguments: json!({"cmd": "ls"}),
            reasoning: None,
        }],
        content: Some("calling shell".into()),
    };
    let back = roundtrip(&sample);
    match back {
        RespondResult::ToolCalls {
            tool_calls,
            content,
        } => {
            assert_eq!(tool_calls.len(), 1);
            assert_eq!(tool_calls[0].id, "abc123def");
            assert_eq!(content.as_deref(), Some("calling shell"));
        }
        other => panic!("unexpected variant: {other:?}"),
    }
}

// -------------------------------------------------------------------------
// req_dasclaw_core_909_respond_output_roundtrip
// -------------------------------------------------------------------------

#[test]
fn req_dasclaw_core_909_respond_output_roundtrip() {
    let sample = RespondOutput {
        result: RespondResult::Text("done".into()),
        usage: TokenUsage {
            input_tokens: 1,
            output_tokens: 2,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        },
        finish_reason: FinishReason::Stop,
        metadata: ResponseMetadata::default(),
    };
    let back = roundtrip(&sample);
    assert!(matches!(back.result, RespondResult::Text(ref s) if s == "done"));
    assert_eq!(back.finish_reason, FinishReason::Stop);
}

// -------------------------------------------------------------------------
// req_dasclaw_core_909_loop_outcome_simple_variants_roundtrip
// -------------------------------------------------------------------------
//
// Covers every variant except `NeedApproval(Box<PendingApproval>)`, which
// is exercised separately because `PendingApproval` has non-trivial inner
// state (chrono timestamps, uuids) that we leave the upstream type to
// manage.

#[test]
fn req_dasclaw_core_909_loop_outcome_simple_variants_roundtrip() {
    let samples = [
        LoopOutcome::Response {
            text: "hi".into(),
            usage: TokenUsage::default(),
            metadata: ResponseMetadata::default(),
        },
        LoopOutcome::Stopped,
        LoopOutcome::MaxIterations,
        LoopOutcome::Failure("hook blocked".into()),
    ];
    for sample in samples {
        let json = serde_json::to_string(&sample).expect("serialize");
        let back: LoopOutcome = serde_json::from_str(&json).expect("deserialize");
        // Compare via re-serialization since LoopOutcome has no PartialEq.
        let again = serde_json::to_string(&back).expect("re-serialize");
        assert_eq!(json, again);
    }
}

#[test]
fn loop_outcome_response_deserializes_legacy_string_payload() {
    let payload = json!({
        "kind": "response",
        "data": "done"
    });

    let outcome: LoopOutcome =
        serde_json::from_value(payload).expect("deserialize legacy loop response");

    match outcome {
        LoopOutcome::Response {
            text,
            usage,
            metadata,
        } => {
            assert_eq!(text, "done");
            assert_eq!(usage, TokenUsage::default());
            assert_eq!(metadata, ResponseMetadata::default());
        }
        other => panic!("unexpected variant: {other:?}"),
    }
}

// -------------------------------------------------------------------------
// insta snapshots (pin wire shape, not just round-trip equality)
// -------------------------------------------------------------------------

#[test]
fn req_dasclaw_core_909_snapshot_token_usage() {
    let sample = TokenUsage {
        input_tokens: 10,
        output_tokens: 20,
        cache_read_input_tokens: 3,
        cache_creation_input_tokens: 4,
    };
    insta::assert_json_snapshot!("token_usage", sample);
}

#[test]
fn req_dasclaw_core_909_snapshot_finish_reason_tool_use() {
    insta::assert_json_snapshot!("finish_reason_tool_use", FinishReason::ToolUse);
}

#[test]
fn req_dasclaw_core_909_snapshot_tool_result() {
    let sample = ToolResult {
        tool_call_id: "call-1".into(),
        name: "shell".into(),
        content: "ok".into(),
        is_error: false,
    };
    insta::assert_json_snapshot!("tool_result", sample);
}

#[test]
fn req_dasclaw_core_909_snapshot_respond_output_text() {
    let sample = RespondOutput {
        result: RespondResult::Text("hello".into()),
        usage: TokenUsage {
            input_tokens: 5,
            output_tokens: 7,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        },
        finish_reason: FinishReason::Stop,
        metadata: ResponseMetadata::default(),
    };
    insta::assert_json_snapshot!("respond_output_text", sample);
}

#[test]
fn req_dasclaw_core_909_snapshot_respond_result_tool_calls() {
    let sample = RespondResult::ToolCalls {
        tool_calls: vec![ToolCall {
            id: "abc123def".into(),
            name: "shell".into(),
            arguments: json!({"cmd": "ls"}),
            reasoning: None,
        }],
        content: Some("calling shell".into()),
    };
    insta::assert_json_snapshot!("respond_result_tool_calls", sample);
}

#[test]
fn req_dasclaw_core_909_snapshot_loop_outcome_response() {
    insta::assert_json_snapshot!(
        "loop_outcome_response",
        LoopOutcome::Response {
            text: "done".into(),
            usage: TokenUsage::default(),
            metadata: ResponseMetadata::default(),
        }
    );
}

#[test]
fn req_dasclaw_core_909_snapshot_loop_outcome_failure() {
    insta::assert_json_snapshot!(
        "loop_outcome_failure",
        LoopOutcome::Failure("hook blocked egress".into())
    );
}

#[test]
fn req_dasclaw_core_909_snapshot_loop_outcome_max_iterations() {
    insta::assert_json_snapshot!("loop_outcome_max_iterations", LoopOutcome::MaxIterations);
}
