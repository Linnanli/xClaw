# Dasclaw App Server V2-Shaped Protocol Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Collapse the Dasclaw app-server external wire contract into one native protocol whose implemented chat-session surface is shaped like a strict subset of Codex app-server v2, with no legacy method aliases or parallel native response fields.

**Architecture:** Dasclaw remains a native app-server, not a full Codex app-server clone. The public JSON-RPC method names, event names, and payload fields for implemented thread/turn/item/model features should follow Codex app-server v2 shapes as closely as Dasclaw can truthfully support. The compatibility profile becomes a descriptive capability statement for this subset, not a second protocol layer or a runtime switch.

**Tech Stack:** Rust crates `dasclaw_app_server_protocol`, `dasclaw_app_server`, `dasclaw_app_server_client`; JSON-RPC over stdio; Serde; desktop TypeScript bridge and assistant runtime.

---

## Startup Four Questions

1. **是否新增模块 / crate / 文件？** 是。本计划新增 `docs/superpowers/plans/2026-06-19-dasclaw-app-server-v2-shaped-protocol.md`。已先做语义搜索，未发现等价的“单一 native 协议、v2-shaped 子集、删除 legacy aliases”执行计划。
2. **结论里是否含否定语？** 是。结论包含“当前不是单一协议形状”“legacy aliases 不应保留”。已做语义、LSP、`rg` 三层核验。
3. **是否做跨项目对账？** 是。参考 `codex-cli-main/codex-rs/app-server-protocol` 的 `common.rs` 和 `v2.rs`，并对照 Dasclaw 当前协议/server/desktop 消费点。
4. **是否写架构对账类文档？** 是。已按仓库规则先完成三层验证，再写计划。

## Evidence Captured Before Writing

- Level 1 semantic search:
  - Query: `Dasclaw app-server native protocol v2-shaped subset remove legacy aliases codex app-server protocol plan`
  - Query: `Codex app-server Thread Turn Item protocol schema thread start turn completed compatibility profile`
  - Result: no existing implementation plan for the single native v2-shaped subset approach; existing P0-P3 plans still describe a transitional dual-field or alias strategy.
- Level 2 LSP:
  - `workspace_symbols` confirmed current local protocol symbols including `ThreadStartResponse`, `TurnStartResponse`, `CompatibilityProfile`, and `CodexThread` in `crates/dasclaw_app_server_protocol/src/lib.rs`.
- Level 3 `rg`:
  - Current Dasclaw protocol exports `thread/create`, `turn/cancel`, `thread/created`, `turn/delta`, `turn/failed`, `turn/cancelled`, and response-level `threadId` / `turnId` fields.
  - Current desktop runtime reads native `threadId` / `turnId` first, then nested `thread.id` / `turn.id`.
  - Existing plans `docs/superpowers/plans/2026-06-17-dasclaw-app-server-codex-p0-p2.md` and `docs/superpowers/plans/2026-06-18-dasclaw-app-server-codex-p3-full-approval-tool-sandbox.md` still preserve aliases and optional Codex views.
- Codex reference points:
  - `codex-cli-main/codex-rs/app-server-protocol/src/protocol/common.rs` maps `thread/start`, `turn/start`, and `turn/interrupt` as the v2 public methods.
  - `codex-cli-main/codex-rs/app-server-protocol/src/protocol/v2.rs` defines `ThreadStartResponse { thread, ... }`, `TurnStartResponse { turn }`, `Thread { ... turns }`, `Turn { id, items, status, error, ... }`, and item notifications with `item`, `thread_id`, `turn_id`.

## Scope

In scope:

- Public protocol schema exposes only the implemented v2-shaped chat-session surface.
- Remove legacy method aliases from the public surface: `thread/create` and `turn/cancel`.
- Remove legacy event aliases from the public surface: `thread/created`, `turn/delta`, `turn/failed`, `turn/cancelled`.
- Replace response-level native fields with nested v2-shaped objects:
  - `thread/start` returns `thread`, plus truthful subset metadata such as `model`, `modelProvider`, and `cwd`.
  - `turn/start` returns `turn`.
  - `thread/list` returns `data`, `nextCursor`, and `backwardsCursor`.
  - `thread/read` returns `thread`.
  - `thread/turns/list` returns `data`, `nextCursor`, and `backwardsCursor`.
  - `turn/read` returns `turn`.
- Replace event-level native fields where Codex has an object:
  - `turn/started` carries `turn`.
  - `turn/completed` carries `turn`.
  - `item/started` and `item/completed` carry `item`.
- `turn/interrupt` keeps the Codex v2 empty response and reports state through `turn/completed` notifications plus `turn/read`.
- Keep item delta events v2-shaped enough for the current stream: they may still carry `threadId`, `turnId`, `itemId`, and `delta`, because Codex v2 delta notifications identify the item being updated.
- Update desktop-app and client crate consumers to read the new shape only.
- Update docs so they say “Dasclaw native protocol is a Codex app-server v2-shaped subset,” not “native plus v2.”

Out of scope:

- Implementing missing Codex capabilities such as `thread/resume`, `thread/fork`, `thread/archive`, `thread/unsubscribe`, `thread/name/set`, `thread/metadata/update`, `thread/unarchive`, `thread/compact/start`, `thread/shellCommand`, `thread/approveGuardianDeniedAction`, `thread/rollback`, `thread/loaded/list`, or `thread/inject_items`.
- Implementing full Codex product semantics for MCP, dynamic tools, command execution, shell sessions, filesystem changes, subscriptions, archival storage, compaction, or rollback.
- Preserving dev-stage legacy aliases for external consumers. This repository has not published the alias contract, so the plan intentionally removes it.

## Compatibility Profile Meaning

The profile is a label on the single protocol, not another protocol. It should answer:

- “Which Codex app-server v2 concepts does this native protocol currently resemble?”
- “Which methods/events are implemented now?”
- “Which Codex domains are explicitly out of scope for this phase?”

It should not:

- Add alias methods.
- Toggle event payload shapes.
- Let clients request a different wire contract.
- Pretend Dasclaw implements the full Codex app-server v2 surface.

Use a profile id that cannot be mistaken for full compatibility:

```rust
pub const CODEX_APP_SERVER_V2_CHAT_SESSION_SUBSET_ID: &str =
    "codex_app_server_v2_chat_session_subset";
```

## File Structure

- Modify `crates/dasclaw_app_server_protocol/src/lib.rs`
  - Owns public method/event constants, schema, capability profile, and serializable request/response/event types.
  - This is the source of truth for the wire contract.
- Modify `crates/dasclaw_app_server/src/lib.rs`
  - Owns runtime behavior, JSON-RPC routing, notification emission, and mapping internal thread/turn state into protocol objects.
  - Keep native ID helpers private and prevent them from shaping public JSON-RPC responses.
- Modify `crates/dasclaw_app_server_client/src/lib.rs`
  - Owns typed client helpers and line-delimited JSON-RPC notification decoding tests.
  - Update typed helpers and fixtures to consume the single v2-shaped protocol.
- Modify `desktop-app/src/main/appServerManager.ts`
  - Stops requesting `codex_app_server_v2` as a capability switch.
- Modify `desktop-app/src/renderer/src/hooks/useDasclawAssistantRuntime.ts`
  - Reads `thread.id` and `turn.id` only.
- Modify `desktop-app/src/renderer/src/lib/appServerTurnTracker.ts`
  - Parses `turn/completed` from nested `turn` only.
- Modify `desktop-app/src/shared/appServerApi.ts`
  - Keeps shared API state types aligned with nested thread/turn responses and item events.
- Modify `desktop-app/src/main/appServerRpc.test.ts`
  - Updates line-delimited notification fixtures away from `turn/delta` and response-level IDs.
- Modify `desktop-app/src/renderer/src/lib/assistantMessages.test.ts`
  - Updates assistant runtime mocks that still return response-level `threadId` / `turnId`.
- Modify desktop tests under `desktop-app/src/**`
  - Update mocks and assertions to the new response/event shape.
- Modify docs:
  - `docs/plans/dasclaw-app-server-protocol-v0.md`
  - `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`
  - `docs/plans/dasclaw-codex-app-server-ai-sdk-compat-plan.md`

## Task 1: Lock the Protocol Contract With Failing Tests

**Files:**
- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`

- [ ] **Step 1: Replace the transitional response-shape test**

Replace the existing test named `thread_and_turn_responses_can_carry_native_ids_and_codex_views` with this test. It intentionally describes the target contract and should fail before implementation because the current structs still expose `threadId`, `turnId`, `status`, `lifecycle`, and optional Codex views.

```rust
fn codex_thread_fixture() -> CodexThread {
    CodexThread {
        id: "thread_1".to_string(),
        forked_from_id: None,
        preview: "hello".to_string(),
        ephemeral: false,
        model_provider: "openai".to_string(),
        created_at: 1,
        updated_at: 2,
        status: CodexThreadStatus::Idle,
        path: None,
        cwd: "/workspace".to_string(),
        cli_version: "0.0.0".to_string(),
        source: CodexSessionSource::AppServer,
        agent_nickname: None,
        agent_role: None,
        git_info: None,
        name: None,
        turns: vec![CodexTurn::in_progress("turn_1")],
    }
}

#[test]
fn thread_and_turn_responses_are_v2_shaped_without_legacy_id_aliases() {
    let thread_response = ThreadStartResponse {
        thread: codex_thread_fixture(),
        model: "gpt-5".to_string(),
        model_provider: "openai".to_string(),
        cwd: "/workspace".to_string(),
    };
    let turn_response = TurnStartResponse {
        turn: CodexTurn::in_progress("turn_1"),
    };

    let thread_value =
        serde_json::to_value(thread_response).expect("thread/start response should serialize");
    let turn_value =
        serde_json::to_value(turn_response).expect("turn/start response should serialize");

    assert_eq!(thread_value["thread"]["id"], "thread_1");
    assert_eq!(thread_value["thread"]["status"]["type"], "idle");
    assert_eq!(thread_value["thread"]["source"], "appServer");
    assert_eq!(thread_value["model"], "gpt-5");
    assert_eq!(thread_value["modelProvider"], "openai");
    assert_eq!(thread_value["cwd"], "/workspace");
    assert!(thread_value.get("threadId").is_none());
    assert!(thread_value.get("lifecycle").is_none());

    assert_eq!(turn_value["turn"]["id"], "turn_1");
    assert_eq!(turn_value["turn"]["status"], "inProgress");
    assert!(turn_value.get("turnId").is_none());
    assert!(turn_value.get("status").is_none());
    assert!(turn_value.get("lifecycle").is_none());
}
```

- [ ] **Step 2: Replace the schema surface assertions**

Replace the body of `phase_one_schema_describes_routable_protocol_surface` with this body. It locks that `thread/start` and `turn/interrupt` are the public names, while `thread/create`, `turn/cancel`, and legacy events disappear from the schema.

```rust
let schema = ProtocolSchemaResponse::phase_one(CapabilityMatrix::phase_one());
let method_names = schema
    .methods
    .iter()
    .map(|method| method.method.as_str())
    .collect::<Vec<_>>();
let event_names = schema
    .events
    .iter()
    .map(|event| event.event.as_str())
    .collect::<Vec<_>>();

assert_eq!(schema.protocol_version, ProtocolVersion::current());
assert!(method_names.contains(&method::INITIALIZE));
assert!(method_names.contains(&method::PROTOCOL_SCHEMA));
assert!(method_names.contains(&method::HEALTH_CHECK));
assert!(method_names.contains(&method::THREAD_START));
assert!(method_names.contains(&method::THREAD_LIST));
assert!(method_names.contains(&method::THREAD_READ));
assert!(method_names.contains(&method::THREAD_TURNS_LIST));
assert!(method_names.contains(&method::TURN_START));
assert!(method_names.contains(&method::TURN_INTERRUPT));
assert!(method_names.contains(&method::TURN_READ));
assert!(method_names.contains(&method::MODEL_LIST));
assert!(method_names.contains(&method::MODEL_PROVIDER_SELECT_FOR_NEXT_TURN));

assert!(!method_names.contains(&"thread/create"));
assert!(!method_names.contains(&"turn/cancel"));
assert!(!method_names.contains(&"turns/list"));
assert!(!method_names.contains(&"turn/list"));

assert!(event_names.contains(&event::LIFECYCLE_CHANGED));
assert!(event_names.contains(&event::CAPABILITIES_CHANGED));
assert!(event_names.contains(&event::THREAD_STARTED));
assert!(event_names.contains(&event::TURN_STARTED));
assert!(event_names.contains(&event::TURN_COMPLETED));
assert!(event_names.contains(&event::ITEM_STARTED));
assert!(event_names.contains(&event::ITEM_AGENT_MESSAGE_DELTA));
assert!(event_names.contains(&event::ITEM_COMPLETED));

assert!(!event_names.contains(&"thread/created"));
assert!(!event_names.contains(&"turn/delta"));
assert!(!event_names.contains(&"turn/failed"));
assert!(!event_names.contains(&"turn/cancelled"));
assert_eq!(schema.capabilities.logs.status, CapabilityStatus::Declared);
```

- [ ] **Step 3: Add a profile test directly below the schema tests**

```rust
#[test]
fn compatibility_profile_is_descriptive_subset_without_aliases() {
    let schema = ProtocolSchemaResponse::phase_one(CapabilityMatrix::phase_one());
    let profile = schema
        .compatibility_profiles
        .iter()
        .find(|profile| profile.id == CompatibilityProfile::CODEX_APP_SERVER_V2_CHAT_SESSION_SUBSET_ID)
        .expect("schema should describe the Codex v2-shaped chat-session subset");

    assert_eq!(profile.scope, CompatibilityProfileScope::ChatSessionSubset);
    assert!(profile.aliases.is_empty());
    assert!(profile.methods.contains(&method::THREAD_START.to_string()));
    assert!(profile.methods.contains(&method::THREAD_LIST.to_string()));
    assert!(profile.methods.contains(&method::THREAD_TURNS_LIST.to_string()));
    assert!(profile.methods.contains(&method::TURN_START.to_string()));
    assert!(profile.methods.contains(&method::TURN_INTERRUPT.to_string()));
    assert!(!profile.methods.contains(&"thread/create".to_string()));
    assert!(!profile.methods.contains(&"turn/cancel".to_string()));
    assert!(!profile.methods.contains(&"turns/list".to_string()));
    assert!(!profile.methods.contains(&"turn/list".to_string()));
    assert!(profile.events.contains(&event::THREAD_STARTED.to_string()));
    assert!(profile.events.contains(&event::TURN_COMPLETED.to_string()));
    assert!(!profile.events.contains(&"turn/delta".to_string()));
}
```

- [ ] **Step 4: Add a Codex-shaped interrupt/list response test**

Add this test below the profile test. It prevents the implementation from inventing a third response shape for `turn/interrupt` and list methods.

```rust
#[test]
fn interrupt_and_list_responses_match_codex_v2_shapes() {
    let interrupt_response = TurnInterruptResponse {};
    let thread_list_response = ThreadListResponse {
        data: vec![codex_thread_fixture()],
        next_cursor: None,
        backwards_cursor: None,
    };
    let turn_list_response = ThreadTurnsListResponse {
        data: vec![CodexTurn::in_progress("turn_1")],
        next_cursor: None,
        backwards_cursor: None,
    };

    let interrupt_value =
        serde_json::to_value(interrupt_response).expect("turn/interrupt response should serialize");
    let thread_list_value =
        serde_json::to_value(thread_list_response).expect("thread/list response should serialize");
    let turn_list_value =
        serde_json::to_value(turn_list_response).expect("thread/turns/list response should serialize");

    assert_eq!(interrupt_value, serde_json::json!({}));
    assert!(interrupt_value.get("turn").is_none());
    assert_eq!(thread_list_value["data"][0]["id"], "thread_1");
    assert!(thread_list_value.get("threads").is_none());
    assert_eq!(turn_list_value["data"][0]["id"], "turn_1");
    assert!(turn_list_value.get("turns").is_none());
}
```

- [ ] **Step 5: Run the protocol tests and confirm failure**

Run:

```bash
cargo check -p dasclaw_app_server_protocol --tests
cargo nextest run -p dasclaw_app_server_protocol thread_and_turn_responses_are_v2_shaped_without_legacy_id_aliases
cargo nextest run -p dasclaw_app_server_protocol interrupt_and_list_responses_match_codex_v2_shapes
```

Expected:

- `cargo check` fails because `ThreadStartResponse` and `TurnStartResponse` do not yet have the target fields.
- The nextest command does not run until the crate compiles.

Do not commit yet; Task 2 makes these tests pass.

## Task 2: Implement the Protocol Types and Schema

**Files:**
- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`

- [ ] **Step 1: Remove legacy aliases from the public method and event constants**

Edit the method and event constant modules so the public chat-session surface contains the v2-shaped names only.

```rust
pub mod method {
    pub const INITIALIZE: &str = "initialize";
    pub const PROTOCOL_SCHEMA: &str = "protocol/schema";
    pub const CAPABILITIES_LIST: &str = "capabilities/list";
    pub const HEALTH_CHECK: &str = "health/check";
    pub const THREAD_START: &str = "thread/start";
    pub const THREAD_LIST: &str = "thread/list";
    pub const THREAD_READ: &str = "thread/read";
    pub const THREAD_TURNS_LIST: &str = "thread/turns/list";
    pub const TURN_START: &str = "turn/start";
    pub const TURN_INTERRUPT: &str = "turn/interrupt";
    pub const TURN_READ: &str = "turn/read";
    pub const MODEL_LIST: &str = "model/list";
    pub const MODEL_PROVIDER_SELECT_FOR_NEXT_TURN: &str = "modelProvider/selectForNextTurn";
    pub const APPROVAL_RESPOND: &str = "approval/respond";
}

pub mod event {
    pub const NOTIFICATIONS_INITIALIZED: &str = "notifications/initialized";
    pub const LIFECYCLE_CHANGED: &str = "lifecycle/changed";
    pub const CAPABILITIES_CHANGED: &str = "capabilities/changed";
    pub const THREAD_STARTED: &str = "thread/started";
    pub const TURN_STARTED: &str = "turn/started";
    pub const TURN_COMPLETED: &str = "turn/completed";
    pub const ITEM_STARTED: &str = "item/started";
    pub const ITEM_AGENT_MESSAGE_DELTA: &str = "item/agentMessage/delta";
    pub const ITEM_REASONING_SUMMARY_TEXT_DELTA: &str = "item/reasoning/summaryTextDelta";
    pub const ITEM_REASONING_SUMMARY_PART_ADDED: &str = "item/reasoning/summaryPartAdded";
    pub const ITEM_REASONING_TEXT_DELTA: &str = "item/reasoning/textDelta";
    pub const ITEM_COMPLETED: &str = "item/completed";
    pub const ITEM_COMMAND_EXECUTION_REQUEST_APPROVAL: &str =
        "item/commandExecution/requestApproval";
    pub const ITEM_COMMAND_EXECUTION_APPROVAL_SUBMITTED: &str =
        "item/commandExecution/approvalSubmitted";
    pub const ITEM_COMMAND_EXECUTION_OUTPUT_DELTA: &str = "item/commandExecution/outputDelta";
    pub const ITEM_COMMAND_EXECUTION_TERMINAL_INTERACTION: &str =
        "item/commandExecution/terminalInteraction";
    pub const ERROR: &str = "error";
}
```

- [ ] **Step 2: Update the compatibility profile**

Replace `CompatibilityProfile::CODEX_APP_SERVER_V2_ID` and `codex_app_server_v2()` with this narrower subset declaration. Keep `aliases: Vec<CompatibilityAlias>` in the struct for schema stability if removing it causes broad churn, but always serialize an empty vector for this phase.

```rust
impl CompatibilityProfile {
    pub const CODEX_APP_SERVER_V2_CHAT_SESSION_SUBSET_ID: &'static str =
        "codex_app_server_v2_chat_session_subset";

    #[must_use]
    pub fn codex_app_server_v2_chat_session_subset() -> Self {
        Self {
            id: Self::CODEX_APP_SERVER_V2_CHAT_SESSION_SUBSET_ID.to_string(),
            version: "2.0.0-chat-session-subset".to_string(),
            scope: CompatibilityProfileScope::ChatSessionSubset,
            description: "Dasclaw native app-server protocol shaped as a truthful Codex app-server v2 chat-session subset".to_string(),
            methods: vec![
                method::INITIALIZE.to_string(),
                method::THREAD_START.to_string(),
                method::THREAD_READ.to_string(),
                method::THREAD_LIST.to_string(),
                method::THREAD_TURNS_LIST.to_string(),
                method::TURN_START.to_string(),
                method::TURN_INTERRUPT.to_string(),
                method::TURN_READ.to_string(),
                method::MODEL_LIST.to_string(),
            ],
            events: CODEX_APP_SERVER_V2_CHAT_SESSION_SUBSET_EVENTS
                .iter()
                .map(|event| (*event).to_string())
                .collect(),
            aliases: Vec::new(),
            capability_opt_outs: vec![
                CapabilityOptOut::phase_one("codex.rich_input"),
                CapabilityOptOut::phase_one("codex.tool_calls"),
                CapabilityOptOut::phase_one("codex.diff"),
                CapabilityOptOut::phase_one("codex.plan"),
                CapabilityOptOut::phase_one("tools"),
                CapabilityOptOut::phase_one("mcp"),
                CapabilityOptOut::phase_one("skills"),
                CapabilityOptOut::phase_one("dlp_policy"),
                CapabilityOptOut::phase_one("jobs"),
                CapabilityOptOut::phase_one("sandbox"),
                CapabilityOptOut::phase_one("thread.fork"),
                CapabilityOptOut::phase_one("thread.archive"),
                CapabilityOptOut::phase_one("thread.resume"),
                CapabilityOptOut::phase_one("thread.compact"),
                CapabilityOptOut::phase_one("thread.rollback"),
            ],
            event_queue: NotificationQueuePolicy::bounded_lag_disconnect(),
        }
    }
}

const CODEX_APP_SERVER_V2_CHAT_SESSION_SUBSET_EVENTS: &[&str] = &[
    event::NOTIFICATIONS_INITIALIZED,
    event::LIFECYCLE_CHANGED,
    event::CAPABILITIES_CHANGED,
    event::THREAD_STARTED,
    event::TURN_STARTED,
    event::TURN_COMPLETED,
    event::ITEM_STARTED,
    event::ITEM_AGENT_MESSAGE_DELTA,
    event::ITEM_REASONING_SUMMARY_TEXT_DELTA,
    event::ITEM_REASONING_SUMMARY_PART_ADDED,
    event::ITEM_REASONING_TEXT_DELTA,
    event::ITEM_COMPLETED,
    event::ERROR,
];
```

Also update the schema constructor:

```rust
impl ProtocolSchemaResponse {
    #[must_use]
    pub fn phase_one(capabilities: CapabilityMatrix) -> Self {
        Self {
            protocol_version: ProtocolVersion::current(),
            methods: phase_one_methods(),
            events: phase_one_events(),
            capabilities,
            compatibility_profiles: vec![
                CompatibilityProfile::codex_app_server_v2_chat_session_subset(),
            ],
        }
    }
}
```

- [ ] **Step 3: Replace response DTOs with v2-shaped response DTOs**

Replace the current response structs for thread and turn read/start/list with these public wire types.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadStartParams {
    pub cwd: Option<String>,
}

impl Default for ThreadStartParams {
    fn default() -> Self {
        Self {
            cwd: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadStartResponse {
    pub thread: CodexThread,
    pub model: String,
    pub model_provider: String,
    pub cwd: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadListParams {
    pub cursor: Option<String>,
    pub limit: Option<u32>,
    pub sort_direction: Option<SortDirection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadListResponse {
    pub data: Vec<CodexThread>,
    pub next_cursor: Option<String>,
    pub backwards_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadReadResponse {
    pub thread: CodexThread,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnStartParams {
    pub thread_id: String,
    pub input: Vec<UserInput>,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UserInput {
    Text {
        text: String,
        #[serde(default)]
        text_elements: Vec<serde_json::Value>,
    },
    Image {
        url: String,
    },
}

impl TurnStartParams {
    #[must_use]
    pub fn prompt_text(&self) -> String {
        self.input
            .iter()
            .filter_map(|input| match input {
                UserInput::Text { text, .. } => Some(text.as_str()),
                UserInput::Image { .. } => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnStartResponse {
    pub turn: CodexTurn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnInterruptParams {
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnInterruptResponse {
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadTurnsListParams {
    pub thread_id: String,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
    pub sort_direction: Option<SortDirection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadTurnsListResponse {
    pub data: Vec<CodexTurn>,
    pub next_cursor: Option<String>,
    pub backwards_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnReadResponse {
    pub turn: CodexTurn,
}
```

Replace `ThreadCreateParams` with `ThreadStartParams` instead of renaming the old fields. The public params type must use Codex v2 field names that Dasclaw can support truthfully. `title` and `workspaceRoot` are not Codex v2 `thread/start` fields and must not remain on the public method.

- [ ] **Step 4: Add a minimal v2-shaped ThreadItem subset**

Codex v2 uses `ThreadItem` as a tagged enum under the `item` field. Add this minimal subset near `CodexTurn`, and change `CodexTurn.items` from `Vec<serde_json::Value>` to `Vec<CodexThreadItem>` so completed turns and item events share one type.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CodexThreadItem {
    #[serde(rename_all = "camelCase")]
    AgentMessage { id: String, text: String },
    #[serde(rename_all = "camelCase")]
    Reasoning {
        id: String,
        #[serde(default)]
        summary: Vec<String>,
        #[serde(default)]
        content: Vec<String>,
    },
}

impl CodexThreadItem {
    #[must_use]
    pub fn started_agent_message(id: impl Into<String>) -> Self {
        Self::AgentMessage {
            id: id.into(),
            text: String::new(),
        }
    }

    #[must_use]
    pub fn completed_agent_message(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self::AgentMessage {
            id: id.into(),
            text: text.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexTurn {
    pub id: String,
    pub items: Vec<CodexThreadItem>,
    pub status: CodexTurnStatus,
    pub error: Option<CodexTurnError>,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub duration_ms: Option<i64>,
}
```

- [ ] **Step 5: Replace event DTOs that currently carry legacy flattened fields**

Replace the thread/turn/item lifecycle event structs with these shapes.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadStartedEvent {
    pub thread: CodexThread,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnStartedEvent {
    pub thread_id: String,
    pub turn: CodexTurn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnCompletedEvent {
    pub thread_id: String,
    pub turn: CodexTurn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemStartedEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub item: CodexThreadItem,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemCompletedEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub item: CodexThreadItem,
}
```

Remove `ThreadCreatedEvent`, `TurnDeltaEvent`, `TurnFailedEvent`, and `TurnCancelledEvent` from the public protocol enum and notification constructors in this same file. Failures should be represented by `TurnCompletedEvent { turn: CodexTurn { status: Failed, error: Some(...) } }`.

- [ ] **Step 6: Update phase-one schema method/event builders**

Ensure `phase_one_methods()` includes `THREAD_START`, `TURN_INTERRUPT`, `MODEL_LIST`, and does not include `THREAD_CREATE` or `TURN_CANCEL`. Ensure `phase_one_events()` includes only the event names listed in `CODEX_APP_SERVER_V2_CHAT_SESSION_SUBSET_EVENTS` plus locally implemented approval/command events from P3 if those events are already real in this crate.

- [ ] **Step 7: Run protocol verification**

Run:

```bash
cargo fmt --all
cargo check -p dasclaw_app_server_protocol --tests
cargo nextest run -p dasclaw_app_server_protocol
```

Expected:

- `dasclaw_app_server_protocol` compiles.
- The three new/changed protocol tests pass.
- Other crates may still fail because they still reference removed fields or constants. That is expected before Task 3 and Task 4.

- [ ] **Step 8: Commit protocol crate changes**

```bash
git add crates/dasclaw_app_server_protocol/src/lib.rs
git commit -m "refactor(app-server-protocol): make chat session wire shape v2-like" \
  -m "已检查 Dasclaw app-server v2-shaped protocol 是否已有，结论：未发现单一 native 协议收敛计划；已有 P0-P3 计划仍保留 legacy aliases / native fields。"
```

## Task 3: Lock App Server Routing and Notification Behavior With Failing Tests

**Files:**
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [ ] **Step 1: Add JSON-RPC response shape tests in the existing test module**

Add these tests next to the current JSON-RPC route tests.

```rust
#[test]
fn thread_start_json_rpc_returns_v2_shaped_thread_without_legacy_aliases() {
    let mut server = initialized_test_server();
    let response = server
        .handle_json_rpc(
            r#"{"jsonrpc":"2.0","id":1,"method":"thread/start","params":{"cwd":"/workspace"}}"#,
        )
        .expect("thread/start should return a response");
    let value: serde_json::Value =
        serde_json::from_str(&response).expect("response should be valid JSON");

    assert_eq!(value["result"]["thread"]["id"], "thread_1");
    assert_eq!(value["result"]["thread"]["cwd"], "/workspace");
    assert_eq!(value["result"]["modelProvider"], "openai");
    assert!(value["result"].get("threadId").is_none());
    assert!(value["result"].get("lifecycle").is_none());

    let notifications = server.drain_json_rpc_notifications();
    assert!(notifications.iter().any(|line| {
        let notification: serde_json::Value =
            serde_json::from_str(line).expect("notification should be JSON");
        notification["method"] == "thread/started"
            && notification["params"]["thread"]["id"] == "thread_1"
    }));
    assert!(!notifications.iter().any(|line| line.contains("thread/created")));
}

#[test]
fn thread_create_json_rpc_is_not_a_public_method() {
    let mut server = initialized_test_server();
    let response = server
        .handle_json_rpc(
            r#"{"jsonrpc":"2.0","id":1,"method":"thread/create","params":{"title":"hello"}}"#,
        )
        .expect("unknown method should return an error response");
    let value: serde_json::Value =
        serde_json::from_str(&response).expect("response should be valid JSON");

    assert_eq!(value["error"]["data"]["code"], "UNKNOWN_METHOD");
}

#[test]
fn turn_start_json_rpc_returns_v2_shaped_turn_and_events() {
    let mut server = initialized_test_server();
    server
        .handle_json_rpc(
            r#"{"jsonrpc":"2.0","id":1,"method":"thread/start","params":{"cwd":"/workspace"}}"#,
        )
        .expect("thread/start should return a response");
    server.drain_json_rpc_notifications();

    let response = server
        .handle_json_rpc(
            r#"{"jsonrpc":"2.0","id":2,"method":"turn/start","params":{"threadId":"thread_1","input":[{"type":"text","text":"hi"}]}}"#,
        )
        .expect("turn/start should return a response");
    let value: serde_json::Value =
        serde_json::from_str(&response).expect("response should be valid JSON");

    assert_eq!(value["result"]["turn"]["id"], "turn_1");
    assert_eq!(value["result"]["turn"]["status"], "inProgress");
    assert!(value["result"].get("turnId").is_none());
    assert!(value["result"].get("status").is_none());
    assert!(value["result"].get("lifecycle").is_none());

    let notifications = server.drain_json_rpc_notifications();
    assert!(notifications.iter().any(|line| {
        let notification: serde_json::Value =
            serde_json::from_str(line).expect("notification should be JSON");
        notification["method"] == "turn/started"
            && notification["params"]["threadId"] == "thread_1"
            && notification["params"]["turn"]["id"] == "turn_1"
    }));
    assert!(notifications.iter().any(|line| {
        let notification: serde_json::Value =
            serde_json::from_str(line).expect("notification should be JSON");
        notification["method"] == "item/started"
            && notification["params"]["item"]["type"] == "agentMessage"
            && notification["params"]["item"]["id"] == "turn_1"
    }));
    assert!(!notifications.iter().any(|line| line.contains("turn/delta")));
}

#[test]
fn list_json_rpc_methods_return_codex_paged_data_shapes() {
    let mut server = initialized_test_server();
    server
        .handle_json_rpc(
            r#"{"jsonrpc":"2.0","id":1,"method":"thread/start","params":{"cwd":"/workspace"}}"#,
        )
        .expect("thread/start should return a response");
    server
        .handle_json_rpc(
            r#"{"jsonrpc":"2.0","id":2,"method":"turn/start","params":{"threadId":"thread_1","input":[{"type":"text","text":"hi"}]}}"#,
        )
        .expect("turn/start should return a response");

    let thread_list_response = server
        .handle_json_rpc(r#"{"jsonrpc":"2.0","id":3,"method":"thread/list","params":{}}"#)
        .expect("thread/list should return a response");
    let thread_list_value: serde_json::Value =
        serde_json::from_str(&thread_list_response).expect("response should be valid JSON");
    assert_eq!(thread_list_value["result"]["data"][0]["id"], "thread_1");
    assert!(thread_list_value["result"].get("threads").is_none());
    assert!(thread_list_value["result"].get("nextCursor").is_some());
    assert!(thread_list_value["result"].get("backwardsCursor").is_some());

    let turn_list_response = server
        .handle_json_rpc(
            r#"{"jsonrpc":"2.0","id":4,"method":"thread/turns/list","params":{"threadId":"thread_1"}}"#,
        )
        .expect("thread/turns/list should return a response");
    let turn_list_value: serde_json::Value =
        serde_json::from_str(&turn_list_response).expect("response should be valid JSON");
    assert_eq!(turn_list_value["result"]["data"][0]["id"], "turn_1");
    assert!(turn_list_value["result"].get("turns").is_none());
    assert!(turn_list_value["result"].get("nextCursor").is_some());
    assert!(turn_list_value["result"].get("backwardsCursor").is_some());
}

#[test]
fn turn_interrupt_replaces_turn_cancel_as_public_cancel_method() {
    let mut server = initialized_test_server();
    server
        .handle_json_rpc(
            r#"{"jsonrpc":"2.0","id":1,"method":"thread/start","params":{"cwd":"/workspace"}}"#,
        )
        .expect("thread/start should return a response");
    server
        .handle_json_rpc(
            r#"{"jsonrpc":"2.0","id":2,"method":"turn/start","params":{"threadId":"thread_1","input":[{"type":"text","text":"hi"}]}}"#,
        )
        .expect("turn/start should return a response");

    let legacy_response = server
        .handle_json_rpc(
            r#"{"jsonrpc":"2.0","id":3,"method":"turn/cancel","params":{"threadId":"thread_1","turnId":"turn_1"}}"#,
        )
        .expect("unknown legacy method should return an error");
    let legacy_value: serde_json::Value =
        serde_json::from_str(&legacy_response).expect("response should be valid JSON");
    assert_eq!(legacy_value["error"]["data"]["code"], "UNKNOWN_METHOD");

    let response = server
        .handle_json_rpc(
            r#"{"jsonrpc":"2.0","id":4,"method":"turn/interrupt","params":{"threadId":"thread_1","turnId":"turn_1"}}"#,
        )
        .expect("turn/interrupt should return a response");
    let value: serde_json::Value =
        serde_json::from_str(&response).expect("response should be valid JSON");
    assert_eq!(value["result"], serde_json::json!({}));
    let notifications = server.drain_json_rpc_notifications();
    assert!(notifications.iter().any(|line| {
        let notification: serde_json::Value =
            serde_json::from_str(line).expect("notification should be JSON");
        notification["method"] == "turn/completed"
            && notification["params"]["threadId"] == "thread_1"
            && notification["params"]["turn"]["id"] == "turn_1"
            && notification["params"]["turn"]["status"] == "interrupted"
    }));
}
```

- [ ] **Step 2: Run app-server tests and confirm failure**

Run:

```bash
cargo check -p dasclaw_app_server --tests
```

Expected:

- The crate fails to compile because route handlers and notification constructors still use removed legacy types or fields.

Do not commit yet; Task 4 makes these tests pass.

## Task 4: Implement Server Routing, Responses, and Notifications

**Files:**
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [ ] **Step 1: Delete the runtime compatibility switch**

Remove `codex_v2_compat_enabled` from the server state and stop deriving behavior from `requestedCapabilities`. The profile is descriptive metadata, so notifications must not depend on clients requesting a profile id.

Replace initialize logic shaped like this:

```rust
let codex_v2_compat_requested = params
    .requested_capabilities
    .iter()
    .any(|capability| capability == CompatibilityProfile::CODEX_APP_SERVER_V2_ID);
```

with no profile switch at all. Keep `unavailable_requested_capabilities` for real capabilities such as `protocol`, `health`, `session`, and `model_provider`.

Remove the guard from notification helpers:

```rust
if !self.codex_v2_compat_enabled {
    return;
}
```

After this step, `emit_notifications_initialized`, `emit_thread_started`, `emit_item_started`, item delta helpers, `emit_item_completed`, and `emit_error` all emit according to normal native protocol behavior.

- [ ] **Step 2: Replace public `thread_create` with a private helper**

Replace the public `thread_create` method with this private helper. It creates the internal record and emits only the v2-shaped `thread/started` event.

```rust
fn create_thread_record(&mut self, params: ThreadStartParams) -> Result<String, AppServerError> {
    if self.lifecycle.state == LifecycleState::Stopped {
        return Err(AppServerError::server_stopped(self.lifecycle.clone()));
    }
    self.require_initialized("session")?;

    let thread_id = self.threads.create(params);
    let thread = self.codex_thread_view(&thread_id, true)?;
    self.notifications
        .emit_thread_started(ThreadStartedEvent { thread });
    Ok(thread_id)
}
```

- [ ] **Step 3: Update thread storage to use `cwd` instead of legacy title/workspace params**

Update the thread store create path so `ThreadStartParams { cwd }` maps to the stored workspace root. Do not keep `title` or `workspaceRoot` on the public request.

```rust
pub fn create(&mut self, params: ThreadStartParams) -> String {
    let thread_id = self.next_thread_id();
    self.threads.push(ThreadRecord {
        thread_id: thread_id.clone(),
        title: None,
        workspace_root: params.cwd,
    });
    thread_id
}
```

- [ ] **Step 4: Update `thread_start` to return the v2-shaped response**

```rust
pub fn thread_start(
    &mut self,
    params: ThreadStartParams,
) -> Result<ThreadStartResponse, AppServerError> {
    let thread_id = self.create_thread_record(params)?;
    let thread = self.codex_thread_view(&thread_id, true)?;
    let selected = self.model_provider.selected_snapshot()?;

    Ok(ThreadStartResponse {
        cwd: thread.cwd.clone(),
        thread,
        model: selected.model_id,
        model_provider: selected.provider,
    })
}
```

- [ ] **Step 5: Update thread list/read to return Codex list/read shapes**

```rust
pub fn thread_list(&self, _params: ThreadListParams) -> Result<ThreadListResponse, AppServerError> {
    self.require_initialized("session")?;
    let data = self
        .threads
        .list()
        .into_iter()
        .map(|summary| self.codex_thread_view(&summary.thread_id, false))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ThreadListResponse {
        data,
        next_cursor: None,
        backwards_cursor: None,
    })
}

pub fn thread_read(
    &self,
    params: ThreadReadParams,
) -> Result<ThreadReadResponse, AppServerError> {
    self.require_initialized("session")?;
    Ok(ThreadReadResponse {
        thread: self.codex_thread_view(&params.thread_id, true)?,
    })
}
```

- [ ] **Step 6: Add `thread/turns/list` with Codex list response shape**

Replace the old turn-list public method with `thread/turns/list`.

```rust
pub fn thread_turns_list(
    &self,
    params: ThreadTurnsListParams,
) -> Result<ThreadTurnsListResponse, AppServerError> {
    self.require_initialized("session")?;
    self.require_thread_exists(&params.thread_id)?;
    let data = self
        .threads
        .list_turns(&params.thread_id)
        .into_iter()
        .map(codex_turn_from_summary)
        .collect();
    Ok(ThreadTurnsListResponse {
        data,
        next_cursor: None,
        backwards_cursor: None,
    })
}
```

- [ ] **Step 7: Update `turn_start` to use v2-shaped input and return `TurnStartResponse { turn }`**

Parse the prompt text from `TurnStartParams::prompt_text()` and reject requests whose `input` has no text item. Keep the runtime bridge request and state transition logic intact.

```rust
let prompt = params.prompt_text();
if prompt.trim().is_empty() {
    return Err(AppServerError::invalid_request(
        "turn/start",
        "turn/start input must include at least one text item",
    ));
}
```

Replace the response and notification construction at the end of `turn_start`:

```rust
let thread_id = params.thread_id;
let turn = self.codex_turn_view(&thread_id, &turn_id)?;
self.notifications.emit_turn_started(TurnStartedEvent {
    thread_id: thread_id.clone(),
    turn: turn.clone(),
});
self.emit_codex_item_started(thread_id.clone(), turn_id.clone());

Ok(TurnStartResponse { turn })
```

- [ ] **Step 8: Update `turn_interrupt` and retire `turn_cancel` as public API**

Keep the existing cancellation mechanics as private/shared implementation detail, and route only `method::TURN_INTERRUPT`. The public response is the Codex v2 empty response. Emit the interrupted turn state through `turn/completed`.

```rust
pub fn turn_interrupt(
    &mut self,
    params: TurnInterruptParams,
) -> Result<TurnInterruptResponse, AppServerError> {
    self.require_initialized("session")?;
    self.drain_runtime_turn_updates();
    self.require_thread_exists(&params.thread_id)?;
    self.runtime_bridge
        .cancel_turn(&params.thread_id, &params.turn_id)
        .map_err(AppServerError::runtime_bridge)?;
    self.threads
        .record_cancelled_turn(&params.thread_id, &params.turn_id);

    let turn = CodexTurn {
        id: params.turn_id.clone(),
        items: Vec::new(),
        status: CodexTurnStatus::Interrupted,
        error: None,
        started_at: None,
        completed_at: None,
        duration_ms: None,
    };
    self.notifications.emit_turn_completed(TurnCompletedEvent {
        thread_id: params.thread_id,
        turn,
    });

    Ok(TurnInterruptResponse {})
}
```

- [ ] **Step 9: Update JSON-RPC routing**

In `route_json_rpc`, remove the `method::THREAD_CREATE` and `method::TURN_CANCEL` arms. Keep these arms:

```rust
method::THREAD_START => {
    parse_params(request.params)
        .and_then(|params| self.thread_start(params))
        .map(|result| json_rpc_ok(request.id, result))
}
method::THREAD_LIST => {
    parse_params(request.params)
        .and_then(|params| self.thread_list(params))
        .map(|result| json_rpc_ok(request.id, result))
}
method::THREAD_TURNS_LIST => {
    parse_params(request.params)
        .and_then(|params| self.thread_turns_list(params))
        .map(|result| json_rpc_ok(request.id, result))
}
method::TURN_START => {
    parse_params(request.params)
        .and_then(|params| self.turn_start(params))
        .map(|result| json_rpc_ok(request.id, result))
}
method::TURN_INTERRUPT => {
    parse_params(request.params)
        .and_then(|params| self.turn_interrupt(params))
        .map(|result| json_rpc_ok(request.id, result))
}
```

- [ ] **Step 10: Update item notification helpers**

Replace `emit_codex_item_started` and `emit_codex_item_completed` payload construction with nested `item` objects.

```rust
fn emit_codex_item_started(&mut self, thread_id: String, turn_id: String) {
    self.notifications.emit_item_started(ItemStartedEvent {
        thread_id,
        turn_id: turn_id.clone(),
        item: CodexThreadItem::started_agent_message(turn_id),
    });
}

fn emit_codex_item_completed(
    &mut self,
    thread_id: String,
    turn_id: String,
    text: impl Into<String>,
) {
    self.notifications.emit_item_completed(ItemCompletedEvent {
        thread_id,
        turn_id: turn_id.clone(),
        item: CodexThreadItem::completed_agent_message(turn_id, text),
    });
}
```

- [ ] **Step 11: Update runtime turn update draining**

In `drain_runtime_turn_updates`, replace legacy emissions:

- `emit_turn_delta` disappears. Keep `emit_codex_agent_message_delta`.
- `emit_turn_failed` becomes `emit_turn_completed` with `CodexTurnStatus::Failed` and an error object.
- `emit_turn_cancelled` becomes `emit_turn_completed` with `CodexTurnStatus::Interrupted`.
- `emit_turn_completed` carries `TurnCompletedEvent { thread_id, turn }`.

Use this shape for a completed successful turn:

```rust
let turn = CodexTurn {
    id: turn_id.clone(),
    items: vec![CodexThreadItem::completed_agent_message(
        turn_id.clone(),
        output.clone(),
    )],
    status: CodexTurnStatus::Completed,
    error: None,
    started_at: None,
    completed_at: None,
    duration_ms: None,
};
self.notifications.emit_turn_completed(TurnCompletedEvent {
    thread_id: thread_id.clone(),
    turn: turn.clone(),
});
self.emit_codex_item_completed(thread_id, turn_id, output);
```

Use this shape for a failed turn:

```rust
let turn = CodexTurn {
    id: turn_id,
    items: Vec::new(),
    status: CodexTurnStatus::Failed,
    error: Some(CodexTurnError {
        message: error_message,
        codex_error_info: None,
        additional_details: None,
    }),
    started_at: None,
    completed_at: None,
    duration_ms: None,
};
self.notifications.emit_turn_completed(TurnCompletedEvent { thread_id, turn });
```

- [ ] **Step 12: Run app-server verification**

Run:

```bash
cargo fmt --all
cargo check -p dasclaw_app_server --tests
cargo nextest run -p dasclaw_app_server thread_start_json_rpc_returns_v2_shaped_thread_without_legacy_aliases
cargo nextest run -p dasclaw_app_server turn_start_json_rpc_returns_v2_shaped_turn_and_events
cargo nextest run -p dasclaw_app_server list_json_rpc_methods_return_codex_paged_data_shapes
cargo nextest run -p dasclaw_app_server turn_interrupt_replaces_turn_cancel_as_public_cancel_method
```

Expected:

- App-server compiles.
- New route/notification tests pass.
- Remaining failures should now be in client crate or desktop tests that still use old fixture shape.

- [ ] **Step 13: Commit app-server changes**

```bash
git add crates/dasclaw_app_server/src/lib.rs
git commit -m "refactor(app-server): route chat session through v2-shaped native surface" \
  -m "已检查 Dasclaw app-server v2-shaped protocol 是否已有，结论：未发现单一 native 协议收敛计划；已有 P0-P3 计划仍保留 legacy aliases / native fields。"
```

## Task 5: Update the Rust Client Crate

**Files:**
- Modify: `crates/dasclaw_app_server_client/src/lib.rs`

- [ ] **Step 1: Rename typed helpers away from legacy methods**

Replace public helper names that expose legacy wire methods:

```rust
pub fn thread_start_with_notifications(
    &mut self,
    params: ThreadStartParams,
) -> Result<AppServerRoundTrip<ThreadStartResponse>, AppServerClientError> {
    self.request_typed_with_notifications(method::THREAD_START, params)
}

pub fn turn_interrupt_with_notifications(
    &mut self,
    params: TurnInterruptParams,
) -> Result<AppServerRoundTrip<TurnInterruptResponse>, AppServerClientError> {
    self.request_typed_with_notifications(method::TURN_INTERRUPT, params)
}
```

Remove or privatize `thread_create_with_notifications` and `turn_cancel_with_notifications`. Legacy negative tests should use `request_value("thread/create", ...)` and assert the JSON-RPC error.

- [ ] **Step 2: Update line-delimited notification fixtures**

Replace fixtures shaped like this:

```json
{"jsonrpc":"2.0","method":"item/started","params":{"threadId":"thread_1","turnId":"turn_1","itemId":"turn_1","itemType":"agent_message"}}
```

with this:

```json
{"jsonrpc":"2.0","method":"item/started","params":{"threadId":"thread_1","turnId":"turn_1","item":{"type":"agentMessage","id":"turn_1","text":""}}}
```

Replace completed turn fixtures shaped like this:

```json
{"jsonrpc":"2.0","method":"turn/completed","params":{"threadId":"thread_1","turnId":"turn_1","status":"completed","output":"hello"}}
```

with this:

```json
{"jsonrpc":"2.0","method":"turn/completed","params":{"threadId":"thread_1","turn":{"id":"turn_1","items":[{"type":"agentMessage","id":"turn_1","text":"hello"}],"status":"completed","error":null,"startedAt":null,"completedAt":null,"durationMs":null}}}
```

Replace failed turn fixtures shaped like this:

```json
{"jsonrpc":"2.0","method":"turn/failed","params":{"threadId":"thread_1","turnId":"turn_1","status":"failed","error":"runtime failed after response"}}
```

with this:

```json
{"jsonrpc":"2.0","method":"turn/completed","params":{"threadId":"thread_1","turn":{"id":"turn_1","items":[],"status":"failed","error":{"message":"runtime failed after response","codexErrorInfo":null,"additionalDetails":null},"startedAt":null,"completedAt":null,"durationMs":null}}}
```

- [ ] **Step 3: Update response fixtures**

Replace result fixtures shaped like this:

```json
{"turnId":"turn_1","status":"pending"}
```

with this:

```json
{"turn":{"id":"turn_1","items":[],"status":"inProgress","error":null,"startedAt":null,"completedAt":null,"durationMs":null}}
```

Replace thread result fixtures shaped like this:

```json
{"threadId":"thread_1","lifecycle":{"state":"ready","reason":"runtime_ready","since":"1"}}
```

with this:

```json
{"thread":{"id":"thread_1","forkedFromId":null,"preview":"hello","ephemeral":false,"modelProvider":"openai","createdAt":1,"updatedAt":1,"status":{"type":"idle"},"path":null,"cwd":"/workspace","cliVersion":"0.0.0","source":"appServer","agentNickname":null,"agentRole":null,"gitInfo":null,"name":null,"turns":[]},"model":"gpt-5","modelProvider":"openai","cwd":"/workspace"}
```

- [ ] **Step 4: Run client crate tests**

Run:

```bash
cargo fmt --all
cargo check -p dasclaw_app_server_client --tests
cargo nextest run -p dasclaw_app_server_client
```

Expected:

- Client crate compiles.
- Notification decoding tests pass with nested `turn` and `item` shapes.
- No tests mention `thread/create`, `turn/cancel`, `turn/delta`, `turn/failed`, or `turn/cancelled` except explicit negative tests.

- [ ] **Step 5: Commit client changes**

```bash
git add crates/dasclaw_app_server_client/src/lib.rs
git commit -m "refactor(app-server-client): consume v2-shaped native chat events" \
  -m "已检查 Dasclaw app-server v2-shaped protocol 是否已有，结论：未发现单一 native 协议收敛计划；已有 P0-P3 计划仍保留 legacy aliases / native fields。"
```

## Task 6: Update Desktop App Consumers

**Files:**
- Modify: `desktop-app/src/main/appServerManager.ts`
- Modify: `desktop-app/src/main/appServerRpc.test.ts`
- Modify: `desktop-app/src/renderer/src/hooks/useDasclawAssistantRuntime.ts`
- Modify: `desktop-app/src/renderer/src/lib/appServerTurnTracker.ts`
- Modify: `desktop-app/src/renderer/src/lib/assistantMessages.test.ts`
- Modify: `desktop-app/src/shared/appServerApi.ts`
- Modify: desktop tests under `desktop-app/src/**` that still contain old response or event fixtures.

- [ ] **Step 1: Stop requesting `codex_app_server_v2` as a capability switch**

In `desktop-app/src/main/appServerManager.ts`, replace the initialize capability list with this:

```ts
requestedCapabilities: [
  'protocol',
  'lifecycle',
  'health',
  'session'
],
```

Do not include `codex_app_server_v2`; the server now has one default wire shape.

- [ ] **Step 2: Start threads with v2-shaped params**

In `desktop-app/src/renderer/src/hooks/useDasclawAssistantRuntime.ts`, stop sending a synthetic `title` to `thread/start`. Send the current working directory only when the renderer has one available; otherwise send an empty object.

```ts
const response = await window.desktopAppServer.request<unknown>('thread/start', {})
```

The first user prompt remains the first turn input, not a thread title parameter.

- [ ] **Step 3: Start turns with v2-shaped text input**

Replace `turn/start` params shaped like this:

```ts
{
  threadId,
  input: prompt
}
```

with this:

```ts
{
  threadId,
  input: [{ type: 'text', text: prompt, textElements: [] }]
}
```

- [ ] **Step 4: Read thread id only from `thread.id`**

Replace `readThreadId` with:

```ts
function readThreadId(response: unknown): string {
  if (!response || typeof response !== 'object') {
    throw new Error('thread/start returned an invalid response')
  }
  const record = response as Record<string, unknown>
  const thread = record.thread
  if (thread && typeof thread === 'object') {
    const threadId = (thread as { id?: unknown }).id
    if (typeof threadId === 'string' && threadId.trim()) return threadId
  }
  throw new Error('thread/start response did not include thread.id')
}
```

- [ ] **Step 5: Read turn id only from `turn.id`**

Replace `readTurnId` with:

```ts
function readTurnId(response: unknown): string {
  if (!response || typeof response !== 'object') {
    throw new Error('turn/start returned an invalid response')
  }
  const record = response as Record<string, unknown>
  const turn = record.turn
  if (turn && typeof turn === 'object') {
    const turnId = (turn as { id?: unknown }).id
    if (typeof turnId === 'string' && turnId.trim()) return turnId
  }
  throw new Error('turn/start response did not include turn.id')
}
```

- [ ] **Step 6: Parse completion only from nested `turn`**

Replace `parseTurnCompletion` with:

```ts
function parseTurnCompletion(params: unknown): TurnCompletion | undefined {
  if (!params || typeof params !== 'object') return undefined

  const record = params as Record<string, unknown>
  const threadId = typeof record.threadId === 'string' ? record.threadId : undefined
  const turn =
    record.turn && typeof record.turn === 'object'
      ? (record.turn as Record<string, unknown>)
      : undefined
  const turnId = typeof turn?.id === 'string' ? turn.id : undefined
  const errorRecord =
    turn?.error && typeof turn.error === 'object'
      ? (turn.error as { message?: unknown })
      : undefined
  const error = typeof errorRecord?.message === 'string' ? errorRecord.message : undefined
  const output = readAgentMessageOutput(turn?.items)

  if (!threadId || !turnId) return undefined

  return {
    threadId,
    turnId,
    output,
    error
  }
}

function readAgentMessageOutput(items: unknown): string | undefined {
  if (!Array.isArray(items)) return undefined
  const text = items
    .map((item) => {
      if (!item || typeof item !== 'object') return ''
      const record = item as { type?: unknown; text?: unknown }
      return record.type === 'agentMessage' && typeof record.text === 'string'
        ? record.text
        : ''
    })
    .join('')
  return text.length > 0 ? text : undefined
}
```

- [ ] **Step 7: Keep delta parsing on item delta events only**

`parseTurnContentDelta` may keep reading `threadId`, `turnId`, `itemId`, and `delta` from `item/agentMessage/delta`, `item/reasoning/summaryTextDelta`, and `item/reasoning/textDelta` because v2 delta events identify the active item and stream a fragment. Remove support for `turn/delta` from desktop tests and fixtures found by `rg -n "turn/delta" desktop-app/src`.

- [ ] **Step 8: Update desktop tests, shared types, and mocks**

Update these files explicitly:

- `desktop-app/src/main/appServerRpc.test.ts`
- `desktop-app/src/renderer/src/lib/assistantMessages.test.ts`
- `desktop-app/src/shared/appServerApi.ts`
- `desktop-app/src/renderer/src/lib/appServerTurnTracker.test.ts`
- `desktop-app/src/main/appServerManager.test.ts`

Replace mocked `thread/start` responses like:

```ts
{ threadId: 'thread_1' }
```

with:

```ts
{ thread: { id: 'thread_1' } }
```

Replace mocked `turn/start` responses like:

```ts
{ turnId: 'turn_1', status: 'pending' }
```

with:

```ts
{
  turn: {
    id: 'turn_1',
    items: [],
    status: 'inProgress',
    error: null,
    startedAt: null,
    completedAt: null,
    durationMs: null
  }
}
```

Replace completion notifications like:

```ts
{
  method: 'turn/completed',
  params: {
    threadId: 'thread_1',
    turnId: 'turn_1',
    status: 'completed',
    output: 'hello'
  }
}
```

with:

```ts
{
  method: 'turn/completed',
  params: {
    threadId: 'thread_1',
    turn: {
      id: 'turn_1',
      items: [{ type: 'agentMessage', id: 'turn_1', text: 'hello' }],
      status: 'completed',
      error: null,
      startedAt: null,
      completedAt: null,
      durationMs: null
    }
  }
}
```

Update request expectations for `turn/start` to expect `input: [{ type: 'text', text, textElements: [] }]`.

- [ ] **Step 9: Run desktop verification**

Run:

```bash
npm --prefix desktop-app test -- appServerTurnTracker
npm --prefix desktop-app test -- appServerManager
npm --prefix desktop-app test -- appServerRpc
npm --prefix desktop-app test -- assistantMessages
npm --prefix desktop-app run typecheck
npm --prefix desktop-app test
```

Expected:

- Tests pass with no fallback reads from response-level `threadId` or `turnId`.
- No desktop fixture contains `thread/create`, `turn/cancel`, `turn/delta`, `turn/failed`, or `turn/cancelled`.

- [ ] **Step 10: Commit desktop changes**

```bash
git add desktop-app/src/main/appServerManager.ts \
  desktop-app/src/main/appServerRpc.test.ts \
  desktop-app/src/renderer/src/hooks/useDasclawAssistantRuntime.ts \
  desktop-app/src/renderer/src/lib/appServerTurnTracker.ts \
  desktop-app/src/renderer/src/lib/assistantMessages.test.ts \
  desktop-app/src/shared/appServerApi.ts \
  desktop-app/src
git commit -m "refactor(desktop-app): consume single v2-shaped app-server protocol" \
  -m "已检查 Dasclaw app-server v2-shaped protocol 是否已有，结论：未发现单一 native 协议收敛计划；已有 P0-P3 计划仍保留 legacy aliases / native fields。"
```

## Task 7: Update Documentation to the Single-Protocol Model

**Files:**
- Modify: `docs/plans/dasclaw-app-server-protocol-v0.md`
- Modify: `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`
- Modify: `docs/plans/dasclaw-codex-app-server-ai-sdk-compat-plan.md`

- [ ] **Step 1: Update the protocol v0 design note**

In `docs/plans/dasclaw-app-server-protocol-v0.md`, replace wording that implies “native plus v2 compatibility layer” with this statement:

```markdown
Dasclaw app-server exposes one native protocol. For the implemented chat-session surface, that native protocol intentionally uses Codex app-server v2-shaped method names, event names, and payload fields where Dasclaw can support the semantics truthfully. The compatibility profile is descriptive metadata for this subset; it is not a second wire layer and does not add aliases.
```

- [ ] **Step 2: Update the gap matrix phase framing**

In `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`, add this near the phase summary:

```markdown
Protocol-shape consolidation belongs before capability expansion. The first implementation phase should remove dev-stage aliases and make the existing native chat-session surface v2-shaped. Missing Codex app-server capabilities remain explicit gaps; they are not unlocked by a translation layer.
```

Then update rows that mention `thread/create`, `turn/cancel`, `turn/delta`, `turn/failed`, and `turn/cancelled`:

```markdown
Legacy Dasclaw smoke-surface aliases are intentionally unsupported in the new native contract. The public names are `thread/start`, `turn/start`, `turn/interrupt`, item delta events, and `turn/completed` with nested `turn`.
```

- [ ] **Step 3: Update the AI SDK compatibility plan**

In `docs/plans/dasclaw-codex-app-server-ai-sdk-compat-plan.md`, replace any wording that says Dasclaw offers a separate Codex profile with:

```markdown
For AI SDK integration, consume the Dasclaw app-server native protocol as a Codex app-server v2-shaped chat-session subset. Do not rely on legacy aliases or response-level `threadId` / `turnId`; read `thread.id` and `turn.id`.
```

- [ ] **Step 4: Run documentation searches**

Run:

```bash
rg -n "native \\+ v2|legacy alias|legacy aliases|thread/create|turn/cancel|turn/delta|turn/failed|turn/cancelled|threadId.*turnId" docs/plans docs/superpowers/plans
```

Expected:

- This new plan may mention legacy names as things to remove.
- Updated docs should not present legacy aliases as the desired future surface.
- Historical plans may still mention old names; do not rewrite historical plan documents unless their status is ambiguous to future implementers.

- [ ] **Step 5: Commit documentation changes**

```bash
git add docs/plans/dasclaw-app-server-protocol-v0.md \
  docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md \
  docs/plans/dasclaw-codex-app-server-ai-sdk-compat-plan.md \
  docs/superpowers/plans/2026-06-19-dasclaw-app-server-v2-shaped-protocol.md
git commit -m "docs(app-server): plan single v2-shaped native protocol" \
  -m "已检查 Dasclaw app-server v2-shaped protocol 是否已有，结论：未发现单一 native 协议收敛计划；已有 P0-P3 计划仍保留 legacy aliases / native fields。"
```

## Task 8: Whole-Slice Verification and Cleanup

**Files:**
- Verify the whole changed slice.

- [ ] **Step 1: Search for removed public wire names**

Run:

```bash
rg -n "method::THREAD_CREATE|method::TURN_CANCEL|event::THREAD_CREATED|event::TURN_DELTA|event::TURN_FAILED|event::TURN_CANCELLED|thread/create|turn/cancel|turn/delta|turn/failed|turn/cancelled" \
  crates/dasclaw_app_server_protocol/src/lib.rs \
  crates/dasclaw_app_server/src/lib.rs \
  crates/dasclaw_app_server_client/src/lib.rs \
  desktop-app/src
```

Expected:

- No matches in runtime code.
- Matches may exist only in explicit negative tests that assert old names are rejected.

- [ ] **Step 2: Search for response-level native ID fallbacks**

Run:

```bash
rg -n "record\\.threadId|record\\.turnId|\\[\"threadId\"\\]|\\[\"turnId\"\\]|thread_id:.*ThreadStartResponse|turn_id:.*TurnStartResponse" \
  crates/dasclaw_app_server_protocol/src/lib.rs \
  crates/dasclaw_app_server/src/lib.rs \
  crates/dasclaw_app_server_client/src/lib.rs \
  desktop-app/src
```

Expected:

- No response parsing fallback in desktop runtime.
- `threadId` and `turnId` may still appear inside request params and event params where v2 uses those IDs.

- [ ] **Step 3: Run Rust checks**

Run:

```bash
cargo check -p dasclaw_app_server_protocol --tests
cargo check -p dasclaw_app_server --tests
cargo check -p dasclaw_app_server_client --tests
cargo nextest run -p dasclaw_app_server_protocol -p dasclaw_app_server -p dasclaw_app_server_client
```

Expected:

- All three crates pass check and nextest.

- [ ] **Step 4: Run formatting and panic guard**

Run:

```bash
cargo fmt --all && python3.12 scripts/check_no_panics.py --base origin/xClaw
```

Expected:

- Formatting succeeds.
- Panic guard reports no newly introduced panic paths.

- [ ] **Step 5: Run crate-local clippy**

Run:

```bash
cargo clippy --no-deps -p dasclaw_app_server_protocol --all-targets -- -D warnings
cargo clippy --no-deps -p dasclaw_app_server --all-targets -- -D warnings
cargo clippy --no-deps -p dasclaw_app_server_client --all-targets -- -D warnings
```

Expected:

- No warnings in touched crates.

- [ ] **Step 6: Run desktop checks**

Run the desktop vitest checks:

```bash
npm --prefix desktop-app test -- appServerTurnTracker
npm --prefix desktop-app test -- appServerManager
npm --prefix desktop-app test -- appServerRpc
npm --prefix desktop-app test -- assistantMessages
npm --prefix desktop-app run typecheck
npm --prefix desktop-app test
```

Expected:

- Targeted desktop tests, typecheck, and full vitest pass.

- [ ] **Step 7: Final review search**

Run:

```bash
rg -n "codex_app_server_v2|codex_app_server_v2_chat_session_subset|CompatibilityAlias|aliases|thread/create|turn/cancel|turn/delta|turn/failed|turn/cancelled" \
  crates/dasclaw_app_server_protocol/src/lib.rs \
  crates/dasclaw_app_server/src/lib.rs \
  crates/dasclaw_app_server_client/src/lib.rs \
  desktop-app/src \
  docs/plans
```

Expected:

- `codex_app_server_v2_chat_session_subset` appears as descriptive profile metadata.
- `CompatibilityAlias` may appear only as a schema field type with empty `aliases`, or be removed entirely.
- Legacy method/event strings appear only in negative tests or historical plan context.

- [ ] **Step 8: Commit final cleanup changes**

```bash
git add crates/dasclaw_app_server_protocol/src/lib.rs \
  crates/dasclaw_app_server/src/lib.rs \
  crates/dasclaw_app_server_client/src/lib.rs \
  desktop-app/src \
  docs/plans \
  docs/superpowers/plans/2026-06-19-dasclaw-app-server-v2-shaped-protocol.md
git commit -m "chore(app-server): finish v2-shaped native protocol cleanup" \
  -m "已检查 Dasclaw app-server v2-shaped protocol 是否已有，结论：未发现单一 native 协议收敛计划；已有 P0-P3 计划仍保留 legacy aliases / native fields。"
```

## Acceptance Criteria

- `protocol/schema` does not advertise `thread/create`, `turn/cancel`, `thread/created`, `turn/delta`, `turn/failed`, or `turn/cancelled`.
- `compatibilityProfiles[].aliases` is empty for the v2-shaped subset profile.
- `codex_v2_compat_enabled` or equivalent runtime profile switch no longer controls notification emission.
- `thread/start` accepts only the supported v2-shaped params in this phase, starting with optional `cwd`; it does not accept `title` or `workspaceRoot`.
- `thread/start` returns a nested `thread` object and no response-level `threadId`.
- `turn/start` accepts `input: [{ type: "text", text, textElements: [] }]` and does not require the old string `input` or `prompt` shape.
- `turn/start` returns a nested `turn` object and no response-level `turnId`, `status`, or `lifecycle`.
- `turn/interrupt` returns `{}` and reports interruption through `turn/completed` with nested `turn.status = "interrupted"`.
- `thread/list` and `thread/turns/list` return `data`, `nextCursor`, and `backwardsCursor`; they do not return `threads` or `turns`.
- `thread/started`, `turn/started`, `turn/completed`, `item/started`, and `item/completed` carry the v2-shaped object fields described in this plan.
- Desktop runtime does not fall back to response-level `threadId` or `turnId`.
- Desktop tests, shared app-server types, typecheck, and full vitest all pass.
- Client crate tests use nested `thread`, `turn`, and `item` fixtures.
- Docs explain that Dasclaw has one native protocol shaped as a Codex app-server v2 chat-session subset.
- Missing Codex app-server capabilities remain documented gaps and are not implied by the profile.

## Risks and Guardrails

- **Risk: accidental overclaiming.** Do not add unsupported Codex response fields just because they exist upstream. Only include fields that Dasclaw can fill truthfully.
- **Risk: hidden desktop fixture dependence.** Use `rg` after desktop changes because old `threadId`/`turnId` fixtures may live outside the main hook tests.
- **Risk: confusing profile naming.** Use `codex_app_server_v2_chat_session_subset`, not `codex_app_server_v2`, so users do not infer full compatibility.
- **Risk: tests locking fake semantics.** The tests should assert shape and implemented behavior, not full Codex capabilities that Dasclaw does not have.

## Notes for Implementers

- This is a breaking change to the dev-stage app-server protocol. That is intentional.
- Keep internal Rust helper names boring and local. The important contract is the JSON-RPC wire shape.
- Avoid a translation layer. Implemented features that map naturally to Codex v2 shape should make the native protocol type have that shape directly.
- For any newly added public method/event in a future phase, first check Codex app-server v2. Add an equivalent v2-shaped method/event directly to the native protocol when Dasclaw can support its semantics.
