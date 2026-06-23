# Dasclaw App Server R2 Runtime Request Owners Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete R2 by turning the app-server `ServerRequest` surfaces for user input, permissions, dynamic client tools, and file-change approval into runtime-owned request/response flows. After R2, the renderer may still provide the human UI from R1, but app-server/runtime must no longer rely on fail-closed placeholders or synthetic empty answers for these protocol families.

**Architecture:** Keep the app-server JSON-RPC protocol as the wire boundary, keep `DasclawAgentRuntimeBridge` as the app-server runtime owner, and add a typed pending-request broker plus runtime tool adapters that suspend the model/tool path until the renderer response is delivered. The runtime adapter emits typed requests through app-server, app-server stores the pending response sender keyed by request id, renderer replies through the existing request response channel, and runtime converts accepted/denied/error responses back into tool results or fail-safe refusals.

**Tech Stack:** Rust workspace crates `dasclaw_app_server`, `dasclaw_app_server_protocol`, `dasclaw_protocol`, `dasclaw_runtime`; async `tokio`; JSON serialization through `serde`/`serde_json`; existing app-server tests; targeted `cargo check`/`cargo nextest`; docs update in `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`.

---

## Start Gate

This plan intentionally triggers the repository analysis discipline.

| Question | Answer | Required action |
| --- | --- | --- |
| 新增模块 / crate / 文件？ | Yes: the implementation may add an app-server runtime request owner module and this plan adds a new file. | Level 1 semantic search before planning. |
| 结论里是否含否定语？ | Yes: the plan describes remaining runtime/request-owner gaps. | Level 1 + Level 2 + Level 3 verification. |
| 是否做跨项目对账？ | Yes: R2 is derived from Codex app-server protocol parity. | Three-level verification. |
| 是否写架构对账类文档？ | Yes: this is a protocol gap closure plan. | Three-level verification and evidence recorded below. |

Commit messages for implementation commits under this plan must include:

```text
已检查 app-server runtime request owner 是否已有，结论：协议 DTO/常量已有，runtime/app-server 对 user-input、permissions、dynamic-tool、file-change approval 的真实挂起-响应 owner 未完整闭环。
```

## Verification Evidence Used For This Plan

Level 1 semantic search found existing protocol/request building blocks, not a complete runtime owner:

- `dasclaw_protocol/src/request_user_input.rs` has `RequestUserInputQuestionOption`, `RequestUserInputQuestion`, `RequestUserInputArgs`, `RequestUserInputEvent`, and `RequestUserInputAnswer`.
- `dasclaw_protocol/src/dynamic_tools.rs` has `DynamicToolSpec`, `DynamicToolCallRequest`, and `DynamicToolResponse`.
- `dasclaw_protocol/src/request_permissions.rs` has request/response structures for permission grants.
- `dasclaw_protocol/src/approvals.rs` has execution and patch approval event structures.
- Codex reference search found app-server handlers for file-change approval response and `item/tool/call` request plumbing.

Level 2 LSP verification found the same symbols at definition/workspace-symbol level:

- `RuntimeBridge`, `DasclawAgentRuntimeBridge`, `RuntimeBridgeFeatures`, and `RuntimeApprovalRequest` are currently centered in `crates/dasclaw_app_server/src/lib.rs`.
- `DynamicToolCallRequest` is defined in `crates/dasclaw_protocol/src/dynamic_tools.rs`.
- `RequestUserInputEvent` is defined in `crates/dasclaw_protocol/src/request_user_input.rs` and re-exported through protocol surfaces.
- `FileChangeRequestApprovalParams`, `FileChangeRequestApprovalResponse`, `CommandExecutionApprovalRequest`, and `PermissionsApprovalRequest` exist in app-server protocol definitions.

Level 3 literal search in the current R2 worktree established the implementation gap:

- `crates/dasclaw_app_server_protocol/src/lib.rs` already defines server-request method constants for command approval, file-change approval, permissions approval, user input, and dynamic tool call.
- `crates/dasclaw_app_server/src/lib.rs` currently exposes `RuntimeBridge::resolve_approval` but does not expose a generic typed resolution path for user input, permissions, dynamic tools, and file-change approval in the mainline worktree.
- `crates/dasclaw_core/src/agentic_loop.rs` currently exposes `AgentEvent::TextChunk`, `ReasoningSummaryChunk`, `ToolCallStart`, `ToolResult`, `FinishReason`, `ApprovalNeeded`, and `Completed`; it does not carry typed user-input/dynamic-tool/file-change request events.
- `crates/dasclaw_runtime/src/agent.rs` has `Agent::respond_to_approval`, but no equivalent response method for user input, dynamic client tools, permissions, or file-change approval.
- `desktop-app/src/renderer/src/hooks/useDasclawAssistantRuntime.ts` in the mainline worktree rejects approval-like app-server requests until the R1 UI/product plan is applied.

## Preconditions

- R2 should be implemented on top of the R1 product/request-routing branch. If the working branch does not contain the R1 renderer request queue and typed app-server request response API, implement or merge R1 first.
- R2 must not mark any gap-matrix R2 row complete until there is an integration test proving the runtime waits for a renderer response and consumes that response.
- R2 must preserve fail-safe behavior. Missing renderer handlers, malformed responses, duplicate responses, turn cancellation, and runtime shutdown must unblock pending runtime waits with explicit denial/error results rather than silently approving.

## Scope

In scope:

- Runtime-owned pending request broker for app-server `ServerRequest` flows.
- Dynamic client tool execution path that emits `item/tool/call`, waits for renderer response, and returns a typed tool result.
- Runtime user-input request path that emits `item/tool/requestUserInput`, waits for renderer answers, and converts them into runtime-visible output.
- Runtime permissions request path that emits `item/permissions/requestApproval`, waits for renderer decision, and converts denial into fail-safe tool output.
- Runtime file-change approval path that emits `item/fileChange/requestApproval`, waits for renderer decision, and gates the file-changing operation.
- Integration tests that exercise request emission, pending-state cleanup, successful response, denial response, duplicate response, and shutdown cleanup.
- Updating `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md` only after tests prove the implemented rows.

Out of scope:

- Full sandbox enforcement. Sandbox protocol and command-exec enforcement remain separate from R2 unless the implementation touches those code paths for type sharing.
- A complete patch-apply engine. R2 owns the file-change approval request/decision lifecycle; the actual file mutation may remain delegated to an existing or test-only file-changing executor.
- New renderer design beyond the R1 queue/panel. R2 may extend R1 response payloads but should not redesign the UI.
- New external dependencies.

## Implementation Tasks

### Task 1: Add Runtime Client Request Broker Tests

Create failing tests first in `crates/dasclaw_app_server/src/lib.rs` or a new sibling test module under the same crate. If the test module becomes large, create `crates/dasclaw_app_server/src/runtime_request_owner.rs` and keep tests adjacent under `#[cfg(test)]`.

- [ ] Add a test proving a runtime dynamic-tool request is emitted to the app-server sink, remains pending, accepts a renderer response, and removes the pending entry.
- [ ] Add a test proving a user-input request converts renderer answers into the runtime response shape.
- [ ] Add a test proving a permissions denial returns a fail-safe denied result and removes the pending entry.
- [ ] Add a test proving a file-change approval denial prevents the wrapped file-changing executor from running.
- [ ] Add a test proving duplicate responses return `RuntimeBridgeError::UnknownRequest` or an equivalent explicit error.
- [ ] Add a test proving shutdown cancels all pending request waiters with a fail-safe error.

Use concrete request ids so failures are easy to read:

```rust
use uuid::Uuid;

const TURN_ID: &str = "turn-r2-runtime-owner";

fn fixed_request_id(offset: u128) -> Uuid {
    Uuid::from_u128(0x2222_0000_0000_0000_0000_0000_0000_0000 + offset)
}
```

The first failing test should look like this shape:

```rust
#[tokio::test]
async fn dynamic_tool_request_waits_for_renderer_response_and_cleans_pending() {
    let owner = RuntimeClientRequestOwner::new_for_test();
    let request_id = fixed_request_id(1);

    let response_waiter = owner
        .request_dynamic_tool(RuntimeDynamicToolCallRequest {
            request_id,
            turn_id: TURN_ID.to_string(),
            namespace: "client".to_string(),
            tool: "open_url".to_string(),
            arguments: serde_json::json!({ "url": "https://example.test/r2" }),
        })
        .await
        .expect("request is accepted");

    let emitted = owner.next_emitted_request().await.expect("request emitted");
    assert_eq!(emitted.request_id(), request_id);
    assert_eq!(owner.pending_len(), 1);

    owner
        .resolve_dynamic_tool_response(
            request_id,
            RuntimeDynamicToolCallResponse {
                success: true,
                content: vec![RuntimeDynamicToolContent::Text("opened".to_string())],
            },
        )
        .await
        .expect("response resolves pending request");

    let response = response_waiter.await.expect("waiter receives response");
    assert!(response.success);
    assert_eq!(owner.pending_len(), 0);
}
```

The names in this snippet are the target API for Task 2. If existing R1 names differ, keep the behavior and adapt the final identifiers to existing R1 types instead of introducing a second parallel naming system.

### Task 2: Implement The App-Server Runtime Request Owner

Add a typed owner that app-server and runtime adapters can share. Preferred location:

```text
crates/dasclaw_app_server/src/runtime_request_owner.rs
```

Wire it from `crates/dasclaw_app_server/src/lib.rs` with:

```rust
mod runtime_request_owner;

use runtime_request_owner::{
    RuntimeClientRequestOwner,
    RuntimeClientRequestWaiter,
    RuntimeDynamicToolCallRequest,
    RuntimeDynamicToolCallResponse,
    RuntimeFileChangeApprovalRequest,
    RuntimeFileChangeApprovalResponse,
    RuntimePermissionsApprovalRequest,
    RuntimePermissionsApprovalResponse,
    RuntimeUserInputRequest,
    RuntimeUserInputResponse,
};
```

Core owner shape:

```rust
use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;
use tokio::sync::{mpsc, oneshot, Mutex};
use uuid::Uuid;

#[derive(Clone)]
pub(crate) struct RuntimeClientRequestOwner {
    pending: Arc<Mutex<HashMap<Uuid, PendingRuntimeClientRequest>>>,
    emitted_tx: mpsc::Sender<RuntimeClientRequestEnvelope>,
}

enum PendingRuntimeClientRequest {
    DynamicTool(oneshot::Sender<RuntimeDynamicToolCallResponse>),
    UserInput(oneshot::Sender<RuntimeUserInputResponse>),
    Permissions(oneshot::Sender<RuntimePermissionsApprovalResponse>),
    FileChange(oneshot::Sender<RuntimeFileChangeApprovalResponse>),
}

#[derive(Debug, Clone)]
pub(crate) enum RuntimeClientRequestEnvelope {
    DynamicTool(RuntimeDynamicToolCallRequest),
    UserInput(RuntimeUserInputRequest),
    Permissions(RuntimePermissionsApprovalRequest),
    FileChange(RuntimeFileChangeApprovalRequest),
}

impl RuntimeClientRequestEnvelope {
    pub(crate) fn request_id(&self) -> Uuid {
        match self {
            Self::DynamicTool(request) => request.request_id,
            Self::UserInput(request) => request.request_id,
            Self::Permissions(request) => request.request_id,
            Self::FileChange(request) => request.request_id,
        }
    }
}
```

Required behavior:

- [ ] `request_dynamic_tool`, `request_user_input`, `request_permissions`, and `request_file_change_approval` insert exactly one pending sender before emitting the envelope.
- [ ] If app-server cannot emit the envelope, remove the pending entry immediately and return a `RuntimeBridgeError`.
- [ ] `resolve_*` methods verify request kind before sending responses; kind mismatch must be an error and must not resolve a different pending request.
- [ ] Dropped waiters and shutdown remove pending entries.
- [ ] The owner provides `pending_len()` only under `#[cfg(test)]`.
- [ ] The owner must not auto-approve anything. The default outcome for cancellation is denial/error.

### Task 3: Connect Owner Emissions To App-Server ServerRequest Notifications

Modify `DasclawAgentRuntimeBridge` in `crates/dasclaw_app_server/src/lib.rs`.

- [ ] Add a `RuntimeClientRequestOwner` field to `DasclawAgentRuntimeBridge`.
- [ ] Start a small forwarding task for each turn that reads `RuntimeClientRequestEnvelope` values and converts them to app-server `ServerRequest` messages.
- [ ] Reuse R1 app-server request queue/response plumbing. If R1 exposes a generic `send_server_request` helper, route all R2 envelopes through that helper.
- [ ] Use the existing app-server protocol constants:
  - `item/tool/call`
  - `item/tool/requestUserInput`
  - `item/permissions/requestApproval`
  - `item/fileChange/requestApproval`
- [ ] Store the request id from the runtime owner as the app-server request id. Do not generate a second id at the app-server layer.
- [ ] Keep command-execution approval on its existing path unless Task 6 intentionally deduplicates common pending-map code.

Forwarding function target shape:

```rust
async fn forward_runtime_client_request(
    app_server: AppServerHandle,
    envelope: RuntimeClientRequestEnvelope,
) -> Result<(), RuntimeBridgeError> {
    match envelope {
        RuntimeClientRequestEnvelope::DynamicTool(request) => {
            app_server
                .send_server_request(
                    request.request_id,
                    ITEM_TOOL_CALL,
                    RuntimeDynamicToolCallParams::from(request),
                )
                .await
        }
        RuntimeClientRequestEnvelope::UserInput(request) => {
            app_server
                .send_server_request(
                    request.request_id,
                    ITEM_TOOL_REQUEST_USER_INPUT,
                    RuntimeUserInputParams::from(request),
                )
                .await
        }
        RuntimeClientRequestEnvelope::Permissions(request) => {
            app_server
                .send_server_request(
                    request.request_id,
                    ITEM_PERMISSIONS_REQUEST_APPROVAL,
                    RuntimePermissionsApprovalParams::from(request),
                )
                .await
        }
        RuntimeClientRequestEnvelope::FileChange(request) => {
            app_server
                .send_server_request(
                    request.request_id,
                    ITEM_FILE_CHANGE_REQUEST_APPROVAL,
                    RuntimeFileChangeApprovalParams::from(request),
                )
                .await
        }
    }
}
```

Use the real R1 helper/type names when implementing. The behavioral requirement is that every R2 envelope becomes one app-server request and exactly one pending runtime waiter.

### Task 4: Add Runtime Tool Adapter For Dynamic Client Tools, User Input, And Permissions

Add an adapter in the app-server crate so it can access app-server request owner state without creating a new dependency from `dasclaw_runtime` back to `dasclaw_app_server`.

Preferred file:

```text
crates/dasclaw_app_server/src/runtime_client_tools.rs
```

Wire it with:

```rust
mod runtime_client_tools;
```

Adapter responsibilities:

- [ ] Implement the existing runtime `ToolExecutor` trait for `AppServerClientToolExecutor`.
- [ ] Recognize dynamic client tool calls by namespace/tool name. At minimum support `client.open_url` and any R1 renderer-side `open_url` registration name.
- [ ] Recognize `request_user_input` tool calls and parse arguments as `RequestUserInputArgs`.
- [ ] Recognize `request_permissions` tool calls and parse arguments as `RequestPermissionsArgs`.
- [ ] For each recognized call, create a runtime owner request, wait for response, and return a `ToolResult` whose content is explicit and serializable.
- [ ] For unsupported tool names, delegate to the existing runtime tool executor or return the existing unknown-tool behavior. Do not swallow unknown tools with success.

Target shape:

```rust
#[derive(Clone)]
pub(crate) struct AppServerClientToolExecutor {
    owner: RuntimeClientRequestOwner,
    fallback: Arc<dyn ToolExecutor>,
}

#[async_trait::async_trait]
impl ToolExecutor for AppServerClientToolExecutor {
    async fn execute(&self, call: ToolCall) -> ToolExecutionResult {
        match classify_client_request_tool(&call) {
            ClientRequestToolKind::DynamicTool { namespace, tool } => {
                self.execute_dynamic_tool(call, namespace, tool).await
            }
            ClientRequestToolKind::UserInput => self.execute_user_input(call).await,
            ClientRequestToolKind::Permissions => self.execute_permissions(call).await,
            ClientRequestToolKind::NotClientRequest => self.fallback.execute(call).await,
        }
    }
}
```

Classification must be deterministic:

```rust
fn classify_client_request_tool(call: &ToolCall) -> ClientRequestToolKind {
    match call.name.as_str() {
        "request_user_input" => ClientRequestToolKind::UserInput,
        "request_permissions" => ClientRequestToolKind::Permissions,
        "open_url" => ClientRequestToolKind::DynamicTool {
            namespace: "client".to_string(),
            tool: "open_url".to_string(),
        },
        name if name.starts_with("client.") => {
            let tool = name.trim_start_matches("client.").to_string();
            ClientRequestToolKind::DynamicTool {
                namespace: "client".to_string(),
                tool,
            }
        }
        _ => ClientRequestToolKind::NotClientRequest,
    }
}
```

If existing `ToolCall`/`ToolExecutor` names differ, use the existing trait and field names. Keep the classification behavior exactly covered by tests.

### Task 5: Add File-Change Approval Gate Around File-Changing Tools

Implement file-change approval as a gate, not as silent synthetic approval.

- [ ] Identify the existing tool path that can apply patches or write files from runtime. If no production path exists in this branch, create a test-only `FileChangingToolExecutor` fixture and keep production wiring guarded behind the existing executor path.
- [ ] Add a small wrapper around the file-changing executor:

```rust
#[derive(Clone)]
pub(crate) struct FileChangeApprovalGate<E> {
    owner: RuntimeClientRequestOwner,
    inner: E,
}

impl<E> FileChangeApprovalGate<E>
where
    E: ToolExecutor + Clone + Send + Sync + 'static,
{
    async fn execute_with_approval(&self, call: ToolCall) -> ToolExecutionResult {
        let request = RuntimeFileChangeApprovalRequest::from_tool_call(&call)?;
        let decision = self.owner.request_file_change_approval(request).await?;

        if !decision.approved {
            return ToolExecutionResult::denied("file change denied by user");
        }

        self.inner.execute(call).await
    }
}
```

- [ ] Include the before/after file summary in `RuntimeFileChangeApprovalRequest`; do not send raw unbounded file contents.
- [ ] If a response is malformed or missing, return a denied result and do not run the inner executor.
- [ ] If the user approves but the inner executor fails, return the inner executor failure unchanged.
- [ ] Add a test proving denial prevents the inner executor from running.

### Task 6: Add Typed Response Resolution From App-Server To Runtime Owner

Extend the app-server response handler that R1 uses for renderer replies.

- [ ] Route `item/tool/call` responses into `RuntimeClientRequestOwner::resolve_dynamic_tool_response`.
- [ ] Route `item/tool/requestUserInput` responses into `RuntimeClientRequestOwner::resolve_user_input_response`.
- [ ] Route `item/permissions/requestApproval` responses into `RuntimeClientRequestOwner::resolve_permissions_response`.
- [ ] Route `item/fileChange/requestApproval` responses into `RuntimeClientRequestOwner::resolve_file_change_response`.
- [ ] Reject method/request-id mismatches with an explicit app-server error.
- [ ] Preserve existing command approval handling while sharing response-map cleanup helpers where it reduces duplication.

Typed conversion target:

```rust
fn decode_runtime_response(
    method: &str,
    value: serde_json::Value,
) -> Result<RuntimeClientResponse, RuntimeBridgeError> {
    match method {
        ITEM_TOOL_CALL => serde_json::from_value::<RuntimeDynamicToolCallResponse>(value)
            .map(RuntimeClientResponse::DynamicTool)
            .map_err(RuntimeBridgeError::invalid_response),
        ITEM_TOOL_REQUEST_USER_INPUT => serde_json::from_value::<RuntimeUserInputResponse>(value)
            .map(RuntimeClientResponse::UserInput)
            .map_err(RuntimeBridgeError::invalid_response),
        ITEM_PERMISSIONS_REQUEST_APPROVAL => {
            serde_json::from_value::<RuntimePermissionsApprovalResponse>(value)
                .map(RuntimeClientResponse::Permissions)
                .map_err(RuntimeBridgeError::invalid_response)
        }
        ITEM_FILE_CHANGE_REQUEST_APPROVAL => {
            serde_json::from_value::<RuntimeFileChangeApprovalResponse>(value)
                .map(RuntimeClientResponse::FileChange)
                .map_err(RuntimeBridgeError::invalid_response)
        }
        _ => Err(RuntimeBridgeError::unsupported_method(method)),
    }
}
```

### Task 7: Wire The Adapter Into Agent Startup

Modify `DasclawAgentRuntimeBridge::start_turn` in `crates/dasclaw_app_server/src/lib.rs`.

- [ ] Construct one `RuntimeClientRequestOwner` per bridge or per turn. Prefer per bridge if request ids are globally unique and shutdown cleanup is clear; prefer per turn if turn cancellation needs isolated cleanup.
- [ ] Wrap the existing runtime tool executor with `AppServerClientToolExecutor`.
- [ ] Wrap file-changing tools with `FileChangeApprovalGate` where applicable.
- [ ] Ensure `cancel_turn` cancels outstanding R2 waiters for that turn.
- [ ] Ensure `shutdown` cancels all outstanding R2 waiters.
- [ ] Ensure request ids include enough turn context for logs and tests.

Expected startup pattern:

```rust
let client_request_owner = self.client_request_owner.for_turn(request.turn_id.clone());
let tool_executor = AppServerClientToolExecutor::new(
    client_request_owner.clone(),
    existing_tool_executor,
);

let agent = AgentBuilder::new()
    .with_tool_executor(tool_executor)
    .with_tool_lifecycle_observer(observer)
    .build()
    .map_err(RuntimeBridgeError::from)?;
```

Use the real builder methods from `dasclaw_runtime`. Do not add a second agent construction path.

### Task 8: Add End-To-End App-Server Tests

Add app-server-level tests that exercise the JSON-RPC boundary.

- [ ] Test dynamic tool call:
  - Start a turn with a runtime fixture that triggers `client.open_url`.
  - Assert the app-server emits `item/tool/call`.
  - Send renderer success response.
  - Assert the runtime receives success content and the turn can complete.
- [ ] Test user input:
  - Trigger `request_user_input`.
  - Assert `item/tool/requestUserInput` includes exact question ids.
  - Send answers.
  - Assert runtime output includes the chosen answer ids/text.
- [ ] Test permissions denial:
  - Trigger `request_permissions`.
  - Send denial.
  - Assert runtime output is denied and does not grant any permission profile.
- [ ] Test file-change denial:
  - Trigger a file-changing tool fixture.
  - Send denial.
  - Assert no file write occurs.
- [ ] Test stale response:
  - Send a response for an unknown request id.
  - Assert explicit error and no panic.

Use `cargo nextest` filters for the new tests:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(dynamic_tool_request_waits_for_renderer_response_and_cleans_pending) | test(user_input_request_returns_renderer_answers) | test(permissions_denial_is_fail_safe) | test(file_change_denial_prevents_inner_executor) | test(stale_runtime_request_response_is_rejected)'
```

### Task 9: Update The Gap Matrix With Evidence

Only after Tasks 1-8 pass, update:

```text
docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md
```

Required updates:

- [ ] Add an R2 evidence row under `## 0. 过程透明记录` with the three-level verification summary and test command output.
- [ ] In `## 7. Codex ServerRequest 对 Dasclaw 缺口表`, mark the following rows according to the implemented tests:
  - `item/fileChange/requestApproval`
  - `item/permissions/requestApproval`
  - `item/tool/requestUserInput`
  - `item/tool/call`
- [ ] Do not mark MCP elicitation complete unless R2 actually implements `mcpServer/elicitation/request`.
- [ ] Do not mark sandbox or PTY rows complete unless the sandbox/PTY plan has landed separately.
- [ ] Add a short "R2 remaining gaps" paragraph if any request family remains product-only or test-only.

Evidence paragraph format:

```markdown
### R2 Runtime Request Owner Evidence (2026-06-22)

- Level 1: semantic search found existing protocol DTOs in `dasclaw_protocol`, but no complete runtime-owned pending request flow.
- Level 2: LSP workspace symbols confirmed app-server bridge/protocol/request structs and runtime approval-only response method.
- Level 3: `rg` confirmed app-server method constants exist and runtime bridge only had command approval resolution before R2.
- Validation: `cargo check -p dasclaw_app_server --tests`; `cargo nextest run -p dasclaw_app_server -E '<R2 test filter>'`; `cargo fmt --all`; `python3.12 scripts/check_no_panics.py --base origin/xClaw`.
```

Replace `<R2 test filter>` with the exact command run during implementation.

### Task 10: Required Verification

Run the smallest checks that prove R2:

```bash
cargo check -p dasclaw_app_server --tests
cargo nextest run -p dasclaw_app_server -E 'test(dynamic_tool_request_waits_for_renderer_response_and_cleans_pending) | test(user_input_request_returns_renderer_answers) | test(permissions_denial_is_fail_safe) | test(file_change_denial_prevents_inner_executor) | test(stale_runtime_request_response_is_rejected)'
cargo fmt --all
python3.12 scripts/check_no_panics.py --base origin/xClaw
cargo clippy --no-deps -p dasclaw_app_server --all-targets -- -D warnings
```

If R2 changes `dasclaw_runtime` or `dasclaw_protocol`, add package-specific checks:

```bash
cargo check -p dasclaw_runtime --tests
cargo check -p dasclaw_protocol --tests
cargo nextest run -p dasclaw_runtime
cargo nextest run -p dasclaw_protocol
cargo clippy --no-deps -p dasclaw_runtime --all-targets -- -D warnings
cargo clippy --no-deps -p dasclaw_protocol --all-targets -- -D warnings
```

Do not run workspace-wide build/check/clippy locally unless a failure specifically requires it.

## Code Quality And Review Requirements

- [ ] Run `code-quality-audit` after implementation.
- [ ] Run `code-simplifier` after implementation unless the final change is a verbatim port.
- [ ] Run `adr-compliance-check` if the change touches `crates/dasclaw_*`, drift guards, starlark pins, `.ironclaw` literals, or related CI policy surfaces.
- [ ] Run `code-review-expert` before push or PR.
- [ ] Confirm the final implementation did not introduce a second app-server request stack parallel to R1.
- [ ] Confirm all denied/cancelled/malformed paths fail closed.

## Expected Final State

- App-server protocol constants and DTOs remain the wire source of truth.
- `DasclawAgentRuntimeBridge` owns pending R2 runtime requests and resolves them from renderer responses.
- Dynamic client tool calls can suspend runtime execution until renderer returns success/failure content.
- User-input requests can suspend runtime execution until renderer returns answers.
- Permission requests can suspend runtime execution and deny safely without granting permissions.
- File-change approval can gate file-changing execution and denial prevents mutation.
- Gap matrix R2 rows are updated only for behavior proven by tests.
