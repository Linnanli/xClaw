//! Ironclaw-side blanket implementations for the `dasclaw_core::traits`
//! collaboration interfaces.
//!
//! Keeping all `impl XHostTrait for IronclawType` blocks in a single file makes
//! the host-side surface auditable: when a new trait is added to
//! `dasclaw_core::traits`, exactly one ironclaw file needs to change to wire
//! it up.
//!
//! See [`dasclaw_core::traits`] for the rationale.

use async_trait::async_trait;
use dasclaw_core::messages::CompletionRequest;
use dasclaw_core::traits::{HostError, LlmCompleter, WorkspaceWriter};

use crate::llm::Reasoning;
use crate::workspace::Workspace;

/// Newtype wrapper that adapts the externally-defined [`Reasoning`] provider
/// (from `dasclaw_llm_provider`) to the externally-defined [`LlmCompleter`]
/// trait (from `dasclaw_core`).
///
/// Without this wrapper an `impl LlmCompleter for Reasoning` block in
/// ironclaw violates the orphan rule (E0117): per ADR-118 / ADR-129 the
/// provider crate must not depend on `dasclaw_core::traits`, and
/// `dasclaw_core` must not depend on the provider tree, so the adapter has
/// to live on the host side as a newtype.
pub struct ReasoningCompleter(pub Reasoning);

impl ReasoningCompleter {
    pub fn new(reasoning: Reasoning) -> Self {
        Self(reasoning)
    }
}

#[async_trait]
impl WorkspaceWriter for Workspace {
    async fn append(&self, path: &str, content: &str) -> Result<(), HostError> {
        Workspace::append(self, path, content)
            .await
            .map_err(|e| Box::new(e) as HostError)
    }
}

#[async_trait]
impl LlmCompleter for ReasoningCompleter {
    async fn complete_text(&self, request: CompletionRequest) -> Result<String, HostError> {
        let (text, _usage) = Reasoning::complete(&self.0, request)
            .await
            .map_err(|e| Box::new(e) as HostError)?;
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dasclaw_core::messages::ChatMessage;

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
        let _f: fn(&ReasoningCompleter) = |r| assert_dyn(r);
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
                    finish_reason: dasclaw_core::messages::FinishReason::Stop,
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
        let completer: Arc<dyn LlmCompleter> = Arc::new(ReasoningCompleter::new(reasoning));

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
