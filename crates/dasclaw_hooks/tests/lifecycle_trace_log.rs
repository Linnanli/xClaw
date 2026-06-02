//! W3 #60 — lifecycle trace log instrumentation contract.
//!
//! Verifies that every one of the 6 hook lifecycle trigger points emits a
//! `tracing` event from [`HookRegistry::run`], so an end-to-end run produces
//! one log line per fired event regardless of whether matching hooks are
//! installed.
//!
//! Acceptance pinned by these tests:
//! - Each lifecycle point emits a log carrying `hook_point` + `event_type`.
//! - When a hook is matched, a per-hook log additionally carries `hook_name`.
//! - Firing all 6 [`HookEvent`] variants produces 6 distinct lifecycle lines.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use dasclaw_hooks::{
    Hook, HookContext, HookError, HookEvent, HookFailureMode, HookOutcome, HookPoint, HookRegistry,
};
use tracing_test::traced_test;

const ALL_POINTS: &[HookPoint] = &[
    HookPoint::BeforeInbound,
    HookPoint::BeforeToolCall,
    HookPoint::BeforeOutbound,
    HookPoint::OnSessionStart,
    HookPoint::OnSessionEnd,
    HookPoint::TransformResponse,
];

/// Minimal pass-through hook bound to all six lifecycle points.
struct AllPointsHook;

struct RecordingHook {
    name: &'static str,
    points: &'static [HookPoint],
    calls: Arc<tokio::sync::Mutex<Vec<&'static str>>>,
}

#[async_trait]
impl Hook for AllPointsHook {
    fn name(&self) -> &str {
        "all_points_hook"
    }

    fn hook_points(&self) -> &[HookPoint] {
        ALL_POINTS
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(1)
    }

    fn failure_mode(&self) -> HookFailureMode {
        HookFailureMode::FailOpen
    }

    async fn execute(
        &self,
        _event: &HookEvent,
        _ctx: &HookContext,
    ) -> Result<HookOutcome, HookError> {
        Ok(HookOutcome::ok())
    }
}

#[async_trait]
impl Hook for RecordingHook {
    fn name(&self) -> &str {
        self.name
    }

    fn hook_points(&self) -> &[HookPoint] {
        self.points
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(1)
    }

    fn failure_mode(&self) -> HookFailureMode {
        HookFailureMode::FailClosed
    }

    async fn execute(
        &self,
        _event: &HookEvent,
        _ctx: &HookContext,
    ) -> Result<HookOutcome, HookError> {
        self.calls.lock().await.push(self.name);
        Ok(HookOutcome::ok())
    }
}

fn all_six_events() -> Vec<HookEvent> {
    vec![
        HookEvent::Inbound {
            user_id: "u".into(),
            channel: "c".into(),
            content: "in".into(),
            thread_id: None,
        },
        HookEvent::ToolCall {
            tool_name: "shell".into(),
            parameters: serde_json::json!({}),
            user_id: "u".into(),
            context: "chat".into(),
        },
        HookEvent::Outbound {
            user_id: "u".into(),
            channel: "c".into(),
            content: "out".into(),
            thread_id: None,
        },
        HookEvent::SessionStart {
            user_id: "u".into(),
            session_id: "s".into(),
        },
        HookEvent::SessionEnd {
            user_id: "u".into(),
            session_id: "s".into(),
        },
        HookEvent::ResponseTransform {
            user_id: "u".into(),
            thread_id: "t".into(),
            response: "r".into(),
        },
    ]
}

/// Empty registry still emits a lifecycle log for each fired event so an e2e
/// pass produces 6 distinct log lines (one per HookPoint).
#[tokio::test]
#[traced_test]
async fn req_hooks_60_lifecycle_log_emitted_for_all_six_points() {
    let registry = HookRegistry::new();

    for event in all_six_events() {
        registry
            .run(&event)
            .await
            .expect("empty registry returns Ok");
    }

    // Every HookPoint must appear at least once in the log stream.
    for point in [
        HookPoint::BeforeInbound,
        HookPoint::BeforeToolCall,
        HookPoint::BeforeOutbound,
        HookPoint::OnSessionStart,
        HookPoint::OnSessionEnd,
        HookPoint::TransformResponse,
    ] {
        let needle = format!("hook_point={}", point.as_str());
        assert!(
            logs_contain(&needle),
            "expected lifecycle log to mention `{needle}`"
        );
    }
}

/// Lifecycle log must include `event_type` so consumers can filter by event
/// shape without relying on `hook_point` alone.
#[tokio::test]
#[traced_test]
async fn req_hooks_60_lifecycle_log_includes_event_type_field() {
    let registry = HookRegistry::new();

    registry
        .run(&HookEvent::Inbound {
            user_id: "u".into(),
            channel: "c".into(),
            content: "hi".into(),
            thread_id: None,
        })
        .await
        .expect("ok");

    assert!(
        logs_contain("event_type=Inbound"),
        "lifecycle log must include event_type=Inbound"
    );
}

/// When a matching hook is installed, the per-hook execution log must carry
/// `hook_name` plus the same `hook_point` / `event_type` fields.
#[tokio::test]
#[traced_test]
async fn req_hooks_60_per_hook_log_includes_hook_name_and_event_type() {
    let registry = HookRegistry::new();
    registry.register(Arc::new(AllPointsHook)).await;

    registry
        .run(&HookEvent::ToolCall {
            tool_name: "shell".into(),
            parameters: serde_json::json!({"cmd": "ls"}),
            user_id: "u".into(),
            context: "chat".into(),
        })
        .await
        .expect("ok");

    assert!(
        logs_contain("hook_name=all_points_hook"),
        "per-hook log must carry hook_name field"
    );
    assert!(
        logs_contain("event_type=ToolCall"),
        "per-hook log must carry event_type field"
    );
    assert!(
        logs_contain("hook_point=beforeToolCall"),
        "per-hook log must carry hook_point field"
    );
}

#[tokio::test]
async fn req_hooks_lifecycle_order() {
    let registry = HookRegistry::new();
    let calls = Arc::new(tokio::sync::Mutex::new(Vec::new()));

    registry
        .register(Arc::new(RecordingHook {
            name: "first",
            points: &[HookPoint::BeforeInbound],
            calls: calls.clone(),
        }))
        .await;
    registry
        .register(Arc::new(RecordingHook {
            name: "second",
            points: &[HookPoint::BeforeInbound],
            calls: calls.clone(),
        }))
        .await;

    registry
        .run(&HookEvent::Inbound {
            user_id: "u".into(),
            channel: "c".into(),
            content: "in".into(),
            thread_id: None,
        })
        .await
        .expect("hooks should execute");

    let actual = calls.lock().await.clone();
    assert_eq!(actual, vec!["first", "second"]);
}
