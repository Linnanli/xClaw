# Dasclaw App Server R6 Config Repo Search Owner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the full R6 owner slice for Dasclaw app-server: broad config read/write, repo diff, fuzzy file search sessions, conversation summary, review start, model reroute/verification notifications, and hook/warning notifications.

**Architecture:** Extend `dasclaw_app_server_protocol` first so every R6 method and event has a typed contract, schema entry, and advertised capability. Then add focused app-server services that reuse existing Dasclaw foundations: `dasclaw_fs_tools` for file traversal rules, `dasclaw_git_tools` for read-only git process handling, `ThreadLifecycleHost` for summaries/reviews, existing model-provider state for model events, and `dasclaw_hooks` for hook run summaries. Keep R6 as an umbrella milestone, but implement it as independent testable slices so each service can be reviewed and reverted separately.

**Tech Stack:** Rust 2024, serde/serde_json, tokio current-thread blocking worker pattern already used by app-server services, existing Dasclaw crates (`dasclaw_app_server_protocol`, `dasclaw_app_server`, `dasclaw_fs_tools`, `dasclaw_git_tools`, `dasclaw_hooks`, `dasclaw_core`), cargo nextest, cargo fmt, project panic scan.

---

## Start Gate

Project 4-question gate:

1. New file/module? Yes. This plan creates app-server service files and fs/git helper files during execution.
2. Negative claims? Yes. It states which R6 app-server surfaces are absent today.
3. Cross-project/protocol reconciliation? Yes. It compares Dasclaw app-server with Codex app-server protocol shapes.
4. Architecture reconciliation document? Yes. This is a plan for protocol gap R6.

Required evidence was collected before this plan:

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

## Task 9: Update Matrix And Run Full Local Gate

**Files:**

- Modify: `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`

- [ ] **Step 1: Update R6 row**

Change the R6 row to say:

```markdown
| R6 | Config / repo tools / search owner | broad `config/read`、`config/value/write`、`config/batchWrite`；`gitDiffToRemote`；`fuzzyFileSearch` session；`getConversationSummary`；`review/start`；model reroute / verification；hooks warning channel | R6 app-server owner implemented with typed protocol, config allowlist policy, read-only repo service, path fuzzy search, deterministic conversation summary, review-start routing, model event delivery, and hook/warning notification drain. Existing Dasclaw git/search/hooks/core/model底座被复用，Codex product-only external-agent/feedback/experiment domains remain outside R6. | Method/event schema lists every R6 contract; service tests cover success and fail-closed errors; unsupported R7 domains stay explicit. |
```

- [ ] **Step 2: Run targeted tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server_protocol
cargo nextest run -p dasclaw_fs_tools fuzzy_path_search_matches_ordered_subsequence
cargo nextest run -p dasclaw_git_tools
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
  warning_service_events_drain_to_json_rpc_notifications
```

Expected:

```text
PASS all listed tests
```

- [ ] **Step 3: Run local required checks**

Run:

```bash
cargo check -p dasclaw_app_server_protocol --tests
cargo check -p dasclaw_fs_tools --tests
cargo check -p dasclaw_git_tools --tests
cargo check -p dasclaw_app_server --tests
cargo fmt --all
python3.12 scripts/check_no_panics.py --base origin/xClaw
cargo clippy --no-deps -p dasclaw_app_server_protocol --all-targets -- -D warnings
cargo clippy --no-deps -p dasclaw_fs_tools --all-targets -- -D warnings
cargo clippy --no-deps -p dasclaw_git_tools --all-targets -- -D warnings
cargo clippy --no-deps -p dasclaw_app_server --all-targets -- -D warnings
```

Expected:

```text
0 errors in touched crates
```

- [ ] **Step 4: Commit matrix and verification note**

Run:

```bash
git add docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md
git commit -m "docs: mark app-server R6 owner plan complete" -m "已检查 R6 app-server owner 是否已有，结论：底座存在于 fs/git/hooks/core/model 等 crate，R6 对外 app-server surface 仍需新增"
```

## Self-Review

Spec coverage:

- Broad config read/write: Task 1 and Task 2.
- `gitDiffToRemote`: Task 1 and Task 3.
- `fuzzyFileSearch` response and session notifications: Task 1 and Task 4.
- `getConversationSummary`: Task 1 and Task 5.
- `review/start`: Task 1 and Task 6.
- model reroute / verification notifications: Task 1 and Task 7.
- hooks warning channel: Task 1 and Task 8.
- R6 matrix update and local verification: Task 9.

Type consistency:

- Protocol DTOs use camelCase JSON to match existing app-server style.
- `ConfigWriteResponse` is shared by value write and batch write.
- `FuzzyFileSearchResult` is used by both response and session updated notification.
- Review delivery defaults to inline in implementation.
- Hook notification service uses one internal enum and maps to the ten R6 notification constructors.

Execution boundaries:

- Config write is not arbitrary file write; it is confined to service root and key allowlist.
- Git service is read-only and rejects non-read-only commands in the helper.
- Fuzzy search reuses Dasclaw traversal discipline and does not import Codex crates.
- Conversation summary is deterministic and does not invoke an LLM.
- Review start uses existing turn execution; it does not create a second review engine.
- Model notification delivery does not invent high-risk classification policy.
- Hook/warning drain does not merge event hooks with egress safety policy.
