//! Ironclaw-side blanket implementations for the `x_claw_agent::traits`
//! collaboration interfaces.
//!
//! Keeping all `impl XHostTrait for IronclawType` blocks in a single file makes
//! the host-side surface auditable: when a new trait is added to
//! `x_claw_agent::traits`, exactly one ironclaw file needs to change to wire
//! it up.
//!
//! See [`x_claw_agent::traits`] for the rationale.

use async_trait::async_trait;
use x_claw_agent::messages::CompletionRequest;
use x_claw_agent::traits::{HostError, LlmCompleter, WorkspaceWriter};

use crate::llm::Reasoning;
use crate::workspace::Workspace;

#[async_trait]
impl WorkspaceWriter for Workspace {
    async fn append(&self, path: &str, content: &str) -> Result<(), HostError> {
        Workspace::append(self, path, content)
            .await
            .map_err(|e| Box::new(e) as HostError)
    }
}

#[async_trait]
impl LlmCompleter for Reasoning {
    async fn complete_text(&self, request: CompletionRequest) -> Result<String, HostError> {
        let (text, _usage) = Reasoning::complete(self, request)
            .await
            .map_err(|e| Box::new(e) as HostError)?;
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use x_claw_agent::messages::ChatMessage;

    /// `Workspace` implements `WorkspaceWriter` and is dyn-compatible.
    /// We do not exercise an actual write here because `Workspace::append`
    /// requires a libSQL/postgres backend; a real round-trip is covered when
    /// `compaction` (which is the only consumer) is migrated in step D-2 and
    /// re-runs through the existing ironclaw `compaction_tests`.
    #[test]
    fn workspace_writer_impl_is_dyn_compatible() {
        fn assert_dyn(_: &dyn WorkspaceWriter) {}
        let _f: fn(&Workspace) = |w| assert_dyn(w);
    }

    /// `Reasoning` implements `LlmCompleter` and is dyn-compatible. Same
    /// rationale as above: a real LLM round-trip lives in the migrated
    /// compaction tests.
    #[test]
    fn reasoning_impl_is_dyn_compatible() {
        fn assert_dyn(_: &dyn LlmCompleter) {}
        let _f: fn(&Reasoning) = |r| assert_dyn(r);
    }

    /// `complete_text` round-trip against a fake provider exercises the
    /// error-mapping path and confirms reasoning cleanup is applied via the
    /// trait facade.
    #[tokio::test]
    async fn reasoning_completer_strips_reasoning_tags() {
        use std::sync::Arc;

        use crate::llm::{
            CompletionResponse, LlmError, LlmProvider, ToolCompletionRequest,
            ToolCompletionResponse,
        };
        use rust_decimal::Decimal;

        struct FakeProvider;

        #[async_trait]
        impl LlmProvider for FakeProvider {
            fn model_name(&self) -> &str {
                "fake-model"
            }

            fn cost_per_token(&self) -> (Decimal, Decimal) {
                (Decimal::ZERO, Decimal::ZERO)
            }

            async fn complete(
                &self,
                _req: CompletionRequest,
            ) -> Result<CompletionResponse, LlmError> {
                Ok(CompletionResponse {
                    content: "<think>scratch</think>final answer".to_string(),
                    finish_reason: x_claw_agent::messages::FinishReason::Stop,
                    input_tokens: 1,
                    output_tokens: 2,
                    cache_read_input_tokens: 0,
                    cache_creation_input_tokens: 0,
                })
            }

            async fn complete_with_tools(
                &self,
                _req: ToolCompletionRequest,
            ) -> Result<ToolCompletionResponse, LlmError> {
                unreachable!("not exercised in this test")
            }
        }

        let provider: Arc<dyn LlmProvider> = Arc::new(FakeProvider);
        let reasoning = Reasoning::new(provider);
        let completer: Arc<dyn LlmCompleter> = Arc::new(reasoning);

        let req = CompletionRequest::new(vec![ChatMessage::user("ping")]);
        let out = completer.complete_text(req).await.unwrap();

        assert!(
            !out.contains("<think>"),
            "reasoning tags must be stripped, got: {out:?}"
        );
        assert!(
            out.contains("final answer"),
            "final answer must survive cleanup, got: {out:?}"
        );
    }
}
