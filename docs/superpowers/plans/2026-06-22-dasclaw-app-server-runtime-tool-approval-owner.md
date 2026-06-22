# Dasclaw App Server Runtime Tool Approval Owner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md` 中“当前剩余优先级”的首个切片：让 app-server 成为 runtime tool / approval ServerRequest、file-change 事件和 fail-safe 回填链路的 owner。

**Architecture:** 在 `dasclaw_app_server_protocol` 先锁定 Codex v2 R1 server request / notification wire shape，再在 `dasclaw_app_server` 把现有 command approval pending store 泛化为按 request kind 路由的 server-request store。真实 capability 只在 runtime bridge 报告 owner ready 后广告；desktop 侧先提供 fail-closed 转发/回填壳，后续 UI 可以在同一个通知面上接入。

**Tech Stack:** Rust 2024, `serde` JSON-RPC, `dasclaw_app_server_protocol`, `dasclaw_app_server`, `dasclaw_runtime`, Electron main process TypeScript, React renderer TypeScript, Vitest, Cargo nextest.

---

## Startup Four Questions

1. **是否新增模块 / crate / 文件？** 是。本计划新增 `docs/superpowers/plans/2026-06-22-dasclaw-app-server-runtime-tool-approval-owner.md`。已先用 `semantic_search_nodes_tool` 查询 `dasclaw app server codex protocol gap matrix P1 implementation plan current remaining priority`，返回 0 个匹配节点；再用 `rg --files docs/superpowers/plans docs/plans | rg 'dasclaw|codex|protocol|app-server|gap|priority|p1'` 对照已有计划。
2. **结论里是否含否定语？** 是。本计划会指出 R1 目前只有 command approval 受测链路，dynamic tool / file-change / permissions / user input producer 还需要成为 app-server owner。已做三层证据：语义搜索、LSP `workspace_symbols`、`rg` 精确检索。
3. **是否做跨项目对账？** 是。对照本仓 `codex-cli-main/codex-rs/app-server-protocol/src/protocol/{common,v2}.rs` 与 Dasclaw 当前 app-server protocol/runtime/desktop 消费面。
4. **是否写架构对账类文档？** 是。本计划由 protocol gap matrix 派生，计划内每个能力只声明实现切片，不把 Codex 产品域能力合并进来。

## Evidence Captured Before Writing

- Level 1 semantic search:
  - Query: `dasclaw app server codex protocol gap matrix P1 implementation plan current remaining priority`
  - Result: `search_mode=keyword`，0 个 file node 匹配；已有相邻计划覆盖 P0-P5 历史切片，但当前 R1 owner 计划需要单独成文。
- Level 2 LSP:
  - `workspace_symbols RuntimeApprovalRequest` -> `crates/dasclaw_app_server/src/lib.rs:2535`
  - `workspace_symbols RuntimeTurnOutcome` -> `crates/dasclaw_app_server/src/lib.rs:2672`
  - `workspace_symbols CommandExecutionApprovalRequest` -> `crates/dasclaw_app_server_protocol/src/lib.rs:1852`
- Level 3 literal evidence:
  - `crates/dasclaw_app_server_protocol/src/lib.rs` 已有 server request 常量：`item/tool/call`、`item/tool/requestUserInput`、`item/permissions/requestApproval`、`item/fileChange/requestApproval`。
  - `crates/dasclaw_app_server/src/lib.rs` 已有 `PendingServerRequestStore`、`approval/respond`、command approval timeout / malformed / interrupt / shutdown fail-safe 测试。
  - `DasclawAgentRuntimeBridge` 当前把 `AgentEvent::ToolCallStart` 压成 `RuntimeCommandOutputDeltaUpdate`，把 `AgentEvent::ToolResult` 压成 `RuntimeToolResultUpdate`，R1 需要改为更精确的 tool lifecycle / request producer。
  - Codex v2 源定义 R1 server request：`item/commandExecution/requestApproval`、`item/fileChange/requestApproval`、`item/tool/requestUserInput`、`item/permissions/requestApproval`、`item/tool/call`；notification：`item/autoApprovalReview/started`、`item/autoApprovalReview/completed`、`item/fileChange/outputDelta`、`item/fileChange/patchUpdated`、`serverRequest/resolved`。

## Scope

In scope:

- `dasclaw_app_server_protocol` 增加 R1 强类型 payload：
  - `DynamicToolCallParams` / `DynamicToolCallResponse`
  - `ToolRequestUserInputParams` / `ToolRequestUserInputResponse`
  - `FileChangeRequestApprovalParams` / `FileChangeRequestApprovalResponse`
  - `PermissionsRequestApprovalParams` / `PermissionsRequestApprovalResponse`
  - `FileChangeOutputDeltaEvent` / `FileChangePatchUpdatedEvent`
  - `AutoApprovalReviewStartedEvent` / `AutoApprovalReviewCompletedEvent`
- `CapabilityMatrix` 增加 R1 availability helper，只有全部 owner-ready 子能力满足时把 approval/tools 对应 methods/events 广告为 implemented。
- `dasclaw_app_server` 把 pending server request 从 command approval 专用结构改成 `PendingServerRequestKind` 路由。
- `RuntimeTurnOutcome` 增加 R1 producer variants，让 fake bridge 和真实 bridge 都走同一组 app-server emit 函数。
- `approval/respond` 保持兼容现有 renderer；JSON-RPC client response 支持按 pending kind 解码 dynamic tool / user input / file-change / permissions response。
- desktop main/renderer 扩展 server request union，默认 fail-closed 回填，避免新 request kind 挂起。
- 更新 gap matrix 的 R1 完成判定与本计划链接。

Out of scope:

- 不实现完整 Codex product domains，例如 account、plugin marketplace、external agent import、Windows sandbox setup。
- 不把 R4 thread lifecycle / persistence owner 合入本计划。
- 不实现真实 sandbox enforcement；R2 继续独立。
- 不实现完整 file patch apply engine；本计划只接 file-change request、output delta、patch updated 的 app-server owner surface。
- 不新增 crate 或外部依赖；protocol crate 使用 `serde_json::Value` 表达 Codex permission/profile 子对象，避免把 `dasclaw_protocol` 拉进 app-server protocol crate。

## File Structure

- Modify `crates/dasclaw_app_server_protocol/src/lib.rs`
  - 继续作为 JSON-RPC method/event/server request 常量、wire payload、capability matrix、protocol schema 和协议测试 owner。
  - 增加 R1 request/response/event structs、event constants、notification constructors、schema rows 和 tests。
- Modify `crates/dasclaw_app_server/src/lib.rs`
  - 继续作为 runtime bridge、routing、pending server request、notification emission 和 integration tests owner。
  - 泛化 pending request kind，新增 R1 runtime update variants 和 emit/decode/apply 函数。
- Modify `desktop-app/src/shared/appServerApi.ts`
  - 扩展 renderer 可见 server request union 和 method-specific response union。
- Modify `desktop-app/src/main/appServerManager.ts`
  - 继续作为 app-server process/RPC owner。
  - 转发 R1 server request；对 renderer 未处理的 request 发送 method-specific fail-closed response。
- Modify `desktop-app/src/main/appServerManager.test.ts`
  - 覆盖 R1 request forwarding 和 unsupported fallback response。
- Modify `desktop-app/src/renderer/src/hooks/useDasclawAssistantRuntime.ts`
  - 把临时 auto-reject helper 从 approval-only 扩展到 R1 request union。
- Modify `desktop-app/src/renderer/src/lib/assistantMessages.test.ts`
  - 覆盖 renderer 收到 file-change / dynamic tool / user-input request 时的 fail-closed 回填。
- Modify `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`
  - R1 实现后更新完成判定，不挪动 R2-R7。

## Task 1: Lock R1 Protocol Wire Types

**Files:**
- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`

- [x] **Step 1: Add failing protocol tests for R1 request and notification shapes**

Add these tests inside `#[cfg(test)] mod tests` near `tool_lifecycle_server_requests_are_stable`.

```rust
#[test]
fn r1_server_requests_serialize_codex_v2_shapes() {
    let dynamic_tool = JsonRpcServerRequest::new(
        "tool_1",
        server_request::ITEM_TOOL_CALL,
        DynamicToolCallParams {
            thread_id: "thread_1".to_string(),
            turn_id: "turn_1".to_string(),
            call_id: "call_1".to_string(),
            namespace: Some("client".to_string()),
            tool: "open_url".to_string(),
            arguments: serde_json::json!({"url":"https://example.test"}),
        },
    )
    .expect("dynamic tool server request should serialize");
    let user_input = JsonRpcServerRequest::new(
        "input_1",
        server_request::ITEM_TOOL_REQUEST_USER_INPUT,
        ToolRequestUserInputParams {
            thread_id: "thread_1".to_string(),
            turn_id: "turn_1".to_string(),
            item_id: "turn_1:tool:ask".to_string(),
            questions: vec![ToolRequestUserInputQuestion {
                id: "choice".to_string(),
                header: "Mode".to_string(),
                question: "Pick a mode".to_string(),
                is_other: false,
                is_secret: false,
                options: Some(vec![ToolRequestUserInputOption {
                    label: "Safe".to_string(),
                    description: "Continue with read-only work".to_string(),
                }]),
            }],
        },
    )
    .expect("tool user input server request should serialize");
    let file_change = JsonRpcServerRequest::new(
        "file_1",
        server_request::ITEM_FILE_CHANGE_REQUEST_APPROVAL,
        FileChangeRequestApprovalParams {
            thread_id: "thread_1".to_string(),
            turn_id: "turn_1".to_string(),
            item_id: "turn_1:file:patch".to_string(),
            reason: Some("apply generated patch".to_string()),
            grant_root: Some("/workspace".to_string()),
        },
    )
    .expect("file change approval server request should serialize");
    let permissions = JsonRpcServerRequest::new(
        "perm_1",
        server_request::ITEM_PERMISSIONS_REQUEST_APPROVAL,
        PermissionsRequestApprovalParams {
            thread_id: "thread_1".to_string(),
            turn_id: "turn_1".to_string(),
            item_id: "turn_1:permission:network".to_string(),
            cwd: "/workspace".to_string(),
            reason: Some("network access required".to_string()),
            permissions: serde_json::json!({"network":{"allow":["example.test"]}}),
        },
    )
    .expect("permissions approval server request should serialize");

    assert_eq!(dynamic_tool.method, server_request::ITEM_TOOL_CALL);
    assert_eq!(dynamic_tool.params["callId"], "call_1");
    assert_eq!(dynamic_tool.params["namespace"], "client");
    assert_eq!(dynamic_tool.params["tool"], "open_url");
    assert_eq!(dynamic_tool.params["arguments"]["url"], "https://example.test");

    assert_eq!(user_input.method, server_request::ITEM_TOOL_REQUEST_USER_INPUT);
    assert_eq!(user_input.params["questions"][0]["id"], "choice");
    assert_eq!(user_input.params["questions"][0]["options"][0]["label"], "Safe");

    assert_eq!(file_change.method, server_request::ITEM_FILE_CHANGE_REQUEST_APPROVAL);
    assert_eq!(file_change.params["itemId"], "turn_1:file:patch");
    assert_eq!(file_change.params["grantRoot"], "/workspace");

    assert_eq!(permissions.method, server_request::ITEM_PERMISSIONS_REQUEST_APPROVAL);
    assert_eq!(permissions.params["cwd"], "/workspace");
    assert_eq!(permissions.params["permissions"]["network"]["allow"][0], "example.test");
}

#[test]
fn r1_response_payloads_deserialize_by_kind() {
    let dynamic: DynamicToolCallResponse = serde_json::from_value(serde_json::json!({
        "contentItems": [{"type": "inputText", "text": "opened"}],
        "success": true
    }))
    .expect("dynamic tool response should deserialize");
    let user_input: ToolRequestUserInputResponse = serde_json::from_value(serde_json::json!({
        "answers": {
            "choice": {"answers": ["Safe"]}
        }
    }))
    .expect("tool user input response should deserialize");
    let file_change: FileChangeRequestApprovalResponse = serde_json::from_value(
        serde_json::json!({"decision": "acceptForSession"}),
    )
    .expect("file change response should deserialize");
    let permissions: PermissionsRequestApprovalResponse = serde_json::from_value(serde_json::json!({
        "permissions": {"network":{"allow":["example.test"]}},
        "scope": "session",
        "strictAutoReview": true
    }))
    .expect("permissions response should deserialize");

    assert_eq!(
        dynamic.content_items,
        vec![DynamicToolCallOutputContentItem::InputText {
            text: "opened".to_string(),
        }]
    );
    assert_eq!(
        user_input.answers["choice"].answers,
        vec!["Safe".to_string()]
    );
    assert_eq!(file_change.decision, FileChangeApprovalDecision::AcceptForSession);
    assert_eq!(permissions.scope, PermissionGrantScope::Session);
    assert_eq!(permissions.strict_auto_review, Some(true));
}

#[test]
fn r1_file_change_and_auto_review_notifications_serialize() {
    let output = ServerNotification::file_change_output_delta(FileChangeOutputDeltaEvent {
        thread_id: "thread_1".to_string(),
        turn_id: "turn_1".to_string(),
        item_id: "turn_1:file:patch".to_string(),
        delta: "patched src/lib.rs".to_string(),
    })
    .expect("file change output delta should serialize");
    let patch = ServerNotification::file_change_patch_updated(FileChangePatchUpdatedEvent {
        thread_id: "thread_1".to_string(),
        turn_id: "turn_1".to_string(),
        item_id: "turn_1:file:patch".to_string(),
        changes: vec![FileUpdateChange {
            path: "src/lib.rs".to_string(),
            kind: FileUpdateKind::Update,
            unified_diff: "@@ -1 +1 @@".to_string(),
        }],
    })
    .expect("file change patch update should serialize");
    let started = ServerNotification::auto_approval_review_started(AutoApprovalReviewStartedEvent {
        thread_id: "thread_1".to_string(),
        turn_id: "turn_1".to_string(),
        review_id: "review_1".to_string(),
        target_item_id: Some("turn_1:tool:shell".to_string()),
        review: GuardianApprovalReview {
            status: "inProgress".to_string(),
            risk_level: Some("low".to_string()),
            user_authorization: None,
            rationale: Some("read-only command".to_string()),
        },
        action: "review".to_string(),
    })
    .expect("auto approval review started should serialize");
    let completed =
        ServerNotification::auto_approval_review_completed(AutoApprovalReviewCompletedEvent {
            thread_id: "thread_1".to_string(),
            turn_id: "turn_1".to_string(),
            review_id: "review_1".to_string(),
            target_item_id: Some("turn_1:tool:shell".to_string()),
            review: GuardianApprovalReview {
                status: "approved".to_string(),
                risk_level: Some("low".to_string()),
                user_authorization: Some("allowed".to_string()),
                rationale: Some("command is read-only".to_string()),
            },
            action: "review".to_string(),
        })
        .expect("auto approval review completed should serialize");

    assert_eq!(output.method, event::ITEM_FILE_CHANGE_OUTPUT_DELTA);
    assert_eq!(output.params["delta"], "patched src/lib.rs");
    assert_eq!(patch.method, event::ITEM_FILE_CHANGE_PATCH_UPDATED);
    assert_eq!(patch.params["changes"][0]["kind"], "update");
    assert_eq!(started.method, event::ITEM_AUTO_APPROVAL_REVIEW_STARTED);
    assert_eq!(started.params["review"]["status"], "inProgress");
    assert_eq!(completed.method, event::ITEM_AUTO_APPROVAL_REVIEW_COMPLETED);
    assert_eq!(completed.params["review"]["userAuthorization"], "allowed");
}
```

- [x] **Step 2: Run the new tests and confirm the compile failure**

Run:

```bash
cargo nextest run -p dasclaw_app_server_protocol -E 'test(r1_server_requests_serialize_codex_v2_shapes) | test(r1_response_payloads_deserialize_by_kind) | test(r1_file_change_and_auto_review_notifications_serialize)'
```

Expected: FAIL at compile time with missing items such as `DynamicToolCallParams`, `FileChangeOutputDeltaEvent`, and `ServerNotification::file_change_output_delta`.

- [x] **Step 3: Implement the protocol types and constants**

Change the top import:

```rust
use std::{collections::HashMap, fmt};
```

Add these constants:

```rust
pub mod event {
    // Keep existing constants.
    pub const ITEM_FILE_CHANGE_OUTPUT_DELTA: &str = "item/fileChange/outputDelta";
    pub const ITEM_FILE_CHANGE_PATCH_UPDATED: &str = "item/fileChange/patchUpdated";
}
```

Add these structs after `PermissionsApprovalRequest`:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicToolCallParams {
    pub thread_id: String,
    pub turn_id: String,
    pub call_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    pub tool: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicToolCallResponse {
    pub content_items: Vec<DynamicToolCallOutputContentItem>,
    pub success: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DynamicToolCallOutputContentItem {
    #[serde(rename_all = "camelCase")]
    InputText { text: String },
    #[serde(rename_all = "camelCase")]
    InputImage { image_url: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRequestUserInputOption {
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRequestUserInputQuestion {
    pub id: String,
    pub header: String,
    pub question: String,
    #[serde(default)]
    pub is_other: bool,
    #[serde(default)]
    pub is_secret: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<ToolRequestUserInputOption>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRequestUserInputParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub questions: Vec<ToolRequestUserInputQuestion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRequestUserInputAnswer {
    pub answers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRequestUserInputResponse {
    pub answers: HashMap<String, ToolRequestUserInputAnswer>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChangeRequestApprovalParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grant_root: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileChangeApprovalDecision {
    Accept,
    AcceptForSession,
    Decline,
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChangeRequestApprovalResponse {
    pub decision: FileChangeApprovalDecision,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionsRequestApprovalParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub permissions: serde_json::Value,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionGrantScope {
    #[default]
    Turn,
    Session,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionsRequestApprovalResponse {
    pub permissions: serde_json::Value,
    #[serde(default)]
    pub scope: PermissionGrantScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict_auto_review: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileUpdateKind {
    Add,
    Delete,
    Update,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileUpdateChange {
    pub path: String,
    pub kind: FileUpdateKind,
    pub unified_diff: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChangeOutputDeltaEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub delta: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChangePatchUpdatedEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub changes: Vec<FileUpdateChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardianApprovalReview {
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_level: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_authorization: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoApprovalReviewStartedEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub review_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_item_id: Option<String>,
    pub review: GuardianApprovalReview,
    pub action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoApprovalReviewCompletedEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub review_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_item_id: Option<String>,
    pub review: GuardianApprovalReview,
    pub action: String,
}
```

Add notification constructors inside `impl ServerNotification`:

```rust
pub fn file_change_output_delta(
    event: FileChangeOutputDeltaEvent,
) -> Result<Self, serde_json::Error> {
    Self::new(event::ITEM_FILE_CHANGE_OUTPUT_DELTA, event)
}

pub fn file_change_patch_updated(
    event: FileChangePatchUpdatedEvent,
) -> Result<Self, serde_json::Error> {
    Self::new(event::ITEM_FILE_CHANGE_PATCH_UPDATED, event)
}

pub fn auto_approval_review_started(
    event: AutoApprovalReviewStartedEvent,
) -> Result<Self, serde_json::Error> {
    Self::new(event::ITEM_AUTO_APPROVAL_REVIEW_STARTED, event)
}

pub fn auto_approval_review_completed(
    event: AutoApprovalReviewCompletedEvent,
) -> Result<Self, serde_json::Error> {
    Self::new(event::ITEM_AUTO_APPROVAL_REVIEW_COMPLETED, event)
}
```

- [x] **Step 4: Run the protocol tests and confirm they pass**

Run:

```bash
cargo nextest run -p dasclaw_app_server_protocol -E 'test(r1_server_requests_serialize_codex_v2_shapes) | test(r1_response_payloads_deserialize_by_kind) | test(r1_file_change_and_auto_review_notifications_serialize)'
```

Expected: PASS.

- [x] **Step 5: Commit the protocol contract**

```bash
git add crates/dasclaw_app_server_protocol/src/lib.rs
git commit -m $'feat: add r1 runtime tool approval protocol types\n\n已检查 R1 runtime tool approval 是否已有，结论：已有 command approval 基础，dynamic tool/file-change/permissions/user-input typed payload 需要新增。'
```

## Task 2: Advertise R1 Capabilities Only When Owner Ready

**Files:**
- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [x] **Step 1: Add failing capability/schema tests**

Add these tests to `crates/dasclaw_app_server_protocol/src/lib.rs`.

```rust
#[test]
fn r1_capability_helper_advertises_runtime_tool_approval_surface_only_when_ready() {
    let base = CapabilityMatrix::phase_one();
    assert_eq!(base.approval.status, CapabilityStatus::Declared);
    assert_eq!(base.tools.status, CapabilityStatus::Declared);

    let ready = CapabilityMatrix::phase_one().with_runtime_tool_approval(
        RuntimeToolApprovalAvailability {
            command_approval: true,
            dynamic_tool_call: true,
            tool_user_input: true,
            permissions_approval: true,
            file_change_approval: true,
            file_change_events: true,
            auto_approval_review: true,
        },
    );

    assert_eq!(ready.approval.status, CapabilityStatus::Implemented);
    assert_eq!(ready.tools.status, CapabilityStatus::Implemented);
    assert!(ready.approval.methods.contains(&method::APPROVAL_RESPOND.to_string()));
    assert!(ready.approval.events.contains(&server_request::ITEM_TOOL_CALL.to_string()));
    assert!(ready.approval.events.contains(&server_request::ITEM_TOOL_REQUEST_USER_INPUT.to_string()));
    assert!(ready.approval.events.contains(&server_request::ITEM_PERMISSIONS_REQUEST_APPROVAL.to_string()));
    assert!(ready.approval.events.contains(&server_request::ITEM_FILE_CHANGE_REQUEST_APPROVAL.to_string()));
    assert!(ready.tools.events.contains(&event::ITEM_FILE_CHANGE_OUTPUT_DELTA.to_string()));
    assert!(ready.tools.events.contains(&event::ITEM_FILE_CHANGE_PATCH_UPDATED.to_string()));
    assert!(ready.tools.events.contains(&event::ITEM_AUTO_APPROVAL_REVIEW_STARTED.to_string()));
    assert!(ready.tools.events.contains(&event::ITEM_AUTO_APPROVAL_REVIEW_COMPLETED.to_string()));
}

#[test]
fn phase_one_schema_includes_r1_server_requests_and_events() {
    let schema = ProtocolSchemaResponse::phase_one(
        CapabilityMatrix::phase_one().with_runtime_tool_approval(
            RuntimeToolApprovalAvailability {
                command_approval: true,
                dynamic_tool_call: true,
                tool_user_input: true,
                permissions_approval: true,
                file_change_approval: true,
                file_change_events: true,
                auto_approval_review: true,
            },
        ),
    );
    let event_names = schema
        .events
        .iter()
        .map(|event| event.event.as_str())
        .collect::<Vec<_>>();

    assert!(event_names.contains(&server_request::ITEM_TOOL_CALL));
    assert!(event_names.contains(&server_request::ITEM_TOOL_REQUEST_USER_INPUT));
    assert!(event_names.contains(&server_request::ITEM_PERMISSIONS_REQUEST_APPROVAL));
    assert!(event_names.contains(&server_request::ITEM_FILE_CHANGE_REQUEST_APPROVAL));
    assert!(event_names.contains(&event::ITEM_FILE_CHANGE_OUTPUT_DELTA));
    assert!(event_names.contains(&event::ITEM_FILE_CHANGE_PATCH_UPDATED));
    assert!(event_names.contains(&event::ITEM_AUTO_APPROVAL_REVIEW_STARTED));
    assert!(event_names.contains(&event::ITEM_AUTO_APPROVAL_REVIEW_COMPLETED));
}
```

Add this test to `crates/dasclaw_app_server/src/lib.rs`.

```rust
#[test]
fn runtime_features_gate_r1_capability_advertising() {
    let bridge = Arc::new(R1RuntimeBridge::default());
    let server = initialized_server_with_bridge(bridge);

    assert_eq!(server.capabilities.approval.status, CapabilityStatus::Implemented);
    assert!(server
        .capabilities
        .approval
        .events
        .contains(&server_request::ITEM_TOOL_CALL.to_string()));
    assert!(server
        .capabilities
        .approval
        .events
        .contains(&server_request::ITEM_FILE_CHANGE_REQUEST_APPROVAL.to_string()));
    assert!(server
        .capabilities
        .tools
        .events
        .contains(&event::ITEM_FILE_CHANGE_PATCH_UPDATED.to_string()));
}
```

- [x] **Step 2: Run the tests and confirm the failure**

Run:

```bash
cargo nextest run -p dasclaw_app_server_protocol -p dasclaw_app_server -E 'test(r1_capability_helper_advertises_runtime_tool_approval_surface_only_when_ready) | test(phase_one_schema_includes_r1_server_requests_and_events) | test(runtime_features_gate_r1_capability_advertising)'
```

Expected: FAIL because `RuntimeToolApprovalAvailability`, schema rows, and `R1RuntimeBridge` do not exist yet.

- [x] **Step 3: Add the protocol capability helper and schema rows**

Add this struct near `AppServerP5Availability`:

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeToolApprovalAvailability {
    pub command_approval: bool,
    pub dynamic_tool_call: bool,
    pub tool_user_input: bool,
    pub permissions_approval: bool,
    pub file_change_approval: bool,
    pub file_change_events: bool,
    pub auto_approval_review: bool,
}

impl RuntimeToolApprovalAvailability {
    #[must_use]
    pub fn all_ready(self) -> bool {
        self.command_approval
            && self.dynamic_tool_call
            && self.tool_user_input
            && self.permissions_approval
            && self.file_change_approval
            && self.file_change_events
            && self.auto_approval_review
    }
}
```

Add this method to `impl CapabilityMatrix`:

```rust
#[must_use]
pub fn with_runtime_tool_approval(mut self, availability: RuntimeToolApprovalAvailability) -> Self {
    if availability.command_approval {
        let mut approval_events = vec![
            server_request::ITEM_COMMAND_EXECUTION_REQUEST_APPROVAL,
            event::SERVER_REQUEST_RESOLVED,
        ];
        if availability.dynamic_tool_call {
            approval_events.push(server_request::ITEM_TOOL_CALL);
        }
        if availability.tool_user_input {
            approval_events.push(server_request::ITEM_TOOL_REQUEST_USER_INPUT);
        }
        if availability.permissions_approval {
            approval_events.push(server_request::ITEM_PERMISSIONS_REQUEST_APPROVAL);
        }
        if availability.file_change_approval {
            approval_events.push(server_request::ITEM_FILE_CHANGE_REQUEST_APPROVAL);
        }
        self.approval =
            Capability::implemented("approval", &[method::APPROVAL_RESPOND], &approval_events);
    }

    if availability.file_change_events || availability.auto_approval_review {
        let mut tool_events = vec![
            event::ITEM_COMMAND_EXECUTION_OUTPUT_DELTA,
            event::ITEM_COMMAND_EXECUTION_TERMINAL_INTERACTION,
        ];
        if availability.file_change_events {
            tool_events.push(event::ITEM_FILE_CHANGE_OUTPUT_DELTA);
            tool_events.push(event::ITEM_FILE_CHANGE_PATCH_UPDATED);
        }
        if availability.auto_approval_review {
            tool_events.push(event::ITEM_AUTO_APPROVAL_REVIEW_STARTED);
            tool_events.push(event::ITEM_AUTO_APPROVAL_REVIEW_COMPLETED);
        }
        self.tools = Capability::implemented("tools", &[], &tool_events);
    }

    self
}
```

Update `with_p3_approval_tool_sandbox` so it uses the helper for the existing command-only path:

```rust
self = self.with_runtime_tool_approval(RuntimeToolApprovalAvailability {
    command_approval: true,
    dynamic_tool_call: false,
    tool_user_input: false,
    permissions_approval: false,
    file_change_approval: false,
    file_change_events: false,
    auto_approval_review: false,
});
```

Add these `EventSchema` rows to `phase_one_events()`:

```rust
EventSchema::new(
    server_request::ITEM_TOOL_CALL,
    "approval",
    "DynamicToolCallParams",
),
EventSchema::new(
    server_request::ITEM_TOOL_REQUEST_USER_INPUT,
    "approval",
    "ToolRequestUserInputParams",
),
EventSchema::new(
    server_request::ITEM_PERMISSIONS_REQUEST_APPROVAL,
    "approval",
    "PermissionsRequestApprovalParams",
),
EventSchema::new(
    server_request::ITEM_FILE_CHANGE_REQUEST_APPROVAL,
    "approval",
    "FileChangeRequestApprovalParams",
),
EventSchema::new(
    event::ITEM_FILE_CHANGE_OUTPUT_DELTA,
    "tools",
    "FileChangeOutputDeltaEvent",
),
EventSchema::new(
    event::ITEM_FILE_CHANGE_PATCH_UPDATED,
    "tools",
    "FileChangePatchUpdatedEvent",
),
EventSchema::new(
    event::ITEM_AUTO_APPROVAL_REVIEW_STARTED,
    "tools",
    "AutoApprovalReviewStartedEvent",
),
EventSchema::new(
    event::ITEM_AUTO_APPROVAL_REVIEW_COMPLETED,
    "tools",
    "AutoApprovalReviewCompletedEvent",
),
```

- [x] **Step 4: Gate app-server capabilities from runtime features**

In `crates/dasclaw_app_server/src/lib.rs`, import the new types:

```rust
RuntimeToolApprovalAvailability,
```

Extend `RuntimeBridgeFeatures`:

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeBridgeFeatures {
    pub approval: bool,
    pub tools: bool,
    pub sandbox: bool,
    pub dynamic_tool_call: bool,
    pub tool_user_input: bool,
    pub permissions_approval: bool,
    pub file_change_approval: bool,
    pub file_change_events: bool,
    pub auto_approval_review: bool,
}

impl RuntimeBridgeFeatures {
    fn runtime_tool_approval_availability(self) -> RuntimeToolApprovalAvailability {
        RuntimeToolApprovalAvailability {
            command_approval: self.approval,
            dynamic_tool_call: self.dynamic_tool_call,
            tool_user_input: self.tool_user_input,
            permissions_approval: self.permissions_approval,
            file_change_approval: self.file_change_approval,
            file_change_events: self.file_change_events,
            auto_approval_review: self.auto_approval_review,
        }
    }
}
```

In `AppServer::with_runtime_bridge`, replace the approval/tool capability setup with:

```rust
if features.approval || features.tools {
    server.capabilities = server
        .capabilities
        .with_runtime_tool_approval(features.runtime_tool_approval_availability());
}
if features.sandbox {
    server.capabilities = server.capabilities.with_runtime_sandbox_ready();
}
```

Add this bridge in the test module:

```rust
#[derive(Debug, Default)]
struct R1RuntimeBridge;

impl RuntimeBridge for R1RuntimeBridge {
    fn features(&self) -> RuntimeBridgeFeatures {
        RuntimeBridgeFeatures {
            approval: true,
            tools: true,
            sandbox: true,
            dynamic_tool_call: true,
            tool_user_input: true,
            permissions_approval: true,
            file_change_approval: true,
            file_change_events: true,
            auto_approval_review: true,
        }
    }

    fn start_turn(&self, _request: RuntimeTurnStartRequest) -> Result<(), RuntimeBridgeError> {
        Ok(())
    }

    fn cancel_turn(&self, _request: RuntimeTurnCancelRequest) -> Result<(), RuntimeBridgeError> {
        Ok(())
    }

    fn shutdown(&self) {}
}
```

- [x] **Step 5: Run capability tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server_protocol -p dasclaw_app_server -E 'test(r1_capability_helper_advertises_runtime_tool_approval_surface_only_when_ready) | test(phase_one_schema_includes_r1_server_requests_and_events) | test(runtime_features_gate_r1_capability_advertising)'
```

Expected: PASS.

- [x] **Step 6: Commit capability gating**

```bash
git add crates/dasclaw_app_server_protocol/src/lib.rs crates/dasclaw_app_server/src/lib.rs
git commit -m "feat: gate r1 tool approval capabilities on runtime owner readiness"
```

## Task 3: Generalize Pending Server Request Routing

**Files:**
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [x] **Step 1: Add failing tests for method-specific response decoding**

Add these tests near the existing approval response tests.

```rust
#[test]
fn dynamic_tool_client_response_is_decoded_by_pending_kind() {
    let bridge = Arc::new(R1RuntimeBridge::default());
    let mut server = initialized_server_with_bridge(bridge.clone());
    server.insert_pending_server_request_for_test(
        "tool_1",
        PendingServerRequestKind::DynamicToolCall,
        "runtime_tool_1",
        "thread_1",
        "turn_1",
    );

    let response = server.handle_json_rpc(
        r#"{"jsonrpc":"2.0","id":"tool_1","result":{"contentItems":[{"type":"inputText","text":"ok"}],"success":true}}"#,
    );

    assert!(response.is_none());
    assert_eq!(
        bridge.resolutions(),
        vec![RuntimeServerRequestResolution {
            request_id: "runtime_tool_1".to_string(),
            payload: RuntimeServerRequestResponse::DynamicTool(DynamicToolCallResponse {
                content_items: vec![DynamicToolCallOutputContentItem::InputText {
                    text: "ok".to_string(),
                }],
                success: true,
            }),
        }]
    );
}

#[test]
fn malformed_dynamic_tool_response_fails_turn_and_clears_pending_request() {
    let bridge = Arc::new(R1RuntimeBridge::default());
    let mut server = initialized_server_with_bridge(bridge.clone());
    server.insert_pending_server_request_for_test(
        "tool_1",
        PendingServerRequestKind::DynamicToolCall,
        "runtime_tool_1",
        "thread_1",
        "turn_1",
    );

    let response = server.handle_json_rpc(
        r#"{"jsonrpc":"2.0","id":"tool_1","result":{"success":true}}"#,
    );

    assert!(response.is_none());
    assert!(bridge.resolutions().is_empty());
    let notifications = server.drain_notifications();
    assert!(notifications.iter().any(|notification| {
        notification.method == "serverRequest/resolved"
            && notification.params["requestId"] == "tool_1"
            && notification.params["outcome"] == "failed"
    }));

    server.expire_pending_server_requests_for_tests(Duration::from_secs(301));
    let later = server.drain_notifications();
    assert!(!later.iter().any(|notification| {
        notification.method == "serverRequest/resolved"
            && notification.params["requestId"] == "tool_1"
    }));
}
```

- [x] **Step 2: Run the new tests and confirm the failure**

Run:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(dynamic_tool_client_response_is_decoded_by_pending_kind) | test(malformed_dynamic_tool_response_fails_turn_and_clears_pending_request)'
```

Expected: FAIL because `PendingServerRequestKind`, `RuntimeServerRequestResolution`, and the test helper do not exist.

- [x] **Step 3: Add pending kind and runtime response payloads**

Add imports from `dasclaw_app_server_protocol`:

```rust
DynamicToolCallOutputContentItem, DynamicToolCallResponse,
FileChangeApprovalDecision, FileChangeRequestApprovalResponse,
PermissionsRequestApprovalResponse, ToolRequestUserInputResponse,
```

Replace `PendingServerRequest` with:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingServerRequestKind {
    CommandApproval,
    DynamicToolCall,
    ToolUserInput,
    FileChangeApproval,
    PermissionsApproval,
}

#[derive(Debug, Clone)]
struct PendingServerRequest {
    request_id: String,
    runtime_request_id: String,
    kind: PendingServerRequestKind,
    thread_id: String,
    turn_id: String,
    created_at: Instant,
}
```

Add runtime response types near `RuntimeApprovalDecision`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeServerRequestResolution {
    pub request_id: String,
    pub payload: RuntimeServerRequestResponse,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeServerRequestResponse {
    Approval(dasclaw_runtime::ApprovalDecision),
    DynamicTool(DynamicToolCallResponse),
    ToolUserInput(ToolRequestUserInputResponse),
    FileChange(FileChangeApprovalDecision),
    Permissions(PermissionsRequestApprovalResponse),
}
```

Add this default method to `RuntimeBridge`:

```rust
fn resolve_server_request(
    &self,
    resolution: RuntimeServerRequestResolution,
) -> Result<(), RuntimeBridgeError> {
    match resolution.payload {
        RuntimeServerRequestResponse::Approval(decision) => {
            self.resolve_approval(RuntimeApprovalDecision {
                request_id: resolution.request_id,
                decision,
            })
        }
        RuntimeServerRequestResponse::DynamicTool(_)
        | RuntimeServerRequestResponse::ToolUserInput(_)
        | RuntimeServerRequestResponse::FileChange(_)
        | RuntimeServerRequestResponse::Permissions(_) => Err(RuntimeBridgeError::fatal(
            "runtime bridge does not support this server request response kind",
        )),
    }
}
```

- [x] **Step 4: Route client responses by pending kind**

Replace the successful branch of `handle_client_response` with:

```rust
match self.apply_server_request_response_id(response.id, result) {
    Ok(()) => {}
    Err(reason) => self.fail_pending_approval_response(Value::Null, reason),
}
```

Add this method:

```rust
fn apply_server_request_response_id(
    &mut self,
    response_id: Value,
    result: Value,
) -> Result<(), String> {
    let Some(pending) = self.pending_server_requests.remove(&response_id) else {
        self.emit_failed_server_request_resolution(
            response_id,
            "unknown or expired server request".to_string(),
        );
        return Ok(());
    };

    let payload = decode_server_request_response(pending.kind, result).map_err(|reason| {
        self.fail_pending_server_request(pending.clone(), reason.clone());
        reason
    })?;
    self.apply_server_request_resolution(pending, payload);
    Ok(())
}

fn apply_server_request_resolution(
    &mut self,
    pending: PendingServerRequest,
    payload: RuntimeServerRequestResponse,
) {
    let outcome = match &payload {
        RuntimeServerRequestResponse::Approval(dasclaw_runtime::ApprovalDecision::Reject { .. })
        | RuntimeServerRequestResponse::FileChange(FileChangeApprovalDecision::Decline)
        | RuntimeServerRequestResponse::FileChange(FileChangeApprovalDecision::Cancel) => {
            ServerRequestResolutionOutcome::Rejected
        }
        _ => ServerRequestResolutionOutcome::Approved,
    };
    let result = self.runtime_bridge.resolve_server_request(
        RuntimeServerRequestResolution {
            request_id: pending.runtime_request_id.clone(),
            payload,
        },
    );
    match result {
        Ok(()) => {
            self.notifications.emit_server_request_resolved(ServerRequestResolvedEvent {
                request_id: pending.request_id,
                thread_id: Some(pending.thread_id),
                turn_id: Some(pending.turn_id),
                outcome,
                reason: None,
            });
        }
        Err(error) => {
            let reason = error.message;
            self.notifications.emit_server_request_resolved(ServerRequestResolvedEvent {
                request_id: pending.request_id,
                thread_id: Some(pending.thread_id.clone()),
                turn_id: Some(pending.turn_id.clone()),
                outcome: ServerRequestResolutionOutcome::Failed,
                reason: Some(reason.clone()),
            });
            self.fail_pending_turn(pending.thread_id, pending.turn_id, reason);
        }
    }
}

fn fail_pending_server_request(&mut self, pending: PendingServerRequest, reason: String) {
    let _ = self.runtime_bridge.resolve_server_request(
        RuntimeServerRequestResolution {
            request_id: pending.runtime_request_id.clone(),
            payload: RuntimeServerRequestResponse::Approval(
                dasclaw_runtime::ApprovalDecision::Reject {
                    reason: Some(reason.clone()),
                },
            ),
        },
    );
    self.notifications.emit_server_request_resolved(ServerRequestResolvedEvent {
        request_id: pending.request_id,
        thread_id: Some(pending.thread_id.clone()),
        turn_id: Some(pending.turn_id.clone()),
        outcome: ServerRequestResolutionOutcome::Failed,
        reason: Some(reason.clone()),
    });
    self.fail_pending_turn(pending.thread_id, pending.turn_id, reason);
}
```

Add this free function near `json_rpc_id_to_request_id`:

```rust
fn decode_server_request_response(
    kind: PendingServerRequestKind,
    result: Value,
) -> Result<RuntimeServerRequestResponse, String> {
    match kind {
        PendingServerRequestKind::CommandApproval => {
            let payload: ApprovalResponsePayload =
                serde_json::from_value(result).map_err(|error| error.to_string())?;
            Ok(RuntimeServerRequestResponse::Approval(
                approval_decision_to_runtime(payload.decision),
            ))
        }
        PendingServerRequestKind::DynamicToolCall => {
            let payload: DynamicToolCallResponse =
                serde_json::from_value(result).map_err(|error| error.to_string())?;
            Ok(RuntimeServerRequestResponse::DynamicTool(payload))
        }
        PendingServerRequestKind::ToolUserInput => {
            let payload: ToolRequestUserInputResponse =
                serde_json::from_value(result).map_err(|error| error.to_string())?;
            Ok(RuntimeServerRequestResponse::ToolUserInput(payload))
        }
        PendingServerRequestKind::FileChangeApproval => {
            let payload: FileChangeRequestApprovalResponse =
                serde_json::from_value(result).map_err(|error| error.to_string())?;
            Ok(RuntimeServerRequestResponse::FileChange(payload.decision))
        }
        PendingServerRequestKind::PermissionsApproval => {
            let payload: PermissionsRequestApprovalResponse =
                serde_json::from_value(result).map_err(|error| error.to_string())?;
            Ok(RuntimeServerRequestResponse::Permissions(payload))
        }
    }
}

fn approval_decision_to_runtime(
    decision: AppServerApprovalDecision,
) -> dasclaw_runtime::ApprovalDecision {
    match decision {
        AppServerApprovalDecision::Approve => dasclaw_runtime::ApprovalDecision::Approve,
        AppServerApprovalDecision::ApproveAlways => dasclaw_runtime::ApprovalDecision::ApproveAlways,
        AppServerApprovalDecision::Reject { reason } => {
            dasclaw_runtime::ApprovalDecision::Reject { reason }
        }
    }
}
```

- [x] **Step 5: Keep `approval/respond` compatibility**

Change `apply_approval_response_id` so it wraps the old `ApprovalResponsePayload` into the new route:

```rust
fn apply_approval_response_id(
    &mut self,
    response_id: Value,
    payload: ApprovalResponsePayload,
) -> Result<(), AppServerError> {
    let result = serde_json::to_value(payload).map_err(|error| {
        AppServerError::invalid_request("approval", format!("invalid approval payload: {error}"))
    })?;
    let _ = self.apply_server_request_response_id(response_id, result);
    Ok(())
}
```

When inserting command approval requests in `emit_approval_server_request`, include `kind: PendingServerRequestKind::CommandApproval`.

- [x] **Step 6: Add test helper and R1 bridge resolution storage**

Add this helper under `#[cfg(test)]` in `impl AppServer`:

```rust
#[cfg(test)]
fn insert_pending_server_request_for_test(
    &mut self,
    request_id: &str,
    kind: PendingServerRequestKind,
    runtime_request_id: &str,
    thread_id: &str,
    turn_id: &str,
) {
    self.pending_server_requests.insert(PendingServerRequest {
        request_id: request_id.to_string(),
        runtime_request_id: runtime_request_id.to_string(),
        kind,
        thread_id: thread_id.to_string(),
        turn_id: turn_id.to_string(),
        created_at: Instant::now(),
    });
}
```

Extend `R1RuntimeBridge`:

```rust
#[derive(Debug, Default)]
struct R1RuntimeBridge {
    resolutions: Mutex<Vec<RuntimeServerRequestResolution>>,
}

impl R1RuntimeBridge {
    fn resolutions(&self) -> Vec<RuntimeServerRequestResolution> {
        self.resolutions.lock().expect("resolutions lock").clone()
    }
}

impl RuntimeBridge for R1RuntimeBridge {
    fn resolve_server_request(
        &self,
        resolution: RuntimeServerRequestResolution,
    ) -> Result<(), RuntimeBridgeError> {
        self.resolutions
            .lock()
            .expect("resolutions lock")
            .push(resolution);
        Ok(())
    }
}
```

- [x] **Step 7: Run focused app-server tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(dynamic_tool_client_response_is_decoded_by_pending_kind) | test(malformed_dynamic_tool_response_fails_turn_and_clears_pending_request) | test(approval_response_records_runtime_decision_and_emits_resolved_notification) | test(expired_approval_request_rejects_fail_safe_and_late_response_is_unknown)'
```

Expected: PASS.

- [x] **Step 8: Commit pending routing**

```bash
git add crates/dasclaw_app_server/src/lib.rs
git commit -m "refactor: route app-server pending requests by response kind"
```

## Task 4: Emit R1 Runtime Producers

**Files:**
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [x] **Step 1: Add failing tests for every R1 producer**

Add this test near existing approval bridge tests.

```rust
#[test]
fn r1_runtime_updates_emit_server_requests_and_notifications() {
    let bridge = Arc::new(R1ProducingBridge);
    let mut server = initialized_server_with_bridge(bridge);
    let thread_id = server
        .create_thread_for_test(TestThreadParams { cwd: None })
        .expect("thread should be created")
        .thread_id;
    let _turn = server
        .turn_start(TurnStartParams {
            thread_id: thread_id.clone(),
            input: text_input("run r1 producers".to_string()),
            cwd: None,
            model: None,
            summary: None,
            sandbox_policy: None,
            permission_profile: None,
        })
        .expect("turn should start");

    let values = json_rpc_values(server.drain_json_rpc_notifications());
    let methods = values
        .iter()
        .filter_map(|value| value["method"].as_str())
        .collect::<Vec<_>>();

    assert!(methods.contains(&"item/tool/call"));
    assert!(methods.contains(&"item/tool/requestUserInput"));
    assert!(methods.contains(&"item/fileChange/requestApproval"));
    assert!(methods.contains(&"item/permissions/requestApproval"));
    assert!(methods.contains(&"item/fileChange/outputDelta"));
    assert!(methods.contains(&"item/fileChange/patchUpdated"));
    assert!(methods.contains(&"item/autoApprovalReview/started"));
    assert!(methods.contains(&"item/autoApprovalReview/completed"));
    assert!(values.iter().any(|value| {
        value["method"] == "item/tool/call"
            && value["params"]["tool"] == "open_url"
            && value["params"]["arguments"]["url"] == "https://example.test"
    }));
    assert!(values.iter().any(|value| {
        value["method"] == "item/fileChange/patchUpdated"
            && value["params"]["changes"][0]["path"] == "src/lib.rs"
    }));
}
```

- [x] **Step 2: Run the test and confirm the failure**

Run:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(r1_runtime_updates_emit_server_requests_and_notifications)'
```

Expected: FAIL because `R1ProducingBridge` and `RuntimeTurnOutcome` variants are missing.

- [x] **Step 3: Add runtime request/update structs and outcome variants**

Add these structs near existing runtime update types:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeDynamicToolCallRequest {
    pub request_id: String,
    pub call_id: String,
    pub namespace: Option<String>,
    pub tool: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeToolUserInputRequest {
    pub request_id: String,
    pub item_id: String,
    pub questions: Vec<ToolRequestUserInputQuestion>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeFileChangeApprovalRequest {
    pub request_id: String,
    pub item_id: String,
    pub reason: Option<String>,
    pub grant_root: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimePermissionsApprovalRequest {
    pub request_id: String,
    pub item_id: String,
    pub cwd: String,
    pub reason: Option<String>,
    pub permissions: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeFileChangeOutputDeltaUpdate {
    pub item_id: String,
    pub delta: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeFileChangePatchUpdatedUpdate {
    pub item_id: String,
    pub changes: Vec<FileUpdateChange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeAutoApprovalReviewUpdate {
    pub review_id: String,
    pub target_item_id: Option<String>,
    pub review: GuardianApprovalReview,
    pub action: String,
}
```

Extend `RuntimeTurnOutcome`:

```rust
DynamicToolCallRequested {
    request: RuntimeDynamicToolCallRequest,
},
ToolUserInputRequested {
    request: RuntimeToolUserInputRequest,
},
FileChangeApprovalRequested {
    request: RuntimeFileChangeApprovalRequest,
},
PermissionsApprovalRequested {
    request: RuntimePermissionsApprovalRequest,
},
FileChangeOutputDelta {
    update: RuntimeFileChangeOutputDeltaUpdate,
},
FileChangePatchUpdated {
    update: RuntimeFileChangePatchUpdatedUpdate,
},
AutoApprovalReviewStarted {
    update: RuntimeAutoApprovalReviewUpdate,
},
AutoApprovalReviewCompleted {
    update: RuntimeAutoApprovalReviewUpdate,
},
```

Add push helpers to `RuntimeTurnUpdateSink`:

```rust
pub fn dynamic_tool_call_requested(
    &self,
    thread_id: String,
    turn_id: String,
    request: RuntimeDynamicToolCallRequest,
) {
    self.push(RuntimeTurnUpdate {
        thread_id,
        turn_id,
        outcome: RuntimeTurnOutcome::DynamicToolCallRequested { request },
    });
}

pub fn tool_user_input_requested(
    &self,
    thread_id: String,
    turn_id: String,
    request: RuntimeToolUserInputRequest,
) {
    self.push(RuntimeTurnUpdate {
        thread_id,
        turn_id,
        outcome: RuntimeTurnOutcome::ToolUserInputRequested { request },
    });
}

pub fn file_change_approval_requested(
    &self,
    thread_id: String,
    turn_id: String,
    request: RuntimeFileChangeApprovalRequest,
) {
    self.push(RuntimeTurnUpdate {
        thread_id,
        turn_id,
        outcome: RuntimeTurnOutcome::FileChangeApprovalRequested { request },
    });
}

pub fn permissions_approval_requested(
    &self,
    thread_id: String,
    turn_id: String,
    request: RuntimePermissionsApprovalRequest,
) {
    self.push(RuntimeTurnUpdate {
        thread_id,
        turn_id,
        outcome: RuntimeTurnOutcome::PermissionsApprovalRequested { request },
    });
}

pub fn file_change_output_delta(
    &self,
    thread_id: String,
    turn_id: String,
    update: RuntimeFileChangeOutputDeltaUpdate,
) {
    self.push(RuntimeTurnUpdate {
        thread_id,
        turn_id,
        outcome: RuntimeTurnOutcome::FileChangeOutputDelta { update },
    });
}

pub fn file_change_patch_updated(
    &self,
    thread_id: String,
    turn_id: String,
    update: RuntimeFileChangePatchUpdatedUpdate,
) {
    self.push(RuntimeTurnUpdate {
        thread_id,
        turn_id,
        outcome: RuntimeTurnOutcome::FileChangePatchUpdated { update },
    });
}
```

- [x] **Step 4: Add app-server emit functions**

Add these methods to `impl AppServer` near `emit_approval_server_request`:

```rust
fn emit_dynamic_tool_call_server_request(
    &mut self,
    thread_id: String,
    turn_id: String,
    request: RuntimeDynamicToolCallRequest,
) {
    let request_id = format!("tool_{}", request.request_id);
    let runtime_request_id = request.request_id.clone();
    let server_request = match JsonRpcServerRequest::new(
        request_id.clone(),
        server_request::ITEM_TOOL_CALL,
        DynamicToolCallParams {
            thread_id: thread_id.clone(),
            turn_id: turn_id.clone(),
            call_id: request.call_id,
            namespace: request.namespace,
            tool: request.tool,
            arguments: request.arguments,
        },
    ) {
        Ok(request) => request,
        Err(error) => {
            self.fail_pending_turn(
                thread_id,
                turn_id,
                format!("failed to serialize dynamic tool server request: {error}"),
            );
            return;
        }
    };
    self.pending_server_requests.insert(PendingServerRequest {
        request_id: request_id.clone(),
        runtime_request_id,
        kind: PendingServerRequestKind::DynamicToolCall,
        thread_id,
        turn_id,
        created_at: Instant::now(),
    });
    self.notifications.emit_server_request(server_request);
}

fn emit_tool_user_input_server_request(
    &mut self,
    thread_id: String,
    turn_id: String,
    request: RuntimeToolUserInputRequest,
) {
    let request_id = format!("tool_input_{}", request.request_id);
    let runtime_request_id = request.request_id.clone();
    let server_request = match JsonRpcServerRequest::new(
        request_id.clone(),
        server_request::ITEM_TOOL_REQUEST_USER_INPUT,
        ToolRequestUserInputParams {
            thread_id: thread_id.clone(),
            turn_id: turn_id.clone(),
            item_id: request.item_id,
            questions: request.questions,
        },
    ) {
        Ok(request) => request,
        Err(error) => {
            self.fail_pending_turn(
                thread_id,
                turn_id,
                format!("failed to serialize tool user input server request: {error}"),
            );
            return;
        }
    };
    self.pending_server_requests.insert(PendingServerRequest {
        request_id: request_id.clone(),
        runtime_request_id,
        kind: PendingServerRequestKind::ToolUserInput,
        thread_id,
        turn_id,
        created_at: Instant::now(),
    });
    self.notifications.emit_server_request(server_request);
}
```

Add matching functions for file-change and permissions:

```rust
fn emit_file_change_approval_server_request(
    &mut self,
    thread_id: String,
    turn_id: String,
    request: RuntimeFileChangeApprovalRequest,
) {
    let request_id = format!("file_change_{}", request.request_id);
    let runtime_request_id = request.request_id.clone();
    let server_request = match JsonRpcServerRequest::new(
        request_id.clone(),
        server_request::ITEM_FILE_CHANGE_REQUEST_APPROVAL,
        FileChangeRequestApprovalParams {
            thread_id: thread_id.clone(),
            turn_id: turn_id.clone(),
            item_id: request.item_id,
            reason: request.reason,
            grant_root: request.grant_root,
        },
    ) {
        Ok(request) => request,
        Err(error) => {
            self.fail_pending_turn(
                thread_id,
                turn_id,
                format!("failed to serialize file change approval request: {error}"),
            );
            return;
        }
    };
    self.pending_server_requests.insert(PendingServerRequest {
        request_id: request_id.clone(),
        runtime_request_id,
        kind: PendingServerRequestKind::FileChangeApproval,
        thread_id,
        turn_id,
        created_at: Instant::now(),
    });
    self.notifications.emit_server_request(server_request);
}

fn emit_permissions_approval_server_request(
    &mut self,
    thread_id: String,
    turn_id: String,
    request: RuntimePermissionsApprovalRequest,
) {
    let request_id = format!("permissions_{}", request.request_id);
    let runtime_request_id = request.request_id.clone();
    let server_request = match JsonRpcServerRequest::new(
        request_id.clone(),
        server_request::ITEM_PERMISSIONS_REQUEST_APPROVAL,
        PermissionsRequestApprovalParams {
            thread_id: thread_id.clone(),
            turn_id: turn_id.clone(),
            item_id: request.item_id,
            cwd: request.cwd,
            reason: request.reason,
            permissions: request.permissions,
        },
    ) {
        Ok(request) => request,
        Err(error) => {
            self.fail_pending_turn(
                thread_id,
                turn_id,
                format!("failed to serialize permissions approval request: {error}"),
            );
            return;
        }
    };
    self.pending_server_requests.insert(PendingServerRequest {
        request_id: request_id.clone(),
        runtime_request_id,
        kind: PendingServerRequestKind::PermissionsApproval,
        thread_id,
        turn_id,
        created_at: Instant::now(),
    });
    self.notifications.emit_server_request(server_request);
}
```

- [x] **Step 5: Drain R1 runtime updates**

Add match arms in `drain_runtime_turn_updates`:

```rust
RuntimeTurnOutcome::DynamicToolCallRequested { request } => {
    self.emit_dynamic_tool_call_server_request(update.thread_id, update.turn_id, request);
}
RuntimeTurnOutcome::ToolUserInputRequested { request } => {
    self.emit_tool_user_input_server_request(update.thread_id, update.turn_id, request);
}
RuntimeTurnOutcome::FileChangeApprovalRequested { request } => {
    self.emit_file_change_approval_server_request(update.thread_id, update.turn_id, request);
}
RuntimeTurnOutcome::PermissionsApprovalRequested { request } => {
    self.emit_permissions_approval_server_request(update.thread_id, update.turn_id, request);
}
RuntimeTurnOutcome::FileChangeOutputDelta { update: file_update } => {
    self.notifications
        .emit_file_change_output_delta(FileChangeOutputDeltaEvent {
            thread_id: update.thread_id,
            turn_id: update.turn_id,
            item_id: file_update.item_id,
            delta: file_update.delta,
        });
}
RuntimeTurnOutcome::FileChangePatchUpdated { update: file_update } => {
    self.notifications
        .emit_file_change_patch_updated(FileChangePatchUpdatedEvent {
            thread_id: update.thread_id,
            turn_id: update.turn_id,
            item_id: file_update.item_id,
            changes: file_update.changes,
        });
}
RuntimeTurnOutcome::AutoApprovalReviewStarted { update: review_update } => {
    self.notifications
        .emit_auto_approval_review_started(AutoApprovalReviewStartedEvent {
            thread_id: update.thread_id,
            turn_id: update.turn_id,
            review_id: review_update.review_id,
            target_item_id: review_update.target_item_id,
            review: review_update.review,
            action: review_update.action,
        });
}
RuntimeTurnOutcome::AutoApprovalReviewCompleted { update: review_update } => {
    self.notifications
        .emit_auto_approval_review_completed(AutoApprovalReviewCompletedEvent {
            thread_id: update.thread_id,
            turn_id: update.turn_id,
            review_id: review_update.review_id,
            target_item_id: review_update.target_item_id,
            review: review_update.review,
            action: review_update.action,
        });
}
```

- [x] **Step 6: Add R1 producing test bridge**

Add this test bridge:

```rust
#[derive(Debug)]
struct R1ProducingBridge;

impl RuntimeBridge for R1ProducingBridge {
    fn features(&self) -> RuntimeBridgeFeatures {
        RuntimeBridgeFeatures {
            approval: true,
            tools: true,
            sandbox: true,
            dynamic_tool_call: true,
            tool_user_input: true,
            permissions_approval: true,
            file_change_approval: true,
            file_change_events: true,
            auto_approval_review: true,
        }
    }

    fn start_turn(&self, request: RuntimeTurnStartRequest) -> Result<(), RuntimeBridgeError> {
        request.updates.dynamic_tool_call_requested(
            request.thread_id.clone(),
            request.turn_id.clone(),
            RuntimeDynamicToolCallRequest {
                request_id: "00000000-0000-0000-0000-000000000101".to_string(),
                call_id: "call_1".to_string(),
                namespace: Some("client".to_string()),
                tool: "open_url".to_string(),
                arguments: serde_json::json!({"url":"https://example.test"}),
            },
        );
        request.updates.tool_user_input_requested(
            request.thread_id.clone(),
            request.turn_id.clone(),
            RuntimeToolUserInputRequest {
                request_id: "00000000-0000-0000-0000-000000000102".to_string(),
                item_id: format!("{}:tool:ask", request.turn_id),
                questions: vec![ToolRequestUserInputQuestion {
                    id: "choice".to_string(),
                    header: "Mode".to_string(),
                    question: "Pick a mode".to_string(),
                    is_other: false,
                    is_secret: false,
                    options: None,
                }],
            },
        );
        request.updates.file_change_approval_requested(
            request.thread_id.clone(),
            request.turn_id.clone(),
            RuntimeFileChangeApprovalRequest {
                request_id: "00000000-0000-0000-0000-000000000103".to_string(),
                item_id: format!("{}:file:patch", request.turn_id),
                reason: Some("apply patch".to_string()),
                grant_root: Some("/workspace".to_string()),
            },
        );
        request.updates.permissions_approval_requested(
            request.thread_id.clone(),
            request.turn_id.clone(),
            RuntimePermissionsApprovalRequest {
                request_id: "00000000-0000-0000-0000-000000000104".to_string(),
                item_id: format!("{}:permission:network", request.turn_id),
                cwd: "/workspace".to_string(),
                reason: Some("network access".to_string()),
                permissions: serde_json::json!({"network":{"allow":["example.test"]}}),
            },
        );
        request.updates.file_change_output_delta(
            request.thread_id.clone(),
            request.turn_id.clone(),
            RuntimeFileChangeOutputDeltaUpdate {
                item_id: format!("{}:file:patch", request.turn_id),
                delta: "applying patch".to_string(),
            },
        );
        request.updates.file_change_patch_updated(
            request.thread_id.clone(),
            request.turn_id.clone(),
            RuntimeFileChangePatchUpdatedUpdate {
                item_id: format!("{}:file:patch", request.turn_id),
                changes: vec![FileUpdateChange {
                    path: "src/lib.rs".to_string(),
                    kind: FileUpdateKind::Update,
                    unified_diff: "@@ -1 +1 @@".to_string(),
                }],
            },
        );
        request.updates.push(RuntimeTurnUpdate {
            thread_id: request.thread_id.clone(),
            turn_id: request.turn_id.clone(),
            outcome: RuntimeTurnOutcome::AutoApprovalReviewStarted {
                update: RuntimeAutoApprovalReviewUpdate {
                    review_id: "review_1".to_string(),
                    target_item_id: Some(format!("{}:tool:shell", request.turn_id)),
                    review: GuardianApprovalReview {
                        status: "inProgress".to_string(),
                        risk_level: Some("low".to_string()),
                        user_authorization: None,
                        rationale: Some("read-only".to_string()),
                    },
                    action: "review".to_string(),
                },
            },
        });
        request.updates.push(RuntimeTurnUpdate {
            thread_id: request.thread_id,
            turn_id: request.turn_id,
            outcome: RuntimeTurnOutcome::AutoApprovalReviewCompleted {
                update: RuntimeAutoApprovalReviewUpdate {
                    review_id: "review_1".to_string(),
                    target_item_id: Some("turn_1:tool:shell".to_string()),
                    review: GuardianApprovalReview {
                        status: "approved".to_string(),
                        risk_level: Some("low".to_string()),
                        user_authorization: Some("allowed".to_string()),
                        rationale: Some("read-only".to_string()),
                    },
                    action: "review".to_string(),
                },
            },
        });
        Ok(())
    }

    fn cancel_turn(&self, _request: RuntimeTurnCancelRequest) -> Result<(), RuntimeBridgeError> {
        Ok(())
    }

    fn shutdown(&self) {}
}
```

- [x] **Step 7: Run producer tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(r1_runtime_updates_emit_server_requests_and_notifications)'
```

Expected: PASS.

- [x] **Step 8: Commit R1 producers**

```bash
git add crates/dasclaw_app_server/src/lib.rs
git commit -m "feat: emit r1 runtime tool approval server requests"
```

## Task 5: Wire DasclawAgentRuntimeBridge Into R1 Events

**Files:**
- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Modify: `crates/dasclaw_app_server/src/main.rs`

- [x] **Step 1: Add failing bridge-level test for real runtime event mapping**

Add this test near the existing `DasclawAgentRuntimeBridge` tests in `crates/dasclaw_app_server/src/lib.rs`.

```rust
#[tokio::test]
async fn agent_runtime_bridge_maps_tool_events_to_r1_surface() {
    let factory_executor = Arc::new(CountingExecutor::default());
    let bridge = DasclawAgentRuntimeBridge::builder()
        .with_responder(ScriptedResponder::tool_then_done("open_url"))
        .with_tool_executor(Arc::clone(&factory_executor) as Arc<dyn dasclaw_runtime::ToolExecutor>)
        .build();
    let updates = RuntimeTurnUpdateSink::new();

    bridge
        .start_turn(RuntimeTurnStartRequest {
            thread_id: "thread_1".to_string(),
            turn_id: "turn_1".to_string(),
            prompt: "open the docs".to_string(),
            model_provider: runtime_model_provider_fixture(),
            reasoning_summary: ReasoningSummary::default(),
            sandbox_context: RuntimeSandboxContext::default(),
            updates: updates.clone(),
        })
        .expect("runtime turn should start");

    wait_for_runtime_updates(&updates, 2).await;
    let drained = updates.drain();
    assert!(drained.iter().any(|update| {
        matches!(
            &update.outcome,
            RuntimeTurnOutcome::DynamicToolCallRequested { request }
                if request.tool == "open_url"
                    && request.arguments["url"] == "https://example.test"
        )
    }));
    assert!(drained.iter().any(|update| {
        matches!(
            &update.outcome,
            RuntimeTurnOutcome::ToolResult { update }
                if update.content == "opened"
        )
    }));
}
```

- [x] **Step 2: Run the bridge test and confirm the failure**

Run:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(agent_runtime_bridge_maps_tool_events_to_r1_surface)'
```

Expected: FAIL because `ToolCallStart` is still mapped to command output delta.

- [x] **Step 3: Replace generic tool-start command delta mapping**

In `DasclawAgentRuntimeBridge` event loop, replace the `AgentEvent::ToolCallStart` arm with:

```rust
Ok(dasclaw_runtime::AgentEvent::ToolCallStart { name, arguments }) => {
    let call_id = tool_call_id_from_arguments(&arguments).unwrap_or_else(|| name.clone());
    tool_item_keys_by_name.insert(name.clone(), call_id.clone());
    if is_client_dynamic_tool(&name) {
        event_updates.dynamic_tool_call_requested(
            event_thread_id.clone(),
            event_turn_id.clone(),
            RuntimeDynamicToolCallRequest {
                request_id: call_id.clone(),
                call_id,
                namespace: Some("client".to_string()),
                tool: name,
                arguments,
            },
        );
    } else {
        event_updates.command_output_delta(
            event_thread_id.clone(),
            event_turn_id.clone(),
            RuntimeCommandOutputDeltaUpdate {
                item_id: runtime_tool_item_id(&event_turn_id, &call_id),
                delta: tool_call_started_delta(&name),
            },
        );
    }
}
```

Add this helper near `tool_call_id_from_arguments`:

```rust
fn is_client_dynamic_tool(name: &str) -> bool {
    name.strip_prefix("client.").is_some() || matches!(name, "open_url" | "request_user_input")
}
```

For `AgentEvent::ApprovalNeeded`, keep command approval for shell-like tools and route non-command permission requests:

```rust
let command = command_from_arguments(&tool_arguments);
if command.is_some() {
    event_updates.approval_requested(
        event_thread_id.clone(),
        event_turn_id.clone(),
        RuntimeApprovalRequest {
            request_id: request_id.clone(),
            tool_call_id,
            tool_name,
            command,
            description,
            display_parameters,
            allow_always,
        },
    );
} else {
    event_updates.permissions_approval_requested(
        event_thread_id.clone(),
        event_turn_id.clone(),
        RuntimePermissionsApprovalRequest {
            request_id,
            item_id: runtime_tool_item_id(&event_turn_id, &tool_call_id),
            cwd: event_bridge.cwd.to_string_lossy().to_string(),
            reason: Some(description),
            permissions: display_parameters,
        },
    );
}
```

- [x] **Step 4: Mirror the stdio test bridge in `main.rs`**

In `crates/dasclaw_app_server/src/main.rs`, update `RuntimeBridgeFeatures` literals to include the new fields:

```rust
RuntimeBridgeFeatures {
    approval: true,
    tools: true,
    sandbox: true,
    dynamic_tool_call: false,
    tool_user_input: false,
    permissions_approval: false,
    file_change_approval: false,
    file_change_events: false,
    auto_approval_review: false,
}
```

For any `RuntimeBridgeFeatures { approval: false, tools: false, sandbox: false }` literals, use:

```rust
RuntimeBridgeFeatures::default()
```

- [x] **Step 5: Run bridge tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(agent_runtime_bridge_maps_tool_events_to_r1_surface) | test(runtime_features_gate_r1_capability_advertising)'
```

Expected: PASS.

- [x] **Step 6: Commit runtime bridge mapping**

```bash
git add crates/dasclaw_app_server/src/lib.rs crates/dasclaw_app_server/src/main.rs
git commit -m "feat: map runtime tool events onto r1 app-server surface"
```

## Task 6: Extend Desktop ServerRequest Forwarding

**Files:**
- Modify: `desktop-app/src/shared/appServerApi.ts`
- Modify: `desktop-app/src/main/appServerManager.ts`
- Modify: `desktop-app/src/main/appServerManager.test.ts`
- Modify: `desktop-app/src/renderer/src/hooks/useDasclawAssistantRuntime.ts`
- Modify: `desktop-app/src/renderer/src/lib/assistantMessages.test.ts`

- [x] **Step 1: Add failing main-process tests**

Add this test to `desktop-app/src/main/appServerManager.test.ts` near the existing approval forwarding test.

```ts
it('forwards r1 server requests and fail-closes unsupported dynamic tool responses', async () => {
  const fake = new FakeAppServerClient()
  const manager = new AppServerManager({ clientFactory: () => fake })
  const notifications: AppServerNotification[] = []
  manager.onNotification((notification) => notifications.push(notification))

  await manager.start()
  fake.emitServerRequest({
    jsonrpc: '2.0',
    id: 'tool_1',
    method: 'item/tool/call',
    params: {
      threadId: 'thread_1',
      turnId: 'turn_1',
      callId: 'call_1',
      namespace: 'client',
      tool: 'open_url',
      arguments: { url: 'https://example.test' }
    }
  })

  expect(notifications).toContainEqual(
    expect.objectContaining({
      requestId: 'tool_1',
      method: 'item/tool/call',
      params: expect.objectContaining({
        tool: 'open_url'
      })
    })
  )

  await manager.respondServerRequest('tool_1', {
    contentItems: [{ type: 'inputText', text: 'opened' }],
    success: true
  })

  expect(fake.responses).toContainEqual({
    id: 'tool_1',
    result: {
      contentItems: [{ type: 'inputText', text: 'opened' }],
      success: true
    }
  })
})
```

- [x] **Step 2: Run the main-process test and confirm the failure**

Run:

```bash
cd desktop-app && pnpm vitest run src/main/appServerManager.test.ts -t 'forwards r1 server requests'
```

Expected: FAIL because `respondServerRequest` and R1 notification types do not exist.

- [x] **Step 3: Extend shared API types**

Replace `AppServerApprovalRequest` with:

```ts
export type AppServerServerRequestMethod =
  | 'item/commandExecution/requestApproval'
  | 'item/permissions/requestApproval'
  | 'item/fileChange/requestApproval'
  | 'item/tool/requestUserInput'
  | 'item/tool/call'

export type AppServerServerRequest = {
  requestId: string | number
  hostId: string
  method: AppServerServerRequestMethod
  params: Record<string, unknown>
}

export type AppServerApprovalRequest = AppServerServerRequest & {
  method: 'item/commandExecution/requestApproval' | 'item/permissions/requestApproval'
}

export type AppServerServerRequestResponse =
  | { decision: AppServerApprovalDecision }
  | { decision: 'accept' | 'acceptForSession' | 'decline' | 'cancel' }
  | { contentItems: Array<{ type: 'inputText'; text: string } | { type: 'inputImage'; imageUrl: string }>; success: boolean }
  | { answers: Record<string, { answers: string[] }> }
  | { permissions: unknown; scope?: 'turn' | 'session'; strictAutoReview?: boolean }
```

Extend `DesktopAppServerApi`:

```ts
respondServerRequest(
  requestId: string | number,
  response: AppServerServerRequestResponse,
  options?: AppServerRequestOptions
): Promise<void>
```

- [x] **Step 4: Implement main-process forwarding**

In `AppServerManager`, add:

```ts
async respondServerRequest(
  requestId: string | number,
  response: AppServerServerRequestResponse,
  options: AppServerRequestOptions = {}
): Promise<void> {
  this.assertHost(options.hostId)
  this.requireClient().respond(requestId, response)
}
```

Replace `handleServerRequest` with:

```ts
private handleServerRequest(request: JsonRpcServerRequest): void {
  if (isForwardedServerRequestMethod(request.method)) {
    this.emitNotification({
      hostId: this.hostId,
      requestId: request.id,
      method: request.method,
      params: asRecord(request.params)
    })
    return
  }

  this.requireClient().respond(
    request.id,
    failClosedServerRequestResponse(request.method)
  )
}
```

Add helpers:

```ts
function isForwardedServerRequestMethod(method: string): method is AppServerServerRequestMethod {
  return (
    method === 'item/commandExecution/requestApproval' ||
    method === 'item/permissions/requestApproval' ||
    method === 'item/fileChange/requestApproval' ||
    method === 'item/tool/requestUserInput' ||
    method === 'item/tool/call'
  )
}

function asRecord(value: unknown): Record<string, unknown> {
  return value && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {}
}

function failClosedServerRequestResponse(method: string): AppServerServerRequestResponse {
  if (method === 'item/tool/call') {
    return {
      contentItems: [
        {
          type: 'inputText',
          text: `unsupported app-server request method: ${method}`
        }
      ],
      success: false
    }
  }
  if (method === 'item/tool/requestUserInput') {
    return { answers: {} }
  }
  if (method === 'item/fileChange/requestApproval') {
    return { decision: 'decline' }
  }
  if (method === 'item/permissions/requestApproval') {
    return { permissions: {}, scope: 'turn', strictAutoReview: true }
  }
  return {
    decision: {
      kind: 'reject',
      data: { reason: `unsupported app-server request method: ${method}` }
    }
  }
}
```

- [x] **Step 5: Extend renderer temporary fail-closed handling**

In `useDasclawAssistantRuntime.ts`, replace `isApprovalRequest` with:

```ts
function isServerRequest(
  notification: AppServerNotification
): notification is AppServerServerRequest {
  return 'requestId' in notification && typeof notification.method === 'string'
}
```

Replace the approval-only call site:

```ts
if (isServerRequest(notification)) {
  rejectServerRequestUntilUiExists(notification)
  return
}
```

Add:

```ts
function rejectServerRequestUntilUiExists(notification: AppServerServerRequest): void {
  void window.desktopAppServer.respondServerRequest(
    notification.requestId,
    failClosedRendererServerRequestResponse(notification.method)
  )
}

function failClosedRendererServerRequestResponse(
  method: AppServerServerRequestMethod
): AppServerServerRequestResponse {
  if (method === 'item/tool/call') {
    return {
      contentItems: [{ type: 'inputText', text: 'renderer tool UI is not implemented' }],
      success: false
    }
  }
  if (method === 'item/tool/requestUserInput') {
    return { answers: {} }
  }
  if (method === 'item/fileChange/requestApproval') {
    return { decision: 'decline' }
  }
  if (method === 'item/permissions/requestApproval') {
    return { permissions: {}, scope: 'turn', strictAutoReview: true }
  }
  return {
    decision: {
      kind: 'reject',
      data: { reason: 'renderer approval UI is not implemented' }
    }
  }
}
```

- [x] **Step 6: Run desktop tests**

Run:

```bash
cd desktop-app && pnpm vitest run src/main/appServerManager.test.ts src/renderer/src/lib/assistantMessages.test.ts
```

Expected: PASS.

- [x] **Step 7: Commit desktop forwarding**

```bash
git add desktop-app/src/shared/appServerApi.ts desktop-app/src/main/appServerManager.ts desktop-app/src/main/appServerManager.test.ts desktop-app/src/renderer/src/hooks/useDasclawAssistantRuntime.ts desktop-app/src/renderer/src/lib/assistantMessages.test.ts
git commit -m "feat: forward r1 app-server server requests to desktop"
```

## Task 7: Update Gap Matrix and Run Verification

**Files:**
- Modify: `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`

- [x] **Step 1: Update R1 row after implementation**

Replace the R1 row in section `9.1 当前剩余优先级（2026-06-22）` with:

```markdown
| R1 | Runtime tool / approval owner | ~~`item/tool/call`~~、~~`item/tool/requestUserInput`~~、~~`item/permissions/requestApproval`~~、~~`item/fileChange/requestApproval`~~、~~`item/autoApprovalReview/started`~~、~~`item/autoApprovalReview/completed`~~、~~file-change output / patch events~~ | 已由 `docs/superpowers/plans/2026-06-22-dasclaw-app-server-runtime-tool-approval-owner.md` 接入 protocol types、runtime producer、pending server request routing、desktop fail-closed 转发和 owner-ready capability gating | `capabilities/list` 只在 runtime bridge 报告 R1 owner-ready 时广告；runtime producer 能发对应 ServerRequest / notification；`approval/respond` 或 JSON-RPC response 能回填执行路径；拒绝、超时、malformed response、shutdown 全部 fail-safe 且有测试 |
```

- [x] **Step 2: Run Rust verification**

Run:

```bash
cargo check -p dasclaw_app_server_protocol --tests
cargo check -p dasclaw_app_server --tests
cargo nextest run -p dasclaw_app_server_protocol -p dasclaw_app_server -E 'test(r1_) | test(approval_response_records_runtime_decision_and_emits_resolved_notification) | test(expired_approval_request_rejects_fail_safe_and_late_response_is_unknown) | test(interrupt_turn_resolves_pending_approval_server_request_as_failed) | test(shutdown_resolves_pending_approval_server_request_as_failed)'
```

Expected: all commands exit 0.

- [x] **Step 3: Run desktop verification**

Run:

```bash
cd desktop-app && pnpm vitest run src/main/appServerManager.test.ts src/renderer/src/lib/assistantMessages.test.ts
```

Expected: all selected Vitest tests pass.

- [x] **Step 4: Run formatting and local safety checks**

Run:

```bash
cargo fmt --all && python3.12 scripts/check_no_panics.py --base origin/xClaw
cargo clippy --no-deps -p dasclaw_app_server_protocol --all-targets -- -D warnings
cargo clippy --no-deps -p dasclaw_app_server --all-targets -- -D warnings
```

Expected: all commands exit 0.

- [x] **Step 5: Run required post-change skills before PR**

Run the project-required self checks:

```bash
omx skill run code-quality-audit
omx skill run code-simplifier
omx skill run code-review-expert
```

Expected: each command exits 0 or writes actionable findings. Fix actionable findings before opening the PR.

- [x] **Step 6: Commit docs and verification updates**

```bash
git add docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md
git commit -m "docs: mark r1 runtime tool approval owner plan complete"
```

## Self-Review

Spec coverage:

- R1 `item/tool/call`: Task 1 adds wire type; Task 3 decodes response; Task 4 emits server request; Task 6 forwards desktop response.
- R1 `item/tool/requestUserInput`: Task 1 adds wire type; Task 3 decodes response; Task 4 emits server request; Task 6 fail-closes until UI exists.
- R1 `item/permissions/requestApproval`: Task 1 adds Codex-shaped payload; Task 3 decodes response; Task 4 emits request; Task 6 forwards/fail-closes.
- R1 `item/fileChange/requestApproval`: Task 1 adds payload/decision; Task 3 decodes response; Task 4 emits request; Task 6 forwards/fail-closes.
- R1 auto-approval review notifications: Task 1 adds types; Task 2 schema/capability; Task 4 runtime producer.
- R1 file-change output / patch events: Task 1 adds types; Task 2 schema/capability; Task 4 runtime producer.
- Owner-ready capability gating: Task 2 gates `capabilities/list` and schema against runtime features.
- Fail-safe cases: Task 3 preserves malformed/timeout clearing; Task 7 reruns existing timeout/interrupt/shutdown coverage.

Placeholder scan:

- 占位词扫描已通过，所有任务都写明了具体文件、代码片段、命令、预期结果和提交信息。
- Code steps include concrete snippets, exact paths, exact commands, expected PASS/FAIL states, and commit messages.

Type consistency:

- Server request methods use constants under `server_request::*`.
- JSON-RPC notifications use `ServerNotification::*` constructors.
- Pending request routing uses `PendingServerRequestKind` and `RuntimeServerRequestResponse` consistently across decode, apply, timeout, and tests.
