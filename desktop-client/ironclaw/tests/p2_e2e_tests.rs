#[cfg(feature = "libsql")]
mod support;

#[cfg(feature = "libsql")]
mod tests {
    use std::time::Duration;

    use serde_json::json;

    use crate::support::test_rig::TestRigBuilder;
    use crate::support::trace_llm::{LlmTrace, TraceResponse, TraceStep, TraceToolCall};

    fn text_step(content: &str, input_tokens: u32, output_tokens: u32) -> TraceStep {
        TraceStep {
            request_hint: None,
            response: TraceResponse::Text {
                content: content.to_string(),
                input_tokens,
                output_tokens,
            },
            expected_tool_results: Vec::new(),
        }
    }

    fn tool_call_step(name: &str, arguments: serde_json::Value) -> TraceStep {
        TraceStep {
            request_hint: None,
            response: TraceResponse::ToolCalls {
                tool_calls: vec![TraceToolCall {
                    id: format!("call_{name}"),
                    name: name.to_string(),
                    arguments,
                }],
                input_tokens: 100,
                output_tokens: 30,
            },
            expected_tool_results: Vec::new(),
        }
    }

    #[tokio::test]
    async fn p2_e2e_plan_mode_submit_flow() {
        let trace = LlmTrace::single_turn(
            "p2-plan-mode",
            "Plan the refactor before executing",
            vec![
                tool_call_step(
                    "plan_mode",
                    json!({
                        "action": "submit",
                        "plan": {
                            "goal": "Refactor auth module",
                            "steps": [
                                {
                                    "description": "Inspect auth handler",
                                    "tool_name": "read_file",
                                    "risk": "low",
                                    "files": ["src/auth.rs"]
                                },
                                {
                                    "description": "Patch auth flow",
                                    "tool_name": "code_edit",
                                    "risk": "medium",
                                    "files": ["src/auth.rs", "tests/auth_tests.rs"]
                                }
                            ],
                            "confidence": 0.82
                        }
                    }),
                ),
                text_step("Plan prepared and waiting for approval.", 140, 20),
            ],
        );

        let rig = TestRigBuilder::new()
            .with_trace(trace.clone())
            .with_auto_approve_tools(true)
            .with_skills()
            .build()
            .await;

        rig.send_message("Plan the refactor before executing").await;
        let responses = rig.wait_for_responses(1, Duration::from_secs(15)).await;

        rig.verify_trace_expects(&trace, &responses);

        let completed = rig.tool_calls_completed();
        assert!(
            completed
                .iter()
                .any(|(name, ok)| name == "plan_mode" && *ok),
            "plan_mode should succeed: {completed:?}"
        );

        let tool_results = rig.tool_results();
        let plan_preview = tool_results
            .iter()
            .find(|(name, _)| name == "plan_mode")
            .map(|(_, preview)| preview.clone())
            .unwrap_or_default();
        assert!(plan_preview.contains("submit") || plan_preview.contains("approval"));

        rig.shutdown();
    }

    #[tokio::test]
    async fn p2_e2e_session_fork_flow() {
        let trace = LlmTrace::single_turn(
            "p2-session-fork",
            "Fork this conversation from turn 2",
            vec![
                tool_call_step(
                    "session_fork",
                    json!({ "at_turn": 2, "reason": "compare alternative" }),
                ),
                text_step(
                    "Created a fork from turn 2 for side-by-side comparison.",
                    120,
                    20,
                ),
            ],
        );

        let rig = TestRigBuilder::new()
            .with_trace(trace.clone())
            .with_auto_approve_tools(true)
            .build()
            .await;

        rig.send_message("Fork this conversation from turn 2").await;
        let responses = rig.wait_for_responses(1, Duration::from_secs(15)).await;

        rig.verify_trace_expects(&trace, &responses);

        let completed = rig.tool_calls_completed();
        assert!(
            completed
                .iter()
                .any(|(name, ok)| name == "session_fork" && *ok),
            "session_fork should succeed: {completed:?}"
        );

        let result_preview = rig
            .tool_results()
            .into_iter()
            .find(|(name, _)| name == "session_fork")
            .map(|(_, preview)| preview)
            .unwrap_or_default();
        assert!(result_preview.contains("fork") || result_preview.contains("turn 2"));

        rig.shutdown();
    }

    #[tokio::test]
    async fn p2_e2e_sub_agent_verify_flow() {
        let trace = LlmTrace::single_turn(
            "p2-sub-agent",
            "Spawn a verify sub-agent to inspect test failures",
            vec![
                tool_call_step(
                    "sub_agent",
                    json!({
                        "role": "verify",
                        "goal": "Inspect failing tests and summarize root causes",
                        "max_turns": 6,
                        "inherit_context": true
                    }),
                ),
                text_step(
                    "Verify sub-agent created and summarization started.",
                    130,
                    25,
                ),
            ],
        );

        let rig = TestRigBuilder::new()
            .with_trace(trace.clone())
            .with_auto_approve_tools(true)
            .build()
            .await;

        rig.send_message("Spawn a verify sub-agent to inspect test failures")
            .await;
        let responses = rig.wait_for_responses(1, Duration::from_secs(15)).await;

        rig.verify_trace_expects(&trace, &responses);

        let completed = rig.tool_calls_completed();
        assert!(
            completed
                .iter()
                .any(|(name, ok)| name == "sub_agent" && *ok),
            "sub_agent should succeed: {completed:?}"
        );

        let preview = rig
            .tool_results()
            .into_iter()
            .find(|(name, _)| name == "sub_agent")
            .map(|(_, preview)| preview)
            .unwrap_or_default();
        assert!(preview.contains("verify") || preview.contains("sub-agent"));

        rig.shutdown();
    }
}
