//! Collaboration traits used by `x_claw_agent` to talk to the host application
//! (ironclaw) without taking concrete dependencies on it.
//!
//! ## Why these exist
//!
//! Modules like `compaction` need to (a) write summaries into a workspace and
//! (b) call an LLM with reasoning-cleanup applied. Both capabilities live in
//! the host crate (ironclaw) and pull in heavy dependencies (`rust_decimal`,
//! `WorkspaceError`, `Reasoning`, etc.) that we deliberately keep out of
//! `x_claw_agent`.
//!
//! Instead we expose two narrow traits here. Ironclaw provides blanket
//! implementations on its existing `Workspace` and `Reasoning` types in
//! `desktop-client/ironclaw/src/agent/traits_impl.rs`, so call sites inside
//! `x_claw_agent` see only `Arc<dyn WorkspaceWriter>` / `Arc<dyn LlmCompleter>`.
//!
//! ## Scope
//!
//! - `WorkspaceWriter` exposes the **single** workspace operation that
//!   `compaction` actually needs (`append(path, content)`). We are explicitly
//!   not modelling the full ironclaw `Workspace` surface here — additional
//!   surface should be added one method at a time, only when a new ported
//!   module needs it.
//! - `LlmCompleter` exposes a single "give me a completion, clean it" entry
//!   point. The implementer is responsible for any reasoning-tag stripping
//!   before returning text. The trait deliberately uses
//!   `Box<dyn std::error::Error + Send + Sync>` so concrete error types
//!   (`WorkspaceError`, `LlmError`) stay inside ironclaw.
//!
//! Future trait additions (Database, Channel, Extension) will land here as
//! later phases need them; we do not pre-declare them.

use async_trait::async_trait;

use crate::messages::CompletionRequest;

/// Boxed error returned by host-provided trait methods.
///
/// Concrete error types (`WorkspaceError`, `LlmError`, etc.) stay in ironclaw;
/// `x_claw_agent` only sees the boxed dyn-error and propagates it.
pub type HostError = Box<dyn std::error::Error + Send + Sync>;

/// Append-only workspace writer used by context compaction and similar
/// archival flows.
///
/// Implementations are expected to be cheap to clone via `Arc`. The `path`
/// argument is workspace-relative (e.g. `"daily/2026-04-21.md"`); resolving
/// it against the actual workspace root is the implementer's job.
#[async_trait]
pub trait WorkspaceWriter: Send + Sync {
    /// Append `content` to the file at `path`, creating it (and any parent
    /// directories) if it does not exist.
    async fn append(&self, path: &str, content: &str) -> Result<(), HostError>;
}

/// Single-shot LLM completion with reasoning-tag cleanup applied.
///
/// This is **not** the full LLM provider trait — it is a narrow facade for
/// "give me a clean string back" use cases (compaction summaries, heartbeat
/// notes, slash-command helpers). Implementations are responsible for any
/// post-processing (e.g. stripping `<think>` blocks) before returning.
#[async_trait]
pub trait LlmCompleter: Send + Sync {
    /// Run the request and return the cleaned-up text body.
    ///
    /// Token usage and other metadata are deliberately not returned here:
    /// callers that need them should use the richer ironclaw `Reasoning` API
    /// directly rather than going through this facade.
    async fn complete_text(&self, request: CompletionRequest) -> Result<String, HostError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::ChatMessage;
    use std::sync::Arc;

    struct TestWriter {
        captured: tokio::sync::Mutex<Vec<(String, String)>>,
    }

    #[async_trait]
    impl WorkspaceWriter for TestWriter {
        async fn append(&self, path: &str, content: &str) -> Result<(), HostError> {
            self.captured
                .lock()
                .await
                .push((path.to_string(), content.to_string()));
            Ok(())
        }
    }

    struct EchoCompleter;

    #[async_trait]
    impl LlmCompleter for EchoCompleter {
        async fn complete_text(&self, request: CompletionRequest) -> Result<String, HostError> {
            let last = request
                .messages
                .last()
                .map(|m| m.content.clone())
                .unwrap_or_default();
            Ok(format!("echo: {last}"))
        }
    }

    #[tokio::test]
    async fn workspace_writer_trait_is_dyn_compatible() {
        let writer: Arc<dyn WorkspaceWriter> = Arc::new(TestWriter {
            captured: tokio::sync::Mutex::new(Vec::new()),
        });
        writer.append("foo.md", "hello").await.unwrap();
    }

    #[tokio::test]
    async fn llm_completer_trait_is_dyn_compatible() {
        let llm: Arc<dyn LlmCompleter> = Arc::new(EchoCompleter);
        let req = CompletionRequest::new(vec![ChatMessage::user("ping")]);
        let out = llm.complete_text(req).await.unwrap();
        assert_eq!(out, "echo: ping");
    }
}
