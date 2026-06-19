# Dasclaw App Server Codex P3 Full Approval Tool Sandbox Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 P3 从命令执行审批 MVP 扩展为可验收的完整 Approval Tool Sandbox：工具调用、权限审批、自动审查、动态工具请求、用户输入请求、sandbox/profile 能力声明和 app-server E2E 都必须由实现与测试共同证明。

**Architecture:** `dasclaw_app_server_protocol` 继续拥有 JSON-RPC wire type、capability matrix 和 `codex_app_server_v2` profile；`dasclaw_app_server` 负责 server-request 状态机、runtime bridge 映射、stdio 往返和 fail-safe。新增能力一律受细粒度 `RuntimeBridgeFeatures` gating 控制，只有 command approval、permissions approval、auto-review、dynamic tool call、request user input、tool lifecycle、sandbox 和参数脱敏测试全部通过后，profile 才移除对应 opt-out。

**Tech Stack:** Rust 2024、serde JSON-RPC、`dasclaw_app_server_protocol`、`dasclaw_app_server`、`dasclaw_protocol` dynamic tool / request-user-input / request-permissions / guardian primitives、`cargo nextest`、Electron main-process TypeScript、Vitest。

---

## Scope Check

本计划覆盖 P3 完整化，不进入 P4/P5/P6 产品域。

- 覆盖：`item/tool/call`、tool lifecycle、tool result、tool error、参数脱敏、`item/permissions/requestApproval`、`approval/respond` / JSON-RPC response 双入口、approve/reject/timeout/stale、`item/autoApprovalReview/started` 与 `completed`、`item/tool/requestUserInput`、stdio/json-rpc 往返、desktop main/renderer 桥接测试、必要的 protocol crate contract tests。
- 不覆盖：`fs/*` 文件服务、standalone `command/exec*` PTY 服务、MCP elicitation、account token refresh、plugin marketplace、skills/jobs/logs 服务 owner、完整可视化审批 UI、Windows sandbox setup product flow。
- 保持：`desktop-client` / `client-gui` 仅作为历史参考，不作为当前实现目标；当前目标是 `crates/dasclaw_app_server*` 与 `desktop-app`。

## Implementation Boundary

这不是只改 Electron `desktop-app` 或 app-server 外壳的工作。完整 P3 能力的主干在无头 Rust 层完成，desktop 只是把 server-request 暴露给用户界面并把响应写回 JSON-RPC。

- 必须改 `crates/dasclaw_app_server_protocol/src/lib.rs`：补 wire contract、P3 payload、response shape、capability matrix、`codex_app_server_v2` profile gating 和协议测试。
- 必须改 `crates/dasclaw_app_server/src/lib.rs`：补 server-request 状态机、pending request 分类、runtime bridge event mapping、response dispatch、timeout/stale/fail-safe 和 stdio e2e。
- 可能改 `crates/dasclaw_app_server/src/main.rs`：只有真实 production stdio runtime 能证明 full-P3 bridge 时才调整 advertised capabilities；default/noop runtime 继续保持 declared-only。
- 优先不改 `crates/dasclaw_protocol` 和 `crates/dasclaw_runtime`：现有 dynamic tools、request user input、request permissions、approval/guardian primitives 应先复用。只有 app-server 无法表达完整 P3 语义时，才补小型通用类型或 adapter，不新增 parallel framework。
- 不新增 Rust crate，不新增依赖，除非某个实现步骤用测试证明现有 crate 与标准库无法表达 request state machine。

## Startup 4 Questions

| 问题 | 结论 | 本计划处理 |
|---|---|---|
| 是否新增模块 / crate / 文件？ | 是，本计划文件是新增文档；实现阶段会新增协议类型与测试代码，但不新增 crate。 | 已先做语义搜索；实现 commits 必须带 `已检查 P3 Approval Tool Sandbox 是否已有，结论：...`。 |
| 是否包含否定性结论？ | 是，会说明哪些 profile 能力暂不 claim，哪些 Codex request 不在 P3。 | 否定边界只基于语义、LSP、`rg` 与文件阅读；不把 recall 当证据。 |
| 是否跨项目对账？ | 是，涉及 Dasclaw app-server、`dasclaw_protocol`、desktop-app 与 `codex-cli-main` app-server schema。 | wire name 与 response shape 以 `codex-cli-main/codex-rs/app-server-protocol/schema/typescript` 为准。 |
| 是否写架构对账类文档？ | 是，这是 implementation plan + protocol 能力对账。 | 计划内写明 evidence-vs-inference，并要求实现后更新 gap matrix。 |

## Verification Evidence

Level 1 语义层：

- `semantic_search_nodes_tool` 查询 `app server approval tool sandbox request approval tool lifecycle auto approval review request user input`，`search_mode=hybrid`，命中旧 approval gate / desktop approval UI / command approval 相关实现。
- `semantic_search_nodes_tool` 查询 `approval respond requestApproval autoApprovalReview tool requestUserInput codex app server protocol events`，命中 `persist_approval_needed_ui_event`、`submit_approval_ticket`、`process_approval` 等旧 approval 相关符号。
- `semantic_search_nodes_tool` 查询 `tool call lifecycle tool result arguments redaction sensitive parameter app server`，命中 tool parameter schema，但没有命中 app-server 完整 dynamic tool request loop。
- `cross_repo_search_tool` 查询 `codex_app_server_v2 profile approval tool sandbox autoApprovalReview requestUserInput`，命中 `allows_sandbox_approval`、`submit_approval_ticket`、`auto_approve_tool`、`ApprovalCardView` 等分散底座。

Level 2 符号层：

- 本地 LSP MCP 通过 `http://127.0.0.1:9527/mcp` 初始化成功，`tools/list` 暴露 `execute_lsp`。
- `workspace_symbols(JsonRpcServerRequest)` 定位到 `crates/dasclaw_app_server_protocol/src/lib.rs:153` 与 `desktop-app/src/main/appServerRpc.ts:21`。
- `workspace_symbols(RuntimeTurnOutcome)` 定位到 `crates/dasclaw_app_server/src/lib.rs:2099`。
- `workspace_symbols(RuntimeApprovalRequest)` 定位到 `crates/dasclaw_app_server/src/lib.rs:1962`。
- `workspace_symbols(PermissionsApprovalRequest)` 定位到 `crates/dasclaw_app_server_protocol/src/lib.rs:1584`。
- `workspace_symbols(ToolRequestUserInput)` 只定位到 app-server protocol 常量 `ITEM_TOOL_REQUEST_USER_INPUT`，没有 app-server runtime bridge 类型。
- `workspace_symbols(DynamicToolCall)` 定位到 `crates/dasclaw_protocol/src/dynamic_tools.rs` 和 `crates/dasclaw_protocol/src/protocol.rs`，没有定位到 app-server bridge 类型。
- `workspace_symbols(AutoApprovalReview)` 只定位到 app-server protocol 常量 `ITEM_AUTO_APPROVAL_REVIEW_STARTED/COMPLETED`。

Level 3 字面量层：

- `crates/dasclaw_app_server_protocol/src/lib.rs` 已有 `JsonRpcClientResponse`、`JsonRpcServerRequest`、部分 server_request/event 常量、`CommandExecutionApprovalRequest`、`PermissionsApprovalRequest`，但 `CapabilityMatrix::with_p3_approval_tool_sandbox()` 只把 command approval、`serverRequest/resolved`、`item/commandExecution/outputDelta`、`item/commandExecution/terminalInteraction` 纳入 implemented。
- `crates/dasclaw_app_server/src/lib.rs` 已有 pending server request store、`approval/respond`、JSON-RPC client response parsing、timeout/stale 处理和 command approval stdio e2e；现有 pending request 结构只表达 approval，不区分 permissions/dynamic tool/user input/unsupported request。
- `desktop-app/src/main/appServerRpc.ts` 已能把带 `method` + `id` 的 JSON-RPC message 分类为 `server-request`，并能写 response line。
- `desktop-app/src/main/appServerManager.ts` 只把 `item/commandExecution/requestApproval` 与 `item/permissions/requestApproval` 转成 renderer notification；未知 server request 当前统一用 approval-style reject。
- `desktop-app/src/renderer/src/hooks/useDasclawAssistantRuntime.ts` 对 approval request 采用 fail-safe reject，尚未处理 dynamic tool request 或 request user input。
- Codex schema 证据：`ServerRequest.ts` 列出 `item/commandExecution/requestApproval`、`item/fileChange/requestApproval`、`item/tool/requestUserInput`、`mcpServer/elicitation/request`、`item/permissions/requestApproval`、`item/tool/call`、`account/chatgptAuthTokens/refresh`、`applyPatchApproval`、`execCommandApproval`。
- Codex schema 证据：`v2/ToolRequestUserInputResponse.ts` 为 `{ answers }`，`v2/DynamicToolCallResponse.ts` 为 `{ contentItems, success }`，`v2/PermissionsRequestApprovalResponse.ts` 为 `{ permissions, scope, strictAutoReview? }`。

## Codex Reference Boundary

本计划制定时已经参考 `codex-cli-main/codex-rs/app-server-protocol/schema/typescript` 的 app-server schema 和 wire contract。该证据足以决定 method name、server request vs notification 分类、request/response JSON shape，以及 P3 范围内哪些 surface 属于正式目标。

实现阶段不能只按 schema 写 wire-compatible 类型。Task 1 写测试前必须读取 Codex app-server 的实际实现和测试，把行为细节作为 reference oracle，再映射到 Dasclaw 现有无头 runtime primitives。重点参考：

- `codex-cli-main/codex-rs/app-server-protocol/src/protocol/common.rs`
  - `ServerRequest` / `ServerRequestPayload` / `ServerNotification` method 注册。
- `codex-cli-main/codex-rs/app-server-protocol/src/protocol/v2.rs`
  - `PermissionsRequestApprovalParams`、`PermissionsRequestApprovalResponse`、`DynamicToolCallParams`、`DynamicToolCallResponse`、`ToolRequestUserInputParams`、`ToolRequestUserInputResponse`、guardian auto-review notification structs。
- `codex-cli-main/codex-rs/app-server-protocol/src/protocol/item_builders.rs`
  - guardian assessment 到 `item/autoApprovalReview/started` / `completed` 的转换。
- `codex-cli-main/codex-rs/app-server/src/bespoke_event_handling.rs`
  - command approval、permissions approval、request user input、dynamic tool call、guardian review lifecycle 的 request/response 行为。
- `codex-cli-main/codex-rs/app-server/src/dynamic_tools.rs`
  - dynamic tool response decoding 和 fallback response 行为。
- `codex-cli-main/codex-rs/app-server/src/outgoing_message.rs`
  - server request emission、pending request ids、response routing 测试。
- `codex-cli-main/codex-rs/app-server/tests/suite/v2/request_user_input.rs`
  - request user input JSON-RPC 往返和 resolved notification。
- `codex-cli-main/codex-rs/app-server/tests/suite/v2/request_permissions.rs`
  - permissions request/response 和 resolved notification。
- `codex-cli-main/codex-rs/app-server/tests/suite/v2/dynamic_tools.rs`
  - dynamic tool request、response、thread item lifecycle。
- `codex-cli-main/codex-rs/app-server/tests/suite/v2/thread_unsubscribe.rs`
  - dynamic tool pending request interrupted/unsubscribe behavior。

Do not verbatim-port Codex app-server implementation. Use it to pin behavior, then implement in Dasclaw with `dasclaw_app_server_protocol`, `dasclaw_app_server`, and existing `dasclaw_protocol` primitives.

## File Structure

Modify:

- `crates/dasclaw_app_server_protocol/src/lib.rs`
  - JSON-RPC request/response/server-request structs, method/server_request/event constants, P3 payload structs, response enums, capability matrix, compatibility profile, protocol serialization tests.
- `crates/dasclaw_app_server/src/lib.rs`
  - App-server routing, pending server-request state machine, runtime bridge feature gating, RuntimeTurnOutcome variants, notification mapping, stdio e2e tests, fake bridge tests.
- `crates/dasclaw_app_server/src/main.rs`
  - Only if production stdio runtime mode must advertise full P3 after sandboxed runtime factory is proven; default `noop` must stay declared-only.
- `desktop-app/src/main/appServerRpc.ts`
  - JSON-RPC server-request classification, response/error response writer, handler registration.
- `desktop-app/src/main/appServerRpc.test.ts`
  - JSON-RPC classification and response-line contract tests for approval, permissions, dynamic tool call, request user input, unsupported request error.
- `desktop-app/src/main/appServerManager.ts`
  - Main-process server-request forwarding, safe param scrubbing, response dispatch by request kind, unsupported request fail-safe.
- `desktop-app/src/main/appServerManager.test.ts`
  - Forwarding and response tests for command approval, permissions approval, dynamic tool call, request user input, unsupported request.
- `desktop-app/src/shared/appServerApi.ts`
  - Shared renderer-visible request/response union types.
- `desktop-app/src/renderer/src/hooks/useDasclawAssistantRuntime.ts`
  - Renderer bridge behavior for request classes; keep fail-safe cancellation for request kinds without a UI handler.
- `desktop-app/src/renderer/src/lib/assistantMessages.test.ts`
  - Renderer fail-safe and response tests.
- `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`
  - Update only after implementation and tests prove the full P3 subset.

Do not create a new Rust crate. Do not add a dependency unless an implementation step proves standard library / existing crate types cannot express the state machine.

## Definition of Done

- `AppServer::new()` and `NoopRuntimeBridge` keep approval/tools/sandbox as declared or disabled, not implemented.
- A full-P3 runtime bridge advertises implemented approval/tools/sandbox only when every P3 subfeature is true: command approval, permissions approval, auto approval review, dynamic tool call, request user input, tool lifecycle, sandbox readiness, and redacted display parameters.
- `codex_app_server_v2` profile includes full P3 server-request/event names only for the full-P3 capability matrix. Default chat-session subset keeps `codex.tool_calls`, `codex.approvals`, `tools`, and `sandbox` opt-outs.
- Stdio JSON-RPC e2e covers approve, reject, timeout, stale response, unsupported request, permissions approval, dynamic tool call response, request user input response/cancel, auto-review started/completed, and tool error path.
- Desktop main-process tests prove server requests are not treated as notifications, raw parameters are scrubbed, responses are written with the correct JSON-RPC id, and unsupported requests are fail-safe.
- Renderer tests prove approval/user-input/dynamic-tool requests are either explicitly answered through the bridge or explicitly cancelled with a safe reason; no request silently hangs.
- Protocol tests prove no `rawArguments`, `apiKey`, `secret`, or unsanitized raw tool arguments are serialized in display-facing request payloads.
- Task 1 implementation starts only after Codex implementation files above are read and the behavior deltas are reflected in the first failing tests.

## Commit Slices

1. `test: lock full p3 app-server protocol contracts`
2. `feat: add full p3 app-server protocol types`
3. `feat: generalize app-server server-request state`
4. `feat: bridge full p3 tool and approval runtime events`
5. `test: cover full p3 app-server stdio e2e`
6. `feat: extend desktop app-server server-request bridge`
7. `docs: mark full p3 app-server support after verification`

Each implementation commit message must include:

```text
已检查 P3 Approval Tool Sandbox 是否已有，结论：已有 runtime/protocol primitives 分散存在；本切片补 app-server 完整接线与受测能力，不新增重复 crate。
```

## Task 1: Lock Full P3 Protocol Contracts

**Files:**
- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`

- [ ] **Step 0: Read Codex implementation behavior before writing tests**

Run:

```bash
sed -n '885,935p' codex-cli-main/codex-rs/app-server-protocol/src/protocol/common.rs
sed -n '6501,6550p' codex-cli-main/codex-rs/app-server-protocol/src/protocol/v2.rs
sed -n '7350,7485p' codex-cli-main/codex-rs/app-server-protocol/src/protocol/v2.rs
sed -n '780,1065p' codex-cli-main/codex-rs/app-server/src/bespoke_event_handling.rs
sed -n '2500,2750p' codex-cli-main/codex-rs/app-server/src/bespoke_event_handling.rs
sed -n '1,90p' codex-cli-main/codex-rs/app-server/src/dynamic_tools.rs
sed -n '1,150p' codex-cli-main/codex-rs/app-server/tests/suite/v2/request_user_input.rs
sed -n '1,170p' codex-cli-main/codex-rs/app-server/tests/suite/v2/request_permissions.rs
sed -n '320,410p' codex-cli-main/codex-rs/app-server/tests/suite/v2/dynamic_tools.rs
sed -n '3570,3775p' codex-cli-main/codex-rs/app-server/src/bespoke_event_handling.rs
```

Expected:

- `common.rs` shows the P3 server-request method registration for command approval, permissions approval, dynamic tool call, and request user input.
- `v2.rs` shows Codex payload fields for dynamic tool call, permissions approval, user input, and auto-review notification.
- `bespoke_event_handling.rs` shows behavior for request emission, response decoding, guardian review started/completed, and fail-safe fallback.
- The v2 suite files show the app-server e2e expectations that Dasclaw tests must mirror or intentionally adapt.

Record any intentional Dasclaw-vs-Codex behavior difference in the new failing tests as an explicit assertion name, not as prose-only rationale.

- [ ] **Step 1: Add failing protocol tests**

Add these tests in the existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn full_p3_capability_helper_requires_every_approval_tool_sandbox_contract() {
    let matrix = CapabilityMatrix::phase_one().with_full_p3_approval_tool_sandbox();

    assert_eq!(matrix.approval.status, CapabilityStatus::Implemented);
    assert_eq!(matrix.approval.methods, vec![method::APPROVAL_RESPOND.to_string()]);
    assert!(matrix.approval.events.contains(&server_request::ITEM_COMMAND_EXECUTION_REQUEST_APPROVAL.to_string()));
    assert!(matrix.approval.events.contains(&server_request::ITEM_PERMISSIONS_REQUEST_APPROVAL.to_string()));
    assert!(matrix.approval.events.contains(&event::SERVER_REQUEST_RESOLVED.to_string()));
    assert!(matrix.approval.events.contains(&event::ITEM_AUTO_APPROVAL_REVIEW_STARTED.to_string()));
    assert!(matrix.approval.events.contains(&event::ITEM_AUTO_APPROVAL_REVIEW_COMPLETED.to_string()));

    assert_eq!(matrix.tools.status, CapabilityStatus::Implemented);
    assert!(matrix.tools.events.contains(&server_request::ITEM_TOOL_CALL.to_string()));
    assert!(matrix.tools.events.contains(&server_request::ITEM_TOOL_REQUEST_USER_INPUT.to_string()));
    assert!(matrix.tools.events.contains(&event::ITEM_COMMAND_EXECUTION_OUTPUT_DELTA.to_string()));
    assert!(matrix.tools.events.contains(&event::ITEM_COMMAND_EXECUTION_TERMINAL_INTERACTION.to_string()));

    assert_eq!(matrix.sandbox.status, CapabilityStatus::Implemented);
    assert!(matrix.sandbox.reason.as_deref().unwrap_or_default().contains("full P3 runtime bridge reports sandbox-ready execution"));
}

#[test]
fn full_p3_profile_removes_tool_approval_opt_outs_only_for_full_p3_matrix() {
    let declared = CompatibilityProfile::codex_app_server_v2_for(&CapabilityMatrix::phase_one());
    assert!(declared.capability_opt_outs.iter().any(|opt| opt.capability == "codex.tool_calls"));
    assert!(declared.capability_opt_outs.iter().any(|opt| opt.capability == "codex.approvals"));

    let implemented = CompatibilityProfile::codex_app_server_v2_for(
        &CapabilityMatrix::phase_one().with_full_p3_approval_tool_sandbox(),
    );
    assert!(!implemented.capability_opt_outs.iter().any(|opt| opt.capability == "codex.tool_calls"));
    assert!(!implemented.capability_opt_outs.iter().any(|opt| opt.capability == "codex.approvals"));
    assert!(implemented.events.contains(&event::ITEM_AUTO_APPROVAL_REVIEW_STARTED.to_string()));
    assert!(implemented.events.contains(&event::ITEM_AUTO_APPROVAL_REVIEW_COMPLETED.to_string()));
}

#[test]
fn full_p3_tool_and_user_input_payloads_match_codex_shapes_and_hide_raw_arguments() {
    let dynamic = JsonRpcServerRequest::new(
        "tool_call_1",
        server_request::ITEM_TOOL_CALL,
        DynamicToolCallRequest {
            thread_id: "thread_1".to_string(),
            turn_id: "turn_1".to_string(),
            call_id: "call_1".to_string(),
            namespace: Some("local".to_string()),
            tool: "lookup_ticket".to_string(),
            arguments: serde_json::json!({"ticketId": "123"}),
        },
    )
    .expect("dynamic tool call request should serialize");
    let value = serde_json::to_value(dynamic).expect("request should serialize to JSON");
    assert_eq!(value["method"], server_request::ITEM_TOOL_CALL);
    assert_eq!(value["params"]["callId"], "call_1");
    assert!(value["params"].get("rawArguments").is_none());
    assert!(!value.to_string().contains("secret-token"));

    let user_input = JsonRpcServerRequest::new(
        "tool_input_1",
        server_request::ITEM_TOOL_REQUEST_USER_INPUT,
        ToolRequestUserInputParams {
            thread_id: "thread_1".to_string(),
            turn_id: "turn_1".to_string(),
            item_id: "turn_1:tool:question".to_string(),
            questions: vec![ToolRequestUserInputQuestion {
                id: "choice".to_string(),
                header: "Mode".to_string(),
                question: "Pick one".to_string(),
                is_other: false,
                is_secret: false,
                options: Some(vec![ToolRequestUserInputOption {
                    label: "Safe".to_string(),
                    description: "Continue with safe mode".to_string(),
                }]),
            }],
        },
    )
    .expect("request user input should serialize");
    let value = serde_json::to_value(user_input).expect("request should serialize to JSON");
    assert_eq!(value["method"], server_request::ITEM_TOOL_REQUEST_USER_INPUT);
    assert_eq!(value["params"]["questions"][0]["id"], "choice");
}

#[test]
fn full_p3_permission_and_auto_review_payloads_round_trip() {
    let review = AutoApprovalReviewStartedEvent::from_guardian_assessment(
        "thread_1",
        dasclaw_protocol::approvals::GuardianAssessmentEvent {
            id: "review_1".to_string(),
            target_item_id: Some("turn_1:tool:bash".to_string()),
            turn_id: "turn_1".to_string(),
            status: dasclaw_protocol::approvals::GuardianAssessmentStatus::InProgress,
            risk_level: None,
            user_authorization: None,
            rationale: None,
            decision_source: None,
            action: dasclaw_protocol::approvals::GuardianAssessmentAction::RequestPermissions {
                reason: Some("needs write access".to_string()),
                permissions: dasclaw_protocol::request_permissions::RequestPermissionProfile::default(),
            },
        },
    );
    let notification = ServerNotification::auto_approval_review_started(review)
        .expect("auto review started should serialize");
    assert_eq!(notification.method, event::ITEM_AUTO_APPROVAL_REVIEW_STARTED);

    let response: PermissionsApprovalResponsePayload = serde_json::from_value(serde_json::json!({
        "permissions": {"network": null, "fileSystem": null},
        "scope": "turn",
        "strictAutoReview": true
    }))
    .expect("permissions approval response should deserialize");
    assert!(response.strict_auto_review);
}
```

- [ ] **Step 2: Run protocol tests and confirm they fail before implementation**

Run:

```bash
cargo nextest run -p dasclaw_app_server_protocol -E 'test(full_p3_)'
```

Expected: FAIL with missing `with_full_p3_approval_tool_sandbox`, `codex_app_server_v2_for`, dynamic tool/user-input/auto-review payload types, or constructors.

- [ ] **Step 3: Commit protocol contract tests**

```bash
git add crates/dasclaw_app_server_protocol/src/lib.rs
git commit -m $'test: lock full p3 app-server protocol contracts\n\n已检查 P3 Approval Tool Sandbox 是否已有，结论：已有 runtime/protocol primitives 分散存在；本切片补 app-server 完整接线与受测能力，不新增重复 crate。'
```

## Task 2: Implement Full P3 Protocol Types and Profile Gating

**Files:**
- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`

- [ ] **Step 1: Add protocol payloads that mirror Codex P3 shapes**

Add these structs near the existing approval payloads, reusing `dasclaw_protocol` types where they already exist:

```rust
pub type DynamicToolCallRequest = dasclaw_protocol::dynamic_tools::DynamicToolCallRequest;
pub type DynamicToolCallResponsePayload = dasclaw_protocol::dynamic_tools::DynamicToolResponse;
pub type ToolRequestUserInputQuestion = dasclaw_protocol::request_user_input::RequestUserInputQuestion;
pub type ToolRequestUserInputQuestionOption =
    dasclaw_protocol::request_user_input::RequestUserInputQuestionOption;
pub type ToolRequestUserInputResponsePayload =
    dasclaw_protocol::request_user_input::RequestUserInputResponse;
pub type PermissionsApprovalResponsePayload =
    dasclaw_protocol::request_permissions::RequestPermissionsResponse;

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
pub struct PermissionsRequestApprovalParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub permissions: dasclaw_protocol::request_permissions::RequestPermissionProfile,
}
```

Keep existing `CommandExecutionApprovalRequest` for current command approval compatibility, but add a comment stating it is the Dasclaw command-approval view and must not contain raw tool arguments.

- [ ] **Step 2: Add auto-review notification structs**

Add:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoApprovalReviewStartedEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub review_id: String,
    pub target_item_id: Option<String>,
    pub review: GuardianApprovalReview,
    pub action: dasclaw_protocol::approvals::GuardianAssessmentAction,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoApprovalReviewCompletedEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub review_id: String,
    pub target_item_id: Option<String>,
    pub decision_source: Option<dasclaw_protocol::approvals::GuardianAssessmentDecisionSource>,
    pub review: GuardianApprovalReview,
    pub action: dasclaw_protocol::approvals::GuardianAssessmentAction,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardianApprovalReview {
    pub status: dasclaw_protocol::approvals::GuardianAssessmentStatus,
    pub risk_level: Option<dasclaw_protocol::approvals::GuardianRiskLevel>,
    pub user_authorization: Option<dasclaw_protocol::approvals::GuardianUserAuthorization>,
    pub rationale: Option<String>,
}
```

Add `from_guardian_assessment(thread_id, event)` constructors for started/completed. Started must require `status == InProgress`; completed must require a terminal status (`Approved`, `Denied`, `TimedOut`, `Aborted`) and return `ErrorCode::InvalidParams` if called with `InProgress`.

- [ ] **Step 3: Add notification constructors and schemas**

Extend `ServerNotification`:

```rust
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

Add `EventSchema` entries for both auto-review events. Add schema names for `ToolRequestUserInputParams`, `DynamicToolCallRequest`, `DynamicToolCallResponsePayload`, `PermissionsRequestApprovalParams`, and `PermissionsApprovalResponsePayload`.

- [ ] **Step 4: Replace coarse P3 helper with full-P3 helper**

Keep the existing narrow helper for compatibility tests if needed, but add a full helper:

```rust
#[must_use]
pub fn with_full_p3_approval_tool_sandbox(mut self) -> Self {
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
        Some("full P3 runtime bridge reports sandbox-ready execution and redacted tool parameters".to_string()),
    );
    self
}
```

- [ ] **Step 5: Make `codex_app_server_v2` capability-aware**

Change `CompatibilityProfile::codex_app_server_v2()` to delegate:

```rust
pub fn codex_app_server_v2() -> Self {
    Self::codex_app_server_v2_for(&CapabilityMatrix::phase_one())
}

pub fn codex_app_server_v2_for(capabilities: &CapabilityMatrix) -> Self {
    let full_p3 = capabilities.approval.status == CapabilityStatus::Implemented
        && capabilities.tools.status == CapabilityStatus::Implemented
        && capabilities.sandbox.status == CapabilityStatus::Implemented;
    let mut profile = Self::codex_app_server_v2_base();
    if full_p3 {
        profile.events.extend(FULL_P3_CODEX_APP_SERVER_V2_EVENTS.iter().map(|event| (*event).to_string()));
        profile.capability_opt_outs.retain(|opt| {
            !matches!(opt.capability.as_str(), "codex.tool_calls" | "codex.approvals" | "tools" | "sandbox")
        });
    }
    profile
}
```

`FULL_P3_CODEX_APP_SERVER_V2_EVENTS` must include only the events proven in this plan: `serverRequest/resolved`, `item/autoApprovalReview/started`, `item/autoApprovalReview/completed`, `item/commandExecution/outputDelta`, and `item/commandExecution/terminalInteraction`.

- [ ] **Step 6: Run protocol verification**

Run:

```bash
cargo check -p dasclaw_app_server_protocol --tests
cargo nextest run -p dasclaw_app_server_protocol -E 'test(full_p3_)'
```

Expected: PASS, 0 compile errors.

- [ ] **Step 7: Commit protocol implementation**

```bash
git add crates/dasclaw_app_server_protocol/src/lib.rs
git commit -m $'feat: add full p3 app-server protocol types\n\n已检查 P3 Approval Tool Sandbox 是否已有，结论：已有 runtime/protocol primitives 分散存在；本切片补 app-server 完整接线与受测能力，不新增重复 crate。'
```

## Task 3: Generalize App-Server Server-Request State

**Files:**
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [ ] **Step 1: Add failing state-machine tests**

Add tests:

```rust
#[test]
fn full_p3_bridge_is_required_before_capability_claims_are_implemented() {
    let server = AppServer::with_runtime_bridge(Arc::new(PartialP3Bridge::command_approval_only()));
    assert_ne!(server.capabilities().capabilities.approval.status, CapabilityStatus::Implemented);
    assert!(server.capabilities().compatibility_profiles[0]
        .capability_opt_outs
        .iter()
        .any(|opt| opt.capability == "codex.tool_calls"));
}

#[test]
fn unknown_or_stale_server_request_response_resolves_failed_without_runtime_dispatch() {
    let mut server = initialized_codex_v2_server_with_bridge(Arc::new(FullP3ManualBridge::default()));
    let response = r#"{"jsonrpc":"2.0","id":"tool_call_missing","result":{"success":true,"contentItems":[]}}"#;
    assert!(server.handle_json_rpc(response).is_none());
    let notifications = server.drain_notifications();
    assert!(notifications.iter().any(|notification| {
        notification.method == "serverRequest/resolved"
            && notification.params["outcome"] == "failed"
            && notification.params["reason"].as_str().unwrap_or_default().contains("unknown or expired")
    }));
}
```

- [ ] **Step 2: Replace coarse feature flags**

Change `RuntimeBridgeFeatures`:

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeBridgeFeatures {
    pub command_approval: bool,
    pub permissions_approval: bool,
    pub auto_approval_review: bool,
    pub dynamic_tool_call: bool,
    pub request_user_input: bool,
    pub tool_lifecycle: bool,
    pub sandbox: bool,
    pub redacted_tool_arguments: bool,
}

impl RuntimeBridgeFeatures {
    fn supports_full_p3(self) -> bool {
        self.command_approval
            && self.permissions_approval
            && self.auto_approval_review
            && self.dynamic_tool_call
            && self.request_user_input
            && self.tool_lifecycle
            && self.sandbox
            && self.redacted_tool_arguments
    }
}
```

Update `AppServer::with_runtime_bridge()` to call `with_full_p3_approval_tool_sandbox()` only when `features.supports_full_p3()` is true.

- [ ] **Step 3: Add typed pending request kinds**

Replace the current approval-only pending state with:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
enum PendingServerRequestKind {
    CommandApproval { runtime_request_id: String },
    PermissionsApproval { runtime_request_id: String },
    DynamicToolCall { call_id: String },
    ToolUserInput { call_id: String },
    Unsupported { method: String },
}

#[derive(Debug, Clone)]
struct PendingServerRequest {
    request_id: String,
    thread_id: String,
    turn_id: String,
    item_id: Option<String>,
    kind: PendingServerRequestKind,
    created_at: Instant,
}
```

Use a `HashMap<String, PendingServerRequest>` for lookup by JSON-RPC id. Keep `drain_turn`, `drain_all`, and `expired` behavior.

- [ ] **Step 4: Add response dispatch by pending kind**

Change `handle_client_response()` so it calls:

```rust
fn apply_server_request_response(
    &mut self,
    pending: PendingServerRequest,
    response: JsonRpcClientResponse,
) {
    match pending.kind.clone() {
        PendingServerRequestKind::CommandApproval { .. } => self.apply_command_approval_response(pending, response),
        PendingServerRequestKind::PermissionsApproval { .. } => self.apply_permissions_approval_response(pending, response),
        PendingServerRequestKind::DynamicToolCall { .. } => self.apply_dynamic_tool_response(pending, response),
        PendingServerRequestKind::ToolUserInput { .. } => self.apply_tool_user_input_response(pending, response),
        PendingServerRequestKind::Unsupported { .. } => self.apply_unsupported_response(pending, response),
    }
}
```

Each branch must parse the exact payload for its request kind, emit `serverRequest/resolved`, and call a dedicated `RuntimeBridge` resolver method. Client JSON-RPC `error` must resolve failed and fail the turn.

- [ ] **Step 5: Add timeout policy by request kind**

Timeout behavior:

- Command approval: reject runtime approval with reason `approval request timed out`, fail the turn.
- Permissions approval: send denied permissions response to runtime resolver if supported, then fail the turn.
- Dynamic tool call: send `success=false` dynamic response with one `inputText` item explaining timeout, then fail the turn.
- Tool user input: send an empty answer map plus cancellation reason to runtime resolver, then fail the turn.
- Unsupported: emit failed resolution and fail the turn.

Implement with:

```rust
fn timeout_pending_server_request(&mut self, pending: PendingServerRequest) {
    match pending.kind {
        PendingServerRequestKind::CommandApproval { .. } => { /* reject approval */ }
        PendingServerRequestKind::PermissionsApproval { .. } => { /* deny permissions */ }
        PendingServerRequestKind::DynamicToolCall { .. } => { /* fail dynamic tool */ }
        PendingServerRequestKind::ToolUserInput { .. } => { /* cancel user input */ }
        PendingServerRequestKind::Unsupported { .. } => { /* fail safe */ }
    }
}
```

Use concrete branches, not a shared approval reject payload.

- [ ] **Step 6: Run state-machine tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(full_p3_bridge_is_required_before_capability_claims_are_implemented) | test(unknown_or_stale_server_request_response_resolves_failed_without_runtime_dispatch)'
```

Expected: PASS.

- [ ] **Step 7: Commit server-request state**

```bash
git add crates/dasclaw_app_server/src/lib.rs
git commit -m $'feat: generalize app-server server-request state\n\n已检查 P3 Approval Tool Sandbox 是否已有，结论：已有 runtime/protocol primitives 分散存在；本切片补 app-server 完整接线与受测能力，不新增重复 crate。'
```

## Task 4: Complete Tool Lifecycle and Redaction

**Files:**
- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [ ] **Step 1: Add failing lifecycle/redaction tests**

Add tests:

```rust
#[test]
fn full_p3_tool_lifecycle_emits_started_output_result_and_completed_without_raw_arguments() {
    let bridge = Arc::new(FullP3ManualBridge::scripted(vec![
        RuntimeTurnOutcome::ToolLifecycleStarted(RuntimeToolLifecycleStarted {
            item_id: "turn_1:tool:lookup".to_string(),
            tool_name: "lookup_ticket".to_string(),
            display_arguments: serde_json::json!({"ticketId": "123"}),
            raw_arguments_for_test: serde_json::json!({"apiKey": "secret-token"}),
        }),
        RuntimeTurnOutcome::CommandOutputDelta {
            update: RuntimeCommandOutputDeltaUpdate {
                item_id: "turn_1:tool:lookup".to_string(),
                delta: "running\n".to_string(),
            },
        },
        RuntimeTurnOutcome::ToolResult {
            update: RuntimeToolResultUpdate {
                item_id: "turn_1:tool:lookup".to_string(),
                content: "ticket found".to_string(),
                is_error: false,
            },
        },
        RuntimeTurnOutcome::ToolLifecycleCompleted(RuntimeToolLifecycleCompleted {
            item_id: "turn_1:tool:lookup".to_string(),
            success: true,
        }),
        RuntimeTurnOutcome::Completed { output: "done".to_string() },
    ]));
    let mut server = initialized_codex_v2_server_with_bridge(bridge);
    start_test_turn(&mut server);

    let values = drain_json_rpc_values_for(&mut server, Duration::from_millis(200));
    let serialized = serde_json::to_string(&values).expect("values should serialize");
    assert!(values.iter().any(|value| value["method"] == "item/started"));
    assert!(values.iter().any(|value| value["method"] == "item/commandExecution/outputDelta"));
    assert!(values.iter().any(|value| value["method"] == "item/commandExecution/terminalInteraction"));
    assert!(values.iter().any(|value| value["method"] == "item/completed"));
    assert!(!serialized.contains("secret-token"));
    assert!(!serialized.contains("rawArguments"));
}

#[test]
fn full_p3_tool_error_path_marks_result_error_and_fails_turn_when_runtime_fails() {
    let bridge = Arc::new(FullP3ManualBridge::scripted(vec![
        RuntimeTurnOutcome::ToolResult {
            update: RuntimeToolResultUpdate {
                item_id: "turn_1:tool:lookup".to_string(),
                content: "tool failed".to_string(),
                is_error: true,
            },
        },
        RuntimeTurnOutcome::Failed { error: "tool failed".to_string() },
    ]));
    let mut server = initialized_codex_v2_server_with_bridge(bridge);
    start_test_turn(&mut server);

    let values = drain_json_rpc_values_for(&mut server, Duration::from_millis(200));
    assert!(values.iter().any(|value| {
        value["method"] == "item/commandExecution/terminalInteraction"
            && value["params"]["isError"] == true
    }));
    assert!(values.iter().any(|value| value["method"] == "turn/failed"));
}
```

- [ ] **Step 2: Extend item types**

Change protocol `ItemType`:

```rust
pub enum ItemType {
    AgentMessage,
    CommandExecution,
    DynamicToolCall,
    ToolUserInput,
}
```

Keep `serde(rename_all = "snake_case")`. Existing agent message tests must keep serializing `agent_message`.

- [ ] **Step 3: Add runtime lifecycle variants**

Add:

```rust
pub struct RuntimeToolLifecycleStarted {
    pub item_id: String,
    pub tool_name: String,
    pub display_arguments: Value,
}

pub struct RuntimeToolLifecycleCompleted {
    pub item_id: String,
    pub success: bool,
}

pub enum RuntimeTurnOutcome {
    ToolLifecycleStarted(RuntimeToolLifecycleStarted),
    ToolLifecycleCompleted(RuntimeToolLifecycleCompleted),
    /* existing variants */
}
```

Test-only fake structs may include `raw_arguments_for_test`, but production structs must not.

- [ ] **Step 4: Map lifecycle events**

In `drain_runtime_turn_updates()`:

- `ToolLifecycleStarted` emits `item/started` with `ItemType::CommandExecution` for command tools or `ItemType::DynamicToolCall` for dynamic tools.
- `CommandOutputDelta` emits `item/commandExecution/outputDelta`.
- `ToolResult` emits `item/commandExecution/terminalInteraction` with `isError`.
- `ToolLifecycleCompleted` emits `item/completed` with `TurnStatus::Completed` when success, `TurnStatus::Failed` when not.

Do not include raw arguments in any emitted event. Use `display_arguments` only in dynamic `item/tool/call` params.

- [ ] **Step 5: Run lifecycle tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(full_p3_tool_lifecycle_)'
```

Expected: PASS.

- [ ] **Step 6: Commit lifecycle bridge**

```bash
git add crates/dasclaw_app_server_protocol/src/lib.rs crates/dasclaw_app_server/src/lib.rs
git commit -m $'feat: bridge full p3 tool lifecycle safely\n\n已检查 P3 Approval Tool Sandbox 是否已有，结论：已有 runtime/protocol primitives 分散存在；本切片补 app-server 完整接线与受测能力，不新增重复 crate。'
```

## Task 5: Complete Command and Permissions Approval

**Files:**
- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`

- [ ] **Step 1: Add failing approval tests**

Add tests covering command approval and permissions approval separately:

```rust
#[test]
fn full_p3_permissions_approval_uses_permissions_payload_not_command_decision() {
    let bridge = Arc::new(FullP3ManualBridge::with_permissions_request());
    let mut server = initialized_codex_v2_server_with_bridge(bridge.clone());
    start_test_turn(&mut server);

    let requests = drain_json_rpc_values_for(&mut server, Duration::from_millis(200));
    let request = requests.iter()
        .find(|value| value["method"] == "item/permissions/requestApproval")
        .expect("permissions approval request should be emitted");
    assert_eq!(request["params"]["permissions"]["network"], serde_json::Value::Null);
    assert!(request["params"].get("command").is_none());

    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": request["id"],
        "result": {
            "permissions": {"network": null, "fileSystem": null},
            "scope": "turn",
            "strictAutoReview": true
        }
    })
    .to_string();
    assert!(server.handle_json_rpc(&response).is_none());
    assert_eq!(bridge.permissions_responses().len(), 1);
}

#[test]
fn full_p3_command_and_permissions_timeout_and_stale_are_fail_safe() {
    let bridge = Arc::new(FullP3ManualBridge::with_command_and_permissions_requests());
    let mut server = initialized_codex_v2_server_with_bridge(bridge.clone());
    start_test_turn(&mut server);
    let requests = drain_json_rpc_values_for(&mut server, Duration::from_millis(200));
    assert!(requests.iter().any(|value| value["method"] == "item/commandExecution/requestApproval"));
    assert!(requests.iter().any(|value| value["method"] == "item/permissions/requestApproval"));

    server.expire_pending_server_requests_for_tests(Duration::from_secs(301));
    let notifications = server.drain_notifications();
    assert!(notifications.iter().filter(|notification| {
        notification.method == "serverRequest/resolved"
            && notification.params["outcome"] == "timed_out"
    }).count() >= 2);

    let stale = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "permissions_stale",
        "result": {"permissions": {"network": null, "fileSystem": null}, "scope": "turn"}
    })
    .to_string();
    assert!(server.handle_json_rpc(&stale).is_none());
    assert!(server.drain_notifications().iter().any(|notification| {
        notification.method == "serverRequest/resolved"
            && notification.params["outcome"] == "failed"
    }));
}
```

- [ ] **Step 2: Add bridge resolver methods**

Extend `RuntimeBridge`:

```rust
fn resolve_permissions_approval(
    &self,
    _decision: RuntimePermissionsApprovalDecision,
) -> Result<(), RuntimeBridgeError> {
    Err(RuntimeBridgeError::fatal("runtime bridge does not support permissions approval resolution"))
}
```

Add:

```rust
pub struct RuntimePermissionsApprovalRequest {
    pub request_id: String,
    pub item_id: String,
    pub cwd: String,
    pub reason: Option<String>,
    pub permissions: dasclaw_protocol::request_permissions::RequestPermissionProfile,
}

pub struct RuntimePermissionsApprovalDecision {
    pub request_id: String,
    pub response: dasclaw_protocol::request_permissions::RequestPermissionsResponse,
}
```

- [ ] **Step 3: Emit permissions approval request**

Add `RuntimeTurnOutcome::PermissionsApprovalRequested { request }`. Map it to `server_request::ITEM_PERMISSIONS_REQUEST_APPROVAL`, insert `PendingServerRequestKind::PermissionsApproval`, and include `cwd`, `reason`, and `permissions`. It must not include `command`, `toolName`, or `allowAlways`.

- [ ] **Step 4: Parse command approval separately from permissions approval**

Command approval accepts existing app-server decision payload:

```json
{"decision":{"kind":"approve"}}
```

Permissions approval accepts Codex permissions payload:

```json
{"permissions":{"network":null,"fileSystem":null},"scope":"turn","strictAutoReview":true}
```

Do not parse permissions response as `AppServerApprovalDecision`.

- [ ] **Step 5: Run approval tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(full_p3_permissions_approval_) | test(full_p3_command_and_permissions_timeout_and_stale_are_fail_safe)'
```

Expected: PASS.

- [ ] **Step 6: Commit approval completion**

```bash
git add crates/dasclaw_app_server/src/lib.rs crates/dasclaw_app_server_protocol/src/lib.rs
git commit -m $'feat: complete command and permissions approval flows\n\n已检查 P3 Approval Tool Sandbox 是否已有，结论：已有 runtime/protocol primitives 分散存在；本切片补 app-server 完整接线与受测能力，不新增重复 crate。'
```

## Task 6: Add Auto Approval Review Flow

**Files:**
- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`

- [ ] **Step 1: Add failing auto-review tests**

Add:

```rust
#[test]
fn full_p3_auto_review_started_and_completed_have_ordered_event_semantics() {
    let bridge = Arc::new(FullP3ManualBridge::with_auto_review_then_command_approval());
    let mut server = initialized_codex_v2_server_with_bridge(bridge);
    start_test_turn(&mut server);

    let values = drain_json_rpc_values_for(&mut server, Duration::from_millis(200));
    let methods = values.iter().filter_map(|value| value["method"].as_str()).collect::<Vec<_>>();
    let started = methods.iter().position(|method| *method == "item/autoApprovalReview/started")
        .expect("auto review started should be emitted");
    let completed = methods.iter().position(|method| *method == "item/autoApprovalReview/completed")
        .expect("auto review completed should be emitted");
    let approval = methods.iter().position(|method| *method == "item/commandExecution/requestApproval")
        .expect("manual approval should follow denied review");
    assert!(started < completed);
    assert!(completed < approval);
}

#[test]
fn full_p3_auto_review_denied_without_followup_fails_safe() {
    let bridge = Arc::new(FullP3ManualBridge::with_denied_auto_review_without_approval());
    let mut server = initialized_codex_v2_server_with_bridge(bridge);
    start_test_turn(&mut server);

    let values = drain_json_rpc_values_for(&mut server, Duration::from_millis(200));
    assert!(values.iter().any(|value| value["method"] == "item/autoApprovalReview/completed"
        && value["params"]["review"]["status"] == "denied"));
    assert!(values.iter().any(|value| value["method"] == "turn/failed"));
}
```

- [ ] **Step 2: Add runtime outcome variants**

Add:

```rust
pub enum RuntimeAutoReviewFollowup {
    AutoApproved,
    RequireCommandApproval(RuntimeApprovalRequest),
    RequirePermissionsApproval(RuntimePermissionsApprovalRequest),
    FailSafe { reason: String },
}

pub enum RuntimeTurnOutcome {
    AutoApprovalReviewStarted {
        assessment: dasclaw_protocol::approvals::GuardianAssessmentEvent,
    },
    AutoApprovalReviewCompleted {
        assessment: dasclaw_protocol::approvals::GuardianAssessmentEvent,
        followup: RuntimeAutoReviewFollowup,
    },
    /* existing variants */
}
```

- [ ] **Step 3: Map review events and decisions**

Rules:

- Started emits only `item/autoApprovalReview/started`.
- Completed emits `item/autoApprovalReview/completed`.
- `AutoApproved` continues without a server request.
- `RequireCommandApproval` emits command approval after completed review.
- `RequirePermissionsApproval` emits permissions approval after completed review.
- `FailSafe` fails the turn after emitting completed review.

- [ ] **Step 4: Run auto-review tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(full_p3_auto_review_)'
```

Expected: PASS.

- [ ] **Step 5: Commit auto-review flow**

```bash
git add crates/dasclaw_app_server/src/lib.rs crates/dasclaw_app_server_protocol/src/lib.rs
git commit -m $'feat: add full p3 auto approval review flow\n\n已检查 P3 Approval Tool Sandbox 是否已有，结论：已有 runtime/protocol primitives 分散存在；本切片补 app-server 完整接线与受测能力，不新增重复 crate。'
```

## Task 7: Add Dynamic Tool Call and User Input Requests

**Files:**
- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`

- [ ] **Step 1: Add failing dynamic tool and user-input tests**

Add:

```rust
#[test]
fn full_p3_dynamic_tool_call_round_trips_result_to_runtime() {
    let bridge = Arc::new(FullP3ManualBridge::with_dynamic_tool_call());
    let mut server = initialized_codex_v2_server_with_bridge(bridge.clone());
    start_test_turn(&mut server);

    let requests = drain_json_rpc_values_for(&mut server, Duration::from_millis(200));
    let request = requests.iter().find(|value| value["method"] == "item/tool/call")
        .expect("dynamic tool call request should be emitted");
    assert_eq!(request["params"]["tool"], "lookup_ticket");
    assert!(request["params"].get("rawArguments").is_none());

    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": request["id"],
        "result": {"contentItems": [{"type": "inputText", "text": "ticket 123"}], "success": true}
    })
    .to_string();
    assert!(server.handle_json_rpc(&response).is_none());
    assert_eq!(bridge.dynamic_tool_responses().len(), 1);
}

#[test]
fn full_p3_tool_request_user_input_round_trips_answers_and_cancels_fail_safe() {
    let bridge = Arc::new(FullP3ManualBridge::with_user_input_request());
    let mut server = initialized_codex_v2_server_with_bridge(bridge.clone());
    start_test_turn(&mut server);

    let requests = drain_json_rpc_values_for(&mut server, Duration::from_millis(200));
    let request = requests.iter().find(|value| value["method"] == "item/tool/requestUserInput")
        .expect("request user input should be emitted");
    assert_eq!(request["params"]["questions"][0]["id"], "choice");

    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": request["id"],
        "result": {"answers": {"choice": {"answers": ["Safe"]}}}
    })
    .to_string();
    assert!(server.handle_json_rpc(&response).is_none());
    assert_eq!(bridge.user_input_responses().len(), 1);

    let mut timeout_server = initialized_codex_v2_server_with_bridge(
        Arc::new(FullP3ManualBridge::with_user_input_request()),
    );
    start_test_turn(&mut timeout_server);
    timeout_server.expire_pending_server_requests_for_tests(Duration::from_secs(301));
    assert!(timeout_server.drain_notifications().iter().any(|notification| {
        notification.method == "serverRequest/resolved"
            && notification.params["outcome"] == "timed_out"
    }));
}
```

- [ ] **Step 2: Add bridge request/response types**

Add:

```rust
pub struct RuntimeDynamicToolCallRequest {
    pub call_id: String,
    pub item_id: String,
    pub namespace: Option<String>,
    pub tool: String,
    pub arguments: Value,
}

pub struct RuntimeDynamicToolCallDecision {
    pub call_id: String,
    pub response: dasclaw_protocol::dynamic_tools::DynamicToolResponse,
}

pub struct RuntimeToolUserInputRequest {
    pub call_id: String,
    pub item_id: String,
    pub questions: Vec<dasclaw_protocol::request_user_input::RequestUserInputQuestion>,
}

pub struct RuntimeToolUserInputDecision {
    pub call_id: String,
    pub response: dasclaw_protocol::request_user_input::RequestUserInputResponse,
}
```

Extend `RuntimeBridge` with `resolve_dynamic_tool_call()` and `resolve_tool_user_input()`.

- [ ] **Step 3: Emit request kinds**

Add `RuntimeTurnOutcome::DynamicToolCallRequested` and `RuntimeTurnOutcome::ToolUserInputRequested`. Map them to `item/tool/call` and `item/tool/requestUserInput`, insert matching pending kinds, and emit `item/started` with `ItemType::DynamicToolCall` or `ItemType::ToolUserInput`.

- [ ] **Step 4: Parse responses**

`item/tool/call` result must deserialize as `DynamicToolCallResponsePayload`. `item/tool/requestUserInput` result must deserialize as `ToolRequestUserInputResponsePayload`. Client JSON-RPC error must create failed `serverRequest/resolved`, call the corresponding runtime resolver with a fail-safe response, and fail the turn.

- [ ] **Step 5: Run dynamic request tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(full_p3_dynamic_tool_call_) | test(full_p3_tool_request_user_input_)'
```

Expected: PASS.

- [ ] **Step 6: Commit dynamic requests**

```bash
git add crates/dasclaw_app_server/src/lib.rs crates/dasclaw_app_server_protocol/src/lib.rs
git commit -m $'feat: add full p3 dynamic tool and user input requests\n\n已检查 P3 Approval Tool Sandbox 是否已有，结论：已有 runtime/protocol primitives 分散存在；本切片补 app-server 完整接线与受测能力，不新增重复 crate。'
```

## Task 8: Add Full P3 Stdio E2E Coverage

**Files:**
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [ ] **Step 1: Add stdio e2e tests**

Add these test names with `FullP3ManualBridge`:

```rust
#[test]
fn full_p3_stdio_e2e_command_approval_approve_reject_timeout_and_stale() { /* use line-delimited JSON-RPC input */ }

#[test]
fn full_p3_stdio_e2e_permissions_approval_approve_reject_timeout_and_stale() { /* use permissions response payload */ }

#[test]
fn full_p3_stdio_e2e_dynamic_tool_call_success_error_and_unsupported_request() { /* use item/tool/call response and JSON-RPC error */ }

#[test]
fn full_p3_stdio_e2e_request_user_input_answer_cancel_and_timeout() { /* use answers map and JSON-RPC error */ }

#[test]
fn full_p3_stdio_e2e_auto_review_events_precede_manual_request() { /* assert event ordering */ }
```

Each test must parse stdout lines as JSON and assert request ids are reused in client responses.

- [ ] **Step 2: Implement exact e2e assertions**

For each test assert:

- Every server-request line has `jsonrpc == "2.0"`, `id`, `method`, and `params`.
- Every client response uses the server-request `id`.
- Approve/success emits `serverRequest/resolved`.
- Reject/error emits `serverRequest/resolved` and `turn/failed`.
- Timeout emits `serverRequest/resolved` with `timed_out`.
- Stale late response emits `serverRequest/resolved` with `failed`.
- Unsupported request emits `serverRequest/resolved` with `failed` and does not leave pending state.
- Serialized stdout does not contain `rawArguments`, `apiKey`, `secret-token`, or raw test-only arguments.

- [ ] **Step 3: Run stdio e2e tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(full_p3_stdio_e2e_)'
```

Expected: PASS.

- [ ] **Step 4: Commit e2e tests**

```bash
git add crates/dasclaw_app_server/src/lib.rs
git commit -m $'test: cover full p3 app-server stdio e2e\n\n已检查 P3 Approval Tool Sandbox 是否已有，结论：已有 runtime/protocol primitives 分散存在；本切片补 app-server 完整接线与受测能力，不新增重复 crate。'
```

## Task 9: Extend Desktop Main and Renderer Bridge

**Files:**
- Modify: `desktop-app/src/main/appServerRpc.ts`
- Modify: `desktop-app/src/main/appServerRpc.test.ts`
- Modify: `desktop-app/src/main/appServerManager.ts`
- Modify: `desktop-app/src/main/appServerManager.test.ts`
- Modify: `desktop-app/src/shared/appServerApi.ts`
- Modify: `desktop-app/src/renderer/src/hooks/useDasclawAssistantRuntime.ts`
- Modify: `desktop-app/src/renderer/src/lib/assistantMessages.test.ts`

- [ ] **Step 1: Add failing desktop RPC tests**

In `appServerRpc.test.ts`, add tests for:

- `item/permissions/requestApproval` classified as `server-request`.
- `item/tool/call` classified as `server-request`.
- `item/tool/requestUserInput` classified as `server-request`.
- `buildJsonRpcResponseLine()` can serialize permissions, dynamic tool, user input, and JSON-RPC error responses.

Expected before implementation: FAIL if response/error builder or types are missing.

- [ ] **Step 2: Add shared request union types**

In `appServerApi.ts`, define:

```ts
export type AppServerServerRequest =
  | AppServerCommandApprovalRequest
  | AppServerPermissionsApprovalRequest
  | AppServerDynamicToolCallRequest
  | AppServerToolUserInputRequest

export type AppServerNotification = AppServerGenericNotification | AppServerServerRequest
```

Each request type must include `hostId`, `requestId`, exact `method`, and typed `params`. `AppServerDynamicToolCallRequest.params.arguments` must represent sanitized display arguments only.

- [ ] **Step 3: Generalize manager forwarding**

In `AppServerManager.handleServerRequest()`:

- Command approval: forward as `AppServerCommandApprovalRequest`.
- Permissions approval: forward as `AppServerPermissionsApprovalRequest`.
- Dynamic tool call: forward as `AppServerDynamicToolCallRequest`.
- Request user input: forward as `AppServerToolUserInputRequest`.
- Unknown server request: respond with JSON-RPC error line, not an approval-style reject.

Scrub `rawArguments`, `apiKey`, `secret`, and `requestId` from params before emitting to renderer.

- [ ] **Step 4: Add response aliases**

Support renderer calls:

```text
approval/respond
permissions/respond
tool/respond
toolUserInput/respond
```

Each alias writes a JSON-RPC response with the original `requestId`. Invalid alias payloads throw synchronously before writing to child stdin.

- [ ] **Step 5: Add renderer fail-safe handling**

In `useDasclawAssistantRuntime.ts`:

- Existing approval requests keep fail-safe rejection until a UI handler exists.
- Permissions requests respond with empty denied permissions and `scope: "turn"` plus reason `renderer permissions UI is not implemented`.
- Dynamic tool calls respond with `{ success: false, contentItems: [{ type: "inputText", text: "renderer dynamic tool UI is not implemented" }] }`.
- Request user input responds with `{ answers: {} }` and reason is represented through the manager-side JSON-RPC error when cancellation is required.

This keeps renderer behavior explicit and testable rather than silently hanging.

- [ ] **Step 6: Add manager and renderer tests**

Run:

```bash
cd desktop-app && npm test -- --run src/main/appServerRpc.test.ts src/main/appServerManager.test.ts src/renderer/src/lib/assistantMessages.test.ts
```

Expected: PASS.

- [ ] **Step 7: Commit desktop bridge**

```bash
git add desktop-app/src/main/appServerRpc.ts desktop-app/src/main/appServerRpc.test.ts desktop-app/src/main/appServerManager.ts desktop-app/src/main/appServerManager.test.ts desktop-app/src/shared/appServerApi.ts desktop-app/src/renderer/src/hooks/useDasclawAssistantRuntime.ts desktop-app/src/renderer/src/lib/assistantMessages.test.ts
git commit -m $'feat: extend desktop full p3 server-request bridge\n\n已检查 P3 Approval Tool Sandbox 是否已有，结论：已有 runtime/protocol primitives 分散存在；本切片补 app-server 完整接线与受测能力，不新增重复 crate。'
```

## Task 10: Update Profile and Gap Matrix Only After Tests Pass

**Files:**
- Modify: `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`
- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [ ] **Step 1: Add profile verification tests**

Add:

```rust
#[test]
fn codex_v2_profile_advertises_full_p3_events_only_when_full_p3_bridge_is_active() {
    let no_runtime = AppServer::new();
    let no_runtime_profile = no_runtime.capabilities().compatibility_profiles[0].clone();
    assert!(no_runtime_profile.capability_opt_outs.iter().any(|opt| opt.capability == "tools"));

    let full = AppServer::with_runtime_bridge(Arc::new(FullP3ManualBridge::default()));
    let full_profile = full.capabilities().compatibility_profiles[0].clone();
    assert!(full_profile.events.contains(&"item/autoApprovalReview/started".to_string()));
    assert!(full_profile.events.contains(&"item/autoApprovalReview/completed".to_string()));
    assert!(!full_profile.capability_opt_outs.iter().any(|opt| opt.capability == "tools"));
}
```

- [ ] **Step 2: Wire dynamic profile generation**

Ensure `AppServer::initialize()`, `AppServer::capabilities()`, `AppServer::protocol_schema()`, and `notifications/initialized` all call the capability-aware profile builder with the current matrix.

- [ ] **Step 3: Update gap matrix**

In `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`, mark only the tested P3 subset as complete:

- `item/commandExecution/requestApproval`
- `item/permissions/requestApproval`
- `item/tool/requestUserInput`
- `item/tool/call`
- `item/autoApprovalReview/started`
- `item/autoApprovalReview/completed`
- `serverRequest/resolved`
- `item/commandExecution/outputDelta`
- `item/commandExecution/terminalInteraction`

Keep non-P3 Codex surfaces unmarked: file-change approval, MCP elicitation, account token refresh, legacy `applyPatchApproval`, legacy `execCommandApproval`, `fs/*`, standalone `command/exec*`, account/plugin/marketplace/product surfaces.

- [ ] **Step 4: Run profile tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(codex_v2_profile_advertises_full_p3_events_only_when_full_p3_bridge_is_active)'
```

Expected: PASS.

- [ ] **Step 5: Commit profile/docs**

```bash
git add crates/dasclaw_app_server_protocol/src/lib.rs crates/dasclaw_app_server/src/lib.rs docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md
git commit -m $'docs: mark tested full p3 app-server support\n\n已检查 P3 Approval Tool Sandbox 是否已有，结论：已有 runtime/protocol primitives 分散存在；本切片补 app-server 完整接线与受测能力，不新增重复 crate。'
```

## Task 11: Final Verification and Review Lane

**Files:**
- All touched files from Tasks 1-10.

- [ ] **Step 1: Rust targeted checks**

Run:

```bash
cargo check -p dasclaw_app_server_protocol --tests
cargo check -p dasclaw_app_server --tests
cargo nextest run -p dasclaw_app_server_protocol -E 'test(full_p3_)'
cargo nextest run -p dasclaw_app_server -E 'test(full_p3_) | test(codex_v2_profile_advertises_full_p3_events_only_when_full_p3_bridge_is_active)'
```

Expected: all commands PASS, 0 compile errors.

- [ ] **Step 2: Desktop targeted checks**

Run:

```bash
cd desktop-app && npm test -- --run src/main/appServerRpc.test.ts src/main/appServerManager.test.ts src/renderer/src/lib/assistantMessages.test.ts
cd desktop-app && npm run typecheck
```

Expected: both commands PASS.

- [ ] **Step 3: Formatting, panic guard, clippy**

Run from repo root:

```bash
cargo fmt --all && python3.12 scripts/check_no_panics.py --base origin/xClaw
cargo clippy --no-deps -p dasclaw_app_server_protocol --all-targets -- -D warnings
cargo clippy --no-deps -p dasclaw_app_server --all-targets -- -D warnings
```

Expected: all commands PASS. If clippy reports warnings outside touched crates, record the file path and leave workspace-wide clippy to CI per repo policy.

- [ ] **Step 4: Required review skills before push**

Run the repo-required review lane:

```text
code-quality-audit 自查
code-simplifier 收敛新改 Rust/TypeScript 实现
code-review-expert PR 自审
```

Expected: no blocker findings remain. If a finding affects `crates/dasclaw_*` ADR-sensitive surfaces, run the applicable ADR compliance review before push.

- [ ] **Step 5: Final evidence summary**

Before opening the PR, write a short verification summary containing:

- Exact Rust commands and PASS results.
- Exact desktop commands and PASS results.
- Whether `codex_app_server_v2` profile stayed honest for default/noop runtime.
- Which Codex server requests remain non-P3 non-goals.

Expected: summary contains no unsupported capability claim.

## Self-Review

Spec coverage:

- 完整工具调用流程由 Tasks 1、2、4、7、8 覆盖：`item/tool/call`、tool lifecycle、tool result、error path、参数脱敏。
- 权限审批完整流程由 Tasks 1、2、3、5、8、9 覆盖：permissions request、response、approve/reject/timeout/stale、与 command approval 的 payload 差异。
- 自动审查流程由 Tasks 1、2、6、8、10 覆盖：started/completed 语义、失败路径、与后续审批决策关系。
- 动态工具请求 / 用户输入请求由 Tasks 1、2、7、8、9 覆盖：请求、响应、取消、超时。
- `codex_app_server_v2` profile 和对外能力清单由 Tasks 2、10、11 覆盖：只在实现与测试补齐后宣称。
- app-server e2e 由 Task 8 覆盖，desktop main/renderer 桥接由 Task 9 覆盖，protocol crate contract tests 由 Tasks 1-2 覆盖。
- 无头 Rust 层工作边界由 `Implementation Boundary` 固化：本计划不是只改 desktop/app-server 表层，也不是新增平行框架。
- Codex 参考边界由 `Codex Reference Boundary` 和 Task 1 Step 0 固化：schema 已作为 wire contract；实现前必须再读 Codex app-server Rust 行为和 v2 suite。

占位扫描：

- 本计划没有留空实现步骤；每个任务都有文件、命令、预期结果和 commit 分片。
- 非目标列出的能力不会在本 P3 完整化 PR 中被 profile 宣称。

类型一致性：

- JSON-RPC server request 使用 `JsonRpcServerRequest`。
- 通用 request resolution 使用 `ServerRequestResolvedEvent`。
- Command approval 使用 `AppServerApprovalDecision` / `ApprovalResponsePayload`。
- Permissions approval 使用 `PermissionsApprovalResponsePayload`。
- Dynamic tool call 使用 `DynamicToolCallRequest` / `DynamicToolCallResponsePayload`。
- Tool user input 使用 `ToolRequestUserInputParams` / `ToolRequestUserInputResponsePayload`。
