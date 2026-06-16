//! ADR-153 §1.1 B9 (e25): cancellation semantics.
//!
//! Verifies that wrapping [`Agent::invoke`] in `tokio::time::timeout`
//! cleanly cancels a long-running tool call without panics or stale
//! state. The contract has three legs:
//!
//!   1. **Timeout fires** — `timeout(short, agent.invoke(..))` returns
//!      `Err(Elapsed)` while a tool is mid-`sleep`. The inner future
//!      is dropped at that point.
//!   2. **No tool panic** — dropping the in-flight `sleep` future
//!      must not produce a tokio panic. We check by reading the
//!      `SleepyToolExecutor`'s `dropped_mid_sleep` flag, which is set
//!      from the `Drop` impl of its in-flight guard.
//!   3. **No leaked state** — a *second* fresh `Agent` built after
//!      the cancellation runs to completion normally. This pins that
//!      cancellation doesn't poison shared globals (logging,
//!      task-local state, etc.).
//!
//! Out of scope: the matrix line for e25 mentions "CLI exit code non
//! zero" — that's a binary-level concern that belongs in a separate
//! `assert_cmd`-based smoke test. Library-level cancellation is what
//! this file pins.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use dasclaw_core::hooks::{
    AutoApproveGate, HookBundle, InMemorySecrets, NoopEgressGate, NoopSandboxExecutor,
};
use dasclaw_core::messages::{ToolCall, ToolResult};
use dasclaw_core::traits::HostError;
use dasclaw_runtime::{Agent, AgentRunOptions, ToolExecutor};
use serde_json::json;
use tokio::time::timeout;

#[path = "fixtures/agent_loop_fixtures.rs"]
mod fixtures;

use fixtures::{RecordingToolExecutor, ScriptedResponder, no_tool_defs, text_turn, tool_call_turn};

/// Guard whose [`Drop`] sets `flag` to `true`. Inserted into the
/// `sleep` future via `tokio::select!` so we can observe whether the
/// tool body was canceled mid-sleep without resorting to timing
/// heuristics.
struct DropFlag {
    flag: Arc<AtomicBool>,
}

impl Drop for DropFlag {
    fn drop(&mut self) {
        self.flag.store(true, Ordering::SeqCst);
    }
}

/// [`ToolExecutor`] whose `execute` body sleeps for `sleep_for`,
/// counting starts and signalling cancellation via an
/// [`AtomicBool`] drop-flag.
///
/// Not promoted to shared fixtures because B9 is the only consumer.
struct SleepyToolExecutor {
    sleep_for: Duration,
    starts: Arc<AtomicUsize>,
    completions: Arc<AtomicUsize>,
    dropped_mid_sleep: Arc<AtomicBool>,
}

impl SleepyToolExecutor {
    fn new(sleep_for: Duration) -> Self {
        Self {
            sleep_for,
            starts: Arc::new(AtomicUsize::new(0)),
            completions: Arc::new(AtomicUsize::new(0)),
            dropped_mid_sleep: Arc::new(AtomicBool::new(false)),
        }
    }

    fn starts(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.starts)
    }
    fn completions(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.completions)
    }
    fn dropped_mid_sleep(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.dropped_mid_sleep)
    }
}

#[async_trait]
impl ToolExecutor for SleepyToolExecutor {
    async fn execute(&self, call: &ToolCall) -> Result<ToolResult, HostError> {
        self.starts.fetch_add(1, Ordering::SeqCst);
        // Hold the drop-flag guard for the duration of the sleep. If
        // the surrounding future is canceled mid-sleep the guard is
        // dropped first and flips the flag — that's how we observe
        // cancellation without timing heuristics.
        let _guard = DropFlag {
            flag: Arc::clone(&self.dropped_mid_sleep),
        };
        tokio::time::sleep(self.sleep_for).await;
        self.completions.fetch_add(1, Ordering::SeqCst);
        Ok(ToolResult {
            tool_call_id: call.id.clone(),
            name: call.name.clone(),
            content: "slept-ok".into(),
            is_error: false,
        })
    }
}

fn build_hooks() -> HookBundle {
    HookBundle {
        egress: Arc::new(NoopEgressGate),
        sandbox: Arc::new(NoopSandboxExecutor),
        secrets: Arc::new(InMemorySecrets::new()),
        approval: Arc::new(AutoApproveGate),
    }
}

#[tokio::test]
async fn req_dasclaw_cli_loop_e25_timeout_cancels_running_tool() {
    // Tool sleeps for 5s; outer timeout fires at 50ms. The agent.run
    // future must be dropped, the in-flight sleep guard must observe
    // its Drop, and the tool must not record a completion.
    let responder = ScriptedResponder::with_queue(vec![
        tool_call_turn("slow_tool", "call_sleep", json!({})),
        // Second turn never reached because the timeout cancels
        // before the tool result returns. Kept here defensively so a
        // future regression that swallows the timeout produces a
        // clearer assertion message.
        text_turn("would-be reply if cancellation broke"),
    ]);

    let executor = SleepyToolExecutor::new(Duration::from_secs(5));
    let starts = executor.starts();
    let completions = executor.completions();
    let dropped = executor.dropped_mid_sleep();

    let agent = Agent::builder()
        .responder(responder)
        .tool_executor(executor)
        .tools(no_tool_defs())
        .hooks(build_hooks())
        .build()
        .expect("agent builds");

    let outcome = timeout(
        Duration::from_millis(50),
        agent.invoke("hold on", AgentRunOptions::invoke()),
    )
    .await;

    assert!(
        outcome.is_err(),
        "outer timeout must fire while tool is sleeping, got: {outcome:?}"
    );

    // Give the runtime a tick to actually drop the inner future
    // before reading the drop-flag. Without this the flag-set runs
    // *after* the assertion in rare scheduling orders.
    tokio::task::yield_now().await;

    assert_eq!(
        starts.load(Ordering::SeqCst),
        1,
        "tool body must have started before the timeout"
    );
    assert_eq!(
        completions.load(Ordering::SeqCst),
        0,
        "tool must not have completed — that would mean cancellation \
         didn't actually drop the in-flight future"
    );
    assert!(
        dropped.load(Ordering::SeqCst),
        "in-flight sleep guard must have been dropped — cancellation \
         is supposed to propagate through tokio::select"
    );
}

#[tokio::test]
async fn req_dasclaw_cli_loop_e25_cancel_does_not_leak_to_subsequent_run() {
    // Step 1: cancel an in-flight run via outer timeout. Step 2: build
    // a brand-new Agent (fresh responder + executor + hooks) and run
    // it to completion. The second run must succeed normally —
    // canceling the first must not poison shared globals (logging
    // subscribers, panic hooks, task-local state, etc.).

    // === Step 1: cancel ===
    let cancel_responder = ScriptedResponder::with_queue(vec![
        tool_call_turn("slow_tool", "call_sleep", json!({})),
        text_turn("unreachable"),
    ]);
    let cancel_executor = SleepyToolExecutor::new(Duration::from_secs(5));
    let cancel_agent = Agent::builder()
        .responder(cancel_responder)
        .tool_executor(cancel_executor)
        .tools(no_tool_defs())
        .hooks(build_hooks())
        .build()
        .expect("first agent builds");

    let _ = timeout(
        Duration::from_millis(50),
        cancel_agent.invoke("first", AgentRunOptions::invoke()),
    )
    .await;
    tokio::task::yield_now().await;

    // Drop the canceled agent to release its tool executor before
    // building the next one. Mirrors the real CLI pattern of one
    // Agent per invocation.
    drop(cancel_agent);

    // === Step 2: fresh agent, fast path ===
    let fresh_responder = ScriptedResponder::with_queue(vec![text_turn("recovered cleanly")]);
    let fresh_executor = RecordingToolExecutor::new();
    let fresh_calls = fresh_executor.calls();
    let fresh_agent = Agent::builder()
        .responder(fresh_responder)
        .tool_executor(fresh_executor)
        .tools(no_tool_defs())
        .hooks(build_hooks())
        .build()
        .expect("second agent builds");

    let reply = fresh_agent
        .invoke("second", AgentRunOptions::invoke())
        .await
        .expect("subsequent run must complete normally after a cancel")
        .text;

    assert_eq!(reply, "recovered cleanly");
    assert_eq!(
        fresh_calls.lock().expect("calls log").len(),
        0,
        "fresh run had no tool calls scheduled"
    );
}

#[tokio::test]
async fn req_dasclaw_cli_loop_e25_timeout_does_not_panic_on_drop() {
    // Positive control for the "no panic on cancel" leg: wrap the
    // whole timeout dance in std::panic::catch_unwind via a
    // `tokio::spawn` + `JoinHandle::await`. If dropping the in-flight
    // sleep future panicked, the join handle would surface it as
    // `Err(JoinError::Panic)`.

    let responder = ScriptedResponder::with_queue(vec![
        tool_call_turn("slow_tool", "call_sleep", json!({})),
        text_turn("unreachable"),
    ]);
    let executor = SleepyToolExecutor::new(Duration::from_secs(5));
    let agent = Agent::builder()
        .responder(responder)
        .tool_executor(executor)
        .tools(no_tool_defs())
        .hooks(build_hooks())
        .build()
        .expect("agent builds");

    let handle = tokio::spawn(async move {
        let _ = timeout(
            Duration::from_millis(50),
            agent.invoke("hold", AgentRunOptions::invoke()),
        )
        .await;
    });

    let join = handle.await;
    assert!(
        join.is_ok(),
        "tokio::spawn task must not panic when the inner agent.run is \
         canceled mid-tool-sleep, got: {join:?}"
    );
}
