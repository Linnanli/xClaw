# Dasclaw App Server ContextCompactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current deterministic `thread/compact/start` placeholder with a real model-backed compact owner using `dasclaw_core::ContextCompactor`.

**Architecture:** Keep `ThreadLifecycleHost` as the app-server history owner and keep compact persistence in app-server. Convert the app-server turn snapshot into a `dasclaw_core::session::Thread`, run `ContextCompactor` with a runtime-backed `LlmCompleter`, persist the returned model summary as the compacted turn, and emit `thread/compacted` only after that succeeds. Do not add Codex-style remote compact endpoint compatibility in this slice.

**Tech Stack:** Rust 2024, `dasclaw_app_server`, `dasclaw_core::compaction::ContextCompactor`, `dasclaw_llm_provider::provider::ClawCodeLlmProvider`, `dasclaw_runtime`, JSON-RPC protocol tests, cargo-nextest.

---

## Scope

This plan deliberately uses one compact path:

- `thread/compact/start`
- app-server reads thread turns from `ThreadLifecycleHost`
- `DasclawAgentRuntimeBridge` calls `dasclaw_core::ContextCompactor`
- `ContextCompactor` calls a real LLM through `LlmCompleter`
- app-server persists the compacted turn and emits `thread/compacted`

Out of scope:

- Codex-style `/responses/compact`.
- Importing `codex-cli-main` session, rollout, telemetry, `ResponseItem`, or `CompactedItem`.
- Changing MCP, command execution, approvals, sandboxing, or tool execution.
- Storing a new provenance field in thread snapshots. Tests prove behavior through persisted summary text and absence of deterministic placeholder text.
- Creating commits. Current repo rules say do not commit unless explicitly asked, so checkpoint steps use `git diff --check`.

## Existing Evidence

- `crates/dasclaw_app_server/src/lib.rs:1352` routes `thread/compact/start`.
- `crates/dasclaw_app_server/src/lib.rs:5693` currently returns deterministic compact output.
- `crates/dasclaw_app_server/src/lib.rs:5801` currently says no model-generated summary was produced.
- `crates/dasclaw_core/src/compaction.rs:41` already exposes `ContextCompactor`.
- `crates/dasclaw_core/src/compaction.rs:194` already generates a summary through `LlmCompleter`.
- `desktop-client/ironclaw/src/agent/thread_ops.rs:1102` already uses `ContextCompactor` for manual compact.

## File Structure

- Modify `crates/dasclaw_app_server/src/lib.rs`
  - Register `mod thread_compact;`.
  - Add selected model snapshot to `RuntimeThreadCompactRequest`.
  - Route `DasclawAgentRuntimeBridge::compact_thread` through `thread_compact::compact_with_context_compactor`.
  - Remove deterministic compact output helpers.
  - Update compact tests and capability tests.

- Create `crates/dasclaw_app_server/src/thread_compact.rs`
  - Convert app-server completed turns into `dasclaw_core::session::Thread`.
  - Implement `RuntimeSnapshotCompleter` as a `dasclaw_core::LlmCompleter`.
  - Run `ContextCompactor` with `CompactionStrategy::Summarize { keep_recent: 5 }`.
  - Return `RuntimeThreadCompactResult` with model-generated summary text.

- Modify `crates/dasclaw_app_server/src/thread_lifecycle.rs`
  - Keep current snapshot/history persistence behavior.
  - Do not add provider logic here.

- Modify `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`
  - Mark compact as backed by the default runtime only after tests prove `ContextCompactor` is called.

## Task 1: Lock The Contract With Failing Tests

**Files:**
- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Test: `crates/dasclaw_app_server/src/lib.rs`

- [ ] **Step 1: Replace deterministic compact success test**

Find `thread_compact_start_with_real_runtime_persists_deterministic_compaction_turn` and replace it with:

```rust
#[test]
fn thread_compact_start_with_real_runtime_persists_model_generated_summary_turn() {
    let bridge = Arc::new(ModelSummaryCompactBridge::new(
        "model summary: keep issue #42 and run cargo check",
    ));
    let mut server = initialized_codex_v2_server_with_bridge(bridge);
    let thread = server
        .create_thread_for_test(TestThreadParams { cwd: None })
        .expect("thread should be created");

    server
        .turn_start(TurnStartParams {
            thread_id: thread.thread_id.clone(),
            input: text_input("please remember issue #42".to_string()),
            cwd: None,
            model: None,
            summary: None,
            sandbox_policy: None,
            permission_profile: None,
        })
        .expect("turn should start");
    drain_until_method(&mut server, "turn/completed", Duration::from_secs(2));

    let response = server
        .handle_json_rpc(&format!(
            r#"{{"jsonrpc":"2.0","id":"compact","method":"thread/compact/start","params":{{"threadId":"{}"}}}}"#,
            thread.thread_id
        ))
        .expect("thread/compact/start should return a structured response");
    let value: Value = serde_json::from_str(&response).expect("response should decode");
    assert_eq!(value["result"], serde_json::json!({}));

    let notifications = server.drain_json_rpc_notifications();
    assert!(
        notifications
            .iter()
            .any(|line| line.contains(r#""method":"thread/compacted""#)),
        "compact success should emit thread/compacted: {notifications:?}"
    );

    let read = server
        .thread_turns_list(ThreadTurnsListParams {
            thread_id: thread.thread_id,
            limit: None,
            after: None,
        })
        .expect("thread turns should be readable");
    let text = read
        .turns
        .iter()
        .filter_map(turn_text)
        .find(|text| text.contains("model summary: keep issue #42"))
        .expect("compacted model summary should be persisted");
    assert!(text.contains("run cargo check"));
    assert!(!text.contains("Deterministic local compaction"));
    assert!(!text.contains("No model-generated summary was produced."));
}
```

- [ ] **Step 2: Add a compact bridge helper used by route-level tests**

Add this helper in the `#[cfg(test)]` module near the existing compact bridge helpers:

```rust
#[derive(Debug)]
struct ModelSummaryCompactBridge {
    summary: String,
}

impl ModelSummaryCompactBridge {
    fn new(summary: &str) -> Self {
        Self {
            summary: summary.to_string(),
        }
    }
}

impl RuntimeBridge for ModelSummaryCompactBridge {
    fn features(&self) -> RuntimeBridgeFeatures {
        RuntimeBridgeFeatures {
            thread_compact: true,
            ..RuntimeBridgeFeatures::default()
        }
    }

    fn start_turn(&self, request: RuntimeTurnStartRequest) -> Result<(), RuntimeBridgeError> {
        request
            .updates
            .completed(
                request.thread_id,
                request.turn_id,
                request.prompt,
                dasclaw_core::response_types::TokenUsage::default(),
                dasclaw_core::messages::FinishReason::Stop,
                dasclaw_core::response_types::ResponseMetadata::default(),
            )
            .map_err(|error| RuntimeBridgeError::retryable(error.to_string()))
    }

    fn cancel_turn(&self, _request: RuntimeTurnCancelRequest) -> Result<(), RuntimeBridgeError> {
        Ok(())
    }

    fn compact_thread(
        &self,
        request: RuntimeThreadCompactRequest,
    ) -> Result<RuntimeThreadCompactResult, RuntimeBridgeError> {
        Ok(RuntimeThreadCompactResult {
            turn_id: request.compacted_turn_id.clone(),
            output: self.summary.clone(),
            items: vec![CodexThreadItem::completed_agent_message(
                request.compacted_turn_id,
                self.summary.clone(),
            )],
        })
    }
}
```

- [ ] **Step 3: Run the focused route test**

Run:

```bash
CARGO_TARGET_DIR=/Users/nallylin/Documents/code/x-claw/target RUSTC_WRAPPER= \
cargo nextest run -p dasclaw_app_server -E 'test(thread_compact_start_with_real_runtime_persists_model_generated_summary_turn)'
```

Expected before implementation: fail because the existing deterministic runtime bridge still produces placeholder text when this test is converted to use the default bridge in Task 4.

- [ ] **Step 4: Checkpoint without committing**

Run:

```bash
git diff --check
```

Expected: no whitespace errors.

## Task 2: Pass The Selected Model Snapshot Into Compact

**Files:**
- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Test: `crates/dasclaw_app_server/src/lib.rs`

- [ ] **Step 1: Extend compact request**

Replace `RuntimeThreadCompactRequest` with:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeThreadCompactRequest {
    pub thread_id: String,
    pub compacted_turn_id: String,
    pub model_provider: RuntimeModelProviderSnapshot,
    pub turns: Vec<RuntimeThreadCompactTurn>,
}
```

- [ ] **Step 2: Route selected model into the compact request**

In `thread_compact_start`, replace the request construction block with:

```rust
let compacted_turn_id = self.threads.next_turn_id();
let turns = self.threads.compact_turn_snapshot(&params.thread_id)?;
let model_provider = self.model_provider.selected_snapshot()?;
let result = self
    .runtime_bridge
    .compact_thread(RuntimeThreadCompactRequest {
        thread_id: params.thread_id.clone(),
        compacted_turn_id: compacted_turn_id.clone(),
        model_provider,
        turns,
    })
    .map_err(AppServerError::runtime_bridge)?;
```

- [ ] **Step 3: Update all direct compact request constructors in tests**

Every `RuntimeThreadCompactRequest { ... }` in tests must include:

```rust
model_provider: test_runtime_model_snapshot(),
```

For tests that cannot see `test_runtime_model_snapshot()`, use:

```rust
model_provider: RuntimeModelProviderSnapshot {
    model_id: "gpt-test".to_string(),
    provider: "openai".to_string(),
    api_base_url: "http://localhost:11434/v1".to_string(),
    api_key: "test-api-key".to_string(),
    api_format: "openai".to_string(),
    model_call_mode: dasclaw_runtime::ModelCallMode::Stream,
},
```

- [ ] **Step 4: Add a test proving compact receives the selected model**

Add this test near the compact route tests:

```rust
#[test]
fn thread_compact_start_passes_selected_model_snapshot_to_runtime() {
    #[derive(Debug)]
    struct CapturingCompactBridge {
        seen_model: Arc<Mutex<Option<String>>>,
    }

    impl RuntimeBridge for CapturingCompactBridge {
        fn features(&self) -> RuntimeBridgeFeatures {
            RuntimeBridgeFeatures {
                thread_compact: true,
                ..RuntimeBridgeFeatures::default()
            }
        }

        fn start_turn(&self, request: RuntimeTurnStartRequest) -> Result<(), RuntimeBridgeError> {
            request
                .updates
                .completed(
                    request.thread_id,
                    request.turn_id,
                    "done".to_string(),
                    dasclaw_core::response_types::TokenUsage::default(),
                    dasclaw_core::messages::FinishReason::Stop,
                    dasclaw_core::response_types::ResponseMetadata::default(),
                )
                .map_err(|error| RuntimeBridgeError::retryable(error.to_string()))
        }

        fn cancel_turn(&self, _request: RuntimeTurnCancelRequest) -> Result<(), RuntimeBridgeError> {
            Ok(())
        }

        fn compact_thread(
            &self,
            request: RuntimeThreadCompactRequest,
        ) -> Result<RuntimeThreadCompactResult, RuntimeBridgeError> {
            *self.seen_model.lock().expect("seen model lock") =
                Some(request.model_provider.model_id.clone());
            Ok(RuntimeThreadCompactResult {
                turn_id: request.compacted_turn_id.clone(),
                output: "summary".to_string(),
                items: vec![CodexThreadItem::completed_agent_message(
                    request.compacted_turn_id,
                    "summary".to_string(),
                )],
            })
        }
    }

    let seen_model = Arc::new(Mutex::new(None));
    let bridge = Arc::new(CapturingCompactBridge {
        seen_model: Arc::clone(&seen_model),
    });
    let mut server = initialized_codex_v2_server_with_bridge(bridge);
    server
        .initialize(InitializeParams {
            client: Some(ClientInfo {
                name: "test".to_string(),
                version: "0.0.0".to_string(),
                transport: TransportKind::Stdio,
            }),
            protocol_version: Some(ProtocolVersion::default()),
            requested_capabilities: Vec::new(),
            model_provider: Some(ModelProviderInitializeConfig {
                models: vec![test_client_model("gpt-a"), test_client_model("gpt-b")],
                selected_model: test_client_model("gpt-b"),
            }),
        })
        .expect("initialize should accept selected model");
    let thread = server
        .create_thread_for_test(TestThreadParams { cwd: None })
        .expect("thread should be created");
    server
        .thread_compact_start(ThreadCompactStartParams {
            thread_id: thread.thread_id,
        })
        .expect("compact should call runtime");

    assert_eq!(
        seen_model.lock().expect("seen model lock").as_deref(),
        Some("gpt-b")
    );
}
```

- [ ] **Step 5: Run selected model test**

Run:

```bash
CARGO_TARGET_DIR=/Users/nallylin/Documents/code/x-claw/target RUSTC_WRAPPER= \
cargo nextest run -p dasclaw_app_server -E 'test(thread_compact_start_passes_selected_model_snapshot_to_runtime)'
```

Expected: pass.

- [ ] **Step 6: Checkpoint without committing**

Run:

```bash
git diff --check
```

Expected: no whitespace errors.

## Task 3: Add The ContextCompactor Adapter Module

**Files:**
- Create: `crates/dasclaw_app_server/src/thread_compact.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Test: `crates/dasclaw_app_server/src/thread_compact.rs`

- [ ] **Step 1: Register the module**

Add this near the top of `crates/dasclaw_app_server/src/lib.rs` with other internal modules:

```rust
mod thread_compact;
```

- [ ] **Step 2: Make snapshot helpers crate-visible**

Change:

```rust
fn registry_config_from_snapshot(
fn redact_snapshot_secret(
```

to:

```rust
pub(crate) fn registry_config_from_snapshot(
pub(crate) fn redact_snapshot_secret(
```

- [ ] **Step 3: Create `thread_compact.rs`**

Create `crates/dasclaw_app_server/src/thread_compact.rs`:

```rust
use std::sync::Arc;

use async_trait::async_trait;
use dasclaw_core::compaction::ContextCompactor;
use dasclaw_core::context_monitor::CompactionStrategy;
use dasclaw_core::messages::CompletionRequest;
use dasclaw_core::session::Thread;
use dasclaw_core::traits::{HostError, LlmCompleter};
use dasclaw_llm_provider::provider::{ClawCodeLlmProvider, LlmProvider};
use tokio::runtime::Builder;
use uuid::Uuid;

use crate::{
    CodexThreadItem, RuntimeBridgeError, RuntimeModelProviderSnapshot, RuntimeThreadCompactRequest,
    RuntimeThreadCompactResult, RuntimeThreadCompactTurn, TurnStatus, redact_snapshot_secret,
    registry_config_from_snapshot,
};

const LOCAL_COMPACT_KEEP_RECENT: usize = 5;

pub(crate) fn compact_with_context_compactor(
    request: RuntimeThreadCompactRequest,
) -> Result<RuntimeThreadCompactResult, RuntimeBridgeError> {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| RuntimeBridgeError::fatal(error.to_string()))?;
    runtime.block_on(compact_with_context_compactor_async(request))
}

async fn compact_with_context_compactor_async(
    request: RuntimeThreadCompactRequest,
) -> Result<RuntimeThreadCompactResult, RuntimeBridgeError> {
    let mut thread = core_thread_from_compact_request(&request);
    let compactor = ContextCompactor::new(Arc::new(RuntimeSnapshotCompleter::new(
        request.model_provider.clone(),
    )));
    let result = compactor
        .compact(
            &mut thread,
            CompactionStrategy::Summarize {
                keep_recent: LOCAL_COMPACT_KEEP_RECENT,
            },
            None,
        )
        .await
        .map_err(|error| RuntimeBridgeError::retryable(error.to_string()))?;
    let output = result.summary.unwrap_or_else(|| {
        "No compact summary was generated because the completed thread history fit inside the retention window.".to_string()
    });

    Ok(RuntimeThreadCompactResult {
        turn_id: request.compacted_turn_id.clone(),
        items: vec![CodexThreadItem::completed_agent_message(
            request.compacted_turn_id,
            output.clone(),
        )],
        output,
    })
}

pub(crate) fn core_thread_from_compact_request(request: &RuntimeThreadCompactRequest) -> Thread {
    let mut thread = Thread::new(Uuid::new_v4());
    for (turn_id, output) in completed_turn_outputs(&request.turns) {
        thread.start_turn(format!("Completed app-server turn {turn_id}"));
        thread.complete_turn(output);
    }
    thread
}

fn completed_turn_outputs(turns: &[RuntimeThreadCompactTurn]) -> Vec<(String, String)> {
    turns
        .iter()
        .filter(|turn| turn.status == TurnStatus::Completed)
        .filter_map(|turn| {
            let output = turn.output.clone().or_else(|| compact_turn_text(turn));
            output.map(|text| (turn.turn_id.clone(), text))
        })
        .collect()
}

fn compact_turn_text(turn: &RuntimeThreadCompactTurn) -> Option<String> {
    turn.items.iter().find_map(|item| match item {
        CodexThreadItem::AgentMessage { text, .. } if !text.trim().is_empty() => {
            Some(text.clone())
        }
        _ => None,
    })
}

#[derive(Debug)]
struct RuntimeSnapshotCompleter {
    snapshot: RuntimeModelProviderSnapshot,
}

impl RuntimeSnapshotCompleter {
    fn new(snapshot: RuntimeModelProviderSnapshot) -> Self {
        Self { snapshot }
    }
}

#[async_trait]
impl LlmCompleter for RuntimeSnapshotCompleter {
    async fn complete_text(&self, request: CompletionRequest) -> Result<String, HostError> {
        let config = registry_config_from_snapshot(&self.snapshot)
            .map_err(|error| Box::new(error) as HostError)?;
        let provider = ClawCodeLlmProvider::from_registry_config(&config).map_err(|error| {
            Box::new(RuntimeBridgeError::fatal(redact_snapshot_secret(
                &error.to_string(),
                &self.snapshot,
            ))) as HostError
        })?;
        let response = provider.complete(request).await.map_err(|error| {
            Box::new(RuntimeBridgeError::retryable(redact_snapshot_secret(
                &error.to_string(),
                &self.snapshot,
            ))) as HostError
        })?;
        Ok(response.content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_snapshot() -> RuntimeModelProviderSnapshot {
        RuntimeModelProviderSnapshot {
            model_id: "gpt-test".to_string(),
            provider: "openai".to_string(),
            api_base_url: "http://localhost:11434/v1".to_string(),
            api_key: "test-api-key".to_string(),
            api_format: "openai".to_string(),
            model_call_mode: dasclaw_runtime::ModelCallMode::Stream,
        }
    }

    fn completed_turn(id: &str, text: &str) -> RuntimeThreadCompactTurn {
        RuntimeThreadCompactTurn {
            turn_id: id.to_string(),
            status: TurnStatus::Completed,
            output: Some(text.to_string()),
            items: vec![CodexThreadItem::completed_agent_message(
                id.to_string(),
                text.to_string(),
            )],
            error: None,
        }
    }

    #[test]
    fn core_thread_from_compact_request_preserves_completed_turn_order() {
        let request = RuntimeThreadCompactRequest {
            thread_id: "thread_1".to_string(),
            compacted_turn_id: "turn_compacted".to_string(),
            model_provider: test_snapshot(),
            turns: vec![completed_turn("turn_1", "first"), completed_turn("turn_2", "second")],
        };

        let thread = core_thread_from_compact_request(&request);
        let messages = thread.messages();
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].content, "Completed app-server turn turn_1");
        assert_eq!(messages[1].content, "first");
        assert_eq!(messages[2].content, "Completed app-server turn turn_2");
        assert_eq!(messages[3].content, "second");
    }

    #[test]
    fn core_thread_from_compact_request_ignores_failed_turns() {
        let request = RuntimeThreadCompactRequest {
            thread_id: "thread_1".to_string(),
            compacted_turn_id: "turn_compacted".to_string(),
            model_provider: test_snapshot(),
            turns: vec![
                completed_turn("turn_1", "first"),
                RuntimeThreadCompactTurn {
                    turn_id: "turn_2".to_string(),
                    status: TurnStatus::Failed,
                    output: Some("failed output".to_string()),
                    items: Vec::new(),
                    error: Some("boom".to_string()),
                },
                completed_turn("turn_3", "third"),
            ],
        };

        let thread = core_thread_from_compact_request(&request);
        let messages = thread.messages();
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].content, "Completed app-server turn turn_1");
        assert_eq!(messages[1].content, "first");
        assert_eq!(messages[2].content, "Completed app-server turn turn_3");
        assert_eq!(messages[3].content, "third");
    }
}
```

- [ ] **Step 4: Run adapter tests**

Run:

```bash
CARGO_TARGET_DIR=/Users/nallylin/Documents/code/x-claw/target RUSTC_WRAPPER= \
cargo nextest run -p dasclaw_app_server \
-E 'test(core_thread_from_compact_request_preserves_completed_turn_order) | test(core_thread_from_compact_request_ignores_failed_turns)'
```

Expected: both tests pass.

- [ ] **Step 5: Checkpoint without committing**

Run:

```bash
git diff --check
```

Expected: no whitespace errors.

## Task 4: Wire DasclawAgentRuntimeBridge To ContextCompactor

**Files:**
- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Test: `crates/dasclaw_app_server/src/lib.rs`

- [ ] **Step 1: Import the compact adapter**

Add this import near other crate-local imports:

```rust
use thread_compact::compact_with_context_compactor;
```

- [ ] **Step 2: Replace deterministic compact implementation**

Replace `DasclawAgentRuntimeBridge::compact_thread` with:

```rust
fn compact_thread(
    &self,
    request: RuntimeThreadCompactRequest,
) -> Result<RuntimeThreadCompactResult, RuntimeBridgeError> {
    compact_with_context_compactor(request)
}
```

- [ ] **Step 3: Remove deterministic helper functions**

Delete these functions from `crates/dasclaw_app_server/src/lib.rs`:

```rust
fn deterministic_local_compaction_output(request: &RuntimeThreadCompactRequest) -> String
fn compact_turn_preview(turn: &RuntimeThreadCompactTurn) -> Option<String>
fn preview_text(text: &str, max_chars: usize) -> String
```

If `preview_text` is used by non-compact code, keep only that function and move it near the caller that still uses it.

- [ ] **Step 4: Add a provider-backed compact test**

Add this test near `provider_backed_raw_response_item_reaches_app_server_notification`:

```rust
#[test]
fn provider_backed_thread_compact_uses_context_compactor_summary() {
    let (base_url, fixture_done) = spawn_single_completion_fixture_server(
        "model summary: issue #42, cargo check, no deterministic placeholder",
    );
    let bridge = Arc::new(DasclawAgentRuntimeBridge::from_model_provider_snapshot());
    let mut server = initialized_codex_v2_server_with_bridge(bridge);
    server
        .initialize(InitializeParams {
            client: Some(ClientInfo {
                name: "test".to_string(),
                version: "0.0.0".to_string(),
                transport: TransportKind::Stdio,
            }),
            protocol_version: Some(ProtocolVersion::default()),
            requested_capabilities: Vec::new(),
            model_provider: Some(ModelProviderInitializeConfig {
                models: vec![ClientModelConfig {
                    model_id: "gpt-test".to_string(),
                    display_name: Some("gpt-test".to_string()),
                    provider: Some("openai".to_string()),
                    api_base_url: Some(base_url.clone()),
                    api_key: Some("test-api-key".to_string()),
                    api_format: Some("openai".to_string()),
                    source: Some("test".to_string()),
                }],
                selected_model: ClientModelConfig {
                    model_id: "gpt-test".to_string(),
                    display_name: Some("gpt-test".to_string()),
                    provider: Some("openai".to_string()),
                    api_base_url: Some(base_url),
                    api_key: Some("test-api-key".to_string()),
                    api_format: Some("openai".to_string()),
                    source: Some("test".to_string()),
                },
            }),
        })
        .expect("initialize should accept test provider");
    let thread = server
        .create_thread_for_test(TestThreadParams { cwd: None })
        .expect("thread should be created");
    server
        .turn_start(TurnStartParams {
            thread_id: thread.thread_id.clone(),
            input: text_input("remember issue #42".to_string()),
            cwd: None,
            model: None,
            summary: None,
            sandbox_policy: None,
            permission_profile: None,
        })
        .expect("turn should start");
    drain_until_method(&mut server, "turn/completed", Duration::from_secs(2));

    server
        .thread_compact_start(ThreadCompactStartParams {
            thread_id: thread.thread_id.clone(),
        })
        .expect("compact should succeed through ContextCompactor");

    fixture_done
        .recv_timeout(Duration::from_secs(1))
        .expect("fixture server should receive model summary request");
    let turns = server
        .thread_turns_list(ThreadTurnsListParams {
            thread_id: thread.thread_id,
            limit: None,
            after: None,
        })
        .expect("turns should be readable");
    let compact_text = turns
        .turns
        .iter()
        .filter_map(turn_text)
        .find(|text| text.contains("model summary: issue #42"))
        .expect("compacted model summary should be persisted");
    assert!(!compact_text.contains("Deterministic local compaction"));
    assert!(!compact_text.contains("No model-generated summary was produced."));
}
```

- [ ] **Step 5: Add the single completion fixture**

Add this helper near `spawn_codex_chatgpt_responses_fixture_server`:

```rust
fn spawn_single_completion_fixture_server(
    summary: &'static str,
) -> (String, std::sync::mpsc::Receiver<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("fixture server should bind localhost");
    listener
        .set_nonblocking(true)
        .expect("fixture server should support nonblocking accept");
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let (done_tx, done_rx) = std::sync::mpsc::channel();

    thread::spawn(move || {
        let sse = format!(
            "event: response.output_text.delta\ndata: {{\"delta\":{}}}\n\nevent: response.completed\ndata: {{\"response\":{{\"usage\":{{\"input_tokens\":3,\"output_tokens\":2}}}}}}\n\n",
            serde_json::to_string(summary).expect("summary JSON")
        );
        let responses = [
            (
                "GET /models",
                "application/json",
                r#"{"models":[{"slug":"gpt-test"}]}"#.to_string(),
            ),
            ("POST /responses", "text/event-stream", sse),
        ];
        let deadline = Instant::now() + Duration::from_secs(5);

        for (expected_prefix, content_type, body) in responses {
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(accepted) => break accepted,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < deadline,
                            "fixture server timed out waiting for {expected_prefix}"
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("fixture server accept failed: {error}"),
                }
            };

            let request = read_fixture_http_request(&mut stream);
            let request_line = request.lines().next().unwrap_or_default();
            assert!(
                request_line.starts_with(expected_prefix),
                "expected request line prefix {expected_prefix:?}, got {request_line:?}"
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            stream
                .write_all(response.as_bytes())
                .expect("fixture response should write");
        }
        done_tx.send(()).expect("fixture should signal completion");
    });

    (base_url, done_rx)
}
```

- [ ] **Step 6: Run provider-backed compact test**

Run:

```bash
CARGO_TARGET_DIR=/Users/nallylin/Documents/code/x-claw/target RUSTC_WRAPPER= \
cargo nextest run -p dasclaw_app_server -E 'test(provider_backed_thread_compact_uses_context_compactor_summary)'
```

Expected: pass.

- [ ] **Step 7: Checkpoint without committing**

Run:

```bash
git diff --check
```

Expected: no whitespace errors.

## Task 5: Prove Failure Is Fail-Safe

**Files:**
- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Test: `crates/dasclaw_app_server/src/lib.rs`

- [ ] **Step 1: Add a failing provider fixture**

Add this helper near the success fixture:

```rust
fn spawn_error_completion_fixture_server() -> (String, std::sync::mpsc::Receiver<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("fixture server should bind localhost");
    listener
        .set_nonblocking(true)
        .expect("fixture server should support nonblocking accept");
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let (done_tx, done_rx) = std::sync::mpsc::channel();

    thread::spawn(move || {
        let responses = [
            (
                "GET /models",
                "application/json",
                r#"{"models":[{"slug":"gpt-test"}]}"#.to_string(),
                200,
            ),
            (
                "POST /responses",
                "application/json",
                r#"{"error":"compact failed"}"#.to_string(),
                500,
            ),
        ];
        let deadline = Instant::now() + Duration::from_secs(5);

        for (expected_prefix, content_type, body, status) in responses {
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(accepted) => break accepted,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < deadline,
                            "fixture server timed out waiting for {expected_prefix}"
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("fixture server accept failed: {error}"),
                }
            };

            let request = read_fixture_http_request(&mut stream);
            let request_line = request.lines().next().unwrap_or_default();
            assert!(
                request_line.starts_with(expected_prefix),
                "expected request line prefix {expected_prefix:?}, got {request_line:?}"
            );
            let response = format!(
                "HTTP/1.1 {status} TEST\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            stream
                .write_all(response.as_bytes())
                .expect("fixture response should write");
        }
        done_tx.send(()).expect("fixture should signal completion");
    });

    (base_url, done_rx)
}
```

- [ ] **Step 2: Add the fail-safe test**

Add this test near compact failure tests:

```rust
#[test]
fn thread_compact_start_model_failure_does_not_update_snapshot() {
    let (base_url, fixture_done) = spawn_error_completion_fixture_server();
    let bridge = Arc::new(DasclawAgentRuntimeBridge::from_model_provider_snapshot());
    let mut server = initialized_codex_v2_server_with_bridge(bridge);
    server
        .initialize(InitializeParams {
            client: Some(ClientInfo {
                name: "test".to_string(),
                version: "0.0.0".to_string(),
                transport: TransportKind::Stdio,
            }),
            protocol_version: Some(ProtocolVersion::default()),
            requested_capabilities: Vec::new(),
            model_provider: Some(ModelProviderInitializeConfig {
                models: vec![ClientModelConfig {
                    model_id: "gpt-test".to_string(),
                    display_name: Some("gpt-test".to_string()),
                    provider: Some("openai".to_string()),
                    api_base_url: Some(base_url.clone()),
                    api_key: Some("test-api-key".to_string()),
                    api_format: Some("openai".to_string()),
                    source: Some("test".to_string()),
                }],
                selected_model: ClientModelConfig {
                    model_id: "gpt-test".to_string(),
                    display_name: Some("gpt-test".to_string()),
                    provider: Some("openai".to_string()),
                    api_base_url: Some(base_url),
                    api_key: Some("test-api-key".to_string()),
                    api_format: Some("openai".to_string()),
                    source: Some("test".to_string()),
                },
            }),
        })
        .expect("initialize should accept test provider");
    let thread = server
        .create_thread_for_test(TestThreadParams { cwd: None })
        .expect("thread should be created");
    let before = server
        .thread_read(ThreadReadParams {
            thread_id: thread.thread_id.clone(),
        })
        .expect("thread should be readable before compact")
        .thread;

    let error = server
        .thread_compact_start(ThreadCompactStartParams {
            thread_id: thread.thread_id.clone(),
        })
        .expect_err("model compact failure should fail the request");
    assert_eq!(error.code, AppServerErrorCode::ServiceDegraded);

    fixture_done
        .recv_timeout(Duration::from_secs(1))
        .expect("fixture server should receive failed model request");
    let after = server
        .thread_read(ThreadReadParams {
            thread_id: thread.thread_id,
        })
        .expect("thread should be readable after failed compact")
        .thread;
    assert_eq!(after.compacted_turn_id, before.compacted_turn_id);
    assert!(
        server
            .drain_json_rpc_notifications()
            .iter()
            .all(|line| !line.contains(r#""method":"thread/compacted""#))
    );
}
```

- [ ] **Step 3: Run fail-safe tests**

Run:

```bash
CARGO_TARGET_DIR=/Users/nallylin/Documents/code/x-claw/target RUSTC_WRAPPER= \
cargo nextest run -p dasclaw_app_server \
-E 'test(thread_compact_start_model_failure_does_not_update_snapshot) | test(thread_compact_start_runtime_failure_does_not_update_snapshot) | test(thread_compact_start_rejects_runtime_turn_id_mismatch_without_snapshot_update)'
```

Expected: all selected tests pass.

- [ ] **Step 4: Checkpoint without committing**

Run:

```bash
git diff --check
```

Expected: no whitespace errors.

## Task 6: Capability Gating And Documentation

**Files:**
- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Modify: `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`
- Test: `crates/dasclaw_app_server/src/lib.rs`

- [ ] **Step 1: Keep Noop runtime declared-only**

Keep this expectation in `noop_runtime_bridge_keeps_thread_compact_declared_until_owner_exists`:

```rust
assert_eq!(
    capabilities.thread_compact.status,
    CapabilityStatus::Declared
);
assert!(capabilities.thread_compact.methods.is_empty());
assert!(capabilities.thread_compact.events.is_empty());
```

- [ ] **Step 2: Rename the default runtime capability test**

Rename:

```rust
fn dasclaw_runtime_bridge_default_features_include_thread_compact_owner()
```

to:

```rust
fn dasclaw_runtime_bridge_default_features_include_context_compactor_thread_compact_owner()
```

Keep:

```rust
let bridge = DasclawAgentRuntimeBridge::from_model_provider_snapshot();
assert!(bridge.features().thread_compact);
```

- [ ] **Step 3: Update the gap matrix compact status**

In `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`, replace statements saying default runtime lacks a compact owner with:

```markdown
`thread/compact/start` is now backed by the default Dasclaw runtime compact owner through `dasclaw_core::ContextCompactor`. The owner generates a model-backed summary from completed app-server turns, persists the compacted turn through `ThreadLifecycleHost`, and emits `thread/compacted` only after persistence succeeds. Failures are fail-safe: no compacted turn is written and no compact notification is emitted.
```

- [ ] **Step 4: Add a boundary note**

Add this note near the P1 compact row:

```markdown
Boundary note: this is not a Codex CLI `/responses/compact` compatibility layer. App-server keeps its own thread snapshot model and uses `dasclaw_core::ContextCompactor` directly.
```

- [ ] **Step 5: Run capability tests**

Run:

```bash
CARGO_TARGET_DIR=/Users/nallylin/Documents/code/x-claw/target RUSTC_WRAPPER= \
cargo nextest run -p dasclaw_app_server \
-E 'test(dasclaw_runtime_bridge_default_features_include_context_compactor_thread_compact_owner) | test(noop_runtime_bridge_keeps_thread_compact_declared_until_owner_exists) | test(provider_runtime_bridge_advertises_thread_compact_when_owner_is_ready)'
```

Expected: all selected tests pass.

- [ ] **Step 6: Checkpoint without committing**

Run:

```bash
git diff --check
```

Expected: no whitespace errors.

## Task 7: Full Verification

**Files:**
- Verify only.

- [ ] **Step 1: Format**

Run:

```bash
cargo fmt --all
```

Expected: exit code 0.

- [ ] **Step 2: Compile app-server tests**

Run:

```bash
CARGO_TARGET_DIR=/Users/nallylin/Documents/code/x-claw/target RUSTC_WRAPPER= \
cargo check -p dasclaw_app_server --tests
```

Expected: exit code 0.

- [ ] **Step 3: Run compact and lifecycle tests**

Run:

```bash
CARGO_TARGET_DIR=/Users/nallylin/Documents/code/x-claw/target RUSTC_WRAPPER= \
cargo nextest run -p dasclaw_app_server \
-E 'test(thread_compact_) | test(thread_lifecycle_) | test(provider_backed_thread_compact_uses_context_compactor_summary) | test(thread_compact_start_model_failure_does_not_update_snapshot) | test(dasclaw_runtime_bridge_default_features_include_context_compactor_thread_compact_owner)'
```

Expected: all selected tests pass.

- [ ] **Step 4: Run no-panics check**

Run:

```bash
python3.12 scripts/check_no_panics.py --base origin/xClaw
```

Expected: exit code 0.

- [ ] **Step 5: Run app-server clippy**

Run:

```bash
CARGO_TARGET_DIR=/Users/nallylin/Documents/code/x-claw/target RUSTC_WRAPPER= \
cargo clippy --no-deps -p dasclaw_app_server --all-targets -- -D warnings
```

Expected: exit code 0.

- [ ] **Step 6: Final diff review**

Run:

```bash
git diff --stat
git diff --check
```

Expected: changed files are limited to app-server compact code, thread lifecycle persistence if needed, and compact documentation; no whitespace errors.

## Self-Review

- Spec coverage:
  - Direct `dasclaw_core::ContextCompactor` use: Tasks 3 and 4.
  - No Codex-style remote compact compatibility: Scope and Task 6 boundary note.
  - Capability gating: Task 6.
  - Snapshot/history persistence and `thread/compacted`: Tasks 1, 4, and 5.
  - Failure fail-safe: Task 5.
  - Avoid test-side masking: Task 1 rejects deterministic placeholder text; Task 4 uses a provider fixture; Task 5 verifies failed model summary does not update snapshot.

- Placeholder scan:
  - No forbidden placeholder markers are used.
  - Code-changing steps include concrete snippets.
  - Commands include expected results.

- Type consistency:
  - `RuntimeThreadCompactRequest::model_provider` is introduced before `thread_compact.rs` uses it.
  - `RuntimeThreadCompactResult` remains unchanged except existing constructors now receive real model summary output.
  - The only compact owner added by this plan is `ContextCompactor`.

## Execution Notes

- Use `/Users/nallylin/Documents/code/x-claw/target` through `CARGO_TARGET_DIR` for speed.
- Do not commit during execution unless the user explicitly changes the repo rule.
- If `ContextCompactor` returns no summary because the thread has five or fewer completed turns, preserve the explicit fallback string from Task 3 and keep capability implemented; this still proves the request is routed through the real compactor rather than deterministic app-server text.
