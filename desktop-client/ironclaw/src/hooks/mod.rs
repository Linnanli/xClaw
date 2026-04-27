//! Lifecycle hooks for intercepting and transforming agent operations.
//!
//! The hook system provides 6 well-defined interception points:
//!
//! - **BeforeInbound** — Before processing an inbound user message
//! - **BeforeToolCall** — Before executing a tool call
//! - **BeforeOutbound** — Before sending an outbound response
//! - **OnSessionStart** — When a new session starts
//! - **OnSessionEnd** — When a session ends
//! - **TransformResponse** — Transform the final response before completing a turn
//!
//! Hooks are executed in priority order (lower number = higher priority).
//! Each hook can pass through, modify content, or reject the event.

pub mod bootstrap;
pub mod bundled;
pub mod hook;
pub mod registry;

pub use bootstrap::{HookBootstrapSummary, bootstrap_hooks};
pub use bundled::{
    HookBundleConfig, HookRegistrationSummary, register_bundle, register_bundled_hooks,
};
pub use hook::{Hook, HookContext, HookError, HookEvent, HookFailureMode, HookOutcome, HookPoint};
pub use registry::HookRegistry;

/// Bridge [`HookRegistry`] into the `x_claw_agent::SessionHooks` trait so
/// `SessionManager` (in the runtime crate) can fire OnSessionStart /
/// OnSessionEnd events without depending on ironclaw's concrete registry.
///
/// Errors from hook execution are logged and swallowed — session lifecycle
/// must never be blocked by hook failures (fire-and-forget contract).
#[async_trait::async_trait]
impl x_claw_agent::SessionHooks for HookRegistry {
    async fn on_session_start(&self, user_id: &str, session_id: &str) {
        let event = HookEvent::SessionStart {
            user_id: user_id.to_string(),
            session_id: session_id.to_string(),
        };
        if let Err(e) = self.run(&event).await {
            tracing::warn!("OnSessionStart hook error: {}", e);
        }
    }

    async fn on_session_end(&self, user_id: &str, session_id: &str) {
        let event = HookEvent::SessionEnd {
            user_id: user_id.to_string(),
            session_id: session_id.to_string(),
        };
        if let Err(e) = self.run(&event).await {
            tracing::warn!("OnSessionEnd hook error: {}", e);
        }
    }
}
