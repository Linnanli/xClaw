//! Context compaction for ironclaw.
//!
//! The production implementation now lives in [`dasclaw_core::compaction`]
//! and works against the host traits [`dasclaw_core::WorkspaceWriter`] and
//! [`dasclaw_core::LlmCompleter`]. Ironclaw ships blanket impls for
//! `Workspace` and `Reasoning` (see [`super::traits_impl`]), so call sites
//! pass a `Reasoning`-wrapped `LlmProvider` and cast the `Workspace` to
//! `&dyn WorkspaceWriter`.
//!
//! This shim only re-exports the ported types and hosts the two libsql-backed
//! integration tests that can't live in `dasclaw_core` (the crate must stay
//! free of the `libsql` backend).

pub use dasclaw_core::compaction::{CompactionResult, ContextCompactor};

#[cfg(all(test, feature = "libsql"))]
mod libsql_tests {
    use std::sync::Arc;

    use dasclaw_core::WorkspaceWriter;
    use dasclaw_core::compaction::ContextCompactor;
    use dasclaw_core::context_monitor::CompactionStrategy;
    use dasclaw_core::session::Thread;
    use uuid::Uuid;

    use crate::llm::Reasoning;
    use crate::testing::StubLlm;

    /// Build a thread with `n` completed turns.
    fn make_thread(n: usize) -> Thread {
        let mut thread = Thread::new(Uuid::new_v4());
        for i in 0..n {
            thread.start_turn(format!("msg-{}", i));
            thread.complete_turn(format!("resp-{}", i));
        }
        thread
    }

    /// An un-migrated workspace whose `append` always fails — lets us prove
    /// that archival failure preserves turns end-to-end through the real
    /// `Workspace::append` path.
    async fn make_unmigrated_workspace() -> crate::workspace::Workspace {
        use crate::db::Database;
        use crate::db::libsql::LibSqlBackend;

        let backend = LibSqlBackend::new_memory()
            .await
            .expect("should create in-memory libsql backend");
        let db: Arc<dyn Database> = Arc::new(backend);
        crate::workspace::Workspace::new_with_db("compaction-test", db)
    }

    fn make_compactor(llm: Arc<StubLlm>) -> ContextCompactor {
        ContextCompactor::new(Arc::new(crate::agent::ReasoningCompleter::new(
            Reasoning::new(llm as Arc<dyn crate::llm::LlmProvider>),
        )))
    }

    #[tokio::test]
    async fn test_compact_with_summary_preserves_turns_when_workspace_write_fails() {
        let llm = Arc::new(StubLlm::new("summary"));
        let compactor = make_compactor(llm.clone());
        let mut thread = make_thread(8);
        let original_inputs: Vec<String> =
            thread.turns.iter().map(|t| t.user_input.clone()).collect();
        let workspace = make_unmigrated_workspace().await;

        let result = compactor
            .compact(
                &mut thread,
                CompactionStrategy::Summarize { keep_recent: 3 },
                Some(&workspace as &dyn WorkspaceWriter),
            )
            .await
            .expect("compact should succeed even when workspace write fails");

        assert_eq!(thread.turns.len(), 8);
        assert_eq!(
            thread
                .turns
                .iter()
                .map(|t| t.user_input.as_str())
                .collect::<Vec<_>>(),
            original_inputs
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(result.turns_removed, 0);
        assert!(!result.summary_written);
        assert_eq!(llm.calls(), 1);
    }

    #[tokio::test]
    async fn test_compact_to_workspace_preserves_turns_when_workspace_write_fails() {
        let llm = Arc::new(StubLlm::new("unused"));
        let compactor = make_compactor(llm.clone());
        let mut thread = make_thread(20);
        let original_inputs: Vec<String> =
            thread.turns.iter().map(|t| t.user_input.clone()).collect();
        let workspace = make_unmigrated_workspace().await;

        let result = compactor
            .compact(
                &mut thread,
                CompactionStrategy::MoveToWorkspace,
                Some(&workspace as &dyn WorkspaceWriter),
            )
            .await
            .expect("compact should succeed even when workspace write fails");

        assert_eq!(thread.turns.len(), 20);
        assert_eq!(
            thread
                .turns
                .iter()
                .map(|t| t.user_input.as_str())
                .collect::<Vec<_>>(),
            original_inputs
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(result.turns_removed, 0);
        assert!(!result.summary_written);
        assert_eq!(llm.calls(), 0);
    }
}
