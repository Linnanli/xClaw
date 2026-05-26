//! ADR-153 §2.2 case **e25 (B9)** — D8 取消语义 e2e。
//!
//! Contract:
//! - 调用方用 [`tokio::time::timeout`] 包住 [`run_with_tools_and_hooks`]，
//!   期间工具进入一个长 sleep。
//! - timeout 到点后，整个 `run_with_tools_and_hooks` future 被 drop：
//!   1. 调用方拿到 [`tokio::time::error::Elapsed`]（外层 `Err`）；
//!   2. 工具 future 在 sleep await 处被取消，**不会**走到正常完成路径；
//!   3. 进程没有 panic（用 nextest 进程退 0 隐式保证）。
//!
//! 与 e21 (B5 `max_iterations`) 互补：那个是"达到主动上限"，本例是
//! "外部 drop 导致的协作式取消"。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use dasclaw_cli::run_with_tools_and_hooks;
use dasclaw_core::hooks::HookBundle;
use dasclaw_core::messages::{ToolCall, ToolDefinition, ToolResult};
use dasclaw_core::traits::HostError;
use dasclaw_runtime::ToolExecutor;

#[path = "fixtures/safety_fixtures.rs"]
mod safety_fixtures;

use safety_fixtures::{ScriptedResponder, tool_call_turn};

/// RAII probe：
/// - 进入 `execute` 时构造在栈上；
/// - 离开时（无论 normal return 还是 cancel-drop）触发 [`Drop`]；
/// - 若 Drop 时 `completed == false`，说明 future 在 sleep 处被取消，
///   原子地写下 `cancelled = true`。
///
/// 这样 cancellation 是 **观察式**的，不靠任何 tokio 内部 hook。
struct CancellationProbe {
    cancelled: Arc<AtomicBool>,
    completed: Arc<AtomicBool>,
}

impl Drop for CancellationProbe {
    fn drop(&mut self) {
        if !self.completed.load(Ordering::Acquire) {
            self.cancelled.store(true, Ordering::Release);
        }
    }
}

/// 工具入口睡眠足够久（默认 5s），保证外层 timeout（150ms）一定先触发。
pub struct SleepyToolExecutor {
    call_count: Arc<AtomicUsize>,
    cancelled: Arc<AtomicBool>,
    completed: Arc<AtomicBool>,
    sleep_for: Duration,
}

impl SleepyToolExecutor {
    fn new(sleep_for: Duration) -> Self {
        Self {
            call_count: Arc::new(AtomicUsize::new(0)),
            cancelled: Arc::new(AtomicBool::new(false)),
            completed: Arc::new(AtomicBool::new(false)),
            sleep_for,
        }
    }

    /// 外部观察句柄：在把 executor move 进 `run_with_tools_and_hooks` 之前留一份。
    fn handle(&self) -> SleepyHandle {
        SleepyHandle {
            call_count: Arc::clone(&self.call_count),
            cancelled: Arc::clone(&self.cancelled),
            completed: Arc::clone(&self.completed),
        }
    }
}

struct SleepyHandle {
    call_count: Arc<AtomicUsize>,
    cancelled: Arc<AtomicBool>,
    completed: Arc<AtomicBool>,
}

#[async_trait]
impl ToolExecutor for SleepyToolExecutor {
    async fn execute(&self, call: &ToolCall) -> Result<ToolResult, HostError> {
        // probe 必须先于 sleep 构造，且持有到 sleep 之后。
        let probe = CancellationProbe {
            cancelled: Arc::clone(&self.cancelled),
            completed: Arc::clone(&self.completed),
        };
        self.call_count.fetch_add(1, Ordering::Relaxed);
        tokio::time::sleep(self.sleep_for).await;
        self.completed.store(true, Ordering::Release);
        // 显式 drop probe，让"completed=true → 不算 cancelled"的语义清晰。
        drop(probe);
        Ok(ToolResult {
            tool_call_id: call.id.clone(),
            name: call.name.clone(),
            content: "should never reach here in cancel test".into(),
            is_error: false,
        })
    }
}

fn tool_def() -> ToolDefinition {
    ToolDefinition {
        name: "sleepy".into(),
        description: "tool that sleeps long enough to be cancelled".into(),
        parameters: serde_json::json!({"type":"object","properties":{}}),
    }
}

#[tokio::test]
async fn req_dasclaw_cli_safety_b9_cancel_drops_inflight_tool_future() {
    // Responder turn 1: 让模型马上发 sleepy 的 tool_call。
    // 由于 timeout 在 150ms 内一定触发，turn 2 永远到不了。
    let responder = ScriptedResponder::with_queue(vec![tool_call_turn(
        "sleepy",
        "call_1",
        serde_json::json!({}),
    )]);

    let sleepy = SleepyToolExecutor::new(Duration::from_secs(5));
    let handle = sleepy.handle();

    let outcome = tokio::time::timeout(
        Duration::from_millis(150),
        run_with_tools_and_hooks(
            responder,
            sleepy,
            vec![tool_def()],
            HookBundle::noop(),
            "system",
            "please use the sleepy tool",
        ),
    )
    .await;

    // 1. 外层 timeout 必须触发（Err = tokio::time::error::Elapsed）。
    assert!(
        outcome.is_err(),
        "expected outer tokio timeout to elapse, got: {outcome:?}"
    );

    // 2. 工具 future 进入过 execute（call_count == 1）。
    assert_eq!(
        handle.call_count.load(Ordering::Relaxed),
        1,
        "sleepy tool must have entered execute exactly once before cancellation"
    );

    // 3. 工具 future 未走到完成（completed == false）。
    assert!(
        !handle.completed.load(Ordering::Acquire),
        "sleepy tool must NOT have completed — timeout should drop the sleep mid-await"
    );

    // 4. probe 观察到了取消（cancelled == true）。
    assert!(
        handle.cancelled.load(Ordering::Acquire),
        "CancellationProbe::drop must observe cancellation (completed=false at drop time)"
    );

    // 5. 进程没有 panic：由 nextest 退出码隐式保证；测试函数若 panic，
    //    nextest 会标 FAIL；本测试在所有前述断言通过后正常返回 ⇒ 退 0。
}
