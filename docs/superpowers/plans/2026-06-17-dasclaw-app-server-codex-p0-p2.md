# Dasclaw App Server Codex P0-P2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first independently testable slice from `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`: honest Codex subset boundaries, Codex-compatible chat-session views, and an app-server-owned `model/list`.

**Architecture:** Keep `dasclaw_app_server_protocol` as the protocol/type owner and `dasclaw_app_server` as the local control-plane owner. Add Codex-compatible fields beside the current native fields instead of replacing native responses, so `desktop-app` can normalize both shapes during the migration. Keep P3-P6 out of this plan because approval/tool/sandbox, MCP/jobs/logs/skills, filesystem/command, and Codex product domains are independent subsystems with different safety boundaries.

**Tech Stack:** Rust 2024, serde JSON-RPC protocol structs, `dasclaw_app_server`, `dasclaw_app_server_protocol`, Electron main process TypeScript, React renderer TypeScript, Vitest, Cargo nextest.

---

## Scope Check

The gap matrix covers several independent subsystems. This plan deliberately implements only P0-P2:

- P0: keep `codex_app_server_v2` honest as a subset and keep opt-outs explicit.
- P1: add minimal Codex-compatible `Thread` / `Turn` views to existing chat-session responses and notifications while keeping native `threadId` / `turnId`.
- P2: move `model/list` into `dasclaw-app-server` as an app-server-owned JSON-RPC method and update `desktop-app` to consume it.

P3-P6 should be separate plans:

- P3: approval + tool execution + sandbox request loop.
- P4: MCP / skills / logs / jobs service owners.
- P5: filesystem and command execution app-server services.
- P6: account, plugin marketplace, app list, device key, external agent import, review, feedback, realtime, and Windows-specific product domains.

## Startup 4 Questions

| Question | Answer | Required action |
|---|---|---|
| 是否新增模块 / crate / 文件？ | Yes. This plan creates an implementation plan document and the implementation will add protocol structs/tests. | Run semantic search before implementation and keep evidence in the first commit message. |
| 结论是否包含否定语？ | Yes. The plan preserves explicit “not full Codex” boundaries. | Verify with semantic search plus exact router/schema tests. |
| 是否跨项目对账？ | Yes. Uses `codex-cli-main`, `crates/dasclaw_app_server*`, and `desktop-app`. | Re-check Codex generated TypeScript schema and Dasclaw router before coding. |
| 是否写架构对账类文档？ | Yes. This plan is derived from a protocol gap matrix. | Keep evidence-vs-inference separation and avoid undocumented capability claims. |

Minimum evidence already gathered before this plan:

- Level 1 semantic search: `semantic_search_nodes_tool` with app-server / Codex / model / approval / sandbox terms returned runtime and MCP/tool primitives, but app-server recall is incomplete.
- Level 2 symbol layer: `execute_lsp` is not currently exposed in this Codex App tool surface. Implementation workers should try it again if available; otherwise use Rust tests plus exact `rg` evidence.
- Level 3 literal layer: `crates/dasclaw_app_server/src/lib.rs` router only handles the current method constants; `service_health()` marks tools/sandbox/jobs/skills/MCP disabled; Codex generated files show `ThreadStartResponse`, `TurnStartResponse`, and `ModelListResponse` shapes.

## File Structure

Modify:

- `crates/dasclaw_app_server_protocol/src/lib.rs`
  - Owns method constants, response/event structs, capability matrix, compatibility profile, and protocol tests.
  - Add `method::MODEL_LIST`.
  - Add `ModelListParams`, `ModelListResponse`, `CodexModel`, `CodexReasoningEffort`, `CodexReasoningEffortOption`, `CodexInputModality`.
  - Add `CodexThread`, `CodexTurn`, `CodexThreadStatus`, `CodexTurnStatus`, `CodexTurnError`.
  - Extend current native response/event structs with optional Codex-compatible fields.

- `crates/dasclaw_app_server/src/lib.rs`
  - Owns state, JSON-RPC routing, notification emission, model provider state, and tests.
  - Add `AppServer::model_list`.
  - Add `ModelProviderState::model_list_response`.
  - Route `method::MODEL_LIST`.
  - Build Codex thread/turn views from `SessionThreadHost`.
  - Include Codex fields in thread/turn responses and notifications.

- `desktop-app/src/main/appServerManager.ts`
  - Owns child process lifecycle and request routing from renderer to app-server.
  - Stop serving `modelProvider/list` directly from the main process after startup; use `model/list` from app-server, then adapt to renderer-safe model provider config.
  - Keep `modelProvider/list` as a renderer-facing alias during migration.

- `desktop-app/src/main/appServerManager.test.ts`
  - Cover `model/list` forwarding and renderer-safe secret redaction.

- `desktop-app/src/renderer/src/hooks/useDasclawAssistantRuntime.ts`
  - Normalize `thread/start` and `turn/start` responses that may be native or Codex-compatible.

- `desktop-app/src/renderer/src/lib/appServerTurnTracker.ts`
  - Normalize `turn/completed` and `turn/failed` notifications that may carry native fields or Codex `turn`.

- `desktop-app/src/renderer/src/lib/assistantMessages.test.ts`
  - Cover Codex response normalization in the renderer hook.

No new crate, dependency, or broad file split is required for P0-P2.

## Task 1: Lock P0-P2 Protocol Contracts With Failing Tests

**Files:**
- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`

- [ ] **Step 1: Add protocol tests that describe the target P0-P2 surface**

In `#[cfg(test)] mod tests`, replace the current `codex_v2_profile_declares_compatible_surface_and_legacy_alias_decisions` test with this stricter version:

```rust
#[test]
fn codex_v2_profile_declares_p0_p2_surface_and_security_opt_outs() {
    let profile = CompatibilityProfile::codex_app_server_v2();

    assert_eq!(profile.id, CompatibilityProfile::CODEX_APP_SERVER_V2_ID);
    assert_eq!(profile.scope, CompatibilityProfileScope::ChatSessionSubset);
    assert!(profile.description.contains("chat session streaming"));
    assert_eq!(
        profile.methods,
        vec![
            method::INITIALIZE,
            method::THREAD_START,
            method::THREAD_READ,
            method::THREAD_LIST,
            method::TURN_START,
            method::TURN_INTERRUPT,
            method::MODEL_LIST,
        ]
    );
    assert!(profile.capability_opt_outs.iter().any(|opt_out| {
        opt_out.capability == "codex.tool_calls"
            && opt_out.reason == "phase_1_chat_session_subset"
    }));
    assert!(profile.capability_opt_outs.iter().any(|opt_out| {
        opt_out.capability == "codex.approvals"
            && opt_out.reason == "phase_1_chat_session_subset"
    }));
    assert!(profile.capability_opt_outs.iter().any(|opt_out| {
        opt_out.capability == "mcp" && opt_out.reason == "phase_1_chat_session_subset"
    }));
    assert!(!profile.methods.iter().any(|method| {
        method.contains("approval")
            || method.contains("command/")
            || method.contains("fs/")
            || method.contains("mcpServer/")
            || method.contains("plugin/")
            || method.contains("marketplace/")
    }));
}

#[test]
fn model_provider_capability_advertises_codex_model_list_and_native_selection() {
    let matrix = CapabilityMatrix::phase_one();

    assert_eq!(matrix.model_provider.status, CapabilityStatus::Implemented);
    assert_eq!(
        matrix.model_provider.methods,
        vec![
            method::MODEL_LIST.to_string(),
            method::MODEL_PROVIDER_SELECT_FOR_NEXT_TURN.to_string(),
        ]
    );
}

#[test]
fn codex_model_list_response_serializes_without_provider_secret_fields() {
    let response = ModelListResponse {
        data: vec![CodexModel::from_client_model(
            &ClientModelConfig {
                model_id: "gpt-test".to_string(),
                display_name: Some("GPT Test".to_string()),
                provider: Some("openai".to_string()),
                api_base_url: Some("https://api.test/v1".to_string()),
                api_key: Some("secret-key".to_string()),
                api_format: Some("openai".to_string()),
                model_call_mode: Some("stream".to_string()),
                source: Some("test".to_string()),
                capabilities: vec!["chat".to_string()],
            },
            true,
        )],
        next_cursor: None,
    };

    let value = serde_json::to_value(response).expect("model/list response should serialize");

    assert_eq!(value["data"][0]["id"], "gpt-test");
    assert_eq!(value["data"][0]["model"], "gpt-test");
    assert_eq!(value["data"][0]["displayName"], "GPT Test");
    assert_eq!(value["data"][0]["defaultReasoningEffort"], "none");
    assert_eq!(value["data"][0]["inputModalities"], serde_json::json!(["text"]));
    assert_eq!(value["data"][0]["isDefault"], true);
    assert!(!value.to_string().contains("secret-key"));
    assert!(value["data"][0].get("apiKey").is_none());
    assert!(value["data"][0].get("apiBaseUrl").is_none());
}
```

- [ ] **Step 2: Run the protocol tests and confirm they fail for missing symbols**

Run:

```bash
cargo nextest run -p dasclaw_app_server_protocol -E 'test(codex_v2_profile_declares_p0_p2_surface_and_security_opt_outs) | test(model_provider_capability_advertises_codex_model_list_and_native_selection) | test(codex_model_list_response_serializes_without_provider_secret_fields)'
```

Expected: FAIL at compile time with missing items such as `method::MODEL_LIST`, `ModelListResponse`, and `CodexModel`.

- [ ] **Step 3: Commit the failing contract tests**

```bash
git add crates/dasclaw_app_server_protocol/src/lib.rs
git commit -m $'test: lock app-server codex p0-p2 protocol contracts\n\n已检查 dasclaw-app-server P0-P2 是否已有，结论：已有 chat subset 与 modelProvider/selectForNextTurn；缺 model/list 与 Codex thread/turn view 接线。'
```

## Task 2: Add P2 Model List Protocol Types

**Files:**
- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`

- [ ] **Step 1: Add the `model/list` method constant**

Add inside `pub mod method` after `TURN_READ`:

```rust
pub const MODEL_LIST: &str = "model/list";
```

- [ ] **Step 2: Add Codex model list request and response structs**

Add after `ModelProviderSelectForNextTurnResponse`:

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_hidden: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelListResponse {
    pub data: Vec<CodexModel>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexModel {
    pub id: String,
    pub model: String,
    pub upgrade: Option<String>,
    pub upgrade_info: Option<ModelUpgradeInfo>,
    pub availability_nux: Option<ModelAvailabilityNux>,
    pub display_name: String,
    pub description: String,
    pub hidden: bool,
    pub supported_reasoning_efforts: Vec<CodexReasoningEffortOption>,
    pub default_reasoning_effort: CodexReasoningEffort,
    pub input_modalities: Vec<CodexInputModality>,
    pub supports_personality: bool,
    pub additional_speed_tiers: Vec<String>,
    pub is_default: bool,
}

impl CodexModel {
    #[must_use]
    pub fn from_client_model(model: &ClientModelConfig, is_default: bool) -> Self {
        let display_name = model
            .display_name
            .clone()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| model.model_id.clone());
        Self {
            id: model.model_id.clone(),
            model: model.model_id.clone(),
            upgrade: None,
            upgrade_info: None,
            availability_nux: None,
            display_name,
            description: String::new(),
            hidden: false,
            supported_reasoning_efforts: vec![CodexReasoningEffortOption {
                reasoning_effort: CodexReasoningEffort::None,
                description: "No reasoning effort override".to_string(),
            }],
            default_reasoning_effort: CodexReasoningEffort::None,
            input_modalities: vec![CodexInputModality::Text],
            supports_personality: false,
            additional_speed_tiers: Vec::new(),
            is_default,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodexInputModality {
    Text,
    Image,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodexReasoningEffort {
    None,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexReasoningEffortOption {
    pub reasoning_effort: CodexReasoningEffort,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelAvailabilityNux {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUpgradeInfo {
    pub model: String,
    pub upgrade_copy: Option<String>,
    pub model_link: Option<String>,
    pub migration_markdown: Option<String>,
}
```

- [ ] **Step 3: Advertise `model/list` in capabilities, schema, and profile**

Change `CapabilityMatrix::phase_one()` model provider block to:

```rust
model_provider: Capability::implemented(
    "model_provider",
    &[
        method::MODEL_LIST,
        method::MODEL_PROVIDER_SELECT_FOR_NEXT_TURN,
    ],
    &[],
),
```

Add `method::MODEL_LIST.to_string()` to `CompatibilityProfile::codex_app_server_v2().methods` after `method::TURN_INTERRUPT`.

Add this schema entry to `phase_one_methods()` before `MODEL_PROVIDER_SELECT_FOR_NEXT_TURN`:

```rust
MethodSchema::new(
    method::MODEL_LIST,
    "model_provider",
    Some("ModelListParams"),
    "ModelListResponse",
    true,
),
```

- [ ] **Step 4: Run protocol tests and confirm they pass**

Run:

```bash
cargo nextest run -p dasclaw_app_server_protocol -E 'test(codex_v2_profile_declares_p0_p2_surface_and_security_opt_outs) | test(model_provider_capability_advertises_codex_model_list_and_native_selection) | test(codex_model_list_response_serializes_without_provider_secret_fields)'
```

Expected: PASS for all three tests.

- [ ] **Step 5: Commit protocol model list support**

```bash
git add crates/dasclaw_app_server_protocol/src/lib.rs
git commit -m $'feat: add app-server codex model list protocol\n\n已检查 dasclaw-app-server P0-P2 是否已有，结论：已有 model provider selection；缺 Codex-compatible model/list response。'
```

## Task 3: Route `model/list` Through AppServer

**Files:**
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [ ] **Step 1: Import the new protocol types**

In the `use dasclaw_app_server_protocol::{ ... }` list, add:

```rust
ModelListParams, ModelListResponse,
```

- [ ] **Step 2: Add failing app-server tests for model list routing and redaction**

Add to `#[cfg(test)] mod tests` near the existing model provider tests:

```rust
#[test]
fn json_rpc_model_list_returns_codex_shape_without_api_keys() {
    let mut server = initialized_server();
    let response = server
        .handle_json_rpc(r#"{"jsonrpc":"2.0","id":"models","method":"model/list","params":{}}"#)
        .expect("model/list should return a structured response");
    let value: Value = serde_json::from_str(&response).expect("model/list response JSON");

    assert_eq!(value["id"], "models");
    assert_eq!(value["result"]["nextCursor"], serde_json::Value::Null);
    assert_eq!(value["result"]["data"][0]["id"], "gpt-test");
    assert_eq!(value["result"]["data"][0]["model"], "gpt-test");
    assert_eq!(value["result"]["data"][0]["isDefault"], true);
    assert_eq!(value["result"]["data"][1]["id"], "gpt-next");
    assert_eq!(value["result"]["data"][1]["isDefault"], false);
    assert!(!response.contains("test-api-key"));
    assert!(!response.contains("apiKey"));
    assert!(!response.contains("apiBaseUrl"));
}

#[test]
fn json_rpc_model_list_requires_initialize() {
    let mut server = AppServer::new();
    let response = server
        .handle_json_rpc(r#"{"jsonrpc":"2.0","id":"models","method":"model/list","params":{}}"#)
        .expect("model/list should return a structured error");
    let value: Value = serde_json::from_str(&response).expect("model/list error JSON");

    assert_eq!(value["id"], "models");
    assert_eq!(value["error"]["code"], -32003);
    assert_eq!(value["error"]["data"]["code"], "NOT_INITIALIZED");
    assert_eq!(value["error"]["data"]["capability"], "model_provider");
}
```

- [ ] **Step 3: Run tests to confirm routing is missing**

Run:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(json_rpc_model_list_returns_codex_shape_without_api_keys) | test(json_rpc_model_list_requires_initialize)'
```

Expected: FAIL with `Method not found` or missing `model_list` route.

- [ ] **Step 4: Add model list state and AppServer method**

Inside `impl ModelProviderState`, add:

```rust
fn model_list_response(&self) -> Result<ModelListResponse, AppServerError> {
    let selected_model_id = self.selected_model_id.as_deref().ok_or_else(|| {
        AppServerError::invalid_request(
            "model_provider",
            "model provider config is required before listing models",
        )
    })?;
    let mut models = self
        .models
        .values()
        .map(|model| {
            dasclaw_app_server_protocol::CodexModel::from_client_model(
                model,
                model.model_id == selected_model_id,
            )
        })
        .collect::<Vec<_>>();
    models.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(ModelListResponse {
        data: models,
        next_cursor: None,
    })
}
```

Inside `impl AppServer`, add after `model_provider_select_for_next_turn`:

```rust
pub fn model_list(
    &self,
    _params: ModelListParams,
) -> Result<ModelListResponse, AppServerError> {
    self.require_initialized("model_provider")?;
    self.model_provider.model_list_response()
}
```

- [ ] **Step 5: Route the JSON-RPC method**

In `route_json_rpc`, add before `MODEL_PROVIDER_SELECT_FOR_NEXT_TURN`:

```rust
method::MODEL_LIST => route_with_optional_params(request.id, request.params, |params| {
    self.model_list(params.unwrap_or_default())
}),
```

- [ ] **Step 6: Run app-server model list tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(json_rpc_model_list_returns_codex_shape_without_api_keys) | test(json_rpc_model_list_requires_initialize) | test(implemented_capability_methods_are_routable)'
```

Expected: PASS.

- [ ] **Step 7: Commit app-server model list route**

```bash
git add crates/dasclaw_app_server/src/lib.rs
git commit -m $'feat: route codex model list through app-server\n\n已检查 dasclaw-app-server P0-P2 是否已有，结论：已有 model provider state；缺 JSON-RPC model/list owner。'
```

## Task 4: Add Codex Thread and Turn Views Without Breaking Native Fields

**Files:**
- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [ ] **Step 1: Add failing protocol tests for dual native/Codex response shapes**

Add to `crates/dasclaw_app_server_protocol/src/lib.rs` tests:

```rust
#[test]
fn thread_and_turn_responses_can_carry_native_ids_and_codex_views() {
    let lifecycle = LifecycleSnapshot {
        state: LifecycleState::Ready,
        reason: LifecycleReason::RuntimeReady,
        message: None,
        since: "0".to_string(),
        degraded_services: Vec::new(),
    };
    let response = TurnStartResponse {
        turn_id: "turn_1".to_string(),
        status: TurnStatus::Pending,
        lifecycle,
        turn: Some(CodexTurn::in_progress("turn_1")),
    };

    let value = serde_json::to_value(response).expect("turn/start response should serialize");

    assert_eq!(value["turnId"], "turn_1");
    assert_eq!(value["status"], "pending");
    assert_eq!(value["turn"]["id"], "turn_1");
    assert_eq!(value["turn"]["status"], "inProgress");
}
```

- [ ] **Step 2: Run the test and confirm missing Codex view types**

Run:

```bash
cargo nextest run -p dasclaw_app_server_protocol -E 'test(thread_and_turn_responses_can_carry_native_ids_and_codex_views)'
```

Expected: FAIL at compile time with missing `CodexTurn` or missing `turn` field.

- [ ] **Step 3: Add minimal Codex thread/turn protocol structs**

Add after `TurnReadResponse`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexThread {
    pub id: String,
    pub forked_from_id: Option<String>,
    pub preview: String,
    pub ephemeral: bool,
    pub model_provider: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub status: CodexThreadStatus,
    pub path: Option<String>,
    pub cwd: String,
    pub cli_version: String,
    pub source: CodexSessionSource,
    pub agent_nickname: Option<String>,
    pub agent_role: Option<String>,
    pub git_info: Option<CodexGitInfo>,
    pub name: Option<String>,
    pub turns: Vec<CodexTurn>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexTurn {
    pub id: String,
    pub items: Vec<serde_json::Value>,
    pub status: CodexTurnStatus,
    pub error: Option<CodexTurnError>,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub duration_ms: Option<i64>,
}

impl CodexTurn {
    #[must_use]
    pub fn in_progress(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            status: CodexTurnStatus::InProgress,
            error: None,
            started_at: None,
            completed_at: None,
            duration_ms: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexTurnError {
    pub message: String,
    pub codex_error_info: Option<serde_json::Value>,
    pub additional_details: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CodexThreadStatus {
    Idle,
    Active { active_flags: Vec<String> },
    SystemError,
    NotLoaded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodexTurnStatus {
    Completed,
    Interrupted,
    Failed,
    InProgress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodexSessionSource {
    AppServer,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexGitInfo {
    pub sha: Option<String>,
    pub branch: Option<String>,
    pub origin_url: Option<String>,
}
```

Extend existing structs:

```rust
pub struct ThreadStartResponse {
    pub thread_id: String,
    pub lifecycle: LifecycleSnapshot,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread: Option<CodexThread>,
}

pub struct TurnStartResponse {
    pub turn_id: String,
    pub status: TurnStatus,
    pub lifecycle: LifecycleSnapshot,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn: Option<CodexTurn>,
}

pub struct ThreadStartedEvent {
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread: Option<CodexThread>,
}

pub struct TurnStartedEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub status: TurnStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn: Option<CodexTurn>,
}

pub struct TurnCompletedEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub status: TurnStatus,
    pub output: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn: Option<CodexTurn>,
}

pub struct TurnFailedEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub status: TurnStatus,
    pub error: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn: Option<CodexTurn>,
}
```

- [ ] **Step 4: Update existing constructors and tests to pass `None` for new fields**

Every existing direct construction of `ThreadStartResponse`, `TurnStartResponse`, `ThreadStartedEvent`, `TurnStartedEvent`, `TurnCompletedEvent`, and `TurnFailedEvent` must include the new optional field as `None` until Task 5 wires real views.

Example:

```rust
Ok(TurnStartResponse {
    turn_id,
    status: TurnStatus::Pending,
    lifecycle: self.lifecycle.clone(),
    turn: None,
})
```

- [ ] **Step 5: Run protocol and app-server compile checks**

Run:

```bash
cargo check -p dasclaw_app_server_protocol --tests
cargo check -p dasclaw_app_server --tests
```

Expected: both commands finish with `0 errors`.

- [ ] **Step 6: Commit dual-shape protocol fields**

```bash
git add crates/dasclaw_app_server_protocol/src/lib.rs crates/dasclaw_app_server/src/lib.rs
git commit -m $'feat: add codex thread and turn view fields\n\n已检查 dasclaw-app-server P0-P2 是否已有，结论：已有 native thread/turn ids；缺 Codex-compatible Thread/Turn view fields。'
```

## Task 5: Populate Codex Thread and Turn Views in AppServer

**Files:**
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [ ] **Step 1: Add failing tests for populated Codex fields**

Add to `crates/dasclaw_app_server/src/lib.rs` tests:

```rust
#[test]
fn codex_v2_thread_start_response_includes_thread_view_and_native_id() {
    let mut server = initialized_codex_server();
    let response = server
        .handle_json_rpc(
            r#"{"jsonrpc":"2.0","id":"thread","method":"thread/start","params":{"title":"Draft"}}"#,
        )
        .expect("thread/start should return a response");
    let value: Value = serde_json::from_str(&response).expect("thread/start response JSON");

    assert_eq!(value["result"]["threadId"], "thread_1");
    assert_eq!(value["result"]["thread"]["id"], "thread_1");
    assert_eq!(value["result"]["thread"]["preview"], "Draft");
    assert_eq!(value["result"]["thread"]["source"], "appServer");
    assert_eq!(value["result"]["thread"]["status"]["type"], "idle");
}

#[test]
fn codex_v2_turn_start_and_completed_notifications_include_turn_view() {
    let bridge = Arc::new(SequencedRuntimeBridge::new([RuntimeTurnOutcome::Completed {
        output: "hello".to_string(),
    }]));
    let mut server = initialized_codex_server_with_bridge(bridge);
    let thread = server
        .thread_start(ThreadCreateParams {
            title: Some("Draft".to_string()),
            workspace_root: None,
        })
        .expect("thread/start should succeed");
    let _ = server.drain_notifications();

    let started = server
        .turn_start(TurnStartParams {
            thread_id: thread.thread_id,
            prompt: "hello".to_string(),
            reasoning_summary: None,
        })
        .expect("turn/start should succeed");
    let notifications = server.drain_notifications();

    assert_eq!(started.turn_id, "turn_1");
    assert_eq!(started.turn.as_ref().expect("turn view").id, "turn_1");
    assert_eq!(
        started.turn.as_ref().expect("turn view").status,
        dasclaw_app_server_protocol::CodexTurnStatus::InProgress
    );
    let completed = notifications
        .iter()
        .find(|notification| notification.method == "turn/completed")
        .expect("turn/completed should be emitted");
    assert_eq!(completed.params["turn"]["id"], "turn_1");
    assert_eq!(completed.params["turn"]["status"], "completed");
}
```

Add helper functions in tests near `initialized_server()`:

```rust
fn initialized_codex_server() -> AppServer {
    initialized_codex_server_with_bridge(Arc::new(NoopRuntimeBridge))
}

fn initialized_codex_server_with_bridge(bridge: Arc<dyn RuntimeBridge>) -> AppServer {
    let mut server = AppServer::with_runtime_bridge(bridge);
    server
        .initialize(InitializeParams {
            client: ClientInfo {
                name: "codex".to_string(),
                version: "2.0.0".to_string(),
                transport: TransportKind::Stdio,
            },
            protocol_version: ProtocolVersion::current(),
            workspace: Some(WorkspaceInfo {
                root: Some("/tmp/workspace".to_string()),
                trust: WorkspaceTrust::Unknown,
            }),
            requested_capabilities: vec![
                CompatibilityProfile::CODEX_APP_SERVER_V2_ID.to_string(),
            ],
            model_provider: Some(test_model_provider_config()),
        })
        .expect("codex server should initialize");
    let _ = server.drain_notifications();
    server
}
```

- [ ] **Step 2: Run tests and confirm Codex fields are missing**

Run:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(codex_v2_thread_start_response_includes_thread_view_and_native_id) | test(codex_v2_turn_start_and_completed_notifications_include_turn_view)'
```

Expected: FAIL because `thread` or `turn` fields are `null` or absent.

- [ ] **Step 3: Add Codex view builders**

Import these types at the top:

```rust
CodexSessionSource, CodexThread, CodexThreadStatus, CodexTurn, CodexTurnError,
CodexTurnStatus,
```

Add helper methods inside `impl AppServer`:

```rust
fn codex_thread_view(&self, thread_id: &str, include_turns: bool) -> Result<CodexThread, AppServerError> {
    let summary = self.thread_summary_or_error(thread_id)?;
    let turns = if include_turns {
        self.threads
            .list_turns(thread_id)
            .into_iter()
            .map(codex_turn_from_summary)
            .collect()
    } else {
        Vec::new()
    };
    let has_pending_turn = self
        .threads
        .list_turns(thread_id)
        .iter()
        .any(|turn| turn.status == TurnStatus::Pending);

    Ok(CodexThread {
        id: summary.thread_id,
        forked_from_id: None,
        preview: summary.title.clone().unwrap_or_default(),
        ephemeral: true,
        model_provider: self
            .model_provider
            .selected_model_id
            .clone()
            .unwrap_or_default(),
        created_at: 0,
        updated_at: 0,
        status: if has_pending_turn {
            CodexThreadStatus::Active {
                active_flags: Vec::new(),
            }
        } else {
            CodexThreadStatus::Idle
        },
        path: None,
        cwd: summary.workspace_root.unwrap_or_else(|| ".".to_string()),
        cli_version: SERVER_VERSION.to_string(),
        source: CodexSessionSource::AppServer,
        agent_nickname: None,
        agent_role: None,
        git_info: None,
        name: summary.title,
        turns,
    })
}

fn codex_turn_view(&self, thread_id: &str, turn_id: &str) -> Result<CodexTurn, AppServerError> {
    let summary = self.turn_summary_or_error(thread_id, turn_id)?;
    Ok(codex_turn_from_summary(summary))
}
```

Add a free function near `ThreadRecord` / `TurnRecord`:

```rust
fn codex_turn_from_summary(summary: TurnSummary) -> CodexTurn {
    CodexTurn {
        id: summary.turn_id,
        items: Vec::new(),
        status: match summary.status {
            TurnStatus::Pending => CodexTurnStatus::InProgress,
            TurnStatus::Completed => CodexTurnStatus::Completed,
            TurnStatus::Failed => CodexTurnStatus::Failed,
            TurnStatus::Cancelled => CodexTurnStatus::Interrupted,
        },
        error: summary.error.map(|message| CodexTurnError {
            message,
            codex_error_info: None,
            additional_details: None,
        }),
        started_at: None,
        completed_at: None,
        duration_ms: None,
    }
}
```

- [ ] **Step 4: Populate fields in responses and notifications**

In `thread_create`, change event and response construction to:

```rust
let thread = self.codex_thread_view(&thread_id, false)?;
self.notifications.emit_thread_created(ThreadCreatedEvent {
    thread_id: thread_id.clone(),
});
self.emit_codex_thread_started(thread_id.clone());

Ok(ThreadCreateResponse {
    thread_id,
    lifecycle: self.lifecycle.clone(),
})
```

Keep `ThreadCreateResponse` native-only. In `thread_start`, change response construction to:

```rust
let created = self.thread_create(params)?;
let thread = self.codex_thread_view(&created.thread_id, false)?;
Ok(ThreadStartResponse {
    thread_id: created.thread_id,
    lifecycle: created.lifecycle,
    thread: Some(thread),
})
```

In `turn_start`, change response construction to:

```rust
Ok(TurnStartResponse {
    turn_id: turn_id.clone(),
    status: TurnStatus::Pending,
    lifecycle: self.lifecycle.clone(),
    turn: Some(self.codex_turn_view(&thread_id, &turn_id)?),
})
```

When emitting `TurnStartedEvent`, include:

```rust
turn: Some(CodexTurn::in_progress(turn_id.clone())),
```

In runtime completion/failure paths, pass `Some(codex_turn_from_summary(summary.clone()))` into `TurnCompletedEvent` and `TurnFailedEvent`.

- [ ] **Step 5: Run focused app-server tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(codex_v2_thread_start_response_includes_thread_view_and_native_id) | test(codex_v2_turn_start_and_completed_notifications_include_turn_view) | test(codex_v2_requested_profile_emits_item_notifications_without_replacing_legacy_events)'
```

Expected: PASS.

- [ ] **Step 6: Commit populated Codex views**

```bash
git add crates/dasclaw_app_server/src/lib.rs
git commit -m $'feat: populate codex thread and turn views\n\n已检查 dasclaw-app-server P0-P2 是否已有，结论：已有 native in-memory thread/turn summaries；缺 Codex-compatible view mapping。'
```

## Task 6: Normalize P0-P2 Shapes in Desktop App

**Files:**
- Modify: `desktop-app/src/main/appServerManager.ts`
- Modify: `desktop-app/src/main/appServerManager.test.ts`
- Modify: `desktop-app/src/renderer/src/hooks/useDasclawAssistantRuntime.ts`
- Modify: `desktop-app/src/renderer/src/lib/appServerTurnTracker.ts`
- Modify: `desktop-app/src/renderer/src/lib/assistantMessages.test.ts`

- [ ] **Step 1: Add failing main-process tests for app-server-owned model list**

Change `FakeRpcClient.request` in `desktop-app/src/main/appServerManager.test.ts` so `model/list` returns Codex model data:

```ts
if (method === 'model/list') {
  return {
    data: [
      {
        id: 'gpt-test',
        model: 'gpt-test',
        displayName: 'GPT Test',
        description: '',
        hidden: false,
        supportedReasoningEfforts: [
          { reasoningEffort: 'none', description: 'No reasoning effort override' }
        ],
        defaultReasoningEffort: 'none',
        inputModalities: ['text'],
        supportsPersonality: false,
        additionalSpeedTiers: [],
        isDefault: true,
        upgrade: null,
        upgradeInfo: null,
        availabilityNux: null
      }
    ],
    nextCursor: null
  } as T
}
```

Update the `returns a renderer-safe model provider list without exposing API keys` expectation so it expects app-server startup plus `model/list`:

```ts
expect(fake.requests.map((request) => request.method)).toEqual([
  'initialize',
  'health/check',
  'model/list'
])
```

- [ ] **Step 2: Run the main-process test and confirm current local shortcut fails the expectation**

Run:

```bash
npm --prefix desktop-app test -- appServerManager.test.ts
```

Expected: FAIL because `modelProvider/list` is still served from local cached config without forwarding `model/list`.

- [ ] **Step 3: Add Codex model response normalization**

In `desktop-app/src/main/appServerManager.ts`, add types near the existing model config types:

```ts
type CodexModelListResponse = {
  data: CodexModel[]
  nextCursor: string | null
}

type CodexModel = {
  id: string
  model: string
  displayName: string
  description: string
  hidden: boolean
  inputModalities: string[]
  isDefault: boolean
}
```

Change `request()` handling for model list:

```ts
if (method === 'modelProvider/list') {
  await this.ensureReady()
  const response = await this.requireClient().request<CodexModelListResponse>('model/list', {})
  return toRendererModelProviderConfigFromCodex(response) as T
}
```

Add this function near `toRendererModelProviderConfig`:

```ts
function toRendererModelProviderConfigFromCodex(
  response: CodexModelListResponse
): RendererModelProviderConfig {
  const visibleModels = response.data.filter((model) => !model.hidden)
  return {
    models: visibleModels.map((model) => ({
      modelId: model.id,
      displayName: model.displayName || model.model || model.id,
      ...(model.description ? { description: model.description } : {}),
      provider: 'app-server',
      apiBaseUrl: '',
      apiFormat: 'app-server',
      modelCallMode: 'stream',
      source: 'app-server',
      capabilities: model.inputModalities,
      apiKeyConfigured: false
    })),
    selectedModelId:
      visibleModels.find((model) => model.isDefault)?.id ?? visibleModels[0]?.id
  }
}
```

- [ ] **Step 4: Add renderer response normalization helpers**

In `desktop-app/src/renderer/src/hooks/useDasclawAssistantRuntime.ts`, replace direct response field reads with helpers:

```ts
const started = await window.desktopAppServer.request<unknown>('turn/start', {
  threadId,
  input: [{ type: 'text', text: prompt }]
})
const turnId = readTurnId(started)
pendingTurnMessageIdsRef.current.set(turnId, pendingId)
for (const part of turnTrackerRef.current.getTurnContent(turnId)) {
  appendPendingAssistantMessageContentDelta(setMessages, pendingId, part)
}
try {
  return await turnTrackerRef.current.waitForTurnCompletion(turnId)
} finally {
  pendingTurnMessageIdsRef.current.delete(turnId)
}
```

Change `ensureThread` response handling:

```ts
const response = await window.desktopAppServer.request<unknown>('thread/start', {
  title
})
const threadId = readThreadId(response)
threadIdRef.current = threadId
return threadId
```

Add helpers at the bottom of the file:

```ts
function readThreadId(response: unknown): string {
  if (!response || typeof response !== 'object') {
    throw new Error('thread/start returned an invalid response')
  }
  const record = response as Record<string, unknown>
  if (typeof record.threadId === 'string' && record.threadId.trim()) return record.threadId
  const thread = record.thread
  if (thread && typeof thread === 'object') {
    const threadId = (thread as { id?: unknown }).id
    if (typeof threadId === 'string' && threadId.trim()) return threadId
  }
  throw new Error('thread/start response did not include a thread id')
}

function readTurnId(response: unknown): string {
  if (!response || typeof response !== 'object') {
    throw new Error('turn/start returned an invalid response')
  }
  const record = response as Record<string, unknown>
  if (typeof record.turnId === 'string' && record.turnId.trim()) return record.turnId
  const turn = record.turn
  if (turn && typeof turn === 'object') {
    const turnId = (turn as { id?: unknown }).id
    if (typeof turnId === 'string' && turnId.trim()) return turnId
  }
  throw new Error('turn/start response did not include a turn id')
}
```

- [ ] **Step 5: Normalize Codex turn completion notifications**

In `desktop-app/src/renderer/src/lib/appServerTurnTracker.ts`, replace `parseTurnCompletion` with:

```ts
function parseTurnCompletion(params: unknown): TurnCompletion | undefined {
  if (!params || typeof params !== 'object') return undefined

  const record = params as Record<string, unknown>
  const threadId = typeof record.threadId === 'string' ? record.threadId : undefined
  const nativeTurnId = typeof record.turnId === 'string' ? record.turnId : undefined
  const output = typeof record.output === 'string' ? record.output : undefined
  const nativeError = typeof record.error === 'string' ? record.error : undefined
  const turn = record.turn && typeof record.turn === 'object' ? record.turn as Record<string, unknown> : undefined
  const turnId = nativeTurnId ?? (typeof turn?.id === 'string' ? turn.id : undefined)
  const codexError = turn?.error && typeof turn.error === 'object'
    ? (turn.error as { message?: unknown }).message
    : undefined
  const error = nativeError ?? (typeof codexError === 'string' ? codexError : undefined)

  if (!threadId || !turnId) return undefined
  return { threadId, turnId, output, error }
}
```

- [ ] **Step 6: Add renderer tests for Codex response shapes**

In `desktop-app/src/renderer/src/lib/assistantMessages.test.ts`, change one `thread/start` mock to return:

```ts
if (method === 'thread/start') return { thread: { id: 'thread-1' } }
```

Change one `turn/start` mock to return:

```ts
return { turn: { id: 'turn-1', status: 'inProgress', items: [] } }
```

Add a completion notification with Codex turn payload:

```ts
notificationListener?.({
  hostId: 'local',
  method: 'turn/completed',
  params: {
    threadId: 'thread-1',
    turn: {
      id: 'turn-1',
      status: 'completed',
      items: [],
      error: null,
      startedAt: null,
      completedAt: null,
      durationMs: null
    }
  }
})
```

- [ ] **Step 7: Run desktop-app tests and typecheck**

Run:

```bash
npm --prefix desktop-app test -- appServerManager.test.ts assistantMessages.test.ts appServerTurnTracker.test.ts
npm --prefix desktop-app run typecheck
```

Expected: tests PASS and TypeScript typecheck exits with `0 errors`.

- [ ] **Step 8: Commit desktop-app normalization**

```bash
git add desktop-app/src/main/appServerManager.ts desktop-app/src/main/appServerManager.test.ts desktop-app/src/renderer/src/hooks/useDasclawAssistantRuntime.ts desktop-app/src/renderer/src/lib/appServerTurnTracker.ts desktop-app/src/renderer/src/lib/assistantMessages.test.ts
git commit -m $'feat: normalize codex app-server p0-p2 shapes in desktop-app\n\n已检查 desktop-app app-server response handling 是否已有，结论：已有 native threadId/turnId/modelProvider/list handling；缺 Codex thread/turn/model list normalization。'
```

## Task 7: Final Verification and Documentation Sync

**Files:**
- Modify: `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`

- [ ] **Step 1: Update the gap matrix P0-P2 status**

In section `## 9. 能力补齐优先级`, update P0-P2 rows to say:

```markdown
| P0 | 诚实的协议边界 | `codex_app_server_v2` 继续标成 chat-session subset；P3-P6 opt-out 保持显式 | 已由 capability/profile tests 约束 |
| P1 | Chat-session compatibility view | `thread/start`、`turn/start`、`thread/started`、`turn/started`、`turn/completed` 保留 native fields 并附带 Codex `thread` / `turn` view | desktop-app 通过 normalization 同时兼容 native 与 Codex view |
| P2 | Model catalog/service | `model/list` 由 app-server 返回 Codex `ModelListResponse`；`modelProvider/list` 仅作为 desktop-app renderer alias | app-server 成为模型列表 owner，主进程不再直接暴露 provider secrets |
```

- [ ] **Step 2: Run Rust focused verification**

Run:

```bash
cargo nextest run -p dasclaw_app_server_protocol
cargo nextest run -p dasclaw_app_server -E 'test(model_list) | test(codex_v2) | test(implemented_capability_methods_are_routable)'
cargo check -p dasclaw_app_server --tests
```

Expected: all commands PASS.

- [ ] **Step 3: Run desktop focused verification**

Run:

```bash
npm --prefix desktop-app test -- appServerManager.test.ts assistantMessages.test.ts appServerTurnTracker.test.ts
npm --prefix desktop-app run typecheck
```

Expected: all commands PASS.

- [ ] **Step 4: Run formatting and repository guard checks**

Run:

```bash
cargo fmt --all
python3.12 scripts/check_no_panics.py --base origin/xClaw
```

Expected: `cargo fmt` exits `0`; `check_no_panics.py` exits `0`.

- [ ] **Step 5: Run focused clippy for touched Rust crates**

Run:

```bash
cargo clippy --no-deps -p dasclaw_app_server_protocol -p dasclaw_app_server --all-targets -- -D warnings
```

Expected: exits `0` for touched crates.

- [ ] **Step 6: Commit documentation sync**

```bash
git add docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md
git commit -m $'docs: update codex app-server p0-p2 gap status\n\n已检查 dasclaw-app-server P0-P2 是否已有，结论：本分支已接入 model/list 与 Codex thread/turn view，P3-P6 仍按后续独立计划推进。'
```

## Self-Review

Spec coverage:

- P0 is covered by Task 1 and Task 7.
- P1 is covered by Task 4, Task 5, and Task 6.
- P2 is covered by Task 2, Task 3, and Task 6.
- P3-P6 are intentionally out of scope because the matrix itself classifies them as separate service and safety domains.

Red-flag scan:

- No deferred-work markers.
- No unspecified validation steps.
- Every test step has a concrete command and expected result.
- Every code-changing step includes concrete code.

Type consistency:

- Rust method constant is consistently `method::MODEL_LIST`.
- Rust response type is consistently `ModelListResponse`.
- Renderer normalization consistently reads native `threadId` / `turnId` first, then Codex `thread.id` / `turn.id`.
- Codex turn statuses map from native `Pending -> inProgress`, `Completed -> completed`, `Failed -> failed`, and `Cancelled -> interrupted`.
