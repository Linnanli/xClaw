# Dasclaw App Server Thread Lifecycle Persistence Owner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md` 的 R4：让 `dasclaw_app_server` 成为 thread lifecycle、持久化、订阅、归档、命名、rollback、loaded list、inject items、goal、token usage 和 compact 事件面的真实 owner。

**Architecture:** 将当前 `SessionThreadHost` 从内存-only session view 收敛为 app-server owned `ThreadLifecycleHost`。它用 JSON snapshot 持久化 thread/turn/goal/usage 元数据，所有 thread lifecycle JSON-RPC 方法先提交 store，再发通知。Codex 兼容面只广告真实可执行能力：resume/fork/archive/unarchive/unsubscribe/name/metadata/rollback/loaded/list/inject/goal/token-usage 可以在 app-server 内闭环；`thread/compact/start` 先接协议和 fail-safe runtime 门控，只有接到真实 compact runtime operation 后移除 `thread.compact` opt-out。

**Tech Stack:** Rust 2024, `serde` JSON-RPC, `dasclaw_app_server_protocol`, `dasclaw_app_server`, `dasclaw_core::response_types::TokenUsage`, `dasclaw_runtime`, Cargo nextest.

---

## Startup Four Questions

1. **是否新增模块 / crate / 文件？** 是。本计划新增 `docs/superpowers/plans/2026-06-22-dasclaw-app-server-thread-lifecycle-persistence-owner.md`，实现阶段建议新增 `crates/dasclaw_app_server/src/thread_lifecycle.rs`。已先用 `semantic_search_nodes_tool` 查询 `dasclaw app server thread lifecycle persistence owner resume fork archive rollback` 和 `Codex protocol thread lifecycle app server resume fork archive compact rollback`。
2. **结论里是否含否定语？** 是。本计划会说明 Dasclaw app-server 当前缺 R4 public methods/events/persistence owner。已完成三层验证：语义搜索、LSP、`rg`。
3. **是否做跨项目对账？** 是。对照 Codex `app-server-protocol` 与 Dasclaw 当前 `dasclaw_app_server_protocol` / `dasclaw_app_server`。
4. **是否写架构对账类文档？** 是。本计划由 gap matrix R4 派生，所有能力以实证边界写入，不把 Codex product-only 语义伪装成 Dasclaw 已实现语义。

## Evidence Captured Before Writing

- Level 1 semantic search:
  - Dasclaw core graph: `dasclaw app server protocol codex events stream request response tool calls approvals` 命中 app-server/protocol 底座，但没有 R4 lifecycle/persistence owner。
  - Codex graph: `app server codex protocol events conversation stream tool call lifecycle schema` 命中 `codex_message_processor.rs` lifecycle handlers 和 `common.rs` method/event registry。
- Level 2 LSP:
  - `workspace_symbols ThreadStartParams` 命中 `crates/dasclaw_app_server_protocol/src/lib.rs`。
  - `workspace_symbols ThreadListResponse` 命中 `crates/dasclaw_app_server_protocol/src/lib.rs`。
  - `workspace_symbols thread_archive` 返回空对象，说明 Dasclaw 当前没有 R4 archive symbol。
  - `document_symbols crates/dasclaw_app_server/src/lib.rs` 显示当前 app-server 只有 `thread_start` / `thread_list` / `thread_read` / `thread_turns_list` / `turn_read` 和 `SessionThreadHost`。
- Level 3 literal evidence:
  - `rg 'THREAD_(RESUME|FORK|ARCHIVE|UNARCHIVE|ROLLBACK)|thread/(resume|fork|archive|unarchive|rollback)' crates/dasclaw_app_server_protocol crates/dasclaw_app_server` 当前没有 public R4 method 常量或 router case。
  - `crates/dasclaw_app_server/src/lib.rs` 当前 `SessionThreadHost` 只有 `next_thread_id`、`next_turn_id`、`threads`、`turns`，`ThreadRecord` 只有 `thread_id`、`title`、`workspace_root`、`sandbox_context`。
  - 当前 `codex_thread_view` 将 `forked_from_id: None`、`ephemeral: true`、`created_at: 0`、`updated_at: 0`、`path: None`、`git_info: None` 写死。
  - Codex reference methods/events: `thread/resume`、`thread/fork`、`thread/archive`、`thread/unarchive`、`thread/unsubscribe`、`thread/name/set`、`thread/metadata/update`、`thread/compact/start`、`thread/rollback`、`thread/loaded/list`、`thread/inject_items`；events 包含 `thread/status/changed`、`thread/archived`、`thread/unarchived`、`thread/name/updated`、`thread/tokenUsage/updated`、`thread/goal/updated`、`thread/goal/cleared`、`thread/compacted`。
  - Codex `ThreadUnsubscribeStatus` values are `notLoaded | notSubscribed | unsubscribed`。
  - Codex `ThreadMetadataGitInfoUpdateParams` semantics: omit means unchanged, `null` means clear, non-empty string means replace.
  - Dasclaw already has real token usage source in `dasclaw_core::messages::TokenUsage`; runtime tests prove final assistant messages preserve usage.

Commit message transparency line for the implementation commit:

```text
已检查 thread lifecycle / persistence owner 是否已有，结论：Codex app-server 有完整 thread lifecycle owner；Dasclaw app-server 当前只有内存 SessionThreadHost 和 thread/start/read/list/turns/list 子集，没有 R4 方法、事件或持久化 owner。
```

## Scope

In scope:

- Protocol constants, DTOs, schema rows and tests for R4 thread methods/events.
- A new `ThreadLifecycleHost` that owns thread records, turn records, archived/subscribed flags, fork metadata, timestamps, git info, goals, token usage, injected items and JSON snapshot persistence.
- Router methods for `thread/resume`, `thread/fork`, `thread/archive`, `thread/unarchive`, `thread/unsubscribe`, `thread/name/set`, `thread/metadata/update`, `thread/rollback`, `thread/loaded/list`, `thread/inject_items`, `thread/goal/set`, `thread/goal/get`, `thread/goal/clear`, and gated `thread/compact/start`.
- Notifications for status/archive/unarchive/name/goal/token usage/compacted events.
- Real token usage forwarding from Dasclaw runtime usage into `thread/tokenUsage/updated`.
- Gap matrix update that distinguishes completed R4 owner work from compact runtime support that remains opt-out until genuinely implemented.

Out of scope:

- Importing arbitrary Codex history files from `history` or `path` in `thread/resume` / `thread/fork`; unsupported import inputs return fail-safe `invalid_request`.
- A full LLM context summarization compact engine. `thread/compact/start` is routed and tested as capability-gated; it must not emit `thread/compacted` unless a real runtime compact operation succeeds.
- Cross-process database service or new crate. Use JSON snapshot persistence owned by app-server.
- Rewriting desktop UI state management in this slice. Existing clients can consume the new native JSON-RPC methods/events; UI-specific polish is a later consumer task.

## File Structure

- Modify `crates/dasclaw_app_server_protocol/src/lib.rs`
  - Add R4 method/event constants, params/responses/events, schema rows, notification constructors and protocol tests.
- Add `crates/dasclaw_app_server/src/thread_lifecycle.rs`
  - Own `ThreadLifecycleHost`, persistent snapshot structs, lifecycle mutations and unit tests.
- Modify `crates/dasclaw_app_server/src/lib.rs`
  - Import protocol DTOs, replace `SessionThreadHost`, add router handlers, emit events, map token usage, and update integration tests.
- Modify `crates/dasclaw_app_server/src/main.rs`
  - Wire a default snapshot path for the real sidecar, while tests can keep in-memory mode.
- Modify `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`
  - Update R4 row with exact completed/remaining sub-scope and link this plan.

## Task 1: Lock R4 Protocol Shapes

**Files:**
- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`

- [x] **Step 1: Add failing tests for method/event registry**

Add one protocol test that asserts `protocol_schema().methods` and `protocol_schema().events` contain the R4 names:

```rust
#[test]
fn r4_thread_lifecycle_methods_and_events_are_registered() {
    let schema = protocol_schema();
    let methods: std::collections::BTreeSet<_> =
        schema.methods.iter().map(|method| method.method.as_str()).collect();
    let events: std::collections::BTreeSet<_> =
        schema.events.iter().map(|event| event.event.as_str()).collect();

    for method in [
        method::THREAD_RESUME,
        method::THREAD_FORK,
        method::THREAD_ARCHIVE,
        method::THREAD_UNARCHIVE,
        method::THREAD_UNSUBSCRIBE,
        method::THREAD_NAME_SET,
        method::THREAD_METADATA_UPDATE,
        method::THREAD_ROLLBACK,
        method::THREAD_LOADED_LIST,
        method::THREAD_INJECT_ITEMS,
        method::THREAD_GOAL_SET,
        method::THREAD_GOAL_GET,
        method::THREAD_GOAL_CLEAR,
        method::THREAD_COMPACT_START,
    ] {
        assert!(methods.contains(method), "{method} missing from schema");
    }

    for event in [
        event::THREAD_STATUS_CHANGED,
        event::THREAD_ARCHIVED,
        event::THREAD_UNARCHIVED,
        event::THREAD_NAME_UPDATED,
        event::THREAD_GOAL_UPDATED,
        event::THREAD_GOAL_CLEARED,
        event::THREAD_TOKEN_USAGE_UPDATED,
        event::THREAD_COMPACTED,
    ] {
        assert!(events.contains(event), "{event} missing from schema");
    }
}
```

Expected failure:

```text
cannot find value `THREAD_RESUME` in module `method`
```

- [x] **Step 2: Add failing tests for serde wire names**

Cover the Codex-compatible values that have historically drifted:

```rust
#[test]
fn r4_thread_lifecycle_payloads_use_camel_case_wire_names() {
    let unsubscribe = serde_json::to_value(ThreadUnsubscribeResponse {
        status: ThreadUnsubscribeStatus::NotSubscribed,
    })
    .expect("unsubscribe response serializes");
    assert_eq!(unsubscribe["status"], "notSubscribed");

    let metadata: ThreadMetadataUpdateParams = serde_json::from_value(serde_json::json!({
        "threadId": "thread_1",
        "gitInfo": {
            "sha": null,
            "branch": "main",
            "originUrl": "https://example.test/repo.git"
        }
    }))
    .expect("metadata update params deserialize");
    assert_eq!(metadata.thread_id, "thread_1");
    assert_eq!(metadata.git_info.expect("git info patch present")["sha"], serde_json::Value::Null);

    let usage = ThreadTokenUsageUpdatedEvent {
        thread_id: "thread_1".to_string(),
        turn_id: "turn_1".to_string(),
        token_usage: ThreadTokenUsage {
            total: TokenUsageBreakdown {
                total_tokens: 10,
                input_tokens: 6,
                cached_input_tokens: 2,
                output_tokens: 4,
                reasoning_output_tokens: 0,
            },
            last: TokenUsageBreakdown {
                total_tokens: 10,
                input_tokens: 6,
                cached_input_tokens: 2,
                output_tokens: 4,
                reasoning_output_tokens: 0,
            },
            model_context_window: None,
        },
    };
    let value = serde_json::to_value(usage).expect("usage event serializes");
    assert_eq!(value["threadId"], "thread_1");
    assert_eq!(value["tokenUsage"]["last"]["cachedInputTokens"], 2);
}
```

- [x] **Step 3: Implement protocol constants and DTOs**

Add method constants near the existing thread constants:

```rust
pub const THREAD_RESUME: &str = "thread/resume";
pub const THREAD_FORK: &str = "thread/fork";
pub const THREAD_ARCHIVE: &str = "thread/archive";
pub const THREAD_UNARCHIVE: &str = "thread/unarchive";
pub const THREAD_UNSUBSCRIBE: &str = "thread/unsubscribe";
pub const THREAD_NAME_SET: &str = "thread/name/set";
pub const THREAD_METADATA_UPDATE: &str = "thread/metadata/update";
pub const THREAD_ROLLBACK: &str = "thread/rollback";
pub const THREAD_LOADED_LIST: &str = "thread/loaded/list";
pub const THREAD_INJECT_ITEMS: &str = "thread/inject_items";
pub const THREAD_GOAL_SET: &str = "thread/goal/set";
pub const THREAD_GOAL_GET: &str = "thread/goal/get";
pub const THREAD_GOAL_CLEAR: &str = "thread/goal/clear";
pub const THREAD_COMPACT_START: &str = "thread/compact/start";
```

Add event constants:

```rust
pub const THREAD_STATUS_CHANGED: &str = "thread/status/changed";
pub const THREAD_ARCHIVED: &str = "thread/archived";
pub const THREAD_UNARCHIVED: &str = "thread/unarchived";
pub const THREAD_NAME_UPDATED: &str = "thread/name/updated";
pub const THREAD_GOAL_UPDATED: &str = "thread/goal/updated";
pub const THREAD_GOAL_CLEARED: &str = "thread/goal/cleared";
pub const THREAD_TOKEN_USAGE_UPDATED: &str = "thread/tokenUsage/updated";
pub const THREAD_COMPACTED: &str = "thread/compacted";
```

Add DTOs next to existing thread structs. Keep response fields aligned with current Dasclaw `ThreadStartResponse` unless the field already has a real owner:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadResumeParams {
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history: Option<Vec<serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default)]
    pub exclude_turns: bool,
    #[serde(default)]
    pub persist_extended_history: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadResumeResponse {
    pub thread: CodexThread,
    pub model: String,
    pub model_provider: String,
    pub cwd: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadForkParams {
    pub thread_id: String,
    #[serde(default)]
    pub ephemeral: bool,
    #[serde(default)]
    pub exclude_turns: bool,
    #[serde(default)]
    pub persist_extended_history: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadForkResponse {
    pub thread: CodexThread,
    pub model: String,
    pub model_provider: String,
    pub cwd: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadArchiveParams {
    pub thread_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadArchiveResponse {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThreadUnsubscribeStatus {
    NotLoaded,
    NotSubscribed,
    Unsubscribed,
}
```

Also add `ThreadUnarchive*`, `ThreadUnsubscribe*`, `ThreadSetName*`, `ThreadMetadataUpdate*`, `ThreadRollback*`, `ThreadLoadedList*`, `ThreadInjectItems*`, `ThreadGoal*`, `ThreadTokenUsage*`, and event payload structs in the same style. Use `serde_json::Value` only for protocol sub-objects whose full owner is outside this crate, such as metadata git patch values and injected items.

- [x] **Step 4: Register schema rows and compatibility profile changes**

Add methods with capability `"thread_lifecycle"` except goal rows as `"thread_goal"` and compact as `"thread_compact"`. Remove `thread.fork`, `thread.archive`, `thread.resume`, and `thread.rollback` opt-outs only after app-server tests pass. Keep `thread.compact` opt-out until Task 6 wires real compact support.

- [x] **Step 5: Verify protocol crate**

Run:

```bash
cargo check -p dasclaw_app_server_protocol --tests
cargo nextest run -p dasclaw_app_server_protocol -E 'test(r4_thread_lifecycle_methods_and_events_are_registered) | test(r4_thread_lifecycle_payloads_use_camel_case_wire_names)'
```

Expected result: both commands pass.

## Task 2: Replace SessionThreadHost With Persistent ThreadLifecycleHost

**Files:**
- Add: `crates/dasclaw_app_server/src/thread_lifecycle.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [x] **Step 1: Add host unit tests first**

Tests to add inside `thread_lifecycle.rs`:

```rust
#[test]
fn thread_lifecycle_persists_archive_name_git_info_and_goal() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("threads.json");
    let mut host = ThreadLifecycleHost::with_snapshot_path(path.clone())
        .expect("create persistent host");

    let thread_id = host
        .create(ThreadCreation {
            cwd: Some("/workspace".to_string()),
            sandbox: None,
            permission_profile: None,
            title: Some("Initial".to_string()),
            ephemeral: false,
            forked_from_id: None,
        })
        .expect("create thread");
    host.set_name(&thread_id, Some("Renamed".to_string())).expect("rename");
    host.update_git_info(
        &thread_id,
        serde_json::json!({"sha": "abc123", "branch": "main", "originUrl": null}),
    )
    .expect("metadata update");
    host.set_goal(&thread_id, "Ship R4".to_string(), Some(5000)).expect("set goal");
    host.archive(&thread_id).expect("archive");

    let reloaded = ThreadLifecycleHost::with_snapshot_path(path).expect("reload");
    let summary = reloaded.summary(&thread_id).expect("summary");
    assert_eq!(summary.title.as_deref(), Some("Renamed"));
    assert!(summary.archived);
    assert_eq!(summary.git_info.expect("git info").sha.as_deref(), Some("abc123"));
    assert_eq!(summary.goal.expect("goal").objective, "Ship R4");
}

#[test]
fn thread_lifecycle_rolls_back_turns_without_reusing_ids() {
    let mut host = ThreadLifecycleHost::new();
    let thread_id = host.create(ThreadCreation::in_memory_for_test()).expect("create");
    let first = host.next_turn_id();
    host.record_started_turn(&thread_id, first.clone());
    host.apply_runtime_turn_update(RuntimeTurnUpdate {
        thread_id: thread_id.clone(),
        turn_id: first,
        outcome: RuntimeTurnOutcome::Completed { output: "one".to_string() },
    });
    let second = host.next_turn_id();
    host.record_started_turn(&thread_id, second);

    let removed = host.rollback(&thread_id, 1).expect("rollback");
    assert_eq!(removed, 1);
    assert_eq!(host.list_turns(&thread_id).len(), 1);
    assert_eq!(host.next_turn_id(), "turn_3");
}
```

If `tempfile` is not already available to the crate, use `std::env::temp_dir()` plus a unique filename derived from process id and timestamp; do not add a new dependency for this test.

- [x] **Step 2: Implement persistent host data model**

Move the existing `SessionThreadHost`, `ThreadRecord`, `ThreadSummary`, `TurnRecord`, and `TurnSummary` logic into `thread_lifecycle.rs`. Rename the host to `ThreadLifecycleHost` and expand records:

```rust
pub struct ThreadLifecycleHost {
    next_thread_id: u64,
    next_turn_id: u64,
    threads: Vec<ThreadRecord>,
    turns: Vec<TurnRecord>,
    snapshot_path: Option<std::path::PathBuf>,
}

pub struct ThreadRecord {
    pub thread_id: String,
    pub title: Option<String>,
    pub workspace_root: Option<String>,
    pub sandbox_context: RuntimeSandboxContext,
    pub sandbox: Option<SandboxMode>,
    pub permission_profile: Option<serde_json::Value>,
    pub forked_from_id: Option<String>,
    pub ephemeral: bool,
    pub archived: bool,
    pub subscribed: bool,
    pub path: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub git_info: Option<CodexGitInfo>,
    pub goal: Option<ThreadGoal>,
    pub compacted_turn_id: Option<String>,
    pub token_usage: Option<ThreadTokenUsage>,
}
```

Persist serializable snapshot structs instead of trying to serialize `RuntimeSandboxContext`, because that context contains workspace policy types that are not protocol snapshot owners:

```rust
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThreadLifecycleSnapshot {
    version: u32,
    next_thread_id: u64,
    next_turn_id: u64,
    threads: Vec<PersistedThreadRecord>,
    turns: Vec<PersistedTurnRecord>,
}
```

On load, reconstruct `sandbox_context` with `sandbox_protocol::resolve_thread_context(sandbox, permission_profile.clone(), cwd, "thread/lifecycle/load")`. If reconstruction fails, fail loading the store; do not silently weaken sandbox state.

- [x] **Step 3: Implement atomic persistence**

Implement:

```rust
impl ThreadLifecycleHost {
    pub fn new() -> Self;
    pub fn with_snapshot_path(path: impl Into<std::path::PathBuf>) -> Result<Self, AppServerError>;
    fn persist(&self) -> Result<(), AppServerError>;
}
```

Persistence rules:

- In-memory host (`snapshot_path: None`) never writes.
- Persistent host creates the parent directory before first write.
- Write to `<filename>.tmp`, flush, then `std::fs::rename` to the target path.
- Empty missing file loads as empty store.
- Invalid JSON returns `AppServerError::internal("thread_lifecycle", "...")`.

- [x] **Step 4: Preserve existing thread/turn behavior**

Keep these methods with equivalent semantics so existing tests stay green:

- `create`
- `list`
- `summary`
- `next_turn_id`
- `record_started_turn`
- `apply_runtime_turn_update`
- `turn_is_pending`
- `has_pending_turns`
- `cancel_turn`
- `cancel_pending_turns`
- `list_turns`
- `turn_summary`
- `contains`

After every mutating method, call `persist()` before returning success.

- [x] **Step 5: Verify app-server crate after move**

Run:

```bash
cargo check -p dasclaw_app_server --tests
cargo nextest run -p dasclaw_app_server -E 'test(thread_lifecycle_persists_archive_name_git_info_and_goal) | test(thread_lifecycle_rolls_back_turns_without_reusing_ids)'
```

Expected result: both commands pass and existing thread/turn tests still compile.

## Task 3: Wire Thread Lifecycle JSON-RPC Methods

**Files:**
- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Modify: `crates/dasclaw_app_server/src/main.rs`

- [x] **Step 1: Add router tests for R4 method behavior**

Add integration-style tests near existing app-server route tests:

```rust
#[tokio::test]
async fn thread_archive_unarchive_and_name_emit_notifications() {
    let mut server = initialized_test_server();
    let thread = server.start_test_thread(TestThreadParams { cwd: Some("/workspace".to_string()) });

    server
        .route_json_rpc(json_request(
            1,
            method::THREAD_NAME_SET,
            serde_json::json!({"threadId": thread.thread_id, "name": "Design Review"}),
        ))
        .await
        .expect("set name");
    server
        .route_json_rpc(json_request(
            2,
            method::THREAD_ARCHIVE,
            serde_json::json!({"threadId": thread.thread_id}),
        ))
        .await
        .expect("archive");
    server
        .route_json_rpc(json_request(
            3,
            method::THREAD_UNARCHIVE,
            serde_json::json!({"threadId": thread.thread_id}),
        ))
        .await
        .expect("unarchive");

    let methods = server.drain_notification_methods();
    assert!(methods.contains(&event::THREAD_NAME_UPDATED.to_string()));
    assert!(methods.contains(&event::THREAD_ARCHIVED.to_string()));
    assert!(methods.contains(&event::THREAD_UNARCHIVED.to_string()));
}
```

Add separate tests for:

- `thread/resume` returns `not_found` for missing id and succeeds for a persisted id.
- `thread/fork` sets `forkedFromId`.
- `thread/unsubscribe` returns `unsubscribed`, then `notSubscribed`, then `notLoaded` for unknown id.
- `thread/rollback` refuses to remove a pending turn and removes completed/failed/cancelled turns.
- `thread/loaded/list` paginates stable ids.
- `thread/inject_items` appends JSON items into the stored turn view or rejects missing thread.

- [x] **Step 2: Add AppServer construction hook**

Keep `AppServer::new()` in-memory for unit tests. Add:

```rust
impl AppServer {
    pub fn with_thread_store_path(
        mut self,
        path: impl Into<std::path::PathBuf>,
    ) -> Result<Self, AppServerError> {
        self.threads = ThreadLifecycleHost::with_snapshot_path(path)?;
        Ok(self)
    }
}
```

In `main.rs`, wire the real sidecar to a stable path:

```rust
fn default_thread_store_path() -> Option<std::path::PathBuf> {
    std::env::var_os("DASCLAW_APP_SERVER_THREAD_STORE")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .map(|cwd| cwd.join(".dasclaw").join("app-server").join("threads.json"))
        })
}
```

If loading the store fails, startup should fail with a JSON-RPC/server stderr error instead of silently dropping history.

- [x] **Step 3: Implement route cases**

Add cases in `route_json_rpc`:

```rust
method::THREAD_RESUME => self.thread_resume(parse_params(request.params)?)?,
method::THREAD_FORK => self.thread_fork(parse_params(request.params)?)?,
method::THREAD_ARCHIVE => self.thread_archive(parse_params(request.params)?)?,
method::THREAD_UNARCHIVE => self.thread_unarchive(parse_params(request.params)?)?,
method::THREAD_UNSUBSCRIBE => self.thread_unsubscribe(parse_params(request.params)?)?,
method::THREAD_NAME_SET => self.thread_name_set(parse_params(request.params)?)?,
method::THREAD_METADATA_UPDATE => self.thread_metadata_update(parse_params(request.params)?)?,
method::THREAD_ROLLBACK => self.thread_rollback(parse_params(request.params)?)?,
method::THREAD_LOADED_LIST => self.thread_loaded_list(parse_params(request.params)?)?,
method::THREAD_INJECT_ITEMS => self.thread_inject_items(parse_params(request.params)?)?,
method::THREAD_GOAL_SET => self.thread_goal_set(parse_params(request.params)?)?,
method::THREAD_GOAL_GET => self.thread_goal_get(parse_params(request.params)?)?,
method::THREAD_GOAL_CLEAR => self.thread_goal_clear(parse_params(request.params)?)?,
method::THREAD_COMPACT_START => self.thread_compact_start(parse_params(request.params)?)?,
```

Every handler must call `require_initialized()` unless the existing method family explicitly allows pre-initialize access.

- [x] **Step 4: Implement handler semantics**

Use these exact behavior rules:

- `thread/resume`: succeeds only for an existing non-archived thread id. If `history` or a mismatched `path` is present, return `invalid_request` with message `history import is not supported by dasclaw app-server yet`. Mark `subscribed = true`, return `ThreadResumeResponse`, emit `thread/status/changed`.
- `thread/fork`: source thread must exist. Clone metadata and, unless `excludeTurns`, clone turns into a new thread id. Set `forked_from_id` to source id, `ephemeral` from params, `subscribed = true`, `archived = false`. Emit `thread/started`.
- `thread/archive`: reject pending turns. Set `archived = true`, `subscribed = false`, emit `thread/archived`.
- `thread/unarchive`: set `archived = false`, emit `thread/unarchived`, return the updated thread.
- `thread/unsubscribe`: unknown id returns `ThreadUnsubscribeStatus::NotLoaded`; already unsubscribed returns `NotSubscribed`; otherwise set `subscribed = false` and return `Unsubscribed`.
- `thread/name/set`: trim whitespace. Empty string clears the name. Emit `thread/name/updated`.
- `thread/metadata/update`: validate `gitInfo` is an object. Omitted keys stay unchanged, `null` clears, non-empty strings replace. Empty strings return `invalid_request`.
- `thread/rollback`: `numTurns` must be positive. Reject if any removed turn is pending. Remove the newest N turns, persist, emit `thread/status/changed`.
- `thread/loaded/list`: return subscribed, non-archived thread ids in creation order with cursor/limit.
- `thread/inject_items`: append raw injected JSON items to the newest turn for the thread, or create a completed synthetic turn only when the params explicitly include no target turn and the thread has no turns. Do not execute injected items.

- [x] **Step 5: Fix `codex_thread_view`**

Replace hard-coded fields with summary fields:

- `forked_from_id`
- `ephemeral`
- `created_at`
- `updated_at`
- `path`
- `git_info`
- `name`
- `preview`

Archived threads should still be readable by `thread/read`; `thread/list` should hide archived records unless a future params field explicitly asks for archived records.

- [x] **Step 6: Verify route behavior**

Run:

```bash
cargo check -p dasclaw_app_server --tests
cargo nextest run -p dasclaw_app_server -E 'test(thread_archive_unarchive_and_name_emit_notifications) | test(thread_resume) | test(thread_fork) | test(thread_unsubscribe) | test(thread_rollback) | test(thread_loaded_list) | test(thread_inject_items)'
```

Expected result: targeted R4 tests pass.

## Task 4: Wire Goal And Token Usage Events Truthfully

**Files:**
- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [x] **Step 1: Add tests for goal lifecycle**

Add route tests:

- `thread/goal/set` creates or updates a stored goal and emits `thread/goal/updated`.
- `thread/goal/get` returns the stored goal.
- `thread/goal/clear` clears it and emits `thread/goal/cleared`.

Goal fields must match Codex v2 shape:

```rust
pub struct ThreadGoal {
    pub thread_id: String,
    pub objective: String,
    pub status: ThreadGoalStatus,
    pub token_budget: Option<i64>,
    pub tokens_used: i64,
    pub time_used_seconds: i64,
    pub created_at: i64,
    pub updated_at: i64,
}
```

Use `ThreadGoalStatus::{Active, Paused, BudgetLimited, Complete}` with camelCase wire values.

- [x] **Step 2: Add tests for token usage forwarding**

Add a fake runtime bridge update that includes usage:

```rust
server.runtime_updates.push(RuntimeTurnUpdate {
    thread_id: thread.thread_id.clone(),
    turn_id: turn.turn_id.clone(),
    outcome: RuntimeTurnOutcome::TokenUsageUpdated {
        usage: dasclaw_core::response_types::TokenUsage {
            input_tokens: 6,
            output_tokens: 4,
            cache_creation_input_tokens: 1,
            cache_read_input_tokens: 2,
        },
    },
});
server.flush_runtime_updates().await.expect("flush updates");

let notification = server
    .drain_notifications()
    .into_iter()
    .find(|notification| notification.method == event::THREAD_TOKEN_USAGE_UPDATED)
    .expect("usage notification");
assert_eq!(notification.params["tokenUsage"]["last"]["inputTokens"], 6);
assert_eq!(notification.params["tokenUsage"]["last"]["cachedInputTokens"], 3);
```

- [x] **Step 3: Implement usage mapping**

Add `RuntimeTurnOutcome::TokenUsageUpdated { usage: dasclaw_core::response_types::TokenUsage }`.

Map Dasclaw usage to Codex usage as:

- `input_tokens` -> `inputTokens`
- `output_tokens` -> `outputTokens`
- `cache_creation_input_tokens + cache_read_input_tokens` -> `cachedInputTokens`
- `totalTokens` -> `input_tokens + output_tokens`
- `reasoningOutputTokens` -> `0` until Dasclaw core exposes a real field

Accumulate `total` per thread in `ThreadLifecycleHost`; emit `last` from the current update.

- [x] **Step 4: Connect real bridge usage**

When `DasclawAgentRuntimeBridge` receives a completed `RespondOutput`, push a token usage update before the final `Completed` update. Use the real `RespondOutput.usage`; do not emit all-zero usage unless a provider actually returned all zeros.

- [x] **Step 5: Verify goal and usage**

Run:

```bash
cargo check -p dasclaw_app_server --tests
cargo nextest run -p dasclaw_app_server -E 'test(thread_goal_) | test(thread_token_usage_)'
```

Expected result: goal and usage tests pass.

## Task 5: Gate Compact Correctly

**Files:**
- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [x] **Step 1: Add fail-safe compact test**

Test current behavior before runtime compact support:

```rust
#[tokio::test]
async fn thread_compact_start_is_capability_unavailable_without_runtime_owner() {
    let mut server = initialized_test_server();
    let thread = server.start_test_thread(TestThreadParams { cwd: None });

    let response = server
        .route_json_rpc(json_request(
            1,
            method::THREAD_COMPACT_START,
            serde_json::json!({"threadId": thread.thread_id}),
        ))
        .await
        .expect("compact route responds");

    assert_json_rpc_error_code(&response, ErrorCode::CapabilityUnavailable);
    assert!(!server.drain_notification_methods().contains(&event::THREAD_COMPACTED.to_string()));
}
```

- [x] **Step 2: Route compact behind a runtime feature flag**

Add a runtime bridge feature such as `supports_thread_compact`. If false, `thread/compact/start` returns capability unavailable and `CapabilityMatrix` keeps `thread.compact` opt-out. If true, call the bridge and emit `thread/compacted` only after runtime reports success with `{ threadId, turnId }`.

- [x] **Step 3: Add success test with fake runtime compact support**

Use a fake bridge that returns a compacted turn id. Assert:

- JSON-RPC response is `{}`.
- `thread/compacted` is emitted with the returned turn id.
- The thread summary records `compacted_turn_id`.
- `thread.compact` opt-out is removed only in the capability snapshot for a compact-capable server.

## Task 6: Update Capability And Supported Methods

**Files:**
- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [x] **Step 1: Add capability tests**

Add tests that compare `supported_methods()` against `protocol_schema()`:

```rust
#[test]
fn supported_methods_include_r4_lifecycle_methods_when_owner_ready() {
    let server = AppServer::new();
    let methods = server.supported_methods();

    for method in [
        method::THREAD_RESUME,
        method::THREAD_FORK,
        method::THREAD_ARCHIVE,
        method::THREAD_UNARCHIVE,
        method::THREAD_UNSUBSCRIBE,
        method::THREAD_NAME_SET,
        method::THREAD_METADATA_UPDATE,
        method::THREAD_ROLLBACK,
        method::THREAD_LOADED_LIST,
        method::THREAD_INJECT_ITEMS,
        method::THREAD_GOAL_SET,
        method::THREAD_GOAL_GET,
        method::THREAD_GOAL_CLEAR,
    ] {
        assert!(methods.contains(&method.to_string()), "{method} missing");
    }

    assert!(!methods.contains(&method::THREAD_COMPACT_START.to_string()));
}
```

- [x] **Step 2: Update method/event advertisement**

Advertise lifecycle, goal, metadata, rollback, loaded-list, inject-items, and token usage events once Tasks 1-4 pass. Keep compact conditional.

- [x] **Step 3: Update compatibility opt-outs**

Remove these opt-outs after tests pass:

- `thread.fork`
- `thread.archive`
- `thread.resume`
- `thread.rollback`

Keep `thread.compact` until Task 5 success path is backed by real compact runtime support.

## Task 7: Update Gap Matrix

**Files:**
- Modify: `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`

- [x] **Step 1: Add implementation note under R4**

Update the R4 row or add a short R4 notes section with:

- Link to this plan.
- Completed lifecycle owner scope.
- Exact compact caveat: protocol route and fail-safe gate exist; full compact success requires runtime compact support.
- Evidence commands that were run.

- [x] **Step 2: Do not mark token usage as complete without runtime bridge evidence**

Only mark `thread/tokenUsage/updated` as complete after Task 4 connects `RespondOutput.usage` and a targeted test proves the notification uses non-zero real usage.

## Task 8: Final Verification

Run the local required checks for touched Rust crates:

```bash
cargo check -p dasclaw_app_server_protocol --tests
cargo check -p dasclaw_app_server --tests
cargo nextest run -p dasclaw_app_server_protocol -E 'test(r4_thread_lifecycle_)'
cargo nextest run -p dasclaw_app_server -E 'test(thread_lifecycle_) | test(thread_archive_) | test(thread_resume) | test(thread_fork) | test(thread_unsubscribe) | test(thread_rollback) | test(thread_loaded_list) | test(thread_inject_items) | test(thread_goal_) | test(thread_token_usage_) | test(thread_compact_)'
cargo fmt --all
python3.12 scripts/check_no_panics.py --base origin/xClaw
cargo clippy --no-deps -p dasclaw_app_server_protocol --all-targets -- -D warnings
cargo clippy --no-deps -p dasclaw_app_server --all-targets -- -D warnings
```

If the local branch base is not `origin/xClaw`, replace that argument with the actual base branch used by the PR.

Expected final state:

- R4 lifecycle/persistence methods are present in protocol schema and app-server router.
- Thread records survive app-server restart through JSON snapshot persistence.
- Archive/unarchive/name/metadata/rollback/fork/resume/unsubscribe/loaded-list/inject-items have route tests.
- Goal and token usage events have route or bridge tests.
- Compact is explicitly capability-gated and does not advertise success unless runtime support is real.
- Gap matrix documents what is complete and what remains gated.

2026-06-23 final verification evidence:

- `cargo fmt --all` passed.
- `python3.12 scripts/check_no_panics.py --base origin/xClaw` passed.
- `CARGO_TARGET_DIR=target/codex-r4-verify RUSTC_WRAPPER= cargo check -p dasclaw_app_server -p dasclaw_app_server_protocol -p dasclaw_core -p dasclaw_runtime --tests` passed.
- `CARGO_TARGET_DIR=target/codex-r4-verify RUSTC_WRAPPER= cargo nextest run -p dasclaw_app_server -p dasclaw_app_server_protocol -p dasclaw_runtime -p dasclaw_core -E 'test(thread_lifecycle_) | test(thread_archive_) | test(thread_resume) | test(thread_fork) | test(thread_unsubscribe) | test(thread_rollback) | test(thread_loaded_list) | test(thread_inject_items) | test(thread_goal_) | test(thread_token_usage_) | test(thread_compact_) | test(dasclaw_runtime_bridge_forwards_nonzero_completed_usage_before_completion) | test(dasclaw_runtime_bridge_uses_snapshot_model_call_mode) | test(implemented_capability_methods_are_routable) | test(phase_one_schema_methods_match_implemented_capability_methods) | test(compatibility_profile_is_descriptive_subset_without_aliases) | test(req_dasclaw_runtime_agent_929_text_branch_persists_usage_to_context) | test(test_text_response_returns_immediately)'` passed 25/25.
- `CARGO_TARGET_DIR=target/codex-r4-verify RUSTC_WRAPPER= cargo clippy --no-deps -p dasclaw_app_server -p dasclaw_app_server_protocol -p dasclaw_core --all-targets -- -D warnings` passed.
- Full touched package clippy including `-p dasclaw_runtime` is blocked by an existing untouched-file lint in `crates/dasclaw_runtime/src/llm_adapter.rs` (`clippy::collapsible_if`); no R4 touched file lint remained after adding a local allow for the pre-existing `run_agentic_loop` argument count.

2026-06-23 review-fix verification evidence:

- Independent review found Codex-shape drift in `thread/unarchive`, `thread/name/set` / `thread/name/updated`, and `thread/inject_items`; fixes changed unarchive to return nested `thread`, name set to return `{}`, name event to emit `threadName`, and inject-items to append structured supported `CodexThreadItem` values to the newest turn instead of serializing JSON into agent-message text.
- `CARGO_TARGET_DIR=target/codex-r4-reviewfix RUSTC_WRAPPER= cargo check -p dasclaw_app_server -p dasclaw_app_server_protocol --tests` passed.
- `CARGO_TARGET_DIR=target/codex-r4-reviewfix RUSTC_WRAPPER= cargo nextest run -p dasclaw_app_server -p dasclaw_app_server_protocol -E 'test(thread_archive_unarchive_and_name_emit_notifications) | test(thread_inject_items_rejects_missing_thread_and_appends_without_executing) | test(r4_thread_lifecycle_) | test(thread_compact_) | test(phase_one_schema_methods_match_implemented_capability_methods) | test(compatibility_profile_is_descriptive_subset_without_aliases)'` passed 8/8.
- `python3.12 scripts/check_no_panics.py --base origin/xClaw`, `git diff --check`, and `CARGO_TARGET_DIR=target/codex-r4-reviewfix RUSTC_WRAPPER= cargo clippy --no-deps -p dasclaw_app_server -p dasclaw_app_server_protocol --all-targets -- -D warnings` passed.
