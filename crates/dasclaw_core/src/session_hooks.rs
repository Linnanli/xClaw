//! Session lifecycle hook trait.
//!
//! [`SessionHooks`] is the abstraction that lets `SessionManager` emit
//! `OnSessionStart` / `OnSessionEnd` notifications without depending on
//! ironclaw's concrete `HookRegistry`. Any runtime consumer (ironclaw,
//! admin-backend, tests) can provide its own implementation.
//!
//! Semantics:
//! - **Fire-and-forget.** Implementations MUST NOT block the session manager.
//!   Errors SHOULD be logged, not propagated.
//! - **Concurrency.** `on_session_start` / `on_session_end` are invoked from
//!   `tokio::spawn`, so implementations must be `Send + Sync` and tolerate
//!   concurrent calls for different sessions.

use async_trait::async_trait;

/// Hook trait for session lifecycle events.
///
/// Implement this on any type that wants to observe sessions being created
/// or pruned. [`SessionManager`](crate::session_manager::SessionManager) holds
/// an `Option<Arc<dyn SessionHooks>>` and calls these methods in fire-and-forget
/// tasks.
#[async_trait]
pub trait SessionHooks: Send + Sync {
    /// Called when a new session is created for `user_id`.
    ///
    /// Fired from a detached `tokio::spawn`; returning errors or blocking here
    /// does not impact session creation.
    async fn on_session_start(&self, user_id: &str, session_id: &str);

    /// Called when a session is pruned due to idle timeout.
    async fn on_session_end(&self, user_id: &str, session_id: &str);
}

/// Noop implementation — used when no session hooks are configured.
pub struct NoopSessionHooks;

#[async_trait]
impl SessionHooks for NoopSessionHooks {
    async fn on_session_start(&self, _user_id: &str, _session_id: &str) {}
    async fn on_session_end(&self, _user_id: &str, _session_id: &str) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    /// Capturing hook — records every session lifecycle event so tests can
    /// assert fire-and-forget wiring actually fires.
    struct CapturingHooks {
        events: Mutex<Vec<(String, String, String)>>, // (kind, user, session)
    }

    #[async_trait]
    impl SessionHooks for CapturingHooks {
        async fn on_session_start(&self, user_id: &str, session_id: &str) {
            self.events.lock().await.push((
                "start".to_string(),
                user_id.to_string(),
                session_id.to_string(),
            ));
        }
        async fn on_session_end(&self, user_id: &str, session_id: &str) {
            self.events.lock().await.push((
                "end".to_string(),
                user_id.to_string(),
                session_id.to_string(),
            ));
        }
    }

    #[tokio::test]
    async fn noop_hooks_are_silent() {
        let hooks = NoopSessionHooks;
        hooks.on_session_start("u", "s").await;
        hooks.on_session_end("u", "s").await;
        // Nothing to assert — merely asserting it compiles and does not panic.
    }

    #[tokio::test]
    async fn capturing_hooks_record_both_events() {
        let hooks = Arc::new(CapturingHooks {
            events: Mutex::new(Vec::new()),
        });

        hooks.on_session_start("alice", "sess-1").await;
        hooks.on_session_end("alice", "sess-1").await;

        let events = hooks.events.lock().await;
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].0, "start");
        assert_eq!(events[1].0, "end");
        assert_eq!(events[0].1, "alice");
        assert_eq!(events[0].2, "sess-1");
    }
}
