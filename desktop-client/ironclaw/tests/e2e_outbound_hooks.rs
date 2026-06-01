//! E2E coverage for outbound event hooks in the desktop agent loop.

#[cfg(feature = "libsql")]
mod support;

#[cfg(feature = "libsql")]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use async_trait::async_trait;
    use dasclaw_hooks::{
        Hook, HookContext, HookError, HookEvent, HookFailureMode, HookOutcome, HookPoint,
    };

    use crate::support::test_rig::TestRigBuilder;
    use crate::support::trace_llm::{LlmTrace, TraceResponse, TraceStep};

    struct RejectOutboundHook;

    #[async_trait]
    impl Hook for RejectOutboundHook {
        fn name(&self) -> &str {
            "reject-outbound-test"
        }

        fn hook_points(&self) -> &[HookPoint] {
            &[HookPoint::BeforeOutbound]
        }

        fn timeout(&self) -> Duration {
            Duration::from_secs(1)
        }

        fn failure_mode(&self) -> HookFailureMode {
            HookFailureMode::FailClosed
        }

        async fn execute(
            &self,
            event: &HookEvent,
            _ctx: &HookContext,
        ) -> Result<HookOutcome, HookError> {
            assert!(
                matches!(event, HookEvent::Outbound { .. }),
                "test hook must only receive outbound events"
            );
            Ok(HookOutcome::reject("policy blocked outbound response"))
        }
    }

    fn text_step(content: &str) -> TraceStep {
        TraceStep {
            request_hint: None,
            response: TraceResponse::Text {
                content: content.to_string(),
                input_tokens: 10,
                output_tokens: 5,
            },
            expected_tool_results: Vec::new(),
        }
    }

    #[tokio::test]
    async fn req_desktop_client_chat_before_outbound_rejection_emits_terminal_response() {
        let original = "ORIGINAL_RESPONSE_SHOULD_NOT_LEAK";
        let trace = LlmTrace::single_turn("outbound-hook-test", "你好", vec![text_step(original)]);

        let rig = TestRigBuilder::new()
            .with_trace(trace)
            .with_hook(Arc::new(RejectOutboundHook))
            .build()
            .await;

        rig.send_message("你好").await;
        let responses = rig.wait_for_responses(1, Duration::from_secs(5)).await;

        assert_eq!(
            responses.len(),
            1,
            "BeforeOutbound rejection must emit a terminal response so the UI does not stay loading"
        );
        assert!(
            !responses[0].content.contains(original),
            "blocked outbound content must not be sent to the client"
        );
        assert!(
            responses[0].content.contains("outbound response blocked"),
            "blocked response should explain why the turn ended, got: {:?}",
            responses[0].content
        );

        rig.shutdown();
    }
}
