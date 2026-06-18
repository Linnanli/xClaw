# Dasclaw App Server Codex P3 Approval Tool Sandbox Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the P3 app-server safety boundary: server-initiated approval requests, tool lifecycle notifications, sandbox-aware runtime capability gating, and app-server stdio E2E coverage for approve/reject/fail-safe paths.

**Architecture:** Keep `dasclaw_app_server_protocol` as the JSON-RPC wire/type owner and `dasclaw_app_server` as the local control-plane owner. Extend the existing `RuntimeBridge` seam so runtime `AgentEvent::ApprovalNeeded`, `ToolCallStart`, and `ToolResult` become app-server protocol events without redefining the agent loop or `ToolExecutor`. Add server-initiated JSON-RPC request support to the stdio loop so clients can answer approval prompts with standard JSON-RPC responses.

**Tech Stack:** Rust 2024, serde JSON-RPC structs, `dasclaw_app_server_protocol`, `dasclaw_app_server`, `dasclaw_runtime`, `uuid`, `cargo nextest`, Electron main-process TypeScript, Vitest.

---

## Scope Check

This plan implements P3 from `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`:

- Approval request tracking and decision return.
- Tool lifecycle notification surface for the existing runtime event stream.
- Sandbox readiness as an explicit app-server capability/service gate.
- Fail-safe timeout and stale response handling.
- App-server stdio E2E tests that prove approval requests are emitted and client responses unblock or reject execution.

This plan intentionally keeps these separate from P3:

- `fs/*` methods and `fs/changed` notifications. Those belong to P5 after P3 approval/sandbox guardrails exist.
- Interactive terminal control for `command/exec/write/terminate/resize`. P3 may expose `item/commandExecution/*` lifecycle notifications from runtime tool events, but not a standalone command service.
- MCP, skills, jobs, and logs service owners. Those belong to P4.
- Codex account, plugin marketplace, app list, feedback, external agent import, realtime audio, and Windows-specific Codex product surfaces. Those remain P6/product decisions.

## Startup 4 Questions

| Question | Answer | Required action |
|---|---|---|
| 是否新增模块 / crate / 文件？ | Yes. This plan file is new, and implementation will add new protocol structs plus tests. | Semantic search was run before writing this plan. Implementation commits must mention the reuse check. |
| 结论是否包含否定语？ | Yes. The plan excludes P4/P5/P6 domains and keeps capability opt-outs explicit. | Verify with protocol schema tests, router tests, and stdio E2E. |
| 是否跨项目对账？ | Yes. The plan is derived from Codex app-server method/request shapes and Dasclaw runtime/app-server state. | Re-check Codex generated schema before changing wire names. |
| 是否写架构对账类文档？ | Yes. This is a protocol implementation plan derived from the gap matrix. | Preserve evidence-vs-inference separation and avoid capability claims without tests. |

Evidence gathered before this plan:

- Level 1 semantic search:
  - Query `dasclaw app server approval tool sandbox protocol plan server request end to end test` returned keyword mode with 0 nodes.
  - Query `approval request loop fail safe timeout tool execution sandbox adapter app server` returned hybrid results including `allows_sandbox_approval`, approval timeout references, and sandbox execution tests.
- Level 2 symbol layer:
  - `execute_lsp` was not exposed in this Codex App tool surface. Implementation workers should try LSP again if available; otherwise keep symbol verification through Rust compile/tests and exact `rg`.
- Level 3 literal layer:
  - `crates/dasclaw_app_server_protocol/src/lib.rs` currently declares `approval`, `tools`, and `sandbox` as future/declared capabilities.
  - `crates/dasclaw_app_server/src/lib.rs` currently routes only the existing method constants and parses incoming lines as client requests, not client responses to server-initiated requests.
  - `crates/dasclaw_runtime/src/approval.rs` already provides `ApprovalPolicy`, `ApprovalRequest`, `ApprovalDecision`, `ApprovalInbox`, `Approver`, and `Agent::respond_to_approval`.
  - `crates/dasclaw_core/src/agentic_loop.rs` already emits `AgentEvent::ApprovalNeeded`, `ToolCallStart`, and `ToolResult`.
  - `crates/dasclaw_app_server/src/lib.rs` already has app-server stdio E2E-style tests using `run_stdio_server_with_app_server`.

## File Structure

Modify:

- `crates/dasclaw_app_server_protocol/src/lib.rs`
  - Owns JSON-RPC wire types, method/event constants, capability matrix, compatibility profile, and protocol serialization tests.
  - Add server-initiated request constants for approval/tool request surfaces.
  - Add P3 notification constants for `serverRequest/resolved`, `item/commandExecution/outputDelta`, and `item/commandExecution/terminalInteraction`.
  - Add client-response wire type so app-server can parse JSON-RPC responses from the client.
  - Add request/response payload structs for approval decisions.
  - Add capability helper to mark approval/tools/sandbox implemented only when app-server runtime features prove they are available.

- `crates/dasclaw_app_server/src/lib.rs`
  - Owns routing, lifecycle, health, notification queue, stdio loop, runtime bridge, and app-server tests.
  - Add incoming JSON-RPC response handling.
  - Add pending server-request tracking with timeout and fail-safe rejection.
  - Extend `RuntimeBridge` with feature reporting and approval decision return.
  - Extend `RuntimeTurnOutcome` / `RuntimeTurnUpdateSink` for approval/tool lifecycle events.
  - Map runtime events into server requests and Codex-compatible notifications.
  - Add Rust stdio E2E tests for approve, reject, timeout, and stale response.

- `crates/dasclaw_app_server/Cargo.toml`
  - Add direct `uuid = { version = "1", features = ["serde"] }` because `dasclaw_app_server` must parse approval ids instead of relying on a transitive runtime dependency.

- `desktop-app/src/main/appServerRpc.ts`
  - Teach the child-process JSON-RPC client to classify server requests separately from notifications.
  - Add a response writer for server requests.
  - Keep existing `request()` and `onNotification()` behavior unchanged.

- `desktop-app/src/main/appServerRpc.test.ts`
  - Cover JSON-RPC classification of server requests and response line construction.

- `desktop-app/src/main/appServerManager.ts`
  - Register an app-server server-request listener and re-broadcast approval prompts to the renderer as app-server notifications with host context.
  - Add a narrow method for renderer decisions: `approval/respond`.

- `desktop-app/src/main/appServerManager.test.ts`
  - Cover forwarding of an app-server approval request and decision response without leaking raw tool arguments when display parameters are provided.

- `desktop-app/src/shared/appServerApi.ts`
  - Add shared TypeScript types for app-server approval request notifications and decision payloads.

Do not create a new Rust crate for P3. Keep the plan inside the existing app-server/protocol/runtime bridge boundary.

## Definition of Done

- `approval`, `tools`, and `sandbox` capabilities stay `declared` for `AppServer::new()` / `NoopRuntimeBridge`.
- A runtime bridge that reports P3 features makes those capabilities `implemented` and advertises the P3 wire events.
- `initialize` and `protocol/schema` never claim unsupported P4/P5/P6 methods.
- The stdio loop can emit a server-initiated JSON-RPC request with an id, then accept a client JSON-RPC response with the same id.
- Approve path: app-server E2E proves execution resumes and emits `serverRequest/resolved` plus a non-error tool result notification.
- Reject path: app-server E2E proves the tool is not executed, the pending request resolves as rejected, and the turn fails or completes with an approval rejection error according to runtime outcome.
- Timeout path: app-server E2E/unit coverage proves stale requests fail safe and late client responses are rejected as unknown/stale.
- Desktop-app unit tests prove server requests are no longer misclassified as notifications.

## Task 1: Lock P3 Protocol Contracts With Failing Tests

**Files:**
- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`

- [ ] **Step 1: Add failing protocol tests**

Add these tests inside the existing `#[cfg(test)] mod tests` in `crates/dasclaw_app_server_protocol/src/lib.rs`:

```rust
#[test]
fn p3_capability_helper_advertises_approval_tools_and_sandbox_contracts() {
    let matrix = CapabilityMatrix::phase_one().with_p3_approval_tool_sandbox();

    assert_eq!(matrix.approval.status, CapabilityStatus::Implemented);
    assert_eq!(
        matrix.approval.methods,
        vec![method::APPROVAL_RESPOND.to_string()]
    );
    assert_eq!(
        matrix.approval.events,
        vec![
            server_request::ITEM_COMMAND_EXECUTION_REQUEST_APPROVAL.to_string(),
            server_request::ITEM_PERMISSIONS_REQUEST_APPROVAL.to_string(),
            event::SERVER_REQUEST_RESOLVED.to_string(),
            event::ITEM_AUTO_APPROVAL_REVIEW_STARTED.to_string(),
            event::ITEM_AUTO_APPROVAL_REVIEW_COMPLETED.to_string(),
        ]
    );

    assert_eq!(matrix.tools.status, CapabilityStatus::Implemented);
    assert_eq!(
        matrix.tools.events,
        vec![
            server_request::ITEM_TOOL_CALL.to_string(),
            server_request::ITEM_TOOL_REQUEST_USER_INPUT.to_string(),
            event::ITEM_COMMAND_EXECUTION_OUTPUT_DELTA.to_string(),
            event::ITEM_COMMAND_EXECUTION_TERMINAL_INTERACTION.to_string(),
        ]
    );

    assert_eq!(matrix.sandbox.status, CapabilityStatus::Implemented);
    assert!(
        matrix
            .sandbox
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("runtime bridge reports sandbox-ready execution")
    );
}

#[test]
fn server_initiated_request_serializes_as_json_rpc_request_with_id() {
    let request = JsonRpcServerRequest::new(
        "approval_00000000-0000-0000-0000-000000000001",
        server_request::ITEM_COMMAND_EXECUTION_REQUEST_APPROVAL,
        CommandExecutionApprovalRequest {
            thread_id: "thread_1".to_string(),
            turn_id: "turn_1".to_string(),
            item_id: "turn_1:tool:bash".to_string(),
            tool_call_id: "call_1".to_string(),
            tool_name: "bash".to_string(),
            command: Some("echo hello".to_string()),
            description: "approve call to bash".to_string(),
            display_parameters: serde_json::json!({"cmd": "echo hello"}),
            allow_always: true,
        },
    )
    .expect("server request should serialize");

    let value = serde_json::to_value(&request).expect("server request should serialize to value");

    assert_eq!(value["jsonrpc"], JSON_RPC_VERSION);
    assert_eq!(
        value["id"],
        "approval_00000000-0000-0000-0000-000000000001"
    );
    assert_eq!(
        value["method"],
        server_request::ITEM_COMMAND_EXECUTION_REQUEST_APPROVAL
    );
    assert_eq!(value["params"]["threadId"], "thread_1");
    assert_eq!(value["params"]["displayParameters"]["cmd"], "echo hello");
    assert!(value["params"].get("rawArguments").is_none());
}

#[test]
fn client_response_deserializes_approval_decision_payload() {
    let response = serde_json::from_value::<JsonRpcClientResponse>(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "approval_00000000-0000-0000-0000-000000000001",
        "result": {
            "decision": {"kind": "approve"}
        }
    }))
    .expect("client response should deserialize");

    assert_eq!(
        response.id,
        serde_json::json!("approval_00000000-0000-0000-0000-000000000001")
    );
    let result: ApprovalResponsePayload =
        serde_json::from_value(response.result.expect("result should be present"))
            .expect("approval response payload should deserialize");
    assert_eq!(
        result.decision,
        AppServerApprovalDecision::Approve
    );
}

#[test]
fn server_request_resolved_notification_hides_decision_reason_when_absent() {
    let notification = ServerNotification::server_request_resolved(ServerRequestResolvedEvent {
        request_id: "approval_1".to_string(),
        thread_id: Some("thread_1".to_string()),
        turn_id: Some("turn_1".to_string()),
        outcome: ServerRequestResolutionOutcome::Approved,
        reason: None,
    })
    .expect("notification should serialize");

    let value = serde_json::to_value(notification).expect("notification should serialize");

    assert_eq!(value["method"], event::SERVER_REQUEST_RESOLVED);
    assert_eq!(value["params"]["requestId"], "approval_1");
    assert_eq!(value["params"]["outcome"], "approved");
    assert!(value["params"].get("reason").is_none());
}
```

- [ ] **Step 2: Run the tests and confirm they fail**

Run:

```bash
cargo nextest run -p dasclaw_app_server_protocol -E 'test(p3_capability_helper_advertises_approval_tools_and_sandbox_contracts) | test(server_initiated_request_serializes_as_json_rpc_request_with_id) | test(client_response_deserializes_approval_decision_payload) | test(server_request_resolved_notification_hides_decision_reason_when_absent)'
```

Expected: FAIL at compile time with missing items such as `JsonRpcServerRequest`, `JsonRpcClientResponse`, `server_request`, `method::APPROVAL_RESPOND`, `event::SERVER_REQUEST_RESOLVED`, and `with_p3_approval_tool_sandbox`.

- [ ] **Step 3: Commit the failing tests**

```bash
git add crates/dasclaw_app_server_protocol/src/lib.rs
git commit -m $'test: lock app-server p3 protocol contracts\n\n已检查 P3 approval/tool/sandbox 是否已有，结论：runtime 已有 approval/tool/sandbox primitive，app-server 缺 server-request 协议与接线。'
```

## Task 2: Implement P3 Protocol Types and Capability Schema

**Files:**
- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`

- [ ] **Step 1: Add method, server-request, and event constants**

Add these constants near the existing `pub mod method` and `pub mod event` blocks:

```rust
pub mod method {
    pub const APPROVAL_RESPOND: &str = "approval/respond";
}

pub mod server_request {
    pub const ITEM_COMMAND_EXECUTION_REQUEST_APPROVAL: &str =
        "item/commandExecution/requestApproval";
    pub const ITEM_FILE_CHANGE_REQUEST_APPROVAL: &str = "item/fileChange/requestApproval";
    pub const ITEM_PERMISSIONS_REQUEST_APPROVAL: &str = "item/permissions/requestApproval";
    pub const ITEM_TOOL_REQUEST_USER_INPUT: &str = "item/tool/requestUserInput";
    pub const ITEM_TOOL_CALL: &str = "item/tool/call";
}

pub mod event {
    pub const SERVER_REQUEST_RESOLVED: &str = "serverRequest/resolved";
    pub const ITEM_AUTO_APPROVAL_REVIEW_STARTED: &str = "item/autoApprovalReview/started";
    pub const ITEM_AUTO_APPROVAL_REVIEW_COMPLETED: &str = "item/autoApprovalReview/completed";
    pub const ITEM_COMMAND_EXECUTION_OUTPUT_DELTA: &str = "item/commandExecution/outputDelta";
    pub const ITEM_COMMAND_EXECUTION_TERMINAL_INTERACTION: &str =
        "item/commandExecution/terminalInteraction";
}
```

When editing the real file, merge these constants into the existing modules instead of creating duplicate `pub mod method` or `pub mod event` blocks.

- [ ] **Step 2: Add JSON-RPC client-response and server-request wire structs**

Add this after `JsonRpcResponse`:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonRpcClientResponse {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcClientResponse {
    #[must_use]
    pub fn ok(id: serde_json::Value, result: impl Serialize) -> Self {
        Self {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            id,
            result: Some(serde_json::to_value(result).unwrap_or(serde_json::Value::Null)),
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonRpcServerRequest {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl JsonRpcServerRequest {
    pub fn new(
        id: impl Into<String>,
        method: impl Into<String>,
        params: impl Serialize,
    ) -> Result<Self, serde_json::Error> {
        Ok(Self {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            id: serde_json::Value::String(id.into()),
            method: method.into(),
            params: Some(serde_json::to_value(params)?),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcIncoming {
    Request(JsonRpcRequest),
    ClientResponse(JsonRpcClientResponse),
}
```

- [ ] **Step 3: Add approval request and decision payload structs**

Add these near the session/event payload structs:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecutionApprovalRequest {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub tool_call_id: String,
    pub tool_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    pub description: String,
    pub display_parameters: serde_json::Value,
    pub allow_always: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionsApprovalRequest {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub tool_call_id: String,
    pub tool_name: String,
    pub description: String,
    pub display_parameters: serde_json::Value,
    pub allow_always: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum AppServerApprovalDecision {
    Approve,
    Reject {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    ApproveAlways,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalResponsePayload {
    pub decision: AppServerApprovalDecision,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerRequestResolutionOutcome {
    Approved,
    Rejected,
    TimedOut,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerRequestResolvedEvent {
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    pub outcome: ServerRequestResolutionOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecutionOutputDeltaEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub delta: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecutionTerminalInteractionEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub message: String,
    pub is_error: bool,
}
```

- [ ] **Step 4: Add P3 capability helper**

Add this to `impl CapabilityMatrix`:

```rust
#[must_use]
pub fn with_p3_approval_tool_sandbox(mut self) -> Self {
    self.approval = Capability::implemented(
        "approval",
        &[method::APPROVAL_RESPOND],
        &[
            server_request::ITEM_COMMAND_EXECUTION_REQUEST_APPROVAL,
            server_request::ITEM_PERMISSIONS_REQUEST_APPROVAL,
            event::SERVER_REQUEST_RESOLVED,
            event::ITEM_AUTO_APPROVAL_REVIEW_STARTED,
            event::ITEM_AUTO_APPROVAL_REVIEW_COMPLETED,
        ],
    );
    self.tools = Capability::implemented(
        "tools",
        &[],
        &[
            server_request::ITEM_TOOL_CALL,
            server_request::ITEM_TOOL_REQUEST_USER_INPUT,
            event::ITEM_COMMAND_EXECUTION_OUTPUT_DELTA,
            event::ITEM_COMMAND_EXECUTION_TERMINAL_INTERACTION,
        ],
    );
    self.sandbox = Capability::new(
        "sandbox",
        CapabilityStatus::Implemented,
        &[],
        &[],
        Some("runtime bridge reports sandbox-ready execution".to_string()),
    );
    self
}
```

- [ ] **Step 5: Add notification constructors**

Add constructors beside the existing `ServerNotification` constructors:

```rust
pub fn server_request_resolved(
    event: ServerRequestResolvedEvent,
) -> Result<Self, serde_json::Error> {
    Self::new(event::SERVER_REQUEST_RESOLVED, event)
}

pub fn command_execution_output_delta(
    event: CommandExecutionOutputDeltaEvent,
) -> Result<Self, serde_json::Error> {
    Self::new(event::ITEM_COMMAND_EXECUTION_OUTPUT_DELTA, event)
}

pub fn command_execution_terminal_interaction(
    event: CommandExecutionTerminalInteractionEvent,
) -> Result<Self, serde_json::Error> {
    Self::new(event::ITEM_COMMAND_EXECUTION_TERMINAL_INTERACTION, event)
}
```

- [ ] **Step 6: Extend protocol schema and compatibility profile**

In `phase_one_methods()`, add:

```rust
MethodSchema::new(
    method::APPROVAL_RESPOND,
    "approval",
    Some("ApprovalResponsePayload"),
    "ServerRequestResolvedEvent",
    true,
),
```

In `phase_one_events()`, add:

```rust
EventSchema::new(
    event::SERVER_REQUEST_RESOLVED,
    "approval",
    "ServerRequestResolvedEvent",
),
EventSchema::new(
    event::ITEM_COMMAND_EXECUTION_OUTPUT_DELTA,
    "tools",
    "CommandExecutionOutputDeltaEvent",
),
EventSchema::new(
    event::ITEM_COMMAND_EXECUTION_TERMINAL_INTERACTION,
    "tools",
    "CommandExecutionTerminalInteractionEvent",
),
```

In `CODEX_APP_SERVER_V2_EVENTS`, add:

```rust
event::SERVER_REQUEST_RESOLVED,
event::ITEM_COMMAND_EXECUTION_OUTPUT_DELTA,
event::ITEM_COMMAND_EXECUTION_TERMINAL_INTERACTION,
```

Keep `codex.tool_calls`, `codex.approvals`, `tools`, and `sandbox` opt-outs in the default `codex_app_server_v2()` profile until runtime features are advertised by app-server. The implementation will expose P3 through capability matrix and schema, not by pretending the whole Codex app-server product surface is complete.

- [ ] **Step 7: Run protocol tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server_protocol -E 'test(p3_capability_helper_advertises_approval_tools_and_sandbox_contracts) | test(server_initiated_request_serializes_as_json_rpc_request_with_id) | test(client_response_deserializes_approval_decision_payload) | test(server_request_resolved_notification_hides_decision_reason_when_absent)'
```

Expected: PASS.

- [ ] **Step 8: Commit protocol implementation**

```bash
git add crates/dasclaw_app_server_protocol/src/lib.rs
git commit -m "feat: add app-server p3 protocol contracts"
```

## Task 3: Add App-Server JSON-RPC Server-Request Plumbing

**Files:**
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [ ] **Step 1: Add failing tests for incoming client responses**

Add these tests inside `#[cfg(test)] mod tests`:

```rust
#[test]
fn json_rpc_client_response_without_method_is_not_treated_as_invalid_request() {
    let bridge = Arc::new(ManualApprovalBridge::default());
    let mut server = initialized_codex_server_with_bridge(bridge);

    let response = server.handle_json_rpc(
        r#"{"jsonrpc":"2.0","id":"approval_missing","result":{"decision":{"kind":"approve"}}}"#,
    );

    assert!(response.is_none(), "client responses are consumed by app-server");
    let notifications = server.drain_notifications();
    assert!(
        notifications.iter().any(|notification| {
            notification.method == "serverRequest/resolved"
                && notification.params["outcome"] == "failed"
        }),
        "unknown client response should produce a resolved failure notification: {notifications:?}"
    );
}

#[test]
fn protocol_schema_methods_are_all_routable_after_p3() {
    let server = AppServer::with_runtime_bridge(Arc::new(ManualApprovalBridge::ready()));
    let schema = server.protocol_schema();

    for method in schema.methods {
        assert!(
            supported_methods().contains(&method.method.as_str()),
            "protocol/schema advertised an unroutable method: {}",
            method.method
        );
    }
}
```

Add this test helper in the same test module:

```rust
#[derive(Debug, Default)]
struct ManualApprovalBridge {
    features: RuntimeBridgeFeatures,
    decisions: Mutex<Vec<RuntimeApprovalDecision>>,
}

impl ManualApprovalBridge {
    fn ready() -> Self {
        Self {
            features: RuntimeBridgeFeatures {
                approval: true,
                tools: true,
                sandbox: true,
            },
            decisions: Mutex::new(Vec::new()),
        }
    }
}

impl RuntimeBridge for ManualApprovalBridge {
    fn features(&self) -> RuntimeBridgeFeatures {
        self.features
    }

    fn start_turn(&self, request: RuntimeTurnStartRequest) -> Result<(), RuntimeBridgeError> {
        request.updates.approval_requested(RuntimeApprovalRequest {
            thread_id: request.thread_id,
            turn_id: request.turn_id,
            request_id: "00000000-0000-0000-0000-000000000001".to_string(),
            tool_call_id: "call_1".to_string(),
            tool_name: "bash".to_string(),
            command: Some("echo hello".to_string()),
            description: "approve call to bash".to_string(),
            display_parameters: serde_json::json!({"cmd": "echo hello"}),
            allow_always: true,
        });
        Ok(())
    }

    fn cancel_turn(&self, _request: RuntimeTurnCancelRequest) -> Result<(), RuntimeBridgeError> {
        Ok(())
    }

    fn resolve_approval(
        &self,
        decision: RuntimeApprovalDecision,
    ) -> Result<(), RuntimeBridgeError> {
        self.decisions
            .lock()
            .expect("decision mutex should not be poisoned")
            .push(decision);
        Ok(())
    }

    fn shutdown(&self) {}
}
```

- [ ] **Step 2: Run tests and confirm failure**

Run:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(json_rpc_client_response_without_method_is_not_treated_as_invalid_request) | test(protocol_schema_methods_are_all_routable_after_p3)'
```

Expected: FAIL at compile time with missing `RuntimeBridgeFeatures`, `RuntimeApprovalDecision`, `RuntimeApprovalRequest`, `RuntimeTurnUpdateSink::approval_requested`, and missing `RuntimeBridge::resolve_approval`.

- [ ] **Step 3: Add runtime feature reporting**

In `crates/dasclaw_app_server/src/lib.rs`, add:

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeBridgeFeatures {
    pub approval: bool,
    pub tools: bool,
    pub sandbox: bool,
}
```

Extend `RuntimeBridge`:

```rust
pub trait RuntimeBridge: std::fmt::Debug + Send + Sync {
    fn features(&self) -> RuntimeBridgeFeatures {
        RuntimeBridgeFeatures::default()
    }

    fn start_turn(&self, request: RuntimeTurnStartRequest) -> Result<(), RuntimeBridgeError>;
    fn cancel_turn(&self, request: RuntimeTurnCancelRequest) -> Result<(), RuntimeBridgeError>;

    fn resolve_approval(
        &self,
        _decision: RuntimeApprovalDecision,
    ) -> Result<(), RuntimeBridgeError> {
        Err(RuntimeBridgeError::fatal(
            "runtime bridge does not support approval decisions",
        ))
    }

    fn shutdown(&self);
}
```

In `AppServer::with_runtime_bridge`, apply P3 capabilities only when the bridge reports all required features:

```rust
pub fn with_runtime_bridge(runtime_bridge: Arc<dyn RuntimeBridge>) -> Self {
    let mut server = Self::new();
    let features = runtime_bridge.features();
    if features.approval && features.tools && features.sandbox {
        server.capabilities = server.capabilities.with_p3_approval_tool_sandbox();
    }
    server.runtime_bridge = runtime_bridge;
    server
}
```

- [ ] **Step 4: Add incoming JSON-RPC response parsing**

Change `handle_json_rpc` to parse `JsonRpcIncoming`:

```rust
pub fn handle_json_rpc(&mut self, input: &str) -> Option<String> {
    self.drain_runtime_turn_updates();
    let incoming = match serde_json::from_str::<dasclaw_app_server_protocol::JsonRpcIncoming>(input)
    {
        Ok(incoming) => incoming,
        Err(error) => {
            let response = parse_error_response(error.to_string());
            return serde_json::to_string(&response).ok();
        }
    };

    match incoming {
        dasclaw_app_server_protocol::JsonRpcIncoming::Request(request) => self
            .route_json_rpc(request)
            .and_then(|response| serde_json::to_string(&response).ok()),
        dasclaw_app_server_protocol::JsonRpcIncoming::ClientResponse(response) => {
            self.handle_client_response(response);
            None
        }
    }
}
```

Add:

```rust
fn handle_client_response(
    &mut self,
    response: dasclaw_app_server_protocol::JsonRpcClientResponse,
) {
    let request_id = response
        .id
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| response.id.to_string());

    let Some(pending) = self.pending_server_requests.remove(&request_id) else {
        self.emit_server_request_resolved(
            request_id,
            None,
            None,
            ServerRequestResolutionOutcome::Failed,
            Some("unknown or expired server request id".to_string()),
        );
        return;
    };

    if let Some(error) = response.error {
        self.emit_server_request_resolved(
            pending.request_id,
            Some(pending.thread_id),
            Some(pending.turn_id),
            ServerRequestResolutionOutcome::Failed,
            Some(error.message),
        );
        return;
    }

    let Some(result) = response.result else {
        self.emit_server_request_resolved(
            pending.request_id,
            Some(pending.thread_id),
            Some(pending.turn_id),
            ServerRequestResolutionOutcome::Failed,
            Some("approval response missing result".to_string()),
        );
        return;
    };

    match serde_json::from_value::<ApprovalResponsePayload>(result) {
        Ok(payload) => self.apply_approval_decision(pending, payload.decision),
        Err(error) => self.emit_server_request_resolved(
            pending.request_id,
            Some(pending.thread_id),
            Some(pending.turn_id),
            ServerRequestResolutionOutcome::Failed,
            Some(format!("invalid approval response payload: {error}")),
        ),
    }
}
```

This step requires adding `pending_server_requests` to `AppServer`; Task 4 defines that store and `apply_approval_decision`.

- [ ] **Step 5: Run the targeted tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(json_rpc_client_response_without_method_is_not_treated_as_invalid_request) | test(protocol_schema_methods_are_all_routable_after_p3)'
```

Expected: PASS after Task 4 data structures are also in place. If this step is implemented before Task 4, compile failure should mention the missing pending-request store only.

- [ ] **Step 6: Commit JSON-RPC plumbing**

```bash
git add crates/dasclaw_app_server/src/lib.rs
git commit -m "feat: parse app-server client responses"
```

## Task 4: Add Pending Server-Request Tracking and Fail-Safe Resolution

**Files:**
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [ ] **Step 1: Add failing pending-request tests**

Add:

```rust
#[test]
fn approval_response_records_runtime_decision_and_emits_resolved_notification() {
    let bridge = Arc::new(ManualApprovalBridge::ready());
    let mut server = initialized_codex_server_with_bridge(bridge.clone());

    server
        .turn_start(TurnStartParams {
            thread_id: "thread_1".to_string(),
            prompt: "please run echo".to_string(),
            reasoning_summary: None,
        })
        .expect("turn should start");

    let request_values = drain_json_rpc_for(&mut server, Duration::from_millis(100));
    let approval_request = request_values
        .iter()
        .find(|value| value["method"] == "item/commandExecution/requestApproval")
        .expect("approval request should be emitted");
    let request_id = approval_request["id"]
        .as_str()
        .expect("server request id should be a string")
        .to_string();

    let client_response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "result": {"decision": {"kind": "approve"}}
    })
    .to_string();
    assert!(server.handle_json_rpc(&client_response).is_none());

    let decisions = bridge
        .decisions
        .lock()
        .expect("decision mutex should not be poisoned");
    assert_eq!(decisions.len(), 1);
    assert_eq!(decisions[0].request_id, "00000000-0000-0000-0000-000000000001");
    assert_eq!(
        decisions[0].decision,
        dasclaw_runtime::ApprovalDecision::Approve
    );

    let notifications = server.drain_notifications();
    assert!(notifications.iter().any(|notification| {
        notification.method == "serverRequest/resolved"
            && notification.params["outcome"] == "approved"
    }));
}

#[test]
fn expired_approval_request_rejects_fail_safe_and_late_response_is_unknown() {
    let bridge = Arc::new(ManualApprovalBridge::ready());
    let mut server = initialized_codex_server_with_bridge(bridge.clone());

    server
        .turn_start(TurnStartParams {
            thread_id: "thread_1".to_string(),
            prompt: "please run echo".to_string(),
            reasoning_summary: None,
        })
        .expect("turn should start");
    let request_values = drain_json_rpc_for(&mut server, Duration::from_millis(100));
    let approval_request = request_values
        .iter()
        .find(|value| value["method"] == "item/commandExecution/requestApproval")
        .expect("approval request should be emitted");
    let request_id = approval_request["id"].as_str().unwrap().to_string();

    server.expire_pending_server_requests_for_tests(Duration::from_secs(301));

    let decisions = bridge
        .decisions
        .lock()
        .expect("decision mutex should not be poisoned");
    assert_eq!(decisions.len(), 1);
    assert!(matches!(
        decisions[0].decision,
        dasclaw_runtime::ApprovalDecision::Reject { .. }
    ));
    drop(decisions);

    let late_response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "result": {"decision": {"kind": "approve"}}
    })
    .to_string();
    assert!(server.handle_json_rpc(&late_response).is_none());

    let notifications = server.drain_notifications();
    assert!(notifications.iter().any(|notification| {
        notification.method == "serverRequest/resolved"
            && notification.params["outcome"] == "timed_out"
    }));
    assert!(notifications.iter().any(|notification| {
        notification.method == "serverRequest/resolved"
            && notification.params["outcome"] == "failed"
            && notification.params["reason"]
                .as_str()
                .unwrap_or_default()
                .contains("unknown or expired")
    }));
}
```

Add this helper near the existing test drain helpers:

```rust
fn drain_json_rpc_for(server: &mut AppServer, timeout: Duration) -> Vec<Value> {
    let deadline = Instant::now() + timeout;
    let mut values = Vec::new();
    while Instant::now() < deadline {
        values.extend(
            server
                .drain_json_rpc_notifications()
                .into_iter()
                .map(|line| serde_json::from_str::<Value>(&line).expect("JSON-RPC line")),
        );
        if values.iter().any(|value| {
            value.get("method").and_then(Value::as_str)
                == Some("item/commandExecution/requestApproval")
        }) {
            return values;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    values.extend(
        server
            .drain_json_rpc_notifications()
            .into_iter()
            .map(|line| serde_json::from_str::<Value>(&line).expect("JSON-RPC line")),
    );
    values
}
```

- [ ] **Step 2: Add runtime update structs**

Add near `RuntimeTurnOutcome`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeApprovalRequest {
    pub thread_id: String,
    pub turn_id: String,
    pub request_id: String,
    pub tool_call_id: String,
    pub tool_name: String,
    pub command: Option<String>,
    pub description: String,
    pub display_parameters: serde_json::Value,
    pub allow_always: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeApprovalDecision {
    pub request_id: String,
    pub decision: dasclaw_runtime::ApprovalDecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeToolResultUpdate {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub tool_name: String,
    pub content: String,
    pub is_error: bool,
}
```

Extend `RuntimeTurnOutcome`:

```rust
ApprovalRequested(RuntimeApprovalRequest),
ToolResult(RuntimeToolResultUpdate),
CommandOutputDelta {
    item_id: String,
    delta: String,
},
```

Add methods to `RuntimeTurnUpdateSink`:

```rust
pub fn approval_requested(&self, request: RuntimeApprovalRequest) {
    self.push(RuntimeTurnUpdate {
        thread_id: request.thread_id.clone(),
        turn_id: request.turn_id.clone(),
        outcome: RuntimeTurnOutcome::ApprovalRequested(request),
    });
}

pub fn tool_result(&self, result: RuntimeToolResultUpdate) {
    self.push(RuntimeTurnUpdate {
        thread_id: result.thread_id.clone(),
        turn_id: result.turn_id.clone(),
        outcome: RuntimeTurnOutcome::ToolResult(result),
    });
}

pub fn command_output_delta(
    &self,
    thread_id: String,
    turn_id: String,
    item_id: String,
    delta: String,
) {
    self.push(RuntimeTurnUpdate {
        thread_id,
        turn_id,
        outcome: RuntimeTurnOutcome::CommandOutputDelta { item_id, delta },
    });
}
```

- [ ] **Step 3: Add pending request state**

Add fields to `AppServer`:

```rust
pending_server_requests: PendingServerRequestStore,
```

Add the store:

```rust
const SERVER_REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Debug, Clone)]
struct PendingServerRequest {
    request_id: String,
    runtime_request_id: String,
    thread_id: String,
    turn_id: String,
    created_at: Instant,
}

#[derive(Debug, Clone, Default)]
struct PendingServerRequestStore {
    requests: HashMap<String, PendingServerRequest>,
}

impl PendingServerRequestStore {
    fn insert(&mut self, request: PendingServerRequest) {
        self.requests.insert(request.request_id.clone(), request);
    }

    fn remove(&mut self, request_id: &str) -> Option<PendingServerRequest> {
        self.requests.remove(request_id)
    }

    fn expired(&mut self, now: Instant) -> Vec<PendingServerRequest> {
        let expired_ids = self
            .requests
            .iter()
            .filter_map(|(id, request)| {
                if now.duration_since(request.created_at) >= SERVER_REQUEST_TIMEOUT {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        expired_ids
            .into_iter()
            .filter_map(|id| self.requests.remove(&id))
            .collect()
    }
}
```

Initialize it in `AppServer::new()`:

```rust
pending_server_requests: PendingServerRequestStore::default(),
```

- [ ] **Step 4: Emit server requests from runtime updates**

Add this branch in `drain_runtime_turn_updates()`:

```rust
RuntimeTurnOutcome::ApprovalRequested(request) => {
    self.emit_approval_server_request(request);
}
RuntimeTurnOutcome::ToolResult(result) => {
    self.notifications.emit_command_execution_terminal_interaction(
        CommandExecutionTerminalInteractionEvent {
            thread_id: result.thread_id,
            turn_id: result.turn_id,
            item_id: result.item_id,
            message: result.content,
            is_error: result.is_error,
        },
    );
}
RuntimeTurnOutcome::CommandOutputDelta { item_id, delta } => {
    if self
        .threads
        .turn_is_pending(&update.thread_id, &update.turn_id)
    {
        self.notifications.emit_command_execution_output_delta(
            CommandExecutionOutputDeltaEvent {
                thread_id: update.thread_id,
                turn_id: update.turn_id,
                item_id,
                delta,
            },
        );
    }
}
```

Add:

```rust
fn emit_approval_server_request(&mut self, request: RuntimeApprovalRequest) {
    let server_request_id = format!("approval_{}", request.request_id);
    self.pending_server_requests.insert(PendingServerRequest {
        request_id: server_request_id.clone(),
        runtime_request_id: request.request_id.clone(),
        thread_id: request.thread_id.clone(),
        turn_id: request.turn_id.clone(),
        created_at: Instant::now(),
    });

    let previous_state = self.lifecycle.state;
    self.lifecycle = lifecycle_snapshot(
        LifecycleState::AwaitingApproval,
        LifecycleReason::ApprovalPending,
        Some(format!("waiting for approval for tool {}", request.tool_name)),
        Vec::new(),
    );
    self.emit_lifecycle_changed(previous_state);

    self.notifications.emit_server_request(
        JsonRpcServerRequest::new(
            server_request_id,
            server_request::ITEM_COMMAND_EXECUTION_REQUEST_APPROVAL,
            CommandExecutionApprovalRequest {
                thread_id: request.thread_id,
                turn_id: request.turn_id,
                item_id: format!("{}:tool:{}", request.turn_id, request.tool_call_id),
                tool_call_id: request.tool_call_id,
                tool_name: request.tool_name,
                command: request.command,
                description: request.description,
                display_parameters: request.display_parameters,
                allow_always: request.allow_always,
            },
        )
        .expect("approval server request should serialize"),
    );
}
```

This step requires extending `NotificationBus` to hold both notifications and server requests. Use a single enum:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum OutboundJsonRpcMessage {
    Notification(ServerNotification),
    ServerRequest(JsonRpcServerRequest),
}
```

Then update `drain_json_rpc_with_policy()` so both variants serialize to output lines.

- [ ] **Step 5: Apply approval decisions**

Add:

```rust
fn apply_approval_decision(
    &mut self,
    pending: PendingServerRequest,
    decision: AppServerApprovalDecision,
) {
    let (runtime_decision, outcome, reason) = match decision {
        AppServerApprovalDecision::Approve => (
            dasclaw_runtime::ApprovalDecision::Approve,
            ServerRequestResolutionOutcome::Approved,
            None,
        ),
        AppServerApprovalDecision::ApproveAlways => (
            dasclaw_runtime::ApprovalDecision::ApproveAlways,
            ServerRequestResolutionOutcome::Approved,
            None,
        ),
        AppServerApprovalDecision::Reject { reason } => (
            dasclaw_runtime::ApprovalDecision::Reject {
                reason: reason.clone(),
            },
            ServerRequestResolutionOutcome::Rejected,
            reason,
        ),
    };

    let bridge_result = self.runtime_bridge.resolve_approval(RuntimeApprovalDecision {
        request_id: pending.runtime_request_id.clone(),
        decision: runtime_decision,
    });

    let (outcome, reason) = match bridge_result {
        Ok(()) => (outcome, reason),
        Err(error) => (ServerRequestResolutionOutcome::Failed, Some(error.to_string())),
    };

    self.emit_server_request_resolved(
        pending.request_id,
        Some(pending.thread_id),
        Some(pending.turn_id),
        outcome,
        reason,
    );

    if self.lifecycle.state == LifecycleState::AwaitingApproval {
        self.transition_lifecycle(
            LifecycleState::Running,
            LifecycleReason::RequestInProgress,
            Some("approval decision received".to_string()),
        );
    }
}

fn emit_server_request_resolved(
    &mut self,
    request_id: String,
    thread_id: Option<String>,
    turn_id: Option<String>,
    outcome: ServerRequestResolutionOutcome,
    reason: Option<String>,
) {
    self.notifications
        .emit_server_request_resolved(ServerRequestResolvedEvent {
            request_id,
            thread_id,
            turn_id,
            outcome,
            reason,
        });
}
```

- [ ] **Step 6: Add timeout expiry**

Call this at the beginning of `health_check`, `drain_notifications_with_policy`, and `handle_json_rpc`:

```rust
fn expire_pending_server_requests(&mut self) {
    let expired = self.pending_server_requests.expired(Instant::now());
    for pending in expired {
        let _ = self.runtime_bridge.resolve_approval(RuntimeApprovalDecision {
            request_id: pending.runtime_request_id.clone(),
            decision: dasclaw_runtime::ApprovalDecision::Reject {
                reason: Some("approval request timed out".to_string()),
            },
        });
        self.emit_server_request_resolved(
            pending.request_id,
            Some(pending.thread_id),
            Some(pending.turn_id),
            ServerRequestResolutionOutcome::TimedOut,
            Some("approval request timed out".to_string()),
        );
    }
}
```

Add a test-only helper:

```rust
#[cfg(test)]
fn expire_pending_server_requests_for_tests(&mut self, age: Duration) {
    for request in self.pending_server_requests.requests.values_mut() {
        request.created_at = request.created_at.checked_sub(age).unwrap_or(request.created_at);
    }
    self.expire_pending_server_requests();
}
```

- [ ] **Step 7: Run app-server pending-request tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(approval_response_records_runtime_decision_and_emits_resolved_notification) | test(expired_approval_request_rejects_fail_safe_and_late_response_is_unknown) | test(json_rpc_client_response_without_method_is_not_treated_as_invalid_request)'
```

Expected: PASS.

- [ ] **Step 8: Commit pending-request tracking**

```bash
git add crates/dasclaw_app_server/src/lib.rs
git commit -m "feat: track app-server approval requests fail safe"
```

## Task 5: Connect DasclawAgentRuntimeBridge Approval and Tool Events

**Files:**
- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Modify: `crates/dasclaw_app_server/Cargo.toml`

- [ ] **Step 1: Add `uuid` direct dependency**

In `crates/dasclaw_app_server/Cargo.toml`, add:

```toml
uuid = { version = "1", features = ["serde"] }
```

- [ ] **Step 2: Add failing bridge test**

Add:

```rust
#[test]
fn agent_runtime_bridge_maps_approval_needed_and_tool_result_to_runtime_updates() {
    let bridge = DasclawAgentRuntimeBridge::new_with_model_provider(
        |_token, _snapshot, _reasoning_summary| {
            let executor = Arc::new(CountingExecutor::new());
            dasclaw_runtime::Agent::builder()
                .responder(ScriptedResponder::new(vec![
                    tool_call_output("bash", "call_1"),
                    text_output("done"),
                ]))
                .tool_executor(executor as Arc<dyn dasclaw_runtime::ToolExecutor>)
                .tools(vec![dummy_tool("bash")])
                .approval_policy(Arc::new(AlwaysApprovePolicy))
                .build()
                .map_err(|error| RuntimeBridgeError::fatal(error.to_string()))
        },
    );
    let updates = RuntimeTurnUpdateSink::new();
    bridge
        .start_turn(RuntimeTurnStartRequest {
            thread_id: "thread_1".to_string(),
            turn_id: "turn_1".to_string(),
            prompt: "run bash".to_string(),
            model_provider: test_runtime_model_provider_snapshot(),
            reasoning_summary: ReasoningSummary::default(),
            updates: updates.clone(),
        })
        .expect("turn should start");

    let first_updates = wait_for_runtime_updates(&updates);
    assert!(
        first_updates.iter().any(|update| {
            matches!(update.outcome, RuntimeTurnOutcome::ApprovalRequested(_))
        }),
        "approval request should be bridged: {first_updates:?}"
    );

    let approval = first_updates
        .iter()
        .find_map(|update| match &update.outcome {
            RuntimeTurnOutcome::ApprovalRequested(request) => Some(request.request_id.clone()),
            _ => None,
        })
        .expect("approval request id should be present");
    bridge
        .resolve_approval(RuntimeApprovalDecision {
            request_id: approval,
            decision: dasclaw_runtime::ApprovalDecision::Approve,
        })
        .expect("approval should dispatch");

    let later_updates = wait_for_runtime_updates(&updates);
    assert!(
        later_updates.iter().any(|update| {
            matches!(update.outcome, RuntimeTurnOutcome::ToolResult(_))
        }),
        "tool result should be bridged: {later_updates:?}"
    );
    assert!(
        later_updates.iter().any(|update| {
            matches!(update.outcome, RuntimeTurnOutcome::Completed { .. })
        }),
        "completion should still be bridged: {later_updates:?}"
    );
}
```

Reuse the `ScriptedResponder`, `CountingExecutor`, `AlwaysApprovePolicy`, `tool_call_output`, `text_output`, and `dummy_tool` scaffolding from `crates/dasclaw_runtime/tests/approval_flow.rs`. Keep the copies local to `crates/dasclaw_app_server/src/lib.rs` tests so app-server tests remain standalone.

- [ ] **Step 3: Run and confirm failure**

Run:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(agent_runtime_bridge_maps_approval_needed_and_tool_result_to_runtime_updates)'
```

Expected: FAIL because `DasclawAgentRuntimeBridge` currently ignores `ApprovalNeeded`, `ToolCallStart`, and `ToolResult`, and cannot resolve approval decisions.

- [ ] **Step 4: Store active agents and pending approval ids**

Change `DasclawAgentRuntimeBridge` fields:

```rust
#[derive(Clone)]
pub struct DasclawAgentRuntimeBridge {
    agent_factory: Arc<AgentFactory>,
    in_flight: Arc<Mutex<HashMap<String, CancellationToken>>>,
    active_agents: Arc<Mutex<HashMap<String, Arc<dasclaw_runtime::Agent>>>>,
    pending_approvals: Arc<Mutex<HashMap<String, Arc<dasclaw_runtime::Agent>>>>,
    features: RuntimeBridgeFeatures,
}
```

Initialize the new maps:

```rust
Self {
    agent_factory: Arc::new(agent_factory),
    in_flight: Arc::new(Mutex::new(HashMap::new())),
    active_agents: Arc::new(Mutex::new(HashMap::new())),
    pending_approvals: Arc::new(Mutex::new(HashMap::new())),
    features: RuntimeBridgeFeatures {
        approval: true,
        tools: true,
        sandbox: false,
    },
}
```

Add an explicit feature-aware constructor for sandbox-ready hosts:

```rust
#[must_use]
pub fn new_with_model_provider_and_features(
    agent_factory: impl Fn(
        CancellationToken,
        RuntimeModelProviderSnapshot,
        ReasoningSummary,
    ) -> Result<dasclaw_runtime::Agent, RuntimeBridgeError>
    + Send
    + Sync
    + 'static,
    features: RuntimeBridgeFeatures,
) -> Self {
    let mut bridge = Self::new_with_model_provider(agent_factory);
    bridge.features = features;
    bridge
}
```

Use this constructor in tests and future production wiring when the supplied agent factory actually builds a sandboxed tool executor. Do not set `sandbox: true` for `from_model_provider_snapshot()` until the production factory wires sandboxed tools.

In `start_turn`, create and store the agent:

```rust
let agent = Arc::new((self.agent_factory)(
    token.clone(),
    request.model_provider.clone(),
    request.reasoning_summary,
)?);
self.active_agents
    .lock()
    .map_err(|_| RuntimeBridgeError::retryable("runtime agent registry lock poisoned"))?
    .insert(request.turn_id.clone(), Arc::clone(&agent));
```

Remove active agent and pending approvals for the turn in the cleanup path.

- [ ] **Step 5: Map runtime events**

In the event loop inside `start_turn`, add branches:

```rust
Ok(dasclaw_runtime::AgentEvent::ApprovalNeeded {
    request_id,
    tool_name,
    tool_arguments,
    description,
    display_parameters,
    allow_always,
}) => {
    let request_id_string = request_id.to_string();
    if let Ok(mut pending) = bridge.pending_approvals.lock() {
        pending.insert(request_id_string.clone(), Arc::clone(&agent));
    }
    event_updates.approval_requested(RuntimeApprovalRequest {
        thread_id: event_thread_id.clone(),
        turn_id: event_turn_id.clone(),
        request_id: request_id_string,
        tool_call_id: tool_arguments
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("tool_call")
            .to_string(),
        tool_name: tool_name.clone(),
        command: tool_arguments
            .get("cmd")
            .or_else(|| tool_arguments.get("command"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        description,
        display_parameters,
        allow_always,
    });
}
Ok(dasclaw_runtime::AgentEvent::ToolCallStart { name, arguments }) => {
    event_updates.command_output_delta(
        event_thread_id.clone(),
        event_turn_id.clone(),
        format!("{event_turn_id}:tool:{name}"),
        serde_json::to_string(&arguments).unwrap_or_else(|_| "{}".to_string()),
    );
}
Ok(dasclaw_runtime::AgentEvent::ToolResult {
    name,
    content,
    is_error,
}) => {
    event_updates.tool_result(RuntimeToolResultUpdate {
        thread_id: event_thread_id.clone(),
        turn_id: event_turn_id.clone(),
        item_id: format!("{event_turn_id}:tool:{name}"),
        tool_name: name,
        content,
        is_error,
    });
}
```

- [ ] **Step 6: Implement approval decision dispatch**

Add to `impl RuntimeBridge for DasclawAgentRuntimeBridge`:

```rust
fn resolve_approval(
    &self,
    decision: RuntimeApprovalDecision,
) -> Result<(), RuntimeBridgeError> {
    let request_id = uuid::Uuid::parse_str(&decision.request_id)
        .map_err(|error| RuntimeBridgeError::fatal(format!("invalid approval id: {error}")))?;
    let agent = self
        .pending_approvals
        .lock()
        .map_err(|_| RuntimeBridgeError::retryable("approval registry lock poisoned"))?
        .remove(&decision.request_id)
        .ok_or_else(|| RuntimeBridgeError::retryable("approval request is no longer pending"))?;

    agent
        .respond_to_approval(request_id, decision.decision)
        .map_err(|error| RuntimeBridgeError::retryable(error.to_string()))
}
```

Return features for bridges that can support P3:

```rust
fn features(&self) -> RuntimeBridgeFeatures {
    self.features
}
```

Keep `sandbox: false` for the generic bridge. Tests that need full P3 capability should use `ManualApprovalBridge::ready()` or `new_with_model_provider_and_features(..., RuntimeBridgeFeatures { approval: true, tools: true, sandbox: true })`.

- [ ] **Step 7: Run bridge tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(agent_runtime_bridge_maps_approval_needed_and_tool_result_to_runtime_updates)'
```

Expected: PASS.

- [ ] **Step 8: Commit runtime bridge event mapping**

```bash
git add crates/dasclaw_app_server/src/lib.rs crates/dasclaw_app_server/Cargo.toml
git commit -m "feat: bridge runtime approval and tool events to app-server"
```

## Task 6: Add App-Server Stdio E2E Tests for P3 Approval

**Files:**
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [ ] **Step 1: Add failing approve-path stdio E2E test**

Add:

```rust
#[test]
fn p3_stdio_e2e_approval_approve_unblocks_tool_execution() {
    let bridge = Arc::new(ManualApprovalBridge::ready());
    let server = AppServer::with_runtime_bridge(bridge.clone());
    let input = [
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "initialize",
            "method": "initialize",
            "params": {
                "client": {"name": "codex", "version": "2.0.0", "transport": "stdio"},
                "protocolVersion": ProtocolVersion::current(),
                "requestedCapabilities": [
                    CompatibilityProfile::CODEX_APP_SERVER_V2_ID,
                    "approval",
                    "tools",
                    "sandbox"
                ],
                "modelProvider": test_model_provider_config()
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "thread",
            "method": "thread/create",
            "params": {"title": "P3 approval"}
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "turn",
            "method": "turn/start",
            "params": {"threadId": "thread_1", "prompt": "run echo"}
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "approval_00000000-0000-0000-0000-000000000001",
            "result": {"decision": {"kind": "approve"}}
        }),
    ]
    .into_iter()
    .map(|request| serde_json::to_string(&request).expect("request should serialize"))
    .collect::<Vec<_>>()
    .join("\n")
        + "\n";
    let mut stdout = Vec::new();

    run_stdio_server_with_app_server(
        server,
        std::io::BufReader::new(Cursor::new(input)),
        &mut stdout,
    )
    .expect("stdio loop should complete");

    let values = String::from_utf8(stdout)
        .expect("stdio output should be UTF-8")
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("stdio line should be JSON"))
        .collect::<Vec<_>>();
    let emitted_methods = values
        .iter()
        .filter_map(|value| value.get("method").and_then(Value::as_str))
        .collect::<Vec<_>>();

    assert!(emitted_methods.contains(&"item/commandExecution/requestApproval"));
    assert!(emitted_methods.contains(&"serverRequest/resolved"));
    assert!(emitted_methods.contains(&"item/commandExecution/terminalInteraction"));
    assert!(values.iter().any(|value| {
        value["method"] == "serverRequest/resolved"
            && value["params"]["outcome"] == "approved"
    }));
    let decisions = bridge
        .decisions
        .lock()
        .expect("decision mutex should not be poisoned");
    assert_eq!(decisions.len(), 1);
    assert_eq!(
        decisions[0].decision,
        dasclaw_runtime::ApprovalDecision::Approve
    );
}
```

- [ ] **Step 2: Add failing reject-path stdio E2E test**

Add:

```rust
#[test]
fn p3_stdio_e2e_approval_reject_fails_safe_without_tool_output() {
    let bridge = Arc::new(ManualApprovalBridge::ready());
    let server = AppServer::with_runtime_bridge(bridge.clone());
    let input = [
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "initialize",
            "method": "initialize",
            "params": {
                "client": {"name": "codex", "version": "2.0.0", "transport": "stdio"},
                "protocolVersion": ProtocolVersion::current(),
                "requestedCapabilities": [
                    CompatibilityProfile::CODEX_APP_SERVER_V2_ID,
                    "approval",
                    "tools",
                    "sandbox"
                ],
                "modelProvider": test_model_provider_config()
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "thread",
            "method": "thread/create",
            "params": {"title": "P3 rejection"}
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "turn",
            "method": "turn/start",
            "params": {"threadId": "thread_1", "prompt": "run echo"}
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "approval_00000000-0000-0000-0000-000000000001",
            "result": {"decision": {"kind": "reject", "data": {"reason": "not allowed"}}}
        }),
    ]
    .into_iter()
    .map(|request| serde_json::to_string(&request).expect("request should serialize"))
    .collect::<Vec<_>>()
    .join("\n")
        + "\n";
    let mut stdout = Vec::new();

    run_stdio_server_with_app_server(
        server,
        std::io::BufReader::new(Cursor::new(input)),
        &mut stdout,
    )
    .expect("stdio loop should complete");

    let values = String::from_utf8(stdout)
        .expect("stdio output should be UTF-8")
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("stdio line should be JSON"))
        .collect::<Vec<_>>();

    assert!(values.iter().any(|value| {
        value["method"] == "serverRequest/resolved"
            && value["params"]["outcome"] == "rejected"
            && value["params"]["reason"] == "not allowed"
    }));
    assert!(
        !values.iter().any(|value| {
            value["method"] == "item/commandExecution/terminalInteraction"
                && value["params"]["isError"] == false
        }),
        "reject path must not emit successful tool output"
    );
    let decisions = bridge
        .decisions
        .lock()
        .expect("decision mutex should not be poisoned");
    assert!(matches!(
        decisions[0].decision,
        dasclaw_runtime::ApprovalDecision::Reject { .. }
    ));
}
```

- [ ] **Step 3: Make `ManualApprovalBridge` finish the approve path**

Extend the test bridge so `resolve_approval(Approve)` emits a tool result and completion:

```rust
fn resolve_approval(
    &self,
    decision: RuntimeApprovalDecision,
) -> Result<(), RuntimeBridgeError> {
    self.decisions
        .lock()
        .expect("decision mutex should not be poisoned")
        .push(decision.clone());
    if matches!(decision.decision, dasclaw_runtime::ApprovalDecision::Approve | dasclaw_runtime::ApprovalDecision::ApproveAlways) {
        if let Some(active) = self
            .active
            .lock()
            .expect("active mutex should not be poisoned")
            .clone()
        {
            active.updates.tool_result(RuntimeToolResultUpdate {
                thread_id: active.thread_id.clone(),
                turn_id: active.turn_id.clone(),
                item_id: "turn_1:tool:bash".to_string(),
                tool_name: "bash".to_string(),
                content: "hello from sandbox".to_string(),
                is_error: false,
            });
            active
                .updates
                .complete(active.thread_id, active.turn_id, "done".to_string());
        }
    } else if let Some(active) = self
        .active
        .lock()
        .expect("active mutex should not be poisoned")
        .clone()
    {
        active.updates.fail(
            active.thread_id,
            active.turn_id,
            "approval rejected".to_string(),
        );
    }
    Ok(())
}
```

Add the helper:

```rust
#[derive(Debug, Clone)]
struct ManualActiveTurn {
    thread_id: String,
    turn_id: String,
    updates: RuntimeTurnUpdateSink,
}
```

- [ ] **Step 4: Run app-server stdio E2E tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(p3_stdio_e2e_approval_approve_unblocks_tool_execution) | test(p3_stdio_e2e_approval_reject_fails_safe_without_tool_output)'
```

Expected: PASS.

- [ ] **Step 5: Commit app-server E2E tests**

```bash
git add crates/dasclaw_app_server/src/lib.rs
git commit -m "test: cover app-server p3 approval stdio e2e"
```

## Task 7: Add Desktop-App JSON-RPC Server-Request Contract Tests

**Files:**
- Modify: `desktop-app/src/main/appServerRpc.ts`
- Modify: `desktop-app/src/main/appServerRpc.test.ts`
- Modify: `desktop-app/src/shared/appServerApi.ts`
- Modify: `desktop-app/src/main/appServerManager.ts`
- Modify: `desktop-app/src/main/appServerManager.test.ts`

- [ ] **Step 1: Add failing JSON-RPC classification tests**

In `desktop-app/src/main/appServerRpc.test.ts`, add:

```ts
it('classifies app-server JSON-RPC requests separately from notifications', () => {
  const message = classifyJsonRpcMessage({
    jsonrpc: '2.0',
    id: 'approval_1',
    method: 'item/commandExecution/requestApproval',
    params: {
      threadId: 'thread_1',
      turnId: 'turn_1',
      itemId: 'turn_1:tool:bash',
      toolCallId: 'call_1',
      toolName: 'bash',
      description: 'approve call to bash',
      displayParameters: { cmd: 'echo hello' },
      allowAlways: true
    }
  })

  expect(message).toEqual({
    type: 'server-request',
    id: 'approval_1',
    method: 'item/commandExecution/requestApproval',
    params: {
      threadId: 'thread_1',
      turnId: 'turn_1',
      itemId: 'turn_1:tool:bash',
      toolCallId: 'call_1',
      toolName: 'bash',
      description: 'approve call to bash',
      displayParameters: { cmd: 'echo hello' },
      allowAlways: true
    }
  })
})

it('builds app-server client response lines for approval decisions', () => {
  expect(
    buildJsonRpcResponseLine('approval_1', {
      decision: { kind: 'reject', data: { reason: 'not allowed' } }
    })
  ).toBe(
    '{"jsonrpc":"2.0","id":"approval_1","result":{"decision":{"kind":"reject","data":{"reason":"not allowed"}}}}\n'
  )
})
```

- [ ] **Step 2: Implement TypeScript message types and response builder**

In `desktop-app/src/main/appServerRpc.ts`, add:

```ts
export type JsonRpcServerRequest = {
  type: 'server-request'
  id: JsonRpcId
  method: string
  params?: unknown
}

export type JsonRpcMessage = JsonRpcResponse | JsonRpcNotification | JsonRpcServerRequest
```

Update `classifyJsonRpcMessage`:

```ts
if (typeof message.method === 'string') {
  if (typeof message.id === 'string' || typeof message.id === 'number') {
    return {
      type: 'server-request',
      id: message.id,
      method: message.method,
      params: message.params
    }
  }
  return {
    type: 'notification',
    method: message.method,
    params: message.params
  }
}
```

Add:

```ts
export function buildJsonRpcResponseLine(id: JsonRpcId, result: unknown): string {
  return `${JSON.stringify({ jsonrpc: '2.0', id, result })}\n`
}
```

Extend `AppServerRpcClient`:

```ts
onServerRequest(handler: (request: JsonRpcServerRequest) => void): () => void
respond(id: JsonRpcId, result: unknown): void
```

In `ChildProcessAppServerRpcClient`, add a `serverRequestHandlers` set and call it when `message.type === 'server-request'`. Implement `respond` by writing `buildJsonRpcResponseLine(id, result)` to `child.stdin`.

- [ ] **Step 3: Add shared approval types**

In `desktop-app/src/shared/appServerApi.ts`, add:

```ts
export type AppServerApprovalDecision =
  | { kind: 'approve' }
  | { kind: 'approve_always' }
  | { kind: 'reject'; data?: { reason?: string } }

export type AppServerApprovalRequest = {
  requestId: string | number
  hostId: string
  method: 'item/commandExecution/requestApproval' | 'item/permissions/requestApproval'
  params: {
    threadId: string
    turnId: string
    itemId: string
    toolCallId: string
    toolName: string
    command?: string
    description: string
    displayParameters: unknown
    allowAlways: boolean
  }
}

export type AppServerApprovalRespondParams = {
  requestId: string | number
  decision: AppServerApprovalDecision
}
```

- [ ] **Step 4: Add manager test for forwarding and response**

In `desktop-app/src/main/appServerManager.test.ts`, extend `FakeRpcClient` with server request support and add:

```ts
it('forwards app-server approval requests and writes renderer decisions back', async () => {
  const fake = new FakeRpcClient()
  const manager = createManager(fake)
  const approvals: unknown[] = []
  manager.onNotification((notification) => {
    if (notification.method === 'item/commandExecution/requestApproval') approvals.push(notification)
  })

  await manager.start()
  fake.emitServerRequest({
    type: 'server-request',
    id: 'approval_1',
    method: 'item/commandExecution/requestApproval',
    params: {
      threadId: 'thread_1',
      turnId: 'turn_1',
      itemId: 'turn_1:tool:bash',
      toolCallId: 'call_1',
      toolName: 'bash',
      description: 'approve call to bash',
      displayParameters: { cmd: 'echo hello' },
      allowAlways: true
    }
  })

  expect(approvals).toEqual([
    {
      hostId: 'local',
      method: 'item/commandExecution/requestApproval',
      params: {
        requestId: 'approval_1',
        threadId: 'thread_1',
        turnId: 'turn_1',
        itemId: 'turn_1:tool:bash',
        toolCallId: 'call_1',
        toolName: 'bash',
        description: 'approve call to bash',
        displayParameters: { cmd: 'echo hello' },
        allowAlways: true
      }
    }
  ])

  await manager.request('approval/respond', {
    requestId: 'approval_1',
    decision: { kind: 'approve' }
  })

  expect(fake.responses).toEqual([
    {
      id: 'approval_1',
      result: { decision: { kind: 'approve' } }
    }
  ])
})
```

- [ ] **Step 5: Implement manager forwarding**

In `AppServerManager.startConnection()`, after `onNotification`, register:

```ts
this.unsubscribeServerRequests = this.client.onServerRequest((request) =>
  this.handleServerRequest(request)
)
```

Add `unsubscribeServerRequests?: () => void` as a field and clear it in failure/dispose paths.

Add:

```ts
private handleServerRequest(request: JsonRpcServerRequest): void {
  if (
    request.method === 'item/commandExecution/requestApproval' ||
    request.method === 'item/permissions/requestApproval'
  ) {
    this.handleNotification({
      type: 'notification',
      method: request.method,
      params: {
        requestId: request.id,
        ...(request.params && typeof request.params === 'object' ? request.params : {})
      }
    })
    return
  }

  this.requireClient().respond(request.id, {
    decision: {
      kind: 'reject',
      data: { reason: `unsupported app-server request method: ${request.method}` }
    }
  })
}
```

In `request<T>()`, before `ensureReady()` forwarding:

```ts
if (method === 'approval/respond') {
  await this.ensureReady()
  const payload = params as AppServerApprovalRespondParams
  this.requireClient().respond(payload.requestId, { decision: payload.decision })
  return { accepted: true } as T
}
```

- [ ] **Step 6: Run desktop-app tests**

Run:

```bash
cd desktop-app && npm test -- --run src/main/appServerRpc.test.ts src/main/appServerManager.test.ts
```

Expected: PASS.

- [ ] **Step 7: Commit desktop contract**

```bash
git add desktop-app/src/main/appServerRpc.ts desktop-app/src/main/appServerRpc.test.ts desktop-app/src/main/appServerManager.ts desktop-app/src/main/appServerManager.test.ts desktop-app/src/shared/appServerApi.ts
git commit -m "feat: forward app-server approval server requests"
```

## Task 8: Update Gap Matrix and Verification Gates

**Files:**
- Modify: `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`

- [ ] **Step 1: Update P3 status in the gap matrix**

Change the P3 priority row to mark the completed P3 subset:

```markdown
| P3 | Approval + tool + sandbox 三件套 | ~~ServerRequest request tracking、approval decision、tool lifecycle notification、sandbox capability gate、fail-safe timeout~~；standalone `fs/*` / `command/exec*` 仍留给 P5 | Codex 大量协议依赖这组能力，不能分开虚补；本轮只开放已由 app-server E2E 证明的安全闭环 |
```

In sections 5-7, mark only the implemented request/notification names with strikethrough:

```markdown
~~`item/commandExecution/requestApproval`~~
~~`item/permissions/requestApproval`~~
~~`serverRequest/resolved`~~
~~`item/commandExecution/outputDelta`~~
~~`item/commandExecution/terminalInteraction`~~
```

Keep `fs/*`, full `command/exec*`, MCP elicitation, file-change approval, and `item/tool/requestUserInput` unstruck unless they are actually implemented and covered by tests in the same PR.

- [ ] **Step 2: Run Rust verification**

Run:

```bash
cargo check -p dasclaw_app_server_protocol --tests
cargo check -p dasclaw_app_server --tests
cargo nextest run -p dasclaw_app_server_protocol -E 'test(p3_) | test(server_initiated_request) | test(client_response_deserializes_approval_decision_payload) | test(server_request_resolved_notification_hides_decision_reason_when_absent)'
cargo nextest run -p dasclaw_app_server -E 'test(p3_stdio_e2e_) | test(approval_response_records_runtime_decision_and_emits_resolved_notification) | test(expired_approval_request_rejects_fail_safe_and_late_response_is_unknown) | test(agent_runtime_bridge_maps_approval_needed_and_tool_result_to_runtime_updates)'
```

Expected: all commands PASS with 0 compile errors.

- [ ] **Step 3: Run desktop verification**

Run:

```bash
cd desktop-app && npm test -- --run src/main/appServerRpc.test.ts src/main/appServerManager.test.ts
cd desktop-app && npm run typecheck
```

Expected: both commands PASS.

- [ ] **Step 4: Run formatting and panic guard**

Run from repo root:

```bash
cargo fmt --all && python3.12 scripts/check_no_panics.py --base origin/xClaw
```

Expected: `cargo fmt` exits 0 and `check_no_panics.py` reports no new panic violations for the changed files.

- [ ] **Step 5: Run crate clippy**

Run:

```bash
cargo clippy --no-deps -p dasclaw_app_server_protocol --all-targets -- -D warnings
cargo clippy --no-deps -p dasclaw_app_server --all-targets -- -D warnings
```

Expected: both commands PASS. If clippy reports warnings in untouched downstream crates, keep this plan scoped to the two touched app-server crates and let CI own workspace-wide clippy.

- [ ] **Step 6: Run required self-review skills before push**

Run the project-required review lane before opening or pushing the PR:

```bash
# code-quality-audit 自查
# code-simplifier 收敛 newly modified Rust/TS implementation
# code-review-expert PR 自审
```

Expected: no blocker findings. If findings appear, fix them before push.

- [ ] **Step 7: Commit docs and verification**

```bash
git add docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md
git commit -m $'docs: update app-server p3 protocol matrix\n\n已检查 P3 approval/tool/sandbox 是否已有，结论：runtime primitive 已存在；本轮已补 app-server server-request 协议、接线与 stdio E2E。'
```

## Self-Review

Spec coverage:

- P3 approval request tracking is covered by Tasks 1-4.
- P3 approval decision return is covered by Tasks 3-6.
- P3 tool lifecycle notification is covered by Tasks 2, 4, 5, and 6.
- P3 sandbox gating is covered by Tasks 2-4 through capability/service reporting and by keeping `fs/*` / standalone command execution out of scope.
- App-server E2E testing is covered by Task 6 with stdio approve and reject tests.
- Desktop bridge contract is covered by Task 7.

Placeholder scan:

- The plan contains no placeholder markers or deferred implementation instructions.
- Each task has concrete file paths, test names, commands, and expected outcomes.

Type consistency:

- Protocol decision type is `AppServerApprovalDecision`.
- Wire response payload is `ApprovalResponsePayload`.
- Runtime bridge decision type is `RuntimeApprovalDecision`.
- Server request resolution event is `ServerRequestResolvedEvent`.
- The app-server stdio tests use `run_stdio_server_with_app_server`, matching existing app-server tests.
