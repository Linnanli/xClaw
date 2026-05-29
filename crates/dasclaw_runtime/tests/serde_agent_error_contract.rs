//! Issue #909 (GUI blocker B3): JSON round-trip + `insta` snapshot pins for
//! `AgentError`.
//!
//! `AgentError` has a hand-written `Serialize` / `Deserialize` impl
//! (see `agent.rs`) because serde's adjacent-tagged derive emits a
//! `HostError: Deserialize` bound that `Box<dyn Error>` cannot satisfy.
//! These tests pin the resulting wire shape and the lossy-by-design
//! round trip for the `Responder` variant (`HostError` becomes a
//! `StringHostError` after deserialize).

use dasclaw_runtime::agent::{AgentError, StringHostError};

fn roundtrip(value: &AgentError) -> AgentError {
    let json = serde_json::to_string(value).expect("serialize");
    serde_json::from_str(&json).expect("deserialize")
}

// -------------------------------------------------------------------------
// req_dasclaw_runtime_909_agent_error_simple_variants_roundtrip
// -------------------------------------------------------------------------
//
// Every unit + tuple variant except `Responder` round-trips losslessly
// because the inner types are `String` / `usize` which derive serde.

#[test]
#[allow(deprecated)] // exercises the `ApprovalRequested` wire-compat variant
fn req_dasclaw_runtime_909_agent_error_simple_variants_roundtrip() {
    let samples = [
        AgentError::MissingResponder,
        AgentError::MaxIterations(7),
        AgentError::ToolsNotSupported,
        AgentError::LoopFailure("hook blocked".into()),
        AgentError::Stopped,
        AgentError::ApprovalRequested,
        AgentError::ApprovalRejected {
            tool_name: "bash".into(),
            reason: Some("user denied".into()),
        },
        AgentError::ApprovalRejected {
            tool_name: "web_fetch".into(),
            reason: None,
        },
    ];
    for sample in samples {
        let back = roundtrip(&sample);
        // No PartialEq on AgentError; compare via the Display impl, which is
        // stable per `thiserror` derive and what GUI consumers render anyway.
        assert_eq!(back.to_string(), sample.to_string());
    }
}

// -------------------------------------------------------------------------
// req_dasclaw_runtime_909_agent_error_responder_variant_stringifies
// -------------------------------------------------------------------------
//
// The `Responder(HostError)` variant carries `Box<dyn Error + Send + Sync>`
// which is not serde-friendly. Issue #909 §3 picked the "stringify" option:
// serialize as the `Display` string, deserialize back into a `StringHostError`
// shim. The Rust type is erased, but the message is preserved.

#[test]
fn req_dasclaw_runtime_909_agent_error_responder_variant_stringifies() {
    let original_msg = "upstream provider 502";
    let sample = AgentError::Responder(Box::new(std::io::Error::other(original_msg)));
    let json = serde_json::to_string(&sample).expect("serialize");
    assert!(
        json.contains(original_msg),
        "serialized JSON must embed the host-error message; got: {json}"
    );

    let back: AgentError = serde_json::from_str(&json).expect("deserialize");
    match back {
        AgentError::Responder(err) => {
            assert_eq!(err.to_string(), original_msg);
            // After round-trip the concrete type is StringHostError, not io::Error.
            assert!(
                err.downcast_ref::<StringHostError>().is_some(),
                "deserialized Responder error must downcast to StringHostError"
            );
            assert!(
                err.downcast_ref::<std::io::Error>().is_none(),
                "original io::Error type is intentionally erased on the wire"
            );
        }
        other => panic!("expected Responder variant, got: {other:?}"),
    }
}

// -------------------------------------------------------------------------
// insta snapshots — pin the wire shape so a re-tag or rename breaks the test.
// -------------------------------------------------------------------------

#[test]
fn req_dasclaw_runtime_909_snapshot_agent_error_missing_responder() {
    insta::assert_json_snapshot!(
        "agent_error_missing_responder",
        AgentError::MissingResponder
    );
}

#[test]
fn req_dasclaw_runtime_909_snapshot_agent_error_max_iterations() {
    insta::assert_json_snapshot!("agent_error_max_iterations", AgentError::MaxIterations(50));
}

#[test]
fn req_dasclaw_runtime_909_snapshot_agent_error_loop_failure() {
    insta::assert_json_snapshot!(
        "agent_error_loop_failure",
        AgentError::LoopFailure("hook blocked egress".into())
    );
}

#[test]
fn req_dasclaw_runtime_b4_snapshot_agent_error_approval_rejected_with_reason() {
    insta::assert_json_snapshot!(
        "agent_error_approval_rejected_with_reason",
        AgentError::ApprovalRejected {
            tool_name: "bash".into(),
            reason: Some("user denied".into()),
        }
    );
}

#[test]
fn req_dasclaw_runtime_b4_snapshot_agent_error_approval_rejected_no_reason() {
    insta::assert_json_snapshot!(
        "agent_error_approval_rejected_no_reason",
        AgentError::ApprovalRejected {
            tool_name: "web_fetch".into(),
            reason: None,
        }
    );
}

#[test]
fn req_dasclaw_runtime_909_snapshot_agent_error_responder() {
    let sample = AgentError::Responder(Box::new(std::io::Error::other(
        "upstream provider 502 Bad Gateway",
    )));
    insta::assert_json_snapshot!("agent_error_responder", sample);
}
