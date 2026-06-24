# Dasclaw App Server R6 Config Repo Search Owner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the full R6 owner slice for Dasclaw app-server: broad config read/write, repo diff, fuzzy file search sessions, conversation summary, review start, model reroute/verification notifications, and hook/warning notifications.

**Architecture:** Extend `dasclaw_app_server_protocol` first so every R6 method and event has a typed contract, schema entry, and advertised capability. Then add focused app-server services that reuse existing Dasclaw foundations: `dasclaw_fs_tools` for file traversal rules, `dasclaw_git_tools` for read-only git process handling, `ThreadLifecycleHost` for summaries/reviews, existing model-provider state for model events, and `dasclaw_hooks` for hook run summaries. Keep R6 as an umbrella milestone, but implement it as independent testable slices so each service can be reviewed and reverted separately.

**Tech Stack:** Rust 2024, serde/serde_json, tokio current-thread blocking worker pattern already used by app-server services, existing Dasclaw crates (`dasclaw_app_server_protocol`, `dasclaw_app_server`, `dasclaw_fs_tools`, `dasclaw_git_tools`, `dasclaw_hooks`, `dasclaw_core`), cargo nextest, cargo fmt, project panic scan.

## Post-Review Status

Independent review on 2026-06-24 found that the config, repo diff, fuzzy search, conversation summary, and review-start owner slices are implemented and tested, but the model and hook/warning notification slices are only protocol/drain wiring so far. `model/rerouted` / `model/verification` still need a real app-server producer and readiness advertisement before they can be marked complete. `hook/started` / `hook/completed` / warning events still need a real `dasclaw_hooks::HookRegistry` or warning source integration before they can be advertised as implemented. The gap matrix therefore strikes through only the completed R6 subitems and keeps those producer follow-ups open.

## 2026-06-24 Execution Update After Producer Review

Task 9 through Task 12 were implemented conservatively and then re-reviewed for fake readiness and client-bug masking:

- Completed and advertised: config read/write/batch owner, read-only repo diff, path fuzzy search sessions, deterministic conversation summary, review-start routing, HookRegistry `beforeToolCall` -> app-server `preToolUse` started/completed notifications, and config/deprecation warning producers. Config/deprecation warnings are delivered both during initialize and after runtime `config/read` / `config/batchWrite` paths.
- Implemented only as delivery, not advertised as real producers: `model/rerouted`, `model/verification`, generic `warning`, `guardianWarning`, and HookRegistry lifecycle points other than `preToolUse`.
- Correctness correction: provider `actual_model` metadata is preserved, but app-server no longer infers `model/rerouted(reason=highRiskCyberActivity)` from a plain requested/actual model mismatch. A trustworthy reroute reason must come from a real provider/runtime source before readiness can advertise `model/rerouted`.
- Correctness correction: startup sandbox `warning` is not advertised or emitted from a hard-coded `ReadOnly { network_access: false }` policy. It must be wired to the actual app-server sandbox policy before it can be marked complete.

The R6 row in `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md` is therefore intentionally partial, not fully struck through. This is deliberate: protocol shells and injected test queues do not count as completed producer capability.

## 2026-06-24 Codex Producer Reference Supplement

This supplement records the evidence gathered from `/Users/nallylin/Documents/code/x-claw/codex-cli-main` before extending the remaining R6 plan. The important distinction is producer versus delivery:

- Delivery means app-server can turn an already-created internal event into a JSON-RPC notification.
- Producer means real runtime, provider, hook, startup, or guardian code creates that event during normal product use.

R6 currently has delivery for model/hook/warning notifications. The remaining work is the producer side.

Verification evidence:

| Level | Evidence | Result |
|---|---|---|
| Semantic | `semantic_search_nodes_tool` on `/Users/nallylin/Documents/code/x-claw/codex-cli-main` for "model rerouted verification hook started completed guardian warning config warning deprecation notice app server notification producer" | Hit Codex tests and serializers including `model_verification_emits_structured_event_without_reroute_or_warning`, `verify_model_rerouted_notification_serialization`, and `model_verification_emits_typed_notification_and_warning_v2`. |
| Semantic | `semantic_search_nodes_tool` on `/Users/nallylin/Documents/code/x-claw/crates` for "dasclaw app server model reroute verification hook started completed warning config warning producer service health notification" | Hit weak/noisy Dasclaw matches only, so symbol and literal checks were required before making remaining-work claims. |
| Symbol | LSP `document_symbols` for `crates/dasclaw_app_server/src/hook_service.rs` | `AppServerHookNotification` and `AppServerHookService` exist, but `health()` returns disabled with message `"hook notification producer is not wired"`. |
| Symbol | LSP `document_symbols` for `crates/dasclaw_app_server/src/lib.rs` | `RuntimeTurnUpdateSink`, `RuntimeTurnOutcome::ModelRerouted`, and `RuntimeTurnOutcome::ModelVerification` exist as app-server update delivery plumbing. |
| Literal | `rg` in Dasclaw app-server protocol | `model/rerouted`, `model/verification`, `hook/started`, `hook/completed`, `warning`, `guardianWarning`, `configWarning`, and `deprecationNotice` typed notifications and constructors already exist. |
| Literal | `rg` in Dasclaw runtime/provider/core | Provider responses keep an actual response model in `MessageResponse`, but `ToolCompletionResponse`, `ResponseMetadata`, and `AgentRunOutput` drop it before app-server can compare requested model versus actual model. |

Codex reference implementation:

- `codex-rs/core/src/session/turn.rs` handles `ResponseEvent::ServerModel(server_model)` and `ResponseEvent::ModelVerifications(verifications)` during the real response stream.
- `codex-rs/core/src/session/mod.rs` compares requested model versus server model in `maybe_warn_on_server_model_mismatch`, emits `EventMsg::ModelReroute`, and also emits a user-visible `Warning`.
- `codex-rs/core/src/session/mod.rs` emits `EventMsg::ModelVerification` from the streamed verification data.
- `codex-rs/core/src/hook_runtime.rs` emits `EventMsg::HookStarted` before hook execution and `EventMsg::HookCompleted` after actual hook results.
- `codex-rs/core/src/guardian/review.rs` emits guardian warnings from real denial/timeout/review paths.
- `codex-rs/app-server/src/bespoke_event_handling.rs` maps those core `EventMsg` values to app-server `ServerNotification` values. The app-server is a bridge for model, hook, warning, guardian warning, and deprecation events.
- `codex-rs/app-server/src/lib.rs` and `codex-rs/app-server/src/message_processor.rs` collect startup/config warnings and send them during connection initialization. Codex config warnings are app-server startup/init producer work, not a core `EventMsg::ConfigWarning`.

Dasclaw state after review:

- `crates/dasclaw_app_server_protocol/src/lib.rs` has the R6 app-server notification methods, DTOs, schemas, and `ServerNotification` constructors.
- `crates/dasclaw_app_server/src/lib.rs` has `RuntimeTurnUpdateSink` and drains model updates into JSON-RPC notifications only for active turns.
- `crates/dasclaw_app_server/src/hook_service.rs` has an in-memory queue and drain path, but deliberately advertises disabled health because no real producer is wired.
- `crates/dasclaw_llm_provider/src/providers/openai_compat.rs` normalizes a provider response model, and `crates/dasclaw_llm_provider/src/types.rs` stores it in `MessageResponse`.
- `crates/dasclaw_runtime/src/llm_adapter.rs`, `crates/dasclaw_core/src/response_types.rs`, `crates/dasclaw_core/src/agentic_loop.rs`, and `crates/dasclaw_runtime/src/agent.rs` lose that model metadata before app-server sees the completed turn.

Plan consequence: Task 7 and Task 8 below are historical delivery tasks. They are useful foundations, but R6 cannot be closed until Task 9 through Task 12 add real producers, readiness gates, and non-fake-only tests.

---

## Start Gate

Project 4-question gate:

1. New file/module? Yes. This plan creates app-server service files and fs/git helper files during execution.
2. Negative claims? Yes. It states which R6 app-server surfaces are absent today.
3. Cross-project/protocol reconciliation? Yes. It compares Dasclaw app-server with Codex app-server protocol shapes.
4. Architecture reconciliation document? Yes. This is a plan for protocol gap R6.

Required evidence was collected before this plan. This table records the original pre-implementation state; the 2026-06-24 supplement above supersedes the model/hook/warning rows for the remaining producer work.

| R6 area | Existing Dasclaw evidence | Missing app-server surface evidence | Plan consequence |
|---|---|---|---|
| Config | `configRequirements/read` exists for sandbox requirements in `crates/dasclaw_app_server_protocol/src/lib.rs`; config types exist in `crates/dasclaw_protocol/src/config_types.rs`. | LSP search for `ConfigReadParams` did not find a Dasclaw app-server symbol; `rg` did not find `config/read`, `config/value/write`, or `config/batchWrite` in app-server protocol. | Add a Dasclaw-owned config service with a narrow write allowlist and version checks. |
| Repo diff | LSP found `GitDiffTool` in `crates/dasclaw_git_tools/src/diff.rs`; `dasclaw_git_tools/src/runner.rs` already executes git with timeout and no terminal prompt. | `rg` found no `gitDiffToRemote` method in Dasclaw app-server. | Add a public read-only git helper plus app-server repo service. |
| Fuzzy file search | LSP found `GlobSearchTool`; `rg` found `GrepSearchTool`; `dasclaw_workspace_cap` has workspace search. | LSP search for `FuzzyFileSearchParams` returned no Dasclaw app-server symbol; Codex shape is filename/path fuzzy search with session notifications. | Add path-fuzzy search helper in `dasclaw_fs_tools` and app-server search service/session events. |
| Conversation summary | LSP found `ThreadSummary` in `crates/dasclaw_app_server/src/thread_lifecycle.rs`; semantic search found `dasclaw_core::compaction::{compact_with_summary, generate_summary}`. | `rg` found no `getConversationSummary` app-server method. | Implement deterministic conversation card summary from thread lifecycle first; keep LLM compaction out of the R6 base path. |
| Review start | Codex reference shape is `review/start` with `ReviewTarget` and `ReviewDelivery`; Dasclaw already has `turn_start` and `thread_fork`. | LSP search for `ReviewStartParams` did not find a Dasclaw app-server symbol; `CodexThreadItem` has only `agentMessage` and `reasoning`. | Add review protocol types, review item variants, and route review through existing turn execution. |
| Model events | `ModelProviderState` exists in app-server; `dasclaw_protocol::protocol::ModelVerificationEvent` exists. | App-server protocol has no `model/rerouted` or `model/verification` notification constants. | Add app-server notification types and emit them from runtime/model update paths. |
| Hooks and warnings | `dasclaw_hooks::HookRegistry` exists; `dasclaw_protocol::protocol::{HookStartedEvent, HookCompletedEvent}` exists. | App-server protocol has no `hook/started`, `hook/completed`, `warning`, `guardianWarning`, `configWarning`, or `deprecationNotice`. | Add hook/warning event types and drain them through app-server notifications. |

Commit messages for execution slices that make these additions must include:

```text
已检查 R6 app-server owner 是否已有，结论：底座存在于 fs/git/hooks/core/model 等 crate，R6 对外 app-server surface 仍需新增
```

## Scope Note

R6 spans seven product surfaces. Under the writing-plans scope rule, this would normally be split into smaller plans. The user explicitly requested a complete R6 plan in one document, so this file is an umbrella plan with separable tasks. Each task below produces working software and has its own focused tests.

## File Structure

Protocol:

- Modify `crates/dasclaw_app_server_protocol/src/lib.rs`
  - Add R6 method/event constants.
  - Add R6 capability fields and service availability structs.
  - Add request/response/notification DTOs.
  - Add schema entries and roundtrip tests.

Git helper:

- Modify `crates/dasclaw_git_tools/src/lib.rs`
  - Export read-only git helper.
- Create `crates/dasclaw_git_tools/src/read_only.rs`
  - Expose only read-only git commands needed by app-server.
  - Reject command names outside the R6 allowlist.

Filesystem helper:

- Modify `crates/dasclaw_fs_tools/src/lib.rs`
  - Export path fuzzy search helper.
- Create `crates/dasclaw_fs_tools/src/path_search.rs`
  - Reuse the glob/grep traversal discipline: depth cap, deterministic sort, hidden/large-directory skip, workspace-root confinement.
  - Return Codex-shaped fuzzy file results without importing Codex crates.

App-server services:

- Modify `crates/dasclaw_app_server/Cargo.toml`
  - Add path dependencies on `dasclaw_git_tools` and `dasclaw_hooks`.
- Modify `crates/dasclaw_app_server/src/lib.rs`
  - Add route branches.
  - Add model/runtime update notification emission.
  - Add review-start orchestration.
  - Add summary mapping.
- Modify `crates/dasclaw_app_server/src/app_services.rs`
  - Add service traits, real service construction, noop services, and test fakes for config, repo, search, hooks, and warnings.
- Create `crates/dasclaw_app_server/src/config_service.rs`
  - Own Dasclaw app-server config read/write policy.
- Create `crates/dasclaw_app_server/src/repo_service.rs`
  - Own `gitDiffToRemote`.
- Create `crates/dasclaw_app_server/src/search_service.rs`
  - Own `fuzzyFileSearch` and session notifications.
- Create `crates/dasclaw_app_server/src/hook_service.rs`
  - Own app-server hook run notification drain and warning drain.
- Modify `crates/dasclaw_app_server/src/thread_lifecycle.rs`
  - Add review item persistence and summary helper support.

Docs:

- Modify `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`
  - Mark R6 as planned or completed according to the execution state.
  - Record exact reused Dasclaw foundations.

## R6 Contract

### Methods

- `config/read`
- `config/value/write`
- `config/batchWrite`
- `gitDiffToRemote`
- `fuzzyFileSearch`
- `getConversationSummary`
- `review/start`

### Notifications

- `fuzzyFileSearch/sessionUpdated`
- `fuzzyFileSearch/sessionCompleted`
- `model/rerouted`
- `model/verification`
- `hook/started`
- `hook/completed`
- `warning`
- `guardianWarning`
- `configWarning`
- `deprecationNotice`

### Dasclaw Config Policy

`config/read` returns a Dasclaw-compatible config object shaped like Codex v2 where useful, but Dasclaw owns the writable key policy.

Writable keys:

```rust
const WRITABLE_CONFIG_KEYS: &[&str] = &[
    "model",
    "review_model",
    "model_provider",
    "approval_policy",
    "sandbox_mode",
    "model_reasoning_effort",
    "model_reasoning_summary",
    "profile",
    "instructions",
    "developer_instructions",
];
```

Non-writable keys:

```rust
const NON_WRITABLE_CONFIG_KEYS: &[&str] = &[
    "api_key",
    "forced_chatgpt_workspace_id",
    "forced_login_method",
    "mcp_servers",
    "experimental_features",
    "analytics",
];
```

Persistence:

- User config file: `<service-root>/.dasclaw/app-server-config.json`
- Session layer: selected model and initialize-time model provider snapshot.
- Project layer: same file resolved from request `cwd` if `cwd` is inside service root.
- Secrets: never returned; values under keys containing `key`, `secret`, `token`, or `password` are replaced with `"<redacted>"` in config responses.

Errors:

- Unknown writable key: JSON-RPC error with `ErrorCode::InvalidParams`, capability `config`.
- Version mismatch: JSON-RPC error with `ErrorCode::InvalidParams`, capability `config`, message `config version mismatch`.
- File outside service root: JSON-RPC error with `ErrorCode::CapabilityUnavailable`, capability `config`.

## Task 1: Add R6 Protocol Contract

**Files:**

- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`

- [ ] **Step 1: Add failing protocol schema test**

Add this test to the existing `#[cfg(test)] mod tests` in `crates/dasclaw_app_server_protocol/src/lib.rs`:

```rust
#[test]
fn r6_protocol_schema_lists_config_repo_search_review_and_warning_contracts() {
    let schema = ProtocolSchemaResponse::current();
    let methods: std::collections::BTreeSet<_> =
        schema.methods.iter().map(|entry| entry.method.as_str()).collect();
    for method in [
        method::CONFIG_READ,
        method::CONFIG_VALUE_WRITE,
        method::CONFIG_BATCH_WRITE,
        method::GIT_DIFF_TO_REMOTE,
        method::FUZZY_FILE_SEARCH,
        method::GET_CONVERSATION_SUMMARY,
        method::REVIEW_START,
    ] {
        assert!(methods.contains(method), "missing R6 method {method}");
    }

    let events: std::collections::BTreeSet<_> =
        schema.events.iter().map(|entry| entry.event.as_str()).collect();
    for event in [
        event::FUZZY_FILE_SEARCH_SESSION_UPDATED,
        event::FUZZY_FILE_SEARCH_SESSION_COMPLETED,
        event::MODEL_REROUTED,
        event::MODEL_VERIFICATION,
        event::HOOK_STARTED,
        event::HOOK_COMPLETED,
        event::WARNING,
        event::GUARDIAN_WARNING,
        event::CONFIG_WARNING,
        event::DEPRECATION_NOTICE,
    ] {
        assert!(events.contains(event), "missing R6 event {event}");
    }
}
```

- [ ] **Step 2: Run protocol test and confirm failure**

Run:

```bash
cargo nextest run -p dasclaw_app_server_protocol r6_protocol_schema_lists_config_repo_search_review_and_warning_contracts
```

Expected:

```text
FAIL missing R6 method config/read
```

- [ ] **Step 3: Add R6 method and event constants**

Add these constants beside the existing method/event constants:

```rust
pub mod method {
    pub const CONFIG_READ: &str = "config/read";
    pub const CONFIG_VALUE_WRITE: &str = "config/value/write";
    pub const CONFIG_BATCH_WRITE: &str = "config/batchWrite";
    pub const GIT_DIFF_TO_REMOTE: &str = "gitDiffToRemote";
    pub const FUZZY_FILE_SEARCH: &str = "fuzzyFileSearch";
    pub const GET_CONVERSATION_SUMMARY: &str = "getConversationSummary";
    pub const REVIEW_START: &str = "review/start";
}

pub mod event {
    pub const FUZZY_FILE_SEARCH_SESSION_UPDATED: &str = "fuzzyFileSearch/sessionUpdated";
    pub const FUZZY_FILE_SEARCH_SESSION_COMPLETED: &str = "fuzzyFileSearch/sessionCompleted";
    pub const MODEL_REROUTED: &str = "model/rerouted";
    pub const MODEL_VERIFICATION: &str = "model/verification";
    pub const HOOK_STARTED: &str = "hook/started";
    pub const HOOK_COMPLETED: &str = "hook/completed";
    pub const WARNING: &str = "warning";
    pub const GUARDIAN_WARNING: &str = "guardianWarning";
    pub const CONFIG_WARNING: &str = "configWarning";
    pub const DEPRECATION_NOTICE: &str = "deprecationNotice";
}
```

Merge these into the existing modules rather than creating duplicate `pub mod method` or `pub mod event` blocks.

- [ ] **Step 4: Add R6 capability fields**

Extend `CapabilityMatrix`:

```rust
pub struct CapabilityMatrix {
    pub protocol: Capability,
    pub lifecycle: Capability,
    pub health: Capability,
    pub session: Capability,
    pub approval: Capability,
    pub dlp_policy: Capability,
    pub model_provider: Capability,
    pub tools: Capability,
    pub jobs: Capability,
    pub skills: Capability,
    pub mcp: Capability,
    pub sandbox: Capability,
    pub logs: Capability,
    pub filesystem: Capability,
    pub command_exec: Capability,
    pub thread_lifecycle: Capability,
    pub thread_goal: Capability,
    pub thread_compact: Capability,
    pub config: Capability,
    pub repo: Capability,
    pub search: Capability,
    pub review: Capability,
    pub hooks: Capability,
    pub warnings: Capability,
}
```

Initialize each new field in `phase_one()` using `declared_future_capability`.

- [ ] **Step 5: Add R6 service availability**

Extend `AppServerServiceAvailability`:

```rust
pub struct AppServerServiceAvailability {
    pub logs: bool,
    pub jobs: bool,
    pub skills: bool,
    pub mcp: McpServiceAvailability,
    pub p5: AppServerP5Availability,
    pub r6: AppServerR6Availability,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AppServerR6Availability {
    pub config: bool,
    pub repo: bool,
    pub search: bool,
    pub review: bool,
    pub model_events: bool,
    pub hooks: bool,
    pub warnings: bool,
}
```

In `with_app_services`, advertise implemented R6 capabilities when these booleans are true.

- [ ] **Step 6: Extend service names for R6 health**

Extend `ServiceName` with R6 service variants:

```rust
pub enum ServiceName {
    Protocol,
    Lifecycle,
    Session,
    Runtime,
    DlpPolicy,
    ModelProvider,
    Tools,
    Sandbox,
    Jobs,
    Skills,
    Mcp,
    Logs,
    Filesystem,
    CommandExec,
    Config,
    Repo,
    Search,
    Hooks,
}
```

- [ ] **Step 7: Add DTOs**

Add protocol DTOs with serde camelCase naming. Use `serde_json::Value` for Codex config's open-ended object values.

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigReadParams {
    pub include_layers: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigReadResponse {
    pub config: serde_json::Value,
    pub origins: std::collections::BTreeMap<String, ConfigLayerMetadata>,
    pub layers: Option<Vec<ConfigLayer>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigLayerMetadata {
    pub name: ConfigLayerSource,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigLayer {
    pub name: ConfigLayerSource,
    pub version: String,
    pub config: serde_json::Value,
    pub disabled_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ConfigLayerSource {
    System { file: String },
    User { file: String },
    Project { dot_dasclaw_folder: String },
    SessionFlags,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigValueWriteParams {
    pub key_path: String,
    pub value: serde_json::Value,
    pub merge_strategy: MergeStrategy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MergeStrategy {
    Replace,
    Upsert,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigBatchWriteParams {
    pub edits: Vec<ConfigEdit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reload_user_config: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigEdit {
    pub key_path: String,
    pub value: serde_json::Value,
    pub merge_strategy: MergeStrategy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigWriteResponse {
    pub config: serde_json::Value,
    pub version: String,
}
```

Add the remaining DTOs in the same protocol file:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitDiffToRemoteParams {
    pub cwd: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitDiffToRemoteResponse {
    pub sha: String,
    pub diff: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FuzzyFileSearchParams {
    pub query: String,
    pub roots: Vec<String>,
    pub cancellation_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FuzzyFileSearchResponse {
    pub files: Vec<FuzzyFileSearchResult>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FuzzyFileSearchResult {
    pub root: String,
    pub path: String,
    pub match_type: FuzzyFileSearchMatchType,
    pub file_name: String,
    pub score: f64,
    pub indices: Option<Vec<usize>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FuzzyFileSearchMatchType {
    File,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FuzzyFileSearchSessionUpdatedNotification {
    pub session_id: String,
    pub query: String,
    pub files: Vec<FuzzyFileSearchResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FuzzyFileSearchSessionCompletedNotification {
    pub session_id: String,
}
```

Add summary/review/model/hook/warning DTOs:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GetConversationSummaryParams {
    RolloutPath { rollout_path: String },
    ConversationId { conversation_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetConversationSummaryResponse {
    pub summary: ConversationSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSummary {
    pub conversation_id: String,
    pub path: String,
    pub preview: String,
    pub timestamp: Option<String>,
    pub updated_at: Option<String>,
    pub model_provider: String,
    pub cwd: String,
    pub cli_version: String,
    pub source: CodexSessionSource,
    pub git_info: Option<CodexGitInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewStartParams {
    pub thread_id: String,
    pub target: ReviewTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivery: Option<ReviewDelivery>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ReviewTarget {
    UncommittedChanges,
    BaseBranch { branch: String },
    Commit { sha: String, title: Option<String> },
    Custom { instructions: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReviewDelivery {
    Inline,
    Detached,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewStartResponse {
    pub turn: CodexTurn,
    pub review_thread_id: String,
}
```

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelReroutedNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub from_model: String,
    pub to_model: String,
    pub reason: ModelRerouteReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelRerouteReason {
    HighRiskCyberActivity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelVerificationNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub verifications: Vec<ModelVerification>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelVerification {
    TrustedAccessForCyber,
}
```

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookStartedNotification {
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub run: HookRunSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookCompletedNotification {
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub run: HookRunSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookRunSummary {
    pub id: String,
    pub event_name: HookEventName,
    pub handler_type: HookHandlerType,
    pub execution_mode: HookExecutionMode,
    pub scope: HookScope,
    pub source_path: String,
    pub source: HookSource,
    pub display_order: u64,
    pub status: HookRunStatus,
    pub status_message: Option<String>,
    pub started_at: u64,
    pub completed_at: Option<u64>,
    pub duration_ms: Option<u64>,
    pub entries: Vec<HookOutputEntry>,
}
```

Define hook enums with the exact string names from the Codex reference:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HookEventName {
    PreToolUse,
    PermissionRequest,
    PostToolUse,
    SessionStart,
    UserPromptSubmit,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookExecutionMode {
    Sync,
    Async,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookHandlerType {
    Command,
    Prompt,
    Agent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookScope {
    Thread,
    Turn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HookSource {
    System,
    User,
    Project,
    Mdm,
    SessionFlags,
    LegacyManagedConfigFile,
    LegacyManagedConfigMdm,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookRunStatus {
    Running,
    Completed,
    Failed,
    Blocked,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookOutputEntry {
    pub kind: HookOutputEntryKind,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookOutputEntryKind {
    Warning,
    Stop,
    Feedback,
    Context,
    Error,
}
```

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WarningNotification {
    pub thread_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardianWarningNotification {
    pub thread_id: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigWarningNotification {
    pub summary: String,
    pub details: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<TextRange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextRange {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeprecationNoticeNotification {
    pub summary: String,
    pub details: Option<String>,
}
```

- [ ] **Step 8: Add notification constructors**

Add `ServerNotification` constructor methods for every R6 event:

```rust
pub fn fuzzy_file_search_session_updated(
    event: FuzzyFileSearchSessionUpdatedNotification,
) -> Result<Self, serde_json::Error> {
    Self::new(event::FUZZY_FILE_SEARCH_SESSION_UPDATED, event)
}

pub fn fuzzy_file_search_session_completed(
    event: FuzzyFileSearchSessionCompletedNotification,
) -> Result<Self, serde_json::Error> {
    Self::new(event::FUZZY_FILE_SEARCH_SESSION_COMPLETED, event)
}

pub fn model_rerouted(event: ModelReroutedNotification) -> Result<Self, serde_json::Error> {
    Self::new(event::MODEL_REROUTED, event)
}

pub fn model_verification(event: ModelVerificationNotification) -> Result<Self, serde_json::Error> {
    Self::new(event::MODEL_VERIFICATION, event)
}

pub fn hook_started(event: HookStartedNotification) -> Result<Self, serde_json::Error> {
    Self::new(event::HOOK_STARTED, event)
}

pub fn hook_completed(event: HookCompletedNotification) -> Result<Self, serde_json::Error> {
    Self::new(event::HOOK_COMPLETED, event)
}

pub fn warning(event: WarningNotification) -> Result<Self, serde_json::Error> {
    Self::new(event::WARNING, event)
}

pub fn guardian_warning(event: GuardianWarningNotification) -> Result<Self, serde_json::Error> {
    Self::new(event::GUARDIAN_WARNING, event)
}

pub fn config_warning(event: ConfigWarningNotification) -> Result<Self, serde_json::Error> {
    Self::new(event::CONFIG_WARNING, event)
}

pub fn deprecation_notice(event: DeprecationNoticeNotification) -> Result<Self, serde_json::Error> {
    Self::new(event::DEPRECATION_NOTICE, event)
}
```

- [ ] **Step 9: Run protocol tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server_protocol r6_protocol_schema_lists_config_repo_search_review_and_warning_contracts
cargo nextest run -p dasclaw_app_server_protocol
```

Expected:

```text
PASS r6_protocol_schema_lists_config_repo_search_review_and_warning_contracts
```

- [ ] **Step 10: Commit protocol contract**

Run:

```bash
git add crates/dasclaw_app_server_protocol/src/lib.rs
git commit -m "feat(app-server): add R6 protocol contracts" -m "已检查 R6 app-server owner 是否已有，结论：底座存在于 fs/git/hooks/core/model 等 crate，R6 对外 app-server surface 仍需新增"
```

## Task 2: Add Config Service Owner

**Files:**

- Create: `crates/dasclaw_app_server/src/config_service.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Modify: `crates/dasclaw_app_server/src/app_services.rs`

- [ ] **Step 1: Add failing config route tests**

Add tests to `crates/dasclaw_app_server/src/lib.rs`:

```rust
#[test]
fn config_read_returns_redacted_config_and_layers() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut server = initialized_server_with_root(temp.path());
    let response = server.handle_json_rpc(
        r#"{"jsonrpc":"2.0","id":10,"method":"config/read","params":{"includeLayers":true}}"#,
    ).expect("response");
    let value: serde_json::Value = serde_json::from_str(&response).expect("json");
    assert_eq!(value["result"]["config"]["model"], serde_json::Value::Null);
    assert!(value["result"]["layers"].as_array().expect("layers").len() >= 1);
    assert!(!value.to_string().contains("api_key"));
}

#[test]
fn config_write_rejects_key_outside_policy() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut server = initialized_server_with_root(temp.path());
    let response = server.handle_json_rpc(
        r#"{"jsonrpc":"2.0","id":11,"method":"config/value/write","params":{"keyPath":"api_key","value":"secret","mergeStrategy":"replace"}}"#,
    ).expect("response");
    let value: serde_json::Value = serde_json::from_str(&response).expect("json");
    assert_eq!(value["error"]["data"]["capability"], "config");
    assert!(value["error"]["message"].as_str().expect("message").contains("not writable"));
}
```

Add this helper in the `lib.rs` test module before the R6 route tests:

```rust
fn initialized_server_with_root(root: &std::path::Path) -> AppServer {
    let services = app_services::AppServerServices::real_with_root_for_tests(root.to_path_buf());
    let mut server = AppServer::with_services_and_runtime_bridge(
        services,
        std::sync::Arc::new(NoopRuntimeBridge),
    );
    initialize_test_server(&mut server);
    server
}
```

- [ ] **Step 2: Run tests and confirm failure**

Run:

```bash
cargo nextest run -p dasclaw_app_server config_read_returns_redacted_config_and_layers config_write_rejects_key_outside_policy
```

Expected:

```text
FAIL method not found: config/read
```

- [ ] **Step 3: Implement config service**

Create `crates/dasclaw_app_server/src/config_service.rs`:

```rust
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use dasclaw_app_server_protocol::{
    ConfigBatchWriteParams, ConfigEdit, ConfigLayer, ConfigLayerMetadata, ConfigLayerSource,
    ConfigReadParams, ConfigReadResponse, ConfigValueWriteParams, ConfigWriteResponse,
    MergeStrategy, ServiceHealth, ServiceName,
};
use serde_json::{Map, Value};

use crate::AppServerError;
use crate::app_services::ConfigService;

const CONFIG_CAPABILITY: &str = "config";
const CONFIG_DIR: &str = ".dasclaw";
const CONFIG_FILE: &str = "app-server-config.json";

const WRITABLE_CONFIG_KEYS: &[&str] = &[
    "model",
    "review_model",
    "model_provider",
    "approval_policy",
    "sandbox_mode",
    "model_reasoning_effort",
    "model_reasoning_summary",
    "profile",
    "instructions",
    "developer_instructions",
];

pub struct AppServerConfigService {
    root: PathBuf,
    lock: Mutex<()>,
}

impl AppServerConfigService {
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self { root, lock: Mutex::new(()) }
    }

    fn default_file(&self) -> PathBuf {
        self.root.join(CONFIG_DIR).join(CONFIG_FILE)
    }

    fn resolve_file(&self, raw: Option<&str>) -> Result<PathBuf, AppServerError> {
        let path = raw.map(PathBuf::from).unwrap_or_else(|| self.default_file());
        let lexical = dasclaw_fs_tools::path_utils::normalize_lexical(&path);
        if !lexical.starts_with(&self.root) {
            return Err(AppServerError::capability_unavailable(
                CONFIG_CAPABILITY,
                "config file is outside service root",
            ));
        }
        Ok(lexical)
    }
}
```

Continue the same file with `read_json`, `write_json`, `version_for`, `apply_edit`, and `redact_secrets`:

```rust
fn read_json(path: &Path) -> Result<Value, AppServerError> {
    if !path.exists() {
        return Ok(Value::Object(Map::new()));
    }
    let data = fs::read_to_string(path).map_err(|error| {
        AppServerError::capability_unavailable(CONFIG_CAPABILITY, error.to_string())
    })?;
    serde_json::from_str(&data)
        .map_err(|error| AppServerError::invalid_request(CONFIG_CAPABILITY, error.to_string()))
}

fn write_json(path: &Path, value: &Value) -> Result<(), AppServerError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            AppServerError::capability_unavailable(CONFIG_CAPABILITY, error.to_string())
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        AppServerError::capability_unavailable(CONFIG_CAPABILITY, error.to_string())
    })?;
    fs::write(path, bytes).map_err(|error| {
        AppServerError::capability_unavailable(CONFIG_CAPABILITY, error.to_string())
    })
}

fn version_for(path: &Path) -> String {
    match fs::metadata(path) {
        Ok(metadata) => {
            let modified_ms = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| duration.as_millis())
                .unwrap_or(0);
            format!("{}-{}", metadata.len(), modified_ms)
        }
        Err(_) => "missing-0".to_string(),
    }
}

fn ensure_writable_key(key_path: &str) -> Result<(), AppServerError> {
    if WRITABLE_CONFIG_KEYS.contains(&key_path) {
        return Ok(());
    }
    Err(AppServerError::invalid_request(
        CONFIG_CAPABILITY,
        format!("config key is not writable by app-server: {key_path}"),
    ))
}

fn apply_edit(mut config: Value, edit: &ConfigEdit) -> Result<Value, AppServerError> {
    ensure_writable_key(&edit.key_path)?;
    let object = config.as_object_mut().ok_or_else(|| {
        AppServerError::invalid_request(CONFIG_CAPABILITY, "config root must be an object")
    })?;
    match edit.merge_strategy {
        MergeStrategy::Replace | MergeStrategy::Upsert => {
            object.insert(edit.key_path.clone(), edit.value.clone());
        }
    }
    Ok(config)
}

fn redact_secrets(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        for (key, child) in object.iter_mut() {
            let lowered = key.to_ascii_lowercase();
            if lowered.contains("key")
                || lowered.contains("secret")
                || lowered.contains("token")
                || lowered.contains("password")
            {
                *child = Value::String("<redacted>".to_string());
            } else {
                redact_secrets(child);
            }
        }
    }
}
```

Implement the trait:

```rust
impl ConfigService for AppServerConfigService {
    fn health(&self) -> ServiceHealth {
        ServiceHealth::ready(ServiceName::Config)
    }

    fn read(&self, params: ConfigReadParams) -> Result<ConfigReadResponse, AppServerError> {
        let path = self.resolve_file(params.cwd.as_deref())?;
        let mut config = read_json(&path)?;
        redact_secrets(&mut config);
        let version = version_for(&path);
        let source = ConfigLayerSource::User { file: path.display().to_string() };
        let mut origins = BTreeMap::new();
        if let Some(object) = config.as_object() {
            for key in object.keys() {
                origins.insert(key.clone(), ConfigLayerMetadata {
                    name: source.clone(),
                    version: version.clone(),
                });
            }
        }
        let layers = params.include_layers.then(|| vec![ConfigLayer {
            name: source,
            version,
            config: config.clone(),
            disabled_reason: None,
        }]);
        Ok(ConfigReadResponse { config, origins, layers })
    }

    fn write_value(&self, params: ConfigValueWriteParams) -> Result<ConfigWriteResponse, AppServerError> {
        let edit = ConfigEdit {
            key_path: params.key_path,
            value: params.value,
            merge_strategy: params.merge_strategy,
        };
        self.write_batch(ConfigBatchWriteParams {
            edits: vec![edit],
            file_path: params.file_path,
            expected_version: params.expected_version,
            reload_user_config: Some(false),
        })
    }

    fn write_batch(&self, params: ConfigBatchWriteParams) -> Result<ConfigWriteResponse, AppServerError> {
        let _guard = self.lock.lock().unwrap_or_else(|poison| poison.into_inner());
        let path = self.resolve_file(params.file_path.as_deref())?;
        let current_version = version_for(&path);
        if let Some(expected) = params.expected_version.as_deref() {
            if expected != current_version {
                return Err(AppServerError::invalid_request(CONFIG_CAPABILITY, "config version mismatch"));
            }
        }
        let mut next = read_json(&path)?;
        for edit in &params.edits {
            next = apply_edit(next, edit)?;
        }
        write_json(&path, &next)?;
        let mut redacted = next.clone();
        redact_secrets(&mut redacted);
        Ok(ConfigWriteResponse { config: redacted, version: version_for(&path) })
    }
}
```

- [ ] **Step 4: Wire config service into app services**

In `app_services.rs`, add the trait:

```rust
pub trait ConfigService: Send + Sync {
    fn health(&self) -> ServiceHealth;
    fn read(&self, params: ConfigReadParams) -> Result<ConfigReadResponse, AppServerError>;
    fn write_value(&self, params: ConfigValueWriteParams) -> Result<ConfigWriteResponse, AppServerError>;
    fn write_batch(&self, params: ConfigBatchWriteParams) -> Result<ConfigWriteResponse, AppServerError>;

    fn is_ready(&self) -> bool {
        self.health().status == ServiceStatus::Ready
    }
}
```

Add `pub config: Arc<dyn ConfigService>` to `AppServerServices`, initialize it in `real()`, and add `NoopConfigService` returning capability unavailable for `config`.

Add this test-only constructor so R6 tests can share a temporary service root:

```rust
#[cfg(test)]
pub fn real_with_root_for_tests(root: std::path::PathBuf) -> Self {
    Self {
        logs: Arc::new(AppServerLogService::new()),
        jobs: Arc::new(AppServerJobService::new(Arc::new(ContextManager::default()))),
        skills: Arc::new(AppServerSkillsService::new()),
        mcp: Arc::new(AppServerMcpService::default()),
        filesystem: Arc::new(AppServerFsService::new(root.clone())),
        command: Arc::new(AppServerCommandExecService::new(root.clone())),
        config: Arc::new(crate::config_service::AppServerConfigService::new(root.clone())),
    }
}
```

- [ ] **Step 5: Add config routes**

In `lib.rs`, import the config DTOs and add route branches:

```rust
method::CONFIG_READ => route_with_params(
    request.id,
    request.params,
    |params: ConfigReadParams| self.config_read(params),
),
method::CONFIG_VALUE_WRITE => route_with_params(
    request.id,
    request.params,
    |params: ConfigValueWriteParams| self.config_value_write(params),
),
method::CONFIG_BATCH_WRITE => route_with_params(
    request.id,
    request.params,
    |params: ConfigBatchWriteParams| self.config_batch_write(params),
),
```

Add methods:

```rust
pub fn config_read(&self, params: ConfigReadParams) -> Result<ConfigReadResponse, AppServerError> {
    self.require_initialized("config")?;
    self.app_services.config.read(params)
}

pub fn config_value_write(
    &self,
    params: ConfigValueWriteParams,
) -> Result<ConfigWriteResponse, AppServerError> {
    self.require_initialized("config")?;
    self.app_services.config.write_value(params)
}

pub fn config_batch_write(
    &self,
    params: ConfigBatchWriteParams,
) -> Result<ConfigWriteResponse, AppServerError> {
    self.require_initialized("config")?;
    self.app_services.config.write_batch(params)
}
```

- [ ] **Step 6: Run config tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server config_read_returns_redacted_config_and_layers config_write_rejects_key_outside_policy
cargo check -p dasclaw_app_server --tests
```

Expected:

```text
PASS config_read_returns_redacted_config_and_layers
PASS config_write_rejects_key_outside_policy
```

- [ ] **Step 7: Commit config service**

Run:

```bash
git add crates/dasclaw_app_server/src/config_service.rs crates/dasclaw_app_server/src/app_services.rs crates/dasclaw_app_server/src/lib.rs
git commit -m "feat(app-server): add R6 config owner" -m "已检查 R6 app-server owner 是否已有，结论：底座存在于 fs/git/hooks/core/model 等 crate，R6 对外 app-server surface 仍需新增"
```

## Task 3: Add Repo Service And gitDiffToRemote

**Files:**

- Create: `crates/dasclaw_git_tools/src/read_only.rs`
- Modify: `crates/dasclaw_git_tools/src/lib.rs`
- Modify: `crates/dasclaw_app_server/Cargo.toml`
- Create: `crates/dasclaw_app_server/src/repo_service.rs`
- Modify: `crates/dasclaw_app_server/src/app_services.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [ ] **Step 1: Add failing gitDiffToRemote test**

Add to `crates/dasclaw_app_server/src/lib.rs` tests:

```rust
#[test]
fn git_diff_to_remote_returns_merge_base_sha_and_patch() {
    let temp = tempfile::tempdir().expect("tempdir");
    init_git_fixture(temp.path());
    std::fs::write(temp.path().join("src.txt"), "changed\n").expect("write");

    let mut server = initialized_server_with_root(temp.path());
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 30,
        "method": "gitDiffToRemote",
        "params": { "cwd": temp.path().to_string_lossy() }
    });
    let response = server.handle_json_rpc(&request.to_string()).expect("response");
    let value: serde_json::Value = serde_json::from_str(&response).expect("json");
    assert!(value["result"]["sha"].as_str().expect("sha").len() >= 7);
    assert!(value["result"]["diff"].as_str().expect("diff").contains("changed"));
}
```

Add a fixture helper:

```rust
fn init_git_fixture(root: &std::path::Path) {
    std::fs::write(root.join("src.txt"), "base\n").expect("write base");
    run_git_fixture(root, ["init"]);
    run_git_fixture(root, ["config", "user.email", "test@example.com"]);
    run_git_fixture(root, ["config", "user.name", "Test User"]);
    run_git_fixture(root, ["add", "."]);
    run_git_fixture(root, ["commit", "-m", "base"]);
    run_git_fixture(root, ["branch", "--set-upstream-to", "master"]);
}

fn run_git_fixture<const N: usize>(root: &std::path::Path, args: [&str; N]) {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_TERMINAL_PROMPT", "0")
        .status()
        .expect("git fixture command");
    assert!(status.success());
}
```

- [ ] **Step 2: Run test and confirm failure**

Run:

```bash
cargo nextest run -p dasclaw_app_server git_diff_to_remote_returns_merge_base_sha_and_patch
```

Expected:

```text
FAIL method not found: gitDiffToRemote
```

- [ ] **Step 3: Add public read-only git helper**

Create `crates/dasclaw_git_tools/src/read_only.rs`:

```rust
use std::path::Path;
use std::time::Duration;

use dasclaw_tool::ToolError;

use crate::runner::run_git;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadOnlyGitOutput {
    pub stdout: String,
    pub exit_code: i32,
}

pub async fn run_read_only_git(
    args: &[&str],
    workdir: &Path,
) -> Result<ReadOnlyGitOutput, ToolError> {
    let Some(command) = args.first().copied() else {
        return Err(ToolError::InvalidParameters("git command is required".to_string()));
    };
    match command {
        "diff" | "merge-base" | "rev-parse" | "status" | "branch" | "config" => {}
        other => {
            return Err(ToolError::InvalidParameters(format!(
                "git command is not read-only for app-server repo service: {other}"
            )));
        }
    }
    let output = run_git(args, workdir, Some(Duration::from_secs(30))).await?;
    Ok(ReadOnlyGitOutput {
        stdout: output.stdout,
        exit_code: output.exit_code,
    })
}
```

Modify `crates/dasclaw_git_tools/src/lib.rs`:

```rust
mod read_only;

pub use read_only::{ReadOnlyGitOutput, run_read_only_git};
```

- [ ] **Step 4: Add repo service**

Create `crates/dasclaw_app_server/src/repo_service.rs`:

```rust
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use dasclaw_app_server_protocol::{
    GitDiffToRemoteParams, GitDiffToRemoteResponse, ServiceHealth, ServiceName,
};

use crate::AppServerError;
use crate::app_services::RepoService;
use crate::blocking_runtime::BlockingTokioRuntime;

const REPO_CAPABILITY: &str = "repo";

pub struct AppServerRepoService {
    root: PathBuf,
    runtime: Mutex<Option<Result<BlockingTokioRuntime, AppServerError>>>,
}

impl AppServerRepoService {
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self { root, runtime: Mutex::new(None) }
    }

    fn runtime(&self) -> Result<BlockingTokioRuntime, AppServerError> {
        let mut runtime = self.runtime.lock().unwrap_or_else(|poison| poison.into_inner());
        if runtime.is_none() {
            *runtime = Some(BlockingTokioRuntime::new("dasclaw-app-server-repo", REPO_CAPABILITY));
        }
        runtime.as_ref().expect("runtime initialized").as_ref().cloned().map_err(Clone::clone)
    }

    fn resolve_cwd(&self, cwd: &str) -> Result<PathBuf, AppServerError> {
        let path = dasclaw_fs_tools::path_utils::validate_path(cwd, Some(&self.root))
            .map_err(|error| AppServerError::capability_unavailable(REPO_CAPABILITY, error.to_string()))?;
        let canonical = path.canonicalize()
            .map_err(|error| AppServerError::capability_unavailable(REPO_CAPABILITY, error.to_string()))?;
        if !canonical.starts_with(&self.root) {
            return Err(AppServerError::capability_unavailable(REPO_CAPABILITY, "repo cwd is outside service root"));
        }
        Ok(canonical)
    }
}
```

Continue with helpers:

```rust
async fn git_stdout(args: &[&str], cwd: &Path) -> Result<String, AppServerError> {
    let output = dasclaw_git_tools::run_read_only_git(args, cwd)
        .await
        .map_err(|error| AppServerError::capability_unavailable(REPO_CAPABILITY, error.to_string()))?;
    if output.exit_code != 0 {
        return Err(AppServerError::capability_unavailable(
            REPO_CAPABILITY,
            format!("git {:?} failed: {}", args, output.stdout),
        ));
    }
    Ok(output.stdout.trim().to_string())
}

async fn diff_to_remote(cwd: PathBuf) -> Result<GitDiffToRemoteResponse, AppServerError> {
    let upstream = git_stdout(&["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}"], &cwd)
        .await
        .unwrap_or_else(|_| "HEAD".to_string());
    let sha = if upstream == "HEAD" {
        git_stdout(&["rev-parse", "HEAD"], &cwd).await?
    } else {
        git_stdout(&["merge-base", "HEAD", upstream.as_str()], &cwd).await?
    };
    let diff = git_stdout(&["diff", "--stat", "--patch", sha.as_str(), "--"], &cwd).await?;
    Ok(GitDiffToRemoteResponse { sha, diff })
}
```

Implement trait:

```rust
impl RepoService for AppServerRepoService {
    fn health(&self) -> ServiceHealth {
        ServiceHealth::ready(ServiceName::Repo)
    }

    fn git_diff_to_remote(
        &self,
        params: GitDiffToRemoteParams,
    ) -> Result<GitDiffToRemoteResponse, AppServerError> {
        let cwd = self.resolve_cwd(&params.cwd)?;
        self.runtime()?.block_on("gitDiffToRemote", diff_to_remote(cwd))
    }
}
```

- [ ] **Step 5: Wire repo service route**

Add `RepoService` trait to `app_services.rs`, add field/noop/test fake, and construct `AppServerRepoService::new(root.clone())` in `real()`.

Extend `real_with_root_for_tests` with the repo field at the same time:

```rust
repo: Arc::new(crate::repo_service::AppServerRepoService::new(root.clone())),
```

In `lib.rs`, add:

```rust
method::GIT_DIFF_TO_REMOTE => route_with_params(
    request.id,
    request.params,
    |params: GitDiffToRemoteParams| self.git_diff_to_remote(params),
),
```

Add method:

```rust
pub fn git_diff_to_remote(
    &self,
    params: GitDiffToRemoteParams,
) -> Result<GitDiffToRemoteResponse, AppServerError> {
    self.require_initialized("repo")?;
    self.app_services.repo.git_diff_to_remote(params)
}
```

- [ ] **Step 6: Run repo tests**

Run:

```bash
cargo nextest run -p dasclaw_git_tools
cargo nextest run -p dasclaw_app_server git_diff_to_remote_returns_merge_base_sha_and_patch
cargo check -p dasclaw_app_server --tests
```

Expected:

```text
PASS git_diff_to_remote_returns_merge_base_sha_and_patch
```

- [ ] **Step 7: Commit repo service**

Run:

```bash
git add crates/dasclaw_git_tools/src/read_only.rs crates/dasclaw_git_tools/src/lib.rs crates/dasclaw_app_server/Cargo.toml crates/dasclaw_app_server/src/repo_service.rs crates/dasclaw_app_server/src/app_services.rs crates/dasclaw_app_server/src/lib.rs
git commit -m "feat(app-server): add R6 repo diff owner" -m "已检查 R6 app-server owner 是否已有，结论：底座存在于 fs/git/hooks/core/model 等 crate，R6 对外 app-server surface 仍需新增"
```

## Task 4: Add Fuzzy File Search Helper And Session Owner

**Files:**

- Create: `crates/dasclaw_fs_tools/src/path_search.rs`
- Modify: `crates/dasclaw_fs_tools/src/lib.rs`
- Create: `crates/dasclaw_app_server/src/search_service.rs`
- Modify: `crates/dasclaw_app_server/src/app_services.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [ ] **Step 1: Add failing fs helper tests**

Add tests in `crates/dasclaw_fs_tools/src/path_search.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_path_search_matches_ordered_subsequence() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join("src/app")).expect("mkdir");
        std::fs::write(temp.path().join("src/app/config_service.rs"), "").expect("write");
        std::fs::write(temp.path().join("src/app/command_service.rs"), "").expect("write");

        let results = fuzzy_file_search(temp.path(), "cfg", 10).expect("search");
        assert_eq!(results[0].file_name, "config_service.rs");
        assert_eq!(results[0].match_type, FuzzyPathMatchType::File);
        assert!(results[0].indices.as_ref().expect("indices").len() >= 2);
    }
}
```

- [ ] **Step 2: Implement fs helper**

Create `crates/dasclaw_fs_tools/src/path_search.rs`:

```rust
use std::path::{Path, PathBuf};

use dasclaw_tool::ToolError;

const HARD_MAX_RESULTS: usize = 200;
const MAX_DEPTH: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuzzyPathMatchType {
    File,
    Directory,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FuzzyPathSearchResult {
    pub root: String,
    pub path: String,
    pub match_type: FuzzyPathMatchType,
    pub file_name: String,
    pub score: f64,
    pub indices: Option<Vec<usize>>,
}

pub fn fuzzy_file_search(
    root: &Path,
    query: &str,
    max_results: usize,
) -> Result<Vec<FuzzyPathSearchResult>, ToolError> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let root = root.canonicalize().map_err(|error| {
        ToolError::InvalidParameters(format!("search root is not readable: {error}"))
    })?;
    let mut results = Vec::new();
    collect(&root, &root, query, 0, max_results.min(HARD_MAX_RESULTS), &mut results)?;
    results.sort_by(|left, right| {
        right.score.partial_cmp(&left.score).unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.path.cmp(&right.path))
    });
    results.truncate(max_results.min(HARD_MAX_RESULTS));
    Ok(results)
}
```

Continue with traversal and scoring:

```rust
fn collect(
    root: &Path,
    dir: &Path,
    query: &str,
    depth: usize,
    max_results: usize,
    results: &mut Vec<FuzzyPathSearchResult>,
) -> Result<(), ToolError> {
    if depth > MAX_DEPTH || results.len() >= max_results * 4 {
        return Ok(());
    }
    let entries = std::fs::read_dir(dir)
        .map_err(|error| ToolError::ExecutionFailed(format!("cannot read dir {}: {error}", dir.display())))?;
    let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || is_ignored_dir(&name) {
            continue;
        }
        if let Some((score, indices)) = fuzzy_score(&name, query) {
            let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
            results.push(FuzzyPathSearchResult {
                root: root.display().to_string(),
                path: rel,
                match_type: if path.is_dir() { FuzzyPathMatchType::Directory } else { FuzzyPathMatchType::File },
                file_name: name.clone(),
                score,
                indices: Some(indices),
            });
        }
        if path.is_dir() {
            collect(root, &path, query, depth + 1, max_results, results)?;
        }
    }
    Ok(())
}

fn fuzzy_score(candidate: &str, query: &str) -> Option<(f64, Vec<usize>)> {
    let candidate_lower = candidate.to_ascii_lowercase();
    let query_lower = query.to_ascii_lowercase();
    let mut indices = Vec::new();
    let mut search_from = 0;
    for needle in query_lower.chars() {
        let haystack = &candidate_lower[search_from..];
        let offset = haystack.find(needle)?;
        let index = search_from + offset;
        indices.push(index);
        search_from = index + needle.len_utf8();
    }
    let compactness = indices.last().copied().unwrap_or(0) + 1 - indices[0];
    let prefix_bonus = if indices.first().copied() == Some(0) { 25.0 } else { 0.0 };
    let score = prefix_bonus + (query.len() as f64 * 10.0) - compactness as f64;
    Some((score, indices))
}

fn is_ignored_dir(name: &str) -> bool {
    matches!(name, "node_modules" | "target" | "dist" | "build" | ".git")
}
```

Export it in `crates/dasclaw_fs_tools/src/lib.rs`:

```rust
pub mod path_search;
```

- [ ] **Step 3: Add failing app-server fuzzy search route test**

Add:

```rust
#[test]
fn fuzzy_file_search_returns_files_and_session_notifications() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(temp.path().join("src")).expect("mkdir");
    std::fs::write(temp.path().join("src/config_service.rs"), "").expect("write");

    let mut server = initialized_server_with_root(temp.path());
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 40,
        "method": "fuzzyFileSearch",
        "params": {
            "query": "cfg",
            "roots": [temp.path().to_string_lossy()],
            "cancellationToken": "session-a"
        }
    });
    let response = server.handle_json_rpc(&request.to_string()).expect("response");
    let value: serde_json::Value = serde_json::from_str(&response).expect("json");
    assert_eq!(value["result"]["files"][0]["fileName"], "config_service.rs");

    let notifications = json_rpc_values(server.drain_json_rpc_notifications());
    let methods = methods_from_values(&notifications);
    assert!(methods.contains(&"fuzzyFileSearch/sessionUpdated".to_string()));
    assert!(methods.contains(&"fuzzyFileSearch/sessionCompleted".to_string()));
}
```

- [ ] **Step 4: Implement app-server search service**

Create `crates/dasclaw_app_server/src/search_service.rs`:

```rust
use std::path::PathBuf;
use std::sync::Mutex;

use dasclaw_app_server_protocol::{
    FuzzyFileSearchMatchType, FuzzyFileSearchParams, FuzzyFileSearchResponse,
    FuzzyFileSearchResult, FuzzyFileSearchSessionCompletedNotification,
    FuzzyFileSearchSessionUpdatedNotification, ServiceHealth, ServiceName,
};

use crate::AppServerError;
use crate::app_services::SearchService;

const SEARCH_CAPABILITY: &str = "search";

pub struct AppServerSearchService {
    root: PathBuf,
    events: Mutex<Vec<SearchNotification>>,
}

#[derive(Debug, Clone)]
pub enum SearchNotification {
    Updated(FuzzyFileSearchSessionUpdatedNotification),
    Completed(FuzzyFileSearchSessionCompletedNotification),
}

impl AppServerSearchService {
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self { root, events: Mutex::new(Vec::new()) }
    }
}
```

Implement trait:

```rust
impl SearchService for AppServerSearchService {
    fn health(&self) -> ServiceHealth {
        ServiceHealth::ready(ServiceName::Search)
    }

    fn fuzzy_file_search(
        &self,
        params: FuzzyFileSearchParams,
    ) -> Result<FuzzyFileSearchResponse, AppServerError> {
        if params.query.trim().is_empty() {
            return Ok(FuzzyFileSearchResponse { files: Vec::new() });
        }
        let mut files = Vec::new();
        for root in &params.roots {
            let resolved = dasclaw_fs_tools::path_utils::validate_path(root, Some(&self.root))
                .map_err(|error| AppServerError::capability_unavailable(SEARCH_CAPABILITY, error.to_string()))?;
            let matches = dasclaw_fs_tools::path_search::fuzzy_file_search(&resolved, &params.query, 50)
                .map_err(|error| AppServerError::capability_unavailable(SEARCH_CAPABILITY, error.to_string()))?;
            files.extend(matches.into_iter().map(|entry| FuzzyFileSearchResult {
                root: entry.root,
                path: entry.path,
                match_type: match entry.match_type {
                    dasclaw_fs_tools::path_search::FuzzyPathMatchType::File => FuzzyFileSearchMatchType::File,
                    dasclaw_fs_tools::path_search::FuzzyPathMatchType::Directory => FuzzyFileSearchMatchType::Directory,
                },
                file_name: entry.file_name,
                score: entry.score,
                indices: entry.indices,
            }));
        }
        files.sort_by(|left, right| {
            right.score.partial_cmp(&left.score).unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.path.cmp(&right.path))
        });
        files.truncate(50);
        if let Some(session_id) = params.cancellation_token.clone() {
            let mut events = self.events.lock().unwrap_or_else(|poison| poison.into_inner());
            events.push(SearchNotification::Updated(FuzzyFileSearchSessionUpdatedNotification {
                session_id: session_id.clone(),
                query: params.query,
                files: files.clone(),
            }));
            events.push(SearchNotification::Completed(FuzzyFileSearchSessionCompletedNotification { session_id }));
        }
        Ok(FuzzyFileSearchResponse { files })
    }

    fn drain_search_events(&self) -> Vec<SearchNotification> {
        std::mem::take(&mut *self.events.lock().unwrap_or_else(|poison| poison.into_inner()))
    }
}
```

- [ ] **Step 5: Wire search route and notification drain**

Add `SearchService` trait, app service field, noop service, and test fake in `app_services.rs`.

Extend `real_with_root_for_tests` with the search field at the same time:

```rust
search: Arc::new(crate::search_service::AppServerSearchService::new(root.clone())),
```

In `AppServer::drain_service_updates`, emit drained search events:

```rust
for event in self.app_services.drain_search_events() {
    match event {
        search_service::SearchNotification::Updated(event) => {
            self.notifications.emit_fuzzy_file_search_session_updated(event);
        }
        search_service::SearchNotification::Completed(event) => {
            self.notifications.emit_fuzzy_file_search_session_completed(event);
        }
    }
}
```

Add route:

```rust
method::FUZZY_FILE_SEARCH => route_with_params(
    request.id,
    request.params,
    |params: FuzzyFileSearchParams| self.fuzzy_file_search(params),
),
```

Add method:

```rust
pub fn fuzzy_file_search(
    &self,
    params: FuzzyFileSearchParams,
) -> Result<FuzzyFileSearchResponse, AppServerError> {
    self.require_initialized("search")?;
    self.app_services.search.fuzzy_file_search(params)
}
```

- [ ] **Step 6: Run search tests**

Run:

```bash
cargo nextest run -p dasclaw_fs_tools fuzzy_path_search_matches_ordered_subsequence
cargo nextest run -p dasclaw_app_server fuzzy_file_search_returns_files_and_session_notifications
cargo check -p dasclaw_app_server --tests
```

Expected:

```text
PASS fuzzy_path_search_matches_ordered_subsequence
PASS fuzzy_file_search_returns_files_and_session_notifications
```

- [ ] **Step 7: Commit search service**

Run:

```bash
git add crates/dasclaw_fs_tools/src/path_search.rs crates/dasclaw_fs_tools/src/lib.rs crates/dasclaw_app_server/src/search_service.rs crates/dasclaw_app_server/src/app_services.rs crates/dasclaw_app_server/src/lib.rs
git commit -m "feat(app-server): add R6 fuzzy file search owner" -m "已检查 R6 app-server owner 是否已有，结论：底座存在于 fs/git/hooks/core/model 等 crate，R6 对外 app-server surface 仍需新增"
```

## Task 5: Add Conversation Summary Method

**Files:**

- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Modify: `crates/dasclaw_app_server/src/thread_lifecycle.rs`

- [ ] **Step 1: Add failing summary test**

Add:

```rust
#[test]
fn get_conversation_summary_returns_thread_card_shape() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut server = initialized_server_with_root(temp.path());
    let start = server.thread_start(ThreadStartParams {
        cwd: Some(temp.path().to_string_lossy().to_string()),
        sandbox: None,
        permission_profile: None,
    }).expect("thread");

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 50,
        "method": "getConversationSummary",
        "params": { "conversationId": start.thread.id }
    });
    let response = server.handle_json_rpc(&request.to_string()).expect("response");
    let value: serde_json::Value = serde_json::from_str(&response).expect("json");
    assert_eq!(value["result"]["summary"]["conversationId"], start.thread.id);
    assert_eq!(value["result"]["summary"]["cwd"], temp.path().to_string_lossy().to_string());
    assert_eq!(value["result"]["summary"]["cliVersion"], SERVER_VERSION);
}
```

- [ ] **Step 2: Add summary mapper**

In `lib.rs`, add:

```rust
pub fn get_conversation_summary(
    &self,
    params: GetConversationSummaryParams,
) -> Result<GetConversationSummaryResponse, AppServerError> {
    self.require_initialized("session")?;
    let summary = match params {
        GetConversationSummaryParams::ConversationId { conversation_id } => {
            self.thread_summary_or_error_with_capability(&conversation_id, "session")?
        }
        GetConversationSummaryParams::RolloutPath { rollout_path } => {
            self.threads
                .summary_by_path(&rollout_path)
                .ok_or_else(|| AppServerError::invalid_request("session", "conversation not found"))?
        }
    };
    Ok(GetConversationSummaryResponse {
        summary: self.conversation_summary_view(summary)?,
    })
}

fn conversation_summary_view(
    &self,
    summary: ThreadSummary,
) -> Result<ConversationSummary, AppServerError> {
    let preview = self.thread_preview(&summary.thread_id)?;
    Ok(ConversationSummary {
        conversation_id: summary.thread_id,
        path: summary.path.unwrap_or_default(),
        preview,
        timestamp: Some(summary.created_at.to_string()),
        updated_at: Some(summary.updated_at.to_string()),
        model_provider: self.model_provider.selected_provider_id(),
        cwd: summary.workspace_root.unwrap_or_else(|| ".".to_string()),
        cli_version: SERVER_VERSION.to_string(),
        source: CodexSessionSource::Unknown,
        git_info: summary.git_info,
    })
}
```

In `thread_lifecycle.rs`, add:

```rust
pub fn summary_by_path(&self, path: &str) -> Option<ThreadSummary> {
    self.list()
        .into_iter()
        .find(|thread| thread.path.as_deref() == Some(path))
}
```

- [ ] **Step 3: Add route**

Add:

```rust
method::GET_CONVERSATION_SUMMARY => route_with_params(
    request.id,
    request.params,
    |params: GetConversationSummaryParams| self.get_conversation_summary(params),
),
```

- [ ] **Step 4: Run summary tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server get_conversation_summary_returns_thread_card_shape
cargo check -p dasclaw_app_server --tests
```

Expected:

```text
PASS get_conversation_summary_returns_thread_card_shape
```

- [ ] **Step 5: Commit summary method**

Run:

```bash
git add crates/dasclaw_app_server/src/lib.rs crates/dasclaw_app_server/src/thread_lifecycle.rs
git commit -m "feat(app-server): add R6 conversation summary" -m "已检查 R6 app-server owner 是否已有，结论：底座存在于 fs/git/hooks/core/model 等 crate，R6 对外 app-server surface 仍需新增"
```

## Task 6: Add Review Start Owner

**Files:**

- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Modify: `crates/dasclaw_app_server/src/thread_lifecycle.rs`

- [ ] **Step 1: Add review item variants**

Extend `CodexThreadItem`:

```rust
#[serde(rename_all = "camelCase")]
EnteredReviewMode { id: String, review: String },
#[serde(rename_all = "camelCase")]
ExitedReviewMode { id: String, review: String },
```

Update persisted item parsing in `thread_lifecycle.rs` so `enteredReviewMode` and `exitedReviewMode` roundtrip.

- [ ] **Step 2: Add failing review tests**

Add:

```rust
#[test]
fn review_start_inline_runs_review_turn_on_existing_thread() {
    let temp = tempfile::tempdir().expect("tempdir");
    init_git_fixture(temp.path());
    let mut server = initialized_server_with_root(temp.path());
    let thread = server.thread_start(ThreadStartParams {
        cwd: Some(temp.path().to_string_lossy().to_string()),
        sandbox: None,
        permission_profile: None,
    }).expect("thread");

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 60,
        "method": "review/start",
        "params": {
            "threadId": thread.thread.id,
            "target": { "type": "uncommittedChanges" },
            "delivery": "inline"
        }
    });
    let response = server.handle_json_rpc(&request.to_string()).expect("response");
    let value: serde_json::Value = serde_json::from_str(&response).expect("json");
    assert_eq!(value["result"]["reviewThreadId"], thread.thread.id);
    assert_eq!(value["result"]["turn"]["status"], "inProgress");
}
```

Add detached test:

```rust
#[test]
fn review_start_detached_forks_review_thread() {
    let temp = tempfile::tempdir().expect("tempdir");
    init_git_fixture(temp.path());
    let mut server = initialized_server_with_root(temp.path());
    let thread = server.thread_start(ThreadStartParams {
        cwd: Some(temp.path().to_string_lossy().to_string()),
        sandbox: None,
        permission_profile: None,
    }).expect("thread");

    let response = server.handle_json_rpc(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 61,
        "method": "review/start",
        "params": {
            "threadId": thread.thread.id,
            "target": { "type": "custom", "instructions": "focus on tests" },
            "delivery": "detached"
        }
    }).to_string()).expect("response");
    let value: serde_json::Value = serde_json::from_str(&response).expect("json");
    assert_ne!(value["result"]["reviewThreadId"], thread.thread.id);
}
```

- [ ] **Step 3: Implement review prompt builder**

In `lib.rs`:

```rust
fn review_target_label(target: &ReviewTarget) -> Result<String, AppServerError> {
    match target {
        ReviewTarget::UncommittedChanges => Ok("current changes".to_string()),
        ReviewTarget::BaseBranch { branch } if !branch.trim().is_empty() => {
            Ok(format!("changes against base branch {branch}"))
        }
        ReviewTarget::Commit { sha, title } if !sha.trim().is_empty() => {
            Ok(title
                .as_ref()
                .filter(|title| !title.trim().is_empty())
                .map(|title| format!("commit {sha}: {title}"))
                .unwrap_or_else(|| format!("commit {sha}")))
        }
        ReviewTarget::Custom { instructions } if !instructions.trim().is_empty() => {
            Ok(instructions.trim().to_string())
        }
        ReviewTarget::BaseBranch { .. } => Err(AppServerError::invalid_request("review", "base branch is required")),
        ReviewTarget::Commit { .. } => Err(AppServerError::invalid_request("review", "commit sha is required")),
        ReviewTarget::Custom { .. } => Err(AppServerError::invalid_request("review", "custom review instructions are required")),
    }
}

fn review_prompt(target: &ReviewTarget) -> Result<String, AppServerError> {
    let label = review_target_label(target)?;
    Ok(format!(
        "Review request: {label}\n\nFocus on correctness, regressions, safety, and missing tests. Return findings first with file and line references."
    ))
}
```

- [ ] **Step 4: Implement review_start**

In `lib.rs`:

```rust
pub fn review_start(
    &mut self,
    params: ReviewStartParams,
) -> Result<ReviewStartResponse, AppServerError> {
    self.require_initialized("review")?;
    self.thread_summary_or_error_with_capability(&params.thread_id, "review")?;
    let delivery = params.delivery.unwrap_or(ReviewDelivery::Inline);
    let review_thread_id = match delivery {
        ReviewDelivery::Inline => params.thread_id.clone(),
        ReviewDelivery::Detached => {
            let fork = self.thread_fork(ThreadForkParams {
                thread_id: params.thread_id.clone(),
                cwd: None,
                ephemeral: true,
                exclude_turns: false,
                persist_extended_history: false,
            })?;
            fork.thread.id
        }
    };
    let prompt = review_prompt(&params.target)?;
    let response = self.turn_start(TurnStartParams {
        thread_id: review_thread_id.clone(),
        input: vec![UserInput::Text {
            text: prompt,
            text_elements: Vec::new(),
        }],
        cwd: None,
        model: None,
        summary: None,
        sandbox_policy: None,
        permission_profile: None,
    })?;
    Ok(ReviewStartResponse {
        turn: response.turn,
        review_thread_id,
    })
}
```

Add route:

```rust
method::REVIEW_START => route_with_params(
    request.id,
    request.params,
    |params: ReviewStartParams| self.review_start(params),
),
```

- [ ] **Step 5: Add review capability advertisement**

Set `review` implemented when session and thread lifecycle are ready:

```rust
if availability.r6.review {
    self.review = Capability::implemented("review", &[method::REVIEW_START], &[]);
}
```

- [ ] **Step 6: Run review tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server review_start_inline_runs_review_turn_on_existing_thread review_start_detached_forks_review_thread
cargo check -p dasclaw_app_server --tests
```

Expected:

```text
PASS review_start_inline_runs_review_turn_on_existing_thread
PASS review_start_detached_forks_review_thread
```

- [ ] **Step 7: Commit review owner**

Run:

```bash
git add crates/dasclaw_app_server_protocol/src/lib.rs crates/dasclaw_app_server/src/lib.rs crates/dasclaw_app_server/src/thread_lifecycle.rs
git commit -m "feat(app-server): add R6 review start owner" -m "已检查 R6 app-server owner 是否已有，结论：底座存在于 fs/git/hooks/core/model 等 crate，R6 对外 app-server surface 仍需新增"
```

## Task 7: Add Model Reroute And Verification Notification Delivery

**Files:**

- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`

- [ ] **Step 1: Add failing model notification tests**

Add:

```rust
#[test]
fn runtime_model_verification_update_emits_notification() {
    let mut server = initialized_server();
    server.runtime_turn_updates.push(RuntimeTurnUpdate {
        thread_id: "thread-1".to_string(),
        turn_id: "turn-1".to_string(),
        outcome: RuntimeTurnOutcome::ModelVerification {
            verifications: vec![ModelVerification::TrustedAccessForCyber],
        },
    });
    let notifications = json_rpc_values(server.drain_json_rpc_notifications());
    assert!(methods_from_values(&notifications).contains(&"model/verification".to_string()));
}

#[test]
fn runtime_model_reroute_update_emits_notification() {
    let mut server = initialized_server();
    server.runtime_turn_updates.push(RuntimeTurnUpdate {
        thread_id: "thread-1".to_string(),
        turn_id: "turn-1".to_string(),
        outcome: RuntimeTurnOutcome::ModelRerouted {
            from_model: "gpt-small".to_string(),
            to_model: "gpt-verified".to_string(),
            reason: ModelRerouteReason::HighRiskCyberActivity,
        },
    });
    let notifications = json_rpc_values(server.drain_json_rpc_notifications());
    assert!(methods_from_values(&notifications).contains(&"model/rerouted".to_string()));
}
```

- [ ] **Step 2: Add runtime update variants**

Extend `RuntimeTurnOutcome`:

```rust
ModelRerouted {
    from_model: String,
    to_model: String,
    reason: ModelRerouteReason,
},
ModelVerification {
    verifications: Vec<ModelVerification>,
},
```

- [ ] **Step 3: Emit notifications in drain_runtime_turn_updates**

In `drain_runtime_turn_updates`, add match arms:

```rust
RuntimeTurnOutcome::ModelRerouted { from_model, to_model, reason } => {
    self.notifications.emit_model_rerouted(ModelReroutedNotification {
        thread_id,
        turn_id,
        from_model,
        to_model,
        reason,
    });
}
RuntimeTurnOutcome::ModelVerification { verifications } => {
    self.notifications.emit_model_verification(ModelVerificationNotification {
        thread_id,
        turn_id,
        verifications,
    });
}
```

This task owns app-server delivery. The runtime/model-provider layer owns the future policy that decides when a high-risk cyber reroute or trusted-access verification is needed.

- [ ] **Step 4: Run model event tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server runtime_model_verification_update_emits_notification runtime_model_reroute_update_emits_notification
cargo check -p dasclaw_app_server --tests
```

Expected:

```text
PASS runtime_model_verification_update_emits_notification
PASS runtime_model_reroute_update_emits_notification
```

- [ ] **Step 5: Commit model event delivery**

Run:

```bash
git add crates/dasclaw_app_server/src/lib.rs crates/dasclaw_app_server_protocol/src/lib.rs
git commit -m "feat(app-server): add R6 model event notifications" -m "已检查 R6 app-server owner 是否已有，结论：底座存在于 fs/git/hooks/core/model 等 crate，R6 对外 app-server surface 仍需新增"
```

## Task 8: Add Hook And Warning Notification Channels

**Files:**

- Create: `crates/dasclaw_app_server/src/hook_service.rs`
- Modify: `crates/dasclaw_app_server/Cargo.toml`
- Modify: `crates/dasclaw_app_server/src/app_services.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [ ] **Step 1: Add failing hook/warning drain tests**

Add:

```rust
#[test]
fn hook_service_events_drain_to_json_rpc_notifications() {
    let hook_service = TestHookService::ready(vec![
        AppServerHookNotification::Started(HookStartedNotification {
            thread_id: "thread-1".to_string(),
            turn_id: Some("turn-1".to_string()),
            run: sample_hook_run(HookRunStatus::Running),
        }),
        AppServerHookNotification::Completed(HookCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn_id: Some("turn-1".to_string()),
            run: sample_hook_run(HookRunStatus::Completed),
        }),
    ]);
    let mut server = initialized_server_with_hook_service(hook_service);
    let notifications = json_rpc_values(server.drain_json_rpc_notifications());
    let methods = methods_from_values(&notifications);
    assert!(methods.contains(&"hook/started".to_string()));
    assert!(methods.contains(&"hook/completed".to_string()));
}

#[test]
fn warning_service_events_drain_to_json_rpc_notifications() {
    let hook_service = TestHookService::ready(vec![
        AppServerHookNotification::Warning(WarningNotification {
            thread_id: None,
            message: "config file ignored".to_string(),
        }),
        AppServerHookNotification::ConfigWarning(ConfigWarningNotification {
            summary: "unsupported config key".to_string(),
            details: Some("key experimental.foo is ignored".to_string()),
            path: Some("/tmp/config.json".to_string()),
            range: None,
        }),
    ]);
    let mut server = initialized_server_with_hook_service(hook_service);
    let notifications = json_rpc_values(server.drain_json_rpc_notifications());
    let methods = methods_from_values(&notifications);
    assert!(methods.contains(&"warning".to_string()));
    assert!(methods.contains(&"configWarning".to_string()));
}
```

Add helper:

```rust
fn sample_hook_run(status: HookRunStatus) -> HookRunSummary {
    HookRunSummary {
        id: "hook-run-1".to_string(),
        event_name: HookEventName::PreToolUse,
        handler_type: HookHandlerType::Command,
        execution_mode: HookExecutionMode::Sync,
        scope: HookScope::Turn,
        source_path: "/tmp/hook.sh".to_string(),
        source: HookSource::Project,
        display_order: 1,
        status,
        status_message: None,
        started_at: 1,
        completed_at: Some(2),
        duration_ms: Some(1),
        entries: vec![HookOutputEntry {
            kind: HookOutputEntryKind::Warning,
            text: "review command".to_string(),
        }],
    }
}
```

- [ ] **Step 2: Implement hook service**

Create `crates/dasclaw_app_server/src/hook_service.rs`:

```rust
use std::sync::Mutex;

use dasclaw_app_server_protocol::{
    ConfigWarningNotification, DeprecationNoticeNotification, GuardianWarningNotification,
    HookCompletedNotification, HookStartedNotification, ServiceHealth, ServiceName,
    WarningNotification,
};

use crate::app_services::HookNotificationService;

#[derive(Debug, Clone)]
pub enum AppServerHookNotification {
    Started(HookStartedNotification),
    Completed(HookCompletedNotification),
    Warning(WarningNotification),
    GuardianWarning(GuardianWarningNotification),
    ConfigWarning(ConfigWarningNotification),
    DeprecationNotice(DeprecationNoticeNotification),
}

pub struct AppServerHookService {
    events: Mutex<Vec<AppServerHookNotification>>,
}

impl Default for AppServerHookService {
    fn default() -> Self {
        Self { events: Mutex::new(Vec::new()) }
    }
}

impl AppServerHookService {
    pub fn push(&self, event: AppServerHookNotification) {
        self.events.lock().unwrap_or_else(|poison| poison.into_inner()).push(event);
    }
}

impl HookNotificationService for AppServerHookService {
    fn health(&self) -> ServiceHealth {
        ServiceHealth::ready(ServiceName::Hooks)
    }

    fn drain_hook_notifications(&self) -> Vec<AppServerHookNotification> {
        std::mem::take(&mut *self.events.lock().unwrap_or_else(|poison| poison.into_inner()))
    }
}
```

- [ ] **Step 3: Wire hook service**

Add service trait:

```rust
pub trait HookNotificationService: Send + Sync {
    fn health(&self) -> ServiceHealth;
    fn drain_hook_notifications(&self) -> Vec<hook_service::AppServerHookNotification>;

    fn is_ready(&self) -> bool {
        self.health().status == ServiceStatus::Ready
    }
}
```

Add `hooks: Arc<dyn HookNotificationService>` to `AppServerServices`, real/noop/test fake implementations, and `drain_hook_notifications()`.

Extend `real_with_root_for_tests` with the hooks field at the same time:

```rust
hooks: Arc::new(crate::hook_service::AppServerHookService::default()),
```

In `drain_service_updates`:

```rust
for event in self.app_services.drain_hook_notifications() {
    match event {
        hook_service::AppServerHookNotification::Started(event) => {
            self.notifications.emit_hook_started(event);
        }
        hook_service::AppServerHookNotification::Completed(event) => {
            self.notifications.emit_hook_completed(event);
        }
        hook_service::AppServerHookNotification::Warning(event) => {
            self.notifications.emit_warning(event);
        }
        hook_service::AppServerHookNotification::GuardianWarning(event) => {
            self.notifications.emit_guardian_warning(event);
        }
        hook_service::AppServerHookNotification::ConfigWarning(event) => {
            self.notifications.emit_config_warning(event);
        }
        hook_service::AppServerHookNotification::DeprecationNotice(event) => {
            self.notifications.emit_deprecation_notice(event);
        }
    }
}
```

- [ ] **Step 4: Run hook/warning tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server hook_service_events_drain_to_json_rpc_notifications warning_service_events_drain_to_json_rpc_notifications
cargo check -p dasclaw_app_server --tests
```

Expected:

```text
PASS hook_service_events_drain_to_json_rpc_notifications
PASS warning_service_events_drain_to_json_rpc_notifications
```

- [ ] **Step 5: Commit hook/warning channels**

Run:

```bash
git add crates/dasclaw_app_server/Cargo.toml crates/dasclaw_app_server/src/hook_service.rs crates/dasclaw_app_server/src/app_services.rs crates/dasclaw_app_server/src/lib.rs
git commit -m "feat(app-server): add R6 hook warning channels" -m "已检查 R6 app-server owner 是否已有，结论：底座存在于 fs/git/hooks/core/model 等 crate，R6 对外 app-server surface 仍需新增"
```

## R6 Producer Completion Tasks

Task 7 and Task 8 establish notification delivery. The tasks below complete the real producer side by following the Codex split:

1. Runtime/provider/hook/guardian/startup code creates typed events from real execution.
2. App-server bridges those events to JSON-RPC notifications.
3. Capability readiness advertises only producers that are actually wired.

## Task 9: Preserve Provider Model Metadata And Emit Real Model Events

**Files:**

- Modify: `crates/dasclaw_core/src/messages.rs`
- Modify: `crates/dasclaw_core/src/response_types.rs`
- Modify: `crates/dasclaw_core/src/agentic_loop.rs`
- Modify: `crates/dasclaw_runtime/src/agent.rs`
- Modify: `crates/dasclaw_runtime/src/llm_adapter.rs`
- Modify: `crates/dasclaw_llm_provider/src/provider/codex_chatgpt.rs`
- Modify: `crates/dasclaw_llm_provider/src/provider/openai_codex_provider.rs`
- Modify: `crates/dasclaw_llm_provider/src/provider/reasoning.rs`
- Modify: `crates/dasclaw_llm_provider/src/testing.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [ ] **Step 1: Add failing metadata preservation tests**

Add unit tests that prove actual provider model metadata survives each boundary:

```rust
#[test]
fn tool_completion_response_carries_actual_model_metadata() {
    let response = ToolCompletionResponse {
        content: Some("done".to_string()),
        reasoning: None,
        tool_calls: Vec::new(),
        input_tokens: 1,
        output_tokens: 2,
        finish_reason: FinishReason::Stop,
        cache_read_input_tokens: 0,
        cache_creation_input_tokens: 0,
        metadata: ResponseMetadata {
            anomaly: None,
            actual_model: Some("gpt-5.2-codex".to_string()),
            model_verifications: Vec::new(),
        },
    };

    let output = map_to_respond_output(response);
    assert_eq!(output.metadata.actual_model.as_deref(), Some("gpt-5.2-codex"));
}
```

Add app-server bridge tests:

```rust
#[test]
fn runtime_completed_event_does_not_infer_high_risk_reroute_from_actual_model_metadata() {
    let mut server = initialized_server_with_responder_metadata(ResponseMetadata {
        actual_model: Some("gpt-5.2-codex".to_string()),
        ..ResponseMetadata::default()
    });
    start_turn(&mut server, "gpt-5.3-codex");

    let notifications = json_rpc_values(server.drain_json_rpc_notifications());
    assert_no_json_rpc_method(&notifications, "model/rerouted");
}
```

Delivery-only app-server tests may still cover explicit runtime updates such as `runtime_model_reroute_update_emits_notification`, but those tests must stay out of producer-readiness evidence until a trusted reroute reason source exists.

```rust
#[test]
fn runtime_completed_event_emits_model_verification_from_response_metadata() {
    let mut server = initialized_server();
    let request = runtime_turn_start_request_with_model("gpt-5.3-codex");
    let metadata = ResponseMetadata {
        anomaly: None,
        actual_model: Some("gpt-5.3-codex".to_string()),
        model_verifications: vec![ResponseModelVerification::TrustedAccessForCyber],
    };

    server.record_runtime_response_metadata(&request, &metadata);

    let notifications = json_rpc_values(server.drain_json_rpc_notifications());
    assert_json_rpc_methods(&notifications, vec!["model/verification"]);
    assert_eq!(notifications[0]["params"]["verifications"], json!(["trustedAccessForCyber"]));
}

#[test]
fn runtime_same_model_metadata_does_not_emit_reroute() {
    let mut server = initialized_server();
    let request = runtime_turn_start_request_with_model("gpt-5.3-codex");
    let metadata = ResponseMetadata {
        anomaly: None,
        actual_model: Some("gpt-5.3-codex".to_string()),
        model_verifications: Vec::new(),
    };

    server.record_runtime_response_metadata(&request, &metadata);

    let notifications = json_rpc_values(server.drain_json_rpc_notifications());
    assert_json_rpc_methods(&notifications, vec![]);
}
```

- [ ] **Step 2: Extend core response metadata**

In `crates/dasclaw_core/src/response_types.rs`, change `ResponseMetadata` from copy-only anomaly metadata into response metadata that can carry model facts:

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anomaly: Option<ResponseAnomaly>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_model: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub model_verifications: Vec<ResponseModelVerification>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResponseModelVerification {
    TrustedAccessForCyber,
}
```

Remove `Copy` from call sites that relied on `ResponseMetadata: Copy`; clone the metadata only where the same value must be reused.

- [ ] **Step 3: Add metadata to tool completion responses**

In `crates/dasclaw_core/src/messages.rs`, extend `ToolCompletionResponse`:

```rust
pub struct ToolCompletionResponse {
    pub content: Option<String>,
    pub reasoning: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub finish_reason: FinishReason,
    pub cache_read_input_tokens: u32,
    pub cache_creation_input_tokens: u32,
    pub metadata: ResponseMetadata,
}
```

Set `metadata: ResponseMetadata::default()` in existing test/fake providers first so the compile errors become an explicit checklist.

- [ ] **Step 4: Populate actual model in real providers**

When a provider response already knows the actual model, copy it into `ToolCompletionResponse.metadata.actual_model`.

`crates/dasclaw_llm_provider/src/providers/openai_compat.rs` already normalizes `MessageResponse.model`. Keep that model when adapting into `ToolCompletionResponse`.

For provider implementations that only have the configured model, set:

```rust
metadata: ResponseMetadata {
    actual_model: Some(self.model.clone()),
    ..ResponseMetadata::default()
},
```

For test providers and provider wrappers such as failover/smart-routing/reasoning, preserve inner `response.metadata` instead of replacing it with default metadata.

- [ ] **Step 5: Carry metadata through runtime output**

In `crates/dasclaw_core/src/agentic_loop.rs`, return metadata from the default text responder:

```rust
TextAction::Return(LoopOutcome::Response {
    text: text.to_string(),
    usage,
    metadata,
})
```

If the existing enum still uses `LoopOutcome::Response(String)`, change it to a struct variant so the final response text, usage, and metadata travel together.

In `crates/dasclaw_runtime/src/agent.rs`, extend `AgentRunOutput`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRunOutput {
    pub text: String,
    pub usage: TokenUsage,
    #[serde(default)]
    pub metadata: ResponseMetadata,
}
```

Update `map_outcome` so completed agent output keeps `metadata`.

- [ ] **Step 6: Convert trusted response metadata into app-server runtime updates**

Superseded implementation note: do **not** infer `model/rerouted` from `actual_model != requested_model`, and do **not** label a plain model mismatch as `HighRiskCyberActivity`. Provider `actual_model` is useful metadata, but a real `model/rerouted` producer also needs a trusted reroute reason from the provider/runtime path. Until that reason source exists, preserve `actual_model` without advertising or emitting `model/rerouted`.

`model/verification` may be bridged only when `ResponseMetadata.model_verifications` is populated by a real upstream provider source. Hand-authored metadata in tests proves delivery only, not producer readiness.

Add converter:

```rust
fn model_verification_from_core(value: ResponseModelVerification) -> ModelVerification {
    match value {
        ResponseModelVerification::TrustedAccessForCyber => ModelVerification::TrustedAccessForCyber,
    }
}
```

Call this helper when the runtime bridge receives the real completed output for a turn, before the turn is removed from the pending map. This mirrors Codex: the core/runtime observes the model facts, app-server only bridges them.

- [ ] **Step 7: Keep model verification readiness honest**

If no Dasclaw provider can populate `ResponseMetadata.model_verifications` from a real upstream response yet, keep `model/verification` protocol delivery implemented but mark the producer readiness as disabled with reason:

```text
model verification producer has no provider source wired
```

Do not use `push_test_runtime_outcome` or hand-authored tests as evidence of real model verification support. Test-only injection proves delivery, not producer readiness.

- [ ] **Step 8: Run model producer tests**

Run:

```bash
cargo nextest run -p dasclaw_core response_metadata_roundtrip
cargo nextest run -p dasclaw_runtime agent_run_output_preserves_response_metadata
cargo nextest run -p dasclaw_llm_provider tool_completion_response_carries_actual_model_metadata
cargo nextest run -p dasclaw_app_server \
  runtime_completed_event_does_not_infer_high_risk_reroute_from_actual_model_metadata \
  runtime_completed_event_emits_model_verification_from_response_metadata \
  runtime_same_model_metadata_does_not_emit_reroute
cargo check -p dasclaw_core --tests
cargo check -p dasclaw_runtime --tests
cargo check -p dasclaw_llm_provider --tests
cargo check -p dasclaw_app_server --tests
```

Expected:

```text
PASS actual model metadata survives provider -> core -> runtime without being mislabeled as high-risk reroute
PASS reroute notification is emitted only from an explicit trusted reroute reason source
PASS model verification readiness remains disabled when no provider source is wired
```

- [ ] **Step 9: Commit model producer**

Run:

```bash
git add crates/dasclaw_core/src/messages.rs crates/dasclaw_core/src/response_types.rs crates/dasclaw_core/src/agentic_loop.rs crates/dasclaw_runtime/src/agent.rs crates/dasclaw_runtime/src/llm_adapter.rs crates/dasclaw_llm_provider/src/provider/codex_chatgpt.rs crates/dasclaw_llm_provider/src/provider/openai_codex_provider.rs crates/dasclaw_llm_provider/src/provider/reasoning.rs crates/dasclaw_llm_provider/src/testing.rs crates/dasclaw_app_server/src/lib.rs
git commit -m "feat(app-server): produce R6 model events from response metadata" -m "已检查 R6 app-server owner 是否已有，结论：Codex 在 core/runtime 产生 model 事件再由 app-server 转发；Dasclaw 已有通知交付层但需贯通 provider 实际模型 metadata"
```

## Task 10: Wire HookRegistry Runs To Hook Notifications

**Files:**

- Modify: `crates/dasclaw_hooks/src/registry.rs`
- Modify: `crates/dasclaw_hooks/src/lib.rs`
- Modify: `crates/dasclaw_app_server/src/hook_service.rs`
- Modify: `crates/dasclaw_app_server/src/app_services.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Modify: `crates/dasclaw_runtime/src/agent.rs`

- [ ] **Step 1: Add failing HookRegistry observer tests**

Add tests in `crates/dasclaw_hooks/src/registry.rs`:

```rust
#[tokio::test]
async fn registry_observer_receives_started_and_completed_for_real_run() {
    let observer = RecordingHookObserver::default();
    let registry = HookRegistry::new().with_observer(Arc::new(observer.clone()));
    registry.register(Box::new(TestHook::ok("lint"))).expect("register hook");

    let outcome = registry.run(&test_event()).await.expect("hook run");

    assert_eq!(outcome, HookOutcome::ok());
    assert_eq!(observer.started_count(), 1);
    assert_eq!(observer.completed_count(), 1);
    assert_eq!(observer.completed()[0].status, HookObservedStatus::Completed);
}

#[tokio::test]
async fn registry_observer_marks_reject_as_completed_rejected() {
    let observer = RecordingHookObserver::default();
    let registry = HookRegistry::new().with_observer(Arc::new(observer.clone()));
    registry.register(Box::new(TestHook::reject("blocked"))).expect("register hook");

    let outcome = registry.run(&test_event()).await.expect("hook run");

    assert!(matches!(outcome, HookOutcome::Reject { .. }));
    assert_eq!(observer.completed()[0].status, HookObservedStatus::Rejected);
}
```

- [ ] **Step 2: Add a hook observer seam in `dasclaw_hooks`**

Add observer types in `crates/dasclaw_hooks/src/registry.rs` and export them from `lib.rs`:

```rust
pub trait HookRunObserver: Send + Sync {
    fn hook_started(&self, event: HookObservedRun);
    fn hook_completed(&self, event: HookObservedRun);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookObservedRun {
    pub id: String,
    pub event_name: String,
    pub hook_name: String,
    pub status: HookObservedStatus,
    pub status_message: Option<String>,
    pub started_at_ms: u64,
    pub completed_at_ms: Option<u64>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookObservedStatus {
    Running,
    Completed,
    Rejected,
    Failed,
    TimedOut,
}
```

`HookRegistry::run` must notify `hook_started` immediately before executing each matching hook and notify `hook_completed` after success, reject, failure, or timeout. Use monotonic run IDs that are deterministic in tests.

- [ ] **Step 3: Convert observed runs into app-server notifications**

In `crates/dasclaw_app_server/src/hook_service.rs`, add a ready constructor that carries a real producer identity:

```rust
pub struct AppServerHookService {
    notifications: Mutex<Vec<AppServerHookNotification>>,
    producer_wired: bool,
}

impl AppServerHookService {
    pub fn unwired() -> Self {
        Self {
            notifications: Mutex::new(Vec::new()),
            producer_wired: false,
        }
    }

    pub fn wired() -> Self {
        Self {
            notifications: Mutex::new(Vec::new()),
            producer_wired: true,
        }
    }
}
```

Implement `HookRunObserver` for `AppServerHookService` or for a cloneable sink owned by it. Map:

- `HookObservedStatus::Running` to `HookStartedNotification`.
- `Completed`, `Rejected`, `Failed`, and `TimedOut` to `HookCompletedNotification`.

Use existing protocol enums for `HookRunStatus`, `HookEventName`, `HookHandlerType`, `HookExecutionMode`, `HookScope`, and `HookOutputEntry`. If the current `HookEvent` does not expose an exact Codex-like value, use a deterministic Dasclaw value and document the mapping in the converter test.

- [ ] **Step 4: Wire the observer into real runtime construction**

In app-server service construction, create one hook service and pass its observer sink into the runtime/hook registry creation path:

```rust
let hook_service = Arc::new(AppServerHookService::wired());
let hook_observer = hook_service.observer();

let runtime = DasclawAgentRuntimeBridge::with_hook_observer(
    existing_runtime_args,
    hook_observer,
);

Self {
    hooks: hook_service,
    runtime,
    ...
}
```

Keep `AppServerHookService::unwired()` for no-op/test servers that do not have a real hook registry. Its health remains disabled:

```rust
ServiceHealth::disabled(ServiceName::Hooks, "hook notification producer is not wired")
```

- [ ] **Step 5: Add app-server producer tests**

Add tests in `crates/dasclaw_app_server/src/lib.rs`:

```rust
#[test]
fn real_hook_registry_run_produces_hook_notifications() {
    let mut server = initialized_server_with_wired_hook_registry(sample_ok_hook());

    server.run_hook_registry_for_test(sample_hook_event()).expect("hook run");

    let notifications = json_rpc_values(server.drain_json_rpc_notifications());
    assert_json_rpc_methods(&notifications, vec!["hook/started", "hook/completed"]);
}

#[test]
fn unwired_hook_service_is_not_advertised_ready() {
    let server = initialized_server_with_unwired_hook_service();

    let capabilities = server.get_capabilities_for_test();

    assert_eq!(capabilities.hooks.status, CapabilityStatus::Unavailable);
    assert_eq!(
        capabilities.hooks.reason.as_deref(),
        Some("hook notification producer is not wired")
    );
}
```

- [ ] **Step 6: Run hook producer tests**

Run:

```bash
cargo nextest run -p dasclaw_hooks \
  registry_observer_receives_started_and_completed_for_real_run \
  registry_observer_marks_reject_as_completed_rejected
cargo nextest run -p dasclaw_app_server \
  real_hook_registry_run_produces_hook_notifications \
  unwired_hook_service_is_not_advertised_ready
cargo check -p dasclaw_hooks --tests
cargo check -p dasclaw_app_server --tests
```

Expected:

```text
PASS hook notifications are produced by HookRegistry::run, not by manually pushing fake queue entries
PASS hook capability remains unavailable when the observer is not wired
```

- [ ] **Step 7: Commit hook producer**

Run:

```bash
git add crates/dasclaw_hooks/src/registry.rs crates/dasclaw_hooks/src/lib.rs crates/dasclaw_app_server/src/hook_service.rs crates/dasclaw_app_server/src/app_services.rs crates/dasclaw_app_server/src/lib.rs crates/dasclaw_runtime/src/agent.rs
git commit -m "feat(app-server): produce R6 hook notifications from HookRegistry" -m "已检查 R6 hook producer 是否已有，结论：Dasclaw 已有 HookRegistry::run 真实入口，但 app-server 之前只有队列 drain，没有注册表 observer"
```

## Task 11: Wire Startup, Config, Deprecation, And Guardian Warning Producers

**Files:**

- Modify: `crates/dasclaw_app_server/src/config_service.rs`
- Modify: `crates/dasclaw_app_server/src/hook_service.rs`
- Modify: `crates/dasclaw_app_server/src/app_services.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Modify: `crates/dasclaw_sandboxing/src/lib.rs`
- Modify: `crates/dasclaw_core/src/context/memory.rs`

- [ ] **Step 1: Add failing startup warning tests**

Add tests in `crates/dasclaw_app_server/src/lib.rs`:

```rust
#[test]
fn initialize_emits_config_warning_for_unsupported_config_key() {
    let mut server = initialized_server_with_config_text(r#"
        model = "gpt-5.3-codex"
        experimental_instructions_file = "legacy.md"
    "#);

    let notifications = json_rpc_values(server.drain_json_rpc_notifications());

    assert_json_rpc_methods(&notifications, vec!["deprecationNotice"]);
    assert_eq!(notifications[0]["params"]["key"], "experimental_instructions_file");
}

#[test]
fn initialize_emits_warning_for_missing_system_bwrap_when_sandbox_needs_it() {
    let mut server = initialized_server_with_sandbox_bwrap_warning("sandbox requires bwrap");

    let notifications = json_rpc_values(server.drain_json_rpc_notifications());

    assert_json_rpc_methods(&notifications, vec!["warning"]);
    assert!(notifications[0]["params"]["message"].as_str().unwrap().contains("sandbox"));
}
```

Add a config-source test:

```rust
#[test]
fn config_service_collects_warning_for_unknown_writable_key() {
    let service = AppServerConfigService::from_config_text_for_test(r#"unknown_key = true"#);

    let warnings = service.drain_config_warnings();

    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].summary, "unsupported config key");
}
```

- [ ] **Step 2: Add a warning queue owned by app-server services**

Keep the existing `AppServerHookNotification` enum as the shared app-server notification queue for R6 warnings, but rename the trait method to reflect that it drains hook and warning notifications:

```rust
pub trait HookNotificationService: Send + Sync {
    fn health(&self) -> ServiceHealth;
    fn drain_notifications(&self) -> Vec<AppServerHookNotification>;
}
```

Use `AppServerHookNotification::Warning`, `ConfigWarning`, `GuardianWarning`, and `DeprecationNotice` for non-hook producers until a dedicated warning service becomes worthwhile.

- [ ] **Step 3: Produce config warnings from config parsing**

In `crates/dasclaw_app_server/src/config_service.rs`, add:

```rust
pub fn drain_config_warnings(&self) -> Vec<AppServerHookNotification> {
    self.config_warnings
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .drain(..)
        .map(AppServerHookNotification::ConfigWarning)
        .collect()
}
```

When config read/write sees an unsupported, ignored, or invalid key that is non-fatal, push:

```rust
ConfigWarningNotification {
    summary: "unsupported config key".to_string(),
    details: Some(format!("key {key} is ignored by Dasclaw app-server")),
    path,
    range,
}
```

Fatal config errors still return method errors; they are not downgraded into warnings.

- [ ] **Step 4: Produce deprecation notices from explicit deprecated keys**

Add a concrete deprecated-key table in config service:

```rust
const DEPRECATED_CONFIG_KEYS: &[DeprecatedConfigKey] = &[
    DeprecatedConfigKey {
        key: "experimental_instructions_file",
        message: "experimental_instructions_file is deprecated; use project instructions discovery instead",
        replacement: None,
    },
];

struct DeprecatedConfigKey {
    key: &'static str,
    message: &'static str,
    replacement: Option<&'static str>,
}
```

When a deprecated key is present, push:

```rust
DeprecationNoticeNotification {
    key: key.key.to_string(),
    message: key.message.to_string(),
    replacement: key.replacement.map(str::to_string),
}
```

Do not invent deprecation notices for keys that are still supported.

- [ ] **Step 5: Produce environment warnings from sandboxing**

Use `dasclaw_sandboxing::system_bwrap_warning` when app-server initializes a sandbox mode that needs bwrap. Convert the returned string into:

```rust
WarningNotification {
    thread_id: None,
    message,
}
```

This mirrors Codex startup warnings: the warning is produced during app-server initialization and delivered to the client through the normal notification drain.

- [ ] **Step 6: Produce guardian warnings from real safety/memory warnings**

Use existing Dasclaw warning-bearing structures first. `crates/dasclaw_core/src/context/memory.rs` already has `sanitization_warnings`. When app-server receives a guardian/safety review or memory sanitization warning through the runtime path, convert it to:

```rust
GuardianWarningNotification {
    title: "guardian warning".to_string(),
    message,
}
```

If there is no guardian review runtime hook in this slice, keep `guardianWarning` delivery implemented but producer readiness disabled with reason:

```text
guardian warning producer has no runtime source wired
```

- [ ] **Step 7: Drain startup warnings during initialize**

During app-server initialization, before returning capabilities, drain config/startup warnings into the notification queue:

```rust
self.app_services
    .hooks
    .push_many(self.app_services.config.drain_config_warnings());
self.app_services
    .hooks
    .push_many(self.app_services.sandbox_startup_warnings());
self.drain_service_updates();
```

Do not emit the same startup warning repeatedly on every capability read; startup warnings are one-shot until the underlying source changes.

- [ ] **Step 8: Run warning producer tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server \
  initialize_emits_config_warning_for_unsupported_config_key \
  initialize_emits_warning_for_missing_system_bwrap_when_sandbox_needs_it \
  config_service_collects_warning_for_unknown_writable_key
cargo check -p dasclaw_app_server --tests
```

Expected:

```text
PASS configWarning, deprecationNotice, and warning are produced from real startup/config sources
PASS guardianWarning remains disabled unless a real guardian source is wired
```

- [ ] **Step 9: Commit warning producers**

Run:

```bash
git add crates/dasclaw_app_server/src/config_service.rs crates/dasclaw_app_server/src/hook_service.rs crates/dasclaw_app_server/src/app_services.rs crates/dasclaw_app_server/src/lib.rs crates/dasclaw_sandboxing/src/lib.rs crates/dasclaw_core/src/context/memory.rs
git commit -m "feat(app-server): produce R6 startup and warning notifications" -m "已检查 R6 warning producer 是否已有，结论：Codex 启动/config warning 由 app-server 产生；Dasclaw 已有 system_bwrap_warning 和 config 服务，但之前未接通知 producer"
```

## Task 12: Add Producer Readiness Gates And Close R6 Matrix

**Files:**

- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`
- Modify: `crates/dasclaw_app_server/src/app_services.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Modify: `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`

- [ ] **Step 1: Split delivery readiness from producer readiness**

Add R6 producer readiness fields without removing existing protocol availability:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeBridgeFeatures {
    ...
    pub model_events: ServiceHealth,
    pub hook_events: ServiceHealth,
    pub warning_events: ServiceHealth,
}
```

Advertise:

- `model/rerouted`: ready only when provider actual-model metadata reaches runtime completion.
- `model/verification`: ready only when at least one provider source populates `ResponseMetadata.model_verifications`.
- `hook/started` and `hook/completed`: ready only when `HookRegistry` observer is wired.
- `warning` and `configWarning`: ready when startup/config warning producers are wired.
- `guardianWarning`: ready only when a real guardian/safety source is wired.
- `deprecationNotice`: ready when deprecated config-key detection is wired.

- [ ] **Step 2: Add fail-closed capability tests**

Add tests:

```rust
#[test]
fn capabilities_do_not_advertise_fake_only_model_verification() {
    let server = initialized_server_without_model_verification_source();

    let capabilities = server.get_capabilities_for_test();

    assert_eq!(capabilities.runtime_bridge.model_events.status, ServiceStatus::Disabled);
    assert_eq!(
        capabilities.runtime_bridge.model_events.reason.as_deref(),
        Some("model verification producer has no provider source wired")
    );
}

#[test]
fn capabilities_advertise_hooks_only_when_hook_registry_observer_is_wired() {
    let unwired = initialized_server_with_unwired_hook_service();
    assert_eq!(unwired.get_capabilities_for_test().runtime_bridge.hook_events.status, ServiceStatus::Disabled);

    let wired = initialized_server_with_wired_hook_registry(sample_ok_hook());
    assert_eq!(wired.get_capabilities_for_test().runtime_bridge.hook_events.status, ServiceStatus::Ready);
}
```

- [ ] **Step 3: Update matrix only after producer tests pass**

Current closeout rule: keep the R6 row partial unless every remaining producer has a real upstream source and readiness gate. Do not use protocol delivery, queue drain tests, or hand-authored runtime updates as completion evidence.

Use the partial row shape while model reroute/model verification/generic warning/guardian warning or remaining hook lifecycle producers are still unwired:

```markdown
| R6 | Config / repo tools / search owner | ~~broad `config/read`、`config/value/write`、`config/batchWrite`~~；~~`gitDiffToRemote`~~；~~`fuzzyFileSearch` session~~；~~`getConversationSummary`~~；~~`review/start`~~；model reroute producer / model verification producer；~~HookRegistry `preToolUse` hook events~~ / remaining hook lifecycle events；~~config/deprecation warning producer~~ / `warning` startup producer / guardian warning producer | R6 app-server owner 已完成 config/repo/search/summary/review、`preToolUse` hook notifications 和 config/deprecation warnings；其余 producer 只保留协议/delivery，不能广告完成。 | 从剩余项移除前必须有真实 upstream source、capability readiness 和非 fake-only 测试。 |
```

- [ ] **Step 4: Run closeout gate**

Run:

```bash
cargo nextest run -p dasclaw_app_server_protocol
cargo nextest run -p dasclaw_fs_tools fuzzy_path_search_matches_ordered_subsequence
cargo nextest run -p dasclaw_git_tools
cargo nextest run -p dasclaw_core response_metadata_roundtrip
cargo nextest run -p dasclaw_runtime agent_run_output_preserves_response_metadata
cargo nextest run -p dasclaw_llm_provider tool_completion_response_carries_actual_model_metadata
cargo nextest run -p dasclaw_hooks \
  registry_observer_receives_started_and_completed_for_real_run \
  registry_observer_marks_reject_as_completed_rejected
cargo nextest run -p dasclaw_app_server \
  config_read_returns_redacted_config_and_layers \
  config_write_rejects_key_outside_policy \
  git_diff_to_remote_returns_merge_base_sha_and_patch \
  fuzzy_file_search_returns_files_and_session_notifications \
  get_conversation_summary_returns_thread_card_shape \
  review_start_inline_runs_review_turn_on_existing_thread \
  review_start_detached_forks_review_thread \
  runtime_model_verification_update_emits_notification \
  runtime_model_reroute_update_emits_notification \
  hook_service_events_drain_to_json_rpc_notifications \
  warning_service_events_drain_to_json_rpc_notifications \
  runtime_completed_event_does_not_infer_high_risk_reroute_from_actual_model_metadata \
  runtime_completed_event_emits_model_verification_from_response_metadata \
  runtime_same_model_metadata_does_not_emit_reroute \
  real_hook_registry_run_produces_hook_notifications \
  unwired_hook_service_is_not_advertised_ready \
  initialize_emits_config_warning_for_unsupported_config_key \
  initialize_emits_warning_for_missing_system_bwrap_when_sandbox_needs_it \
  config_service_collects_warning_for_unknown_writable_key \
  capabilities_do_not_advertise_fake_only_model_verification \
  capabilities_advertise_hooks_only_when_hook_registry_observer_is_wired
cargo check -p dasclaw_app_server_protocol --tests
cargo check -p dasclaw_core --tests
cargo check -p dasclaw_runtime --tests
cargo check -p dasclaw_llm_provider --tests
cargo check -p dasclaw_hooks --tests
cargo check -p dasclaw_fs_tools --tests
cargo check -p dasclaw_git_tools --tests
cargo check -p dasclaw_app_server --tests
cargo fmt --all
python3.12 scripts/check_no_panics.py --base origin/xClaw
```

Expected:

```text
PASS producer readiness is truthful
PASS fake-only notification injection is not used as completion evidence
```

- [ ] **Step 5: Commit producer readiness and matrix**

Run:

```bash
git add crates/dasclaw_app_server_protocol/src/lib.rs crates/dasclaw_app_server/src/app_services.rs crates/dasclaw_app_server/src/lib.rs docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md
git commit -m "docs(app-server): close R6 producer readiness matrix" -m "已检查 R6 producer 是否已有，结论：Codex 真实 producer 在 core/runtime/hook/startup 路径，Dasclaw 需以 readiness gate 区分真实 producer 和测试注入 delivery"
```

## Self-Review

Spec coverage:

- Broad config read/write: Task 1 and Task 2.
- `gitDiffToRemote`: Task 1 and Task 3.
- `fuzzyFileSearch` response and session notifications: Task 1 and Task 4.
- `getConversationSummary`: Task 1 and Task 5.
- `review/start`: Task 1 and Task 6.
- model reroute / verification delivery: Task 1 and Task 7.
- hook/warning delivery: Task 1 and Task 8.
- real model producer and readiness: Task 9 and Task 12.
- real HookRegistry producer: Task 10 and Task 12.
- real startup/config/deprecation/guardian warning producers: Task 11 and Task 12.
- R6 matrix closeout and local verification: Task 12.

Type consistency:

- Protocol DTOs use camelCase JSON to match existing app-server style.
- `ConfigWriteResponse` is shared by value write and batch write.
- `FuzzyFileSearchResult` is used by both response and session updated notification.
- Review delivery defaults to inline in implementation.
- Hook notification service uses one internal enum for hook/warning delivery, while producer readiness records whether the enum is backed by `HookRegistry`, startup/config warning, or guardian sources.

Execution boundaries:

- Config write is not arbitrary file write; it is confined to service root and key allowlist.
- Git service is read-only and rejects non-read-only commands in the helper.
- Fuzzy search reuses Dasclaw traversal discipline and does not import Codex crates.
- Conversation summary is deterministic and does not invoke an LLM.
- Review start uses existing turn execution; it does not create a second review engine.
- Model event completion depends on provider/runtime metadata; fake runtime update injection is delivery evidence only.
- Hook/warning producer wiring keeps hook lifecycle, startup/config warnings, and guardian warnings as separate real sources with fail-closed readiness.
