//! End-to-end smoke test for ADR-153 §4.4 step 4: prove that
//! `dasclaw_runtime::Agent` runs headlessly (no desktop) via the
//! `dasclaw_cli` library entry point with the built-in fake responder.

use dasclaw_cli::{EchoResponder, run};

#[tokio::test]
async fn echo_responder_round_trip() {
    let reply = run(EchoResponder::new(), "system prompt", "hello world")
        .await
        .expect("agent run should succeed");

    assert!(
        reply.starts_with("echo:"),
        "expected echo prefix, got: {reply:?}"
    );
    assert!(
        reply.contains("hello world"),
        "expected reply to echo prompt, got: {reply:?}"
    );
}
