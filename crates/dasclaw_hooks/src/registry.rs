//! Hook registry for managing and executing lifecycle hooks.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tokio::sync::RwLock;

use crate::hook::{Hook, HookContext, HookError, HookEvent, HookFailureMode, HookOutcome};

/// A registered hook with its priority.
struct HookEntry {
    hook: Arc<dyn Hook>,
    priority: u32,
}

pub trait HookRunObserver: Send + Sync {
    fn hook_started(&self, run: HookObservedRun);
    fn hook_completed(&self, run: HookObservedRun);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookObservedRun {
    pub id: String,
    pub event_name: String,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub hook_name: String,
    pub status: HookObservedStatus,
    pub status_message: Option<String>,
    pub started_at_ms: u64,
    pub completed_at_ms: Option<u64>,
    pub duration_ms: Option<u64>,
}

impl HookObservedRun {
    fn completed(
        &self,
        started: Instant,
        status: HookObservedStatus,
        status_message: Option<String>,
    ) -> Self {
        let mut completed = self.clone();
        completed.status = status;
        completed.status_message = status_message;
        completed.completed_at_ms = Some(now_ms());
        completed.duration_ms = Some(started.elapsed().as_millis() as u64);
        completed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookObservedStatus {
    Running,
    Completed,
    Rejected,
    Failed,
    TimedOut,
}

/// Registry that manages hooks and executes them at lifecycle points.
///
/// Hooks are executed in priority order (lower number = higher priority).
/// A `Reject` outcome stops the chain immediately.
/// A `Modify` outcome chains through subsequent hooks.
pub struct HookRegistry {
    hooks: RwLock<Vec<HookEntry>>,
    observer: Option<Arc<dyn HookRunObserver>>,
    next_run_id: AtomicU64,
}

impl std::fmt::Debug for HookRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HookRegistry")
            .field("observer_wired", &self.observer.is_some())
            .finish_non_exhaustive()
    }
}

impl HookRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            hooks: RwLock::new(Vec::new()),
            observer: None,
            next_run_id: AtomicU64::new(1),
        }
    }

    #[must_use]
    pub fn with_observer(mut self, observer: Arc<dyn HookRunObserver>) -> Self {
        self.observer = Some(observer);
        self
    }

    /// Register a hook with default priority (100).
    pub async fn register(&self, hook: Arc<dyn Hook>) {
        self.register_with_priority(hook, 100).await;
    }

    /// Register a hook with a specific priority.
    ///
    /// Lower priority number = runs first.
    pub async fn register_with_priority(&self, hook: Arc<dyn Hook>, priority: u32) {
        let mut hooks = self.hooks.write().await;
        let hook_name = hook.name().to_string();

        if let Some(existing) = hooks
            .iter_mut()
            .find(|entry| entry.hook.name() == hook_name)
        {
            tracing::warn!(
                hook = %hook_name,
                "Replacing existing hook registration with same name"
            );
            existing.hook = hook;
            existing.priority = priority;
        } else {
            hooks.push(HookEntry { hook, priority });
        }

        hooks.sort_by_key(|e| e.priority);
    }

    /// Unregister a hook by name. Returns `true` if it was found and removed.
    pub async fn unregister(&self, name: &str) -> bool {
        let mut hooks = self.hooks.write().await;
        let before = hooks.len();
        hooks.retain(|e| e.hook.name() != name);
        hooks.len() < before
    }

    /// List all registered hook names (in priority order).
    pub async fn list(&self) -> Vec<String> {
        let hooks = self.hooks.read().await;
        hooks.iter().map(|e| e.hook.name().to_string()).collect()
    }

    /// Run all hooks matching the event's hook point.
    ///
    /// - Hooks run in priority order (lowest first).
    /// - `Reject` stops the chain immediately.
    /// - `Modify` chains the modification through subsequent hooks.
    /// - Timeout/error handling respects each hook's `failure_mode`.
    pub async fn run(&self, event: &HookEvent) -> Result<HookOutcome, HookError> {
        let point = event.hook_point();
        let event_kind = event.event_kind();
        let ctx = HookContext::default();

        // Clone matching hooks and drop the read guard before executing.
        // Each hook can run up to its timeout, so holding the guard would
        // block concurrent register/unregister/run calls.
        let matching: Vec<Arc<dyn Hook>> = {
            let hooks = self.hooks.read().await;
            hooks
                .iter()
                .filter(|e| e.hook.hook_points().contains(&point))
                .map(|e| e.hook.clone())
                .collect()
        };

        // ADR-113 single-instrumentation point: every lifecycle dispatch emits
        // one log line so e2e runs always show all 6 trigger points, even
        // when no hooks are registered (W3 #60).
        tracing::debug!(
            hook_point = %point.as_str(),
            event_type = %event_kind,
            matching_hooks = matching.len(),
            "lifecycle fired"
        );

        if matching.is_empty() {
            return Ok(HookOutcome::ok());
        }

        let mut current_event = event.clone();

        for hook in &matching {
            let timeout = hook.timeout();
            let started_at_ms = now_ms();
            let started = Instant::now();
            let run = HookObservedRun {
                id: format!(
                    "hook-run-{}",
                    self.next_run_id.fetch_add(1, Ordering::Relaxed)
                ),
                event_name: point.as_str().to_string(),
                thread_id: observed_thread_id(&current_event),
                turn_id: observed_turn_id(&current_event),
                hook_name: hook.name().to_string(),
                status: HookObservedStatus::Running,
                status_message: None,
                started_at_ms,
                completed_at_ms: None,
                duration_ms: None,
            };

            tracing::debug!(
                hook_name = %hook.name(),
                hook_point = %point.as_str(),
                event_type = %event_kind,
                "hook executing"
            );
            self.notify_started(&run);

            let result = tokio::time::timeout(timeout, hook.execute(&current_event, &ctx)).await;

            match result {
                Ok(Ok(HookOutcome::Reject { reason })) => {
                    tracing::debug!(hook = hook.name(), "Hook rejected: {}", reason);
                    self.notify_completed(
                        &run,
                        started,
                        HookObservedStatus::Rejected,
                        Some(reason.clone()),
                    );
                    return Err(HookError::Rejected { reason });
                }
                Ok(Ok(HookOutcome::Continue {
                    modified: Some(value),
                })) => {
                    tracing::debug!(hook = hook.name(), "Hook modified content");
                    current_event.apply_modification(&value);
                    self.notify_completed(&run, started, HookObservedStatus::Completed, None);
                }
                Ok(Ok(HookOutcome::Continue { modified: None })) => {
                    // No-op, continue chain
                    self.notify_completed(&run, started, HookObservedStatus::Completed, None);
                }
                Ok(Err(err)) => match hook.failure_mode() {
                    HookFailureMode::FailOpen => {
                        tracing::warn!(hook = hook.name(), "Hook failed (fail-open): {}", err);
                        self.notify_completed(
                            &run,
                            started,
                            HookObservedStatus::Failed,
                            Some(err.to_string()),
                        );
                    }
                    HookFailureMode::FailClosed => {
                        tracing::warn!(hook = hook.name(), "Hook failed (fail-closed): {}", err);
                        self.notify_completed(
                            &run,
                            started,
                            HookObservedStatus::Failed,
                            Some(err.to_string()),
                        );
                        return Err(HookError::ExecutionFailed {
                            reason: format!("Hook '{}' failed: {}", hook.name(), err),
                        });
                    }
                },
                Err(_elapsed) => match hook.failure_mode() {
                    HookFailureMode::FailOpen => {
                        tracing::warn!(
                            hook = hook.name(),
                            "Hook timed out (fail-open) after {:?}",
                            timeout
                        );
                        self.notify_completed(
                            &run,
                            started,
                            HookObservedStatus::TimedOut,
                            Some(format!("timed out after {timeout:?}")),
                        );
                    }
                    HookFailureMode::FailClosed => {
                        tracing::warn!(
                            hook = hook.name(),
                            "Hook timed out (fail-closed) after {:?}",
                            timeout
                        );
                        self.notify_completed(
                            &run,
                            started,
                            HookObservedStatus::TimedOut,
                            Some(format!("timed out after {timeout:?}")),
                        );
                        return Err(HookError::Timeout { timeout });
                    }
                },
            }
        }

        // Determine final outcome by comparing with original event
        let modified = extract_content(&current_event);
        let original = extract_content(event);

        if modified != original {
            Ok(HookOutcome::modify(modified))
        } else {
            Ok(HookOutcome::ok())
        }
    }

    fn notify_started(&self, run: &HookObservedRun) {
        if let Some(observer) = &self.observer {
            observer.hook_started(run.clone());
        }
    }

    fn notify_completed(
        &self,
        run: &HookObservedRun,
        started: Instant,
        status: HookObservedStatus,
        status_message: Option<String>,
    ) {
        if let Some(observer) = &self.observer {
            observer.hook_completed(run.completed(started, status, status_message));
        }
    }
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract the primary content string from a hook event.
fn extract_content(event: &HookEvent) -> String {
    match event {
        HookEvent::Inbound { content, .. } | HookEvent::Outbound { content, .. } => content.clone(),
        HookEvent::ToolCall { parameters, .. } => {
            serde_json::to_string(parameters).unwrap_or_default()
        }
        HookEvent::ResponseTransform { response, .. } => response.clone(),
        HookEvent::SessionStart { session_id, .. } | HookEvent::SessionEnd { session_id, .. } => {
            session_id.clone()
        }
    }
}

fn observed_thread_id(event: &HookEvent) -> Option<String> {
    match event {
        HookEvent::Inbound { thread_id, .. } | HookEvent::Outbound { thread_id, .. } => {
            thread_id.clone()
        }
        HookEvent::ResponseTransform { thread_id, .. } => Some(thread_id.clone()),
        HookEvent::ToolCall { thread_id, .. } => thread_id.clone(),
        HookEvent::SessionStart { session_id, .. } | HookEvent::SessionEnd { session_id, .. } => {
            Some(session_id.clone())
        }
    }
}

fn observed_turn_id(event: &HookEvent) -> Option<String> {
    match event {
        HookEvent::ToolCall { turn_id, .. } => turn_id.clone(),
        _ => None,
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hook::{HookFailureMode, HookPoint};
    use async_trait::async_trait;
    use std::sync::Mutex;
    use std::time::Duration;

    /// A test hook that always returns ok.
    struct PassthroughHook {
        name: String,
        points: Vec<HookPoint>,
    }

    #[async_trait]
    impl Hook for PassthroughHook {
        fn name(&self) -> &str {
            &self.name
        }
        fn hook_points(&self) -> &[HookPoint] {
            &self.points
        }
        async fn execute(
            &self,
            _event: &HookEvent,
            _ctx: &HookContext,
        ) -> Result<HookOutcome, HookError> {
            Ok(HookOutcome::ok())
        }
    }

    /// A hook that modifies content by appending a suffix.
    struct ModifyHook {
        name: String,
        suffix: String,
        points: Vec<HookPoint>,
    }

    #[async_trait]
    impl Hook for ModifyHook {
        fn name(&self) -> &str {
            &self.name
        }
        fn hook_points(&self) -> &[HookPoint] {
            &self.points
        }
        async fn execute(
            &self,
            event: &HookEvent,
            _ctx: &HookContext,
        ) -> Result<HookOutcome, HookError> {
            let content = extract_content(event);
            Ok(HookOutcome::modify(format!("{}{}", content, self.suffix)))
        }
    }

    /// A hook that always rejects.
    struct RejectHook {
        name: String,
        reason: String,
        points: Vec<HookPoint>,
    }

    #[async_trait]
    impl Hook for RejectHook {
        fn name(&self) -> &str {
            &self.name
        }
        fn hook_points(&self) -> &[HookPoint] {
            &self.points
        }
        async fn execute(
            &self,
            _event: &HookEvent,
            _ctx: &HookContext,
        ) -> Result<HookOutcome, HookError> {
            Ok(HookOutcome::reject(&self.reason))
        }
    }

    /// A hook that always errors.
    struct ErrorHook {
        name: String,
        points: Vec<HookPoint>,
        failure_mode: HookFailureMode,
    }

    #[async_trait]
    impl Hook for ErrorHook {
        fn name(&self) -> &str {
            &self.name
        }
        fn hook_points(&self) -> &[HookPoint] {
            &self.points
        }
        fn failure_mode(&self) -> HookFailureMode {
            self.failure_mode
        }
        async fn execute(
            &self,
            _event: &HookEvent,
            _ctx: &HookContext,
        ) -> Result<HookOutcome, HookError> {
            Err(HookError::ExecutionFailed {
                reason: "test error".into(),
            })
        }
    }

    /// A hook that sleeps longer than its timeout.
    struct SlowHook {
        name: String,
        points: Vec<HookPoint>,
        failure_mode: HookFailureMode,
    }

    #[async_trait]
    impl Hook for SlowHook {
        fn name(&self) -> &str {
            &self.name
        }
        fn hook_points(&self) -> &[HookPoint] {
            &self.points
        }
        fn failure_mode(&self) -> HookFailureMode {
            self.failure_mode
        }
        fn timeout(&self) -> Duration {
            Duration::from_millis(50)
        }
        async fn execute(
            &self,
            _event: &HookEvent,
            _ctx: &HookContext,
        ) -> Result<HookOutcome, HookError> {
            tokio::time::sleep(Duration::from_millis(200)).await;
            Ok(HookOutcome::ok())
        }
    }

    #[derive(Default)]
    struct RecordingHookObserver {
        started: Mutex<Vec<HookObservedRun>>,
        completed: Mutex<Vec<HookObservedRun>>,
    }

    impl RecordingHookObserver {
        fn started(&self) -> Vec<HookObservedRun> {
            self.started
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .clone()
        }

        fn completed(&self) -> Vec<HookObservedRun> {
            self.completed
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .clone()
        }
    }

    impl HookRunObserver for RecordingHookObserver {
        fn hook_started(&self, run: HookObservedRun) {
            self.started
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .push(run);
        }

        fn hook_completed(&self, run: HookObservedRun) {
            self.completed
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .push(run);
        }
    }

    fn test_event() -> HookEvent {
        HookEvent::Inbound {
            user_id: "user-1".into(),
            channel: "test".into(),
            content: "hello".into(),
            thread_id: None,
        }
    }

    #[tokio::test]
    async fn registry_observer_receives_started_and_completed_for_real_run() {
        let observer = Arc::new(RecordingHookObserver::default());
        let registry = HookRegistry::new().with_observer(observer.clone());
        registry
            .register(Arc::new(PassthroughHook {
                name: "lint".into(),
                points: vec![HookPoint::BeforeInbound],
            }))
            .await;

        let outcome = registry.run(&test_event()).await.expect("hook run");

        assert!(matches!(outcome, HookOutcome::Continue { modified: None }));
        let started = observer.started();
        let completed = observer.completed();
        assert_eq!(started.len(), 1);
        assert_eq!(completed.len(), 1);
        assert_eq!(started[0].id, "hook-run-1");
        assert_eq!(completed[0].id, "hook-run-1");
        assert_eq!(started[0].status, HookObservedStatus::Running);
        assert_eq!(completed[0].status, HookObservedStatus::Completed);
        assert_eq!(completed[0].hook_name, "lint");
        assert_eq!(completed[0].event_name, "beforeInbound");
        assert!(completed[0].completed_at_ms.is_some());
        assert!(completed[0].duration_ms.is_some());
    }

    #[tokio::test]
    async fn registry_observer_marks_reject_as_completed_rejected() {
        let observer = Arc::new(RecordingHookObserver::default());
        let registry = HookRegistry::new().with_observer(observer.clone());
        registry
            .register(Arc::new(RejectHook {
                name: "blocker".into(),
                reason: "blocked".into(),
                points: vec![HookPoint::BeforeInbound],
            }))
            .await;

        let outcome = registry.run(&test_event()).await;

        assert!(matches!(outcome, Err(HookError::Rejected { .. })));
        let completed = observer.completed();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].status, HookObservedStatus::Rejected);
        assert_eq!(completed[0].status_message.as_deref(), Some("blocked"));
    }

    #[tokio::test]
    async fn test_empty_registry_returns_ok() {
        let registry = HookRegistry::new();
        let result = registry.run(&test_event()).await;
        assert!(result.is_ok());
        assert!(matches!(
            result.unwrap(),
            HookOutcome::Continue { modified: None }
        ));
    }

    #[tokio::test]
    async fn test_register_and_list() {
        let registry = HookRegistry::new();
        registry
            .register(Arc::new(PassthroughHook {
                name: "hook-a".into(),
                points: vec![HookPoint::BeforeInbound],
            }))
            .await;
        registry
            .register(Arc::new(PassthroughHook {
                name: "hook-b".into(),
                points: vec![HookPoint::BeforeInbound],
            }))
            .await;

        let names = registry.list().await;
        assert_eq!(names, vec!["hook-a", "hook-b"]);
    }

    #[tokio::test]
    async fn test_register_duplicate_name_replaces_existing() {
        let registry = HookRegistry::new();

        registry
            .register_with_priority(
                Arc::new(ModifyHook {
                    name: "dup".into(),
                    suffix: "-A".into(),
                    points: vec![HookPoint::BeforeInbound],
                }),
                100,
            )
            .await;

        registry
            .register_with_priority(
                Arc::new(ModifyHook {
                    name: "dup".into(),
                    suffix: "-B".into(),
                    points: vec![HookPoint::BeforeInbound],
                }),
                10,
            )
            .await;

        let names = registry.list().await;
        assert_eq!(names, vec!["dup"]);

        let result = registry.run(&test_event()).await.unwrap();
        match result {
            HookOutcome::Continue {
                modified: Some(value),
            } => assert_eq!(value, "hello-B"),
            other => panic!("expected modified output, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_priority_ordering() {
        let registry = HookRegistry::new();

        // Register in reverse priority order
        registry
            .register_with_priority(
                Arc::new(ModifyHook {
                    name: "low-prio".into(),
                    suffix: "-LOW".into(),
                    points: vec![HookPoint::BeforeInbound],
                }),
                200,
            )
            .await;
        registry
            .register_with_priority(
                Arc::new(ModifyHook {
                    name: "high-prio".into(),
                    suffix: "-HIGH".into(),
                    points: vec![HookPoint::BeforeInbound],
                }),
                10,
            )
            .await;

        // Should run in priority order: high-prio first, then low-prio
        let names = registry.list().await;
        assert_eq!(names[0], "high-prio");
        assert_eq!(names[1], "low-prio");

        let result = registry.run(&test_event()).await.unwrap();
        match result {
            HookOutcome::Continue { modified: Some(m) } => {
                // "hello" -> "hello-HIGH" -> "hello-HIGH-LOW"
                assert_eq!(m, "hello-HIGH-LOW");
            }
            other => panic!("Expected modification chain, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_reject_stops_chain() {
        let registry = HookRegistry::new();

        registry
            .register_with_priority(
                Arc::new(RejectHook {
                    name: "blocker".into(),
                    reason: "blocked".into(),
                    points: vec![HookPoint::BeforeInbound],
                }),
                10,
            )
            .await;
        registry
            .register_with_priority(
                Arc::new(ModifyHook {
                    name: "modifier".into(),
                    suffix: "-MODIFIED".into(),
                    points: vec![HookPoint::BeforeInbound],
                }),
                20,
            )
            .await;

        let result = registry.run(&test_event()).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            HookError::Rejected { reason } => assert_eq!(reason, "blocked"),
            other => panic!("Expected Rejected, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_modification_chaining() {
        let registry = HookRegistry::new();

        registry
            .register_with_priority(
                Arc::new(ModifyHook {
                    name: "first".into(),
                    suffix: "-A".into(),
                    points: vec![HookPoint::BeforeInbound],
                }),
                10,
            )
            .await;
        registry
            .register_with_priority(
                Arc::new(ModifyHook {
                    name: "second".into(),
                    suffix: "-B".into(),
                    points: vec![HookPoint::BeforeInbound],
                }),
                20,
            )
            .await;

        let result = registry.run(&test_event()).await.unwrap();
        match result {
            HookOutcome::Continue { modified: Some(m) } => {
                assert_eq!(m, "hello-A-B");
            }
            other => panic!("Expected chained modification, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_fail_open_on_error() {
        let registry = HookRegistry::new();
        registry
            .register(Arc::new(ErrorHook {
                name: "err-open".into(),
                points: vec![HookPoint::BeforeInbound],
                failure_mode: HookFailureMode::FailOpen,
            }))
            .await;

        let result = registry.run(&test_event()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_fail_closed_on_error() {
        let registry = HookRegistry::new();
        registry
            .register(Arc::new(ErrorHook {
                name: "err-closed".into(),
                points: vec![HookPoint::BeforeInbound],
                failure_mode: HookFailureMode::FailClosed,
            }))
            .await;

        let result = registry.run(&test_event()).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            HookError::ExecutionFailed { .. }
        ));
    }

    #[tokio::test]
    async fn test_fail_open_on_timeout() {
        let registry = HookRegistry::new();
        registry
            .register(Arc::new(SlowHook {
                name: "slow-open".into(),
                points: vec![HookPoint::BeforeInbound],
                failure_mode: HookFailureMode::FailOpen,
            }))
            .await;

        let result = registry.run(&test_event()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_fail_closed_on_timeout() {
        let registry = HookRegistry::new();
        registry
            .register(Arc::new(SlowHook {
                name: "slow-closed".into(),
                points: vec![HookPoint::BeforeInbound],
                failure_mode: HookFailureMode::FailClosed,
            }))
            .await;

        let result = registry.run(&test_event()).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), HookError::Timeout { .. }));
    }

    #[tokio::test]
    async fn test_unregister() {
        let registry = HookRegistry::new();
        registry
            .register(Arc::new(PassthroughHook {
                name: "removable".into(),
                points: vec![HookPoint::BeforeInbound],
            }))
            .await;

        assert_eq!(registry.list().await.len(), 1);
        assert!(registry.unregister("removable").await);
        assert_eq!(registry.list().await.len(), 0);

        // Unregistering non-existent returns false
        assert!(!registry.unregister("nonexistent").await);
    }

    #[tokio::test]
    async fn test_hooks_only_match_their_points() {
        let registry = HookRegistry::new();
        registry
            .register(Arc::new(RejectHook {
                name: "outbound-only".into(),
                reason: "blocked".into(),
                points: vec![HookPoint::BeforeOutbound],
            }))
            .await;

        // Inbound event should not be affected by outbound-only hook
        let result = registry.run(&test_event()).await;
        assert!(result.is_ok());
    }
}
