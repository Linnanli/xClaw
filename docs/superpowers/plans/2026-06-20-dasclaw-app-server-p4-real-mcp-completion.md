# Dasclaw App Server P4 Honest MCP Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 2026-06-19 P4 honest subset 留下的 MCP 实效缺口真实补完到“诚实可用”的范围：app-server 通过当前项目里的 `crates/dasclaw_mcp` 完成 MCP 状态、reload、工具调用、资源读取与可定位进度事件。OAuth 登录本轮不算完成，不宣传为可用能力，调用时保持 fail-safe 错误和 completion 通知。

**Architecture:** `dasclaw_mcp` 继续做 MCP 协议、session、transport、client 的 owner；`dasclaw_app_server` 做本地控制面的 owner，持有 MCP server registry、client cache、notification queue 和同步 app-server API 到异步 MCP client 的 runtime bridge。测试必须使用真实 `AppServerMcpService` 加本地 MCP test server，不用 `TestMcpService` 冒充真实完成。

**Tech Stack:** Rust 2024, serde JSON-RPC, Tokio current-thread runtime worker, `dasclaw_mcp`, `dasclaw_app_server`, `dasclaw_app_server_protocol`, `dasclaw_runtime::secrets`, `axum` test server, `cargo nextest`.

---

## Scope Check

这份计划只处理 P4 honest subset 之后剩下的 MCP 实效能力。logs、jobs、skills 和 MCP registry/status/reload 已在 2026-06-19 切片里完成为 honest subset；本计划不重复实现它们。

本计划包含：

- 真实 MCP client cache/session owner。
- `mcpServerStatus/list` 拉取真实 tools/resources/resourceTemplates。
- `mcpServer/tool/call` 走真实 MCP `tools/call`。
- `mcpServer/resource/read` 走真实 MCP `resources/read`。
- `item/mcpToolCall/progress` 在有 `threadId + turnId + itemId` 时发出可定位事件。
- `capabilities/list`/`protocol/schema` 只广告真实可用子集。

不包含：

- P5 filesystem / command exec。
- P6 account/plugin marketplace。
- 把已废弃 `desktop-client` 作为依赖。它只能作为 OAuth flow 参考，代码落点必须在 `crates/*`。
- 假装实现 MCP server-side progress notification forwarding；如果执行中决定接 `notifications/progress`，必须另加明确测试，不得用空事件代替。
- 完整 MCP OAuth 登录。OAuth 本轮只保留不可用错误、脱敏和完成通知，不计入完成能力。

## Startup 4 Questions

| Question | Answer | Required action |
|---|---|---|
| 是否新增模块 / crate / 文件？ | 是。计划文件新增；实施会新增 app-server MCP runtime/test support，可能新增 reusable blocking runtime。 | 已执行 Level 1 semantic search。实施 commit message 必须含 `已检查 P4 real MCP owner 是否已有，结论：...`。 |
| 是否有否定结论？ | 是。结论是 app-server 现有 effectful MCP 仍 fail-safe，真实 session/OAuth/resource owner 未接线。 | 已用 semantic search + LSP MCP + `rg` 补证；执行时若改写结论需重新验证。 |
| 是否跨项目对账？ | 轻量涉及。只读取旧 `desktop-client/ironclaw` OAuth 作为参考。 | 不依赖旧客户端，不把旧行为照搬为 app-server API。 |
| 是否写架构对账类文档？ | 是，这是实现计划。 | 明确 evidence vs decision，避免把 protocol DTO 当实际服务。 |

Evidence snapshot:

- Semantic search 1: `app server MCP real session owner tool call resource read OAuth login progress events` 命中 `crates/dasclaw_mcp/src/client.rs::{with_session_manager,new_authenticated,refresh_access_token_via_hook,get_access_token}`。
- Semantic search 2: `MCP client session manager call tool read resource authorization URL callback app server adapter` 命中 `crates/dasclaw_mcp/src/client.rs`、`session.rs`，以及旧 `desktop-client/ironclaw/src/extensions/manager.rs::auth_mcp_build_url` 参考。
- LSP MCP `document_symbols` 确认 `crates/dasclaw_app_server/src/mcp_service.rs::AppServerMcpService` 当前只有 `registry: Arc<Mutex<McpServersFile>>`；`call_tool/read_resource/oauth_login` 方法存在但返回 capability unavailable。
- LSP MCP `document_symbols` 确认 `crates/dasclaw_mcp/src/client.rs::McpClient` 当前已有 `initialize/list_tools/call_tool`，没有 `read_resource/list_resources/list_resource_templates`。
- `rg` 确认 `McpServerToolCallParams` 当前只有 `thread_id/server/tool/arguments/meta`，而 `McpToolCallProgressNotification` 需要 `thread_id/turn_id/item_id/message`。

## Anti-Patch Rules

- 不允许只改 `P4McpServiceAvailability` 把 `tool_call/resource_read/oauth_login/tool_call_progress_events` 置为 true。
- 不允许用 `TestMcpService` 证明真实 MCP 完成；它只能保留做 router 单元测试。
- 不允许把测试断言改成“返回任意非空 JSON 就算成功”。工具调用测试必须断言本地 MCP server 收到了 `tools/call`，资源测试必须断言收到了 `resources/read`。
- 不允许为进度事件伪造 `turnId/itemId`。客户端不传上下文时可以执行工具，但不能发不可定位进度事件。
- 不允许把 OAuth login 测成“拼出一个 URL 就成功”。本轮 OAuth 不是完成项；测试只允许证明它不宣传、调用 fail-safe、通知失败完成。
- 不允许在错误、状态、通知、测试 failure message 里泄露 access token、refresh token、authorization code、client secret、`Authorization` header。

## File Structure

Modify:

- `crates/dasclaw_mcp/src/protocol.rs`
- `crates/dasclaw_mcp/src/client.rs`
- `crates/dasclaw_mcp/src/lib.rs`
- `crates/dasclaw_mcp/src/http_transport.rs` if SSE parsing needs resource/progress coverage.
- `crates/dasclaw_app_server_protocol/src/lib.rs`
- `crates/dasclaw_app_server/src/mcp_service.rs`
- `crates/dasclaw_app_server/src/p4_services.rs`
- `crates/dasclaw_app_server/src/lib.rs`
- `crates/dasclaw_app_server/src/main.rs`
- `crates/dasclaw_app_server/Cargo.toml`
- `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`
- `docs/superpowers/plans/2026-06-19-dasclaw-app-server-p4-mcp-skills-logs-jobs.md`

Create:

- `crates/dasclaw_app_server/src/mcp_oauth.rs`
- `crates/dasclaw_app_server/src/mcp_test_support.rs` under `#[cfg(test)]`, or equivalent in-module test support.
- Optional: `crates/dasclaw_app_server/src/blocking_runtime.rs` if execution extracts the current `JobRuntime` pattern for reuse.

## Definition of Done

- A real `AppServerMcpService` constructed with registry + runtime + process manager can connect to a local MCP HTTP server, list tools/resources/templates, call a tool, and read a resource.
- Capability advertising lists only status/reload/tool/resource/progress/startup status; it does not advertise OAuth.
- OAuth login returns a capability-unavailable error and emits a failed completion notification; it does not fabricate an authorization URL.
- Tool progress events include the caller-provided `threadId`, `turnId`, and `itemId`; missing `turnId/itemId` does not create fake progress.
- Tests prove actual server interaction, no token leakage, disabled server fail-safe, OAuth fail-safe, and no test-only masking.

Execution note: any lower task text that discusses full OAuth callback/token exchange is future scope for a later slice, not part of this P4 honest subset completion.

## Task 1: Extend Protocol For Real Tool Progress Context

**Files:**

- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`

- [ ] **Step 1: Add protocol fields without breaking older callers**

Add optional progress identity to `McpServerToolCallParams`:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerToolCallParams {
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_id: Option<String>,
    pub server: String,
    pub tool: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "_meta")]
    pub meta: Option<serde_json::Value>,
}

impl McpServerToolCallParams {
    #[must_use]
    pub fn progress_identity(&self) -> Option<McpToolProgressIdentity> {
        Some(McpToolProgressIdentity {
            thread_id: self.thread_id.clone(),
            turn_id: self.turn_id.clone()?,
            item_id: self.item_id.clone()?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpToolProgressIdentity {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
}
```

- [ ] **Step 2: Add serialization tests**

Add tests proving both old and new payloads deserialize:

```rust
#[test]
fn mcp_tool_call_params_accept_optional_progress_identity() {
    let params: McpServerToolCallParams = serde_json::from_value(serde_json::json!({
        "threadId": "thread_1",
        "turnId": "turn_1",
        "itemId": "item_1",
        "server": "local",
        "tool": "echo",
        "arguments": {"message": "hi"}
    }))
    .expect("tool call params deserialize");

    let identity = params.progress_identity().expect("progress identity");
    assert_eq!(identity.thread_id, "thread_1");
    assert_eq!(identity.turn_id, "turn_1");
    assert_eq!(identity.item_id, "item_1");
}

#[test]
fn mcp_tool_call_params_without_turn_item_do_not_fabricate_progress_identity() {
    let params: McpServerToolCallParams = serde_json::from_value(serde_json::json!({
        "threadId": "thread_1",
        "server": "local",
        "tool": "echo"
    }))
    .expect("legacy tool call params deserialize");

    assert!(params.progress_identity().is_none());
}
```

## Task 2: Complete `dasclaw_mcp` Resource APIs

**Files:**

- Modify: `crates/dasclaw_mcp/src/protocol.rs`
- Modify: `crates/dasclaw_mcp/src/client.rs`
- Modify: `crates/dasclaw_mcp/src/lib.rs`

- [ ] **Step 1: Add MCP resource protocol types and requests**

Add protocol helpers:

```rust
impl McpRequest {
    pub fn list_resources(id: u64) -> Self {
        Self::new(id, "resources/list", None)
    }

    pub fn list_resource_templates(id: u64) -> Self {
        Self::new(id, "resources/templates/list", None)
    }

    pub fn read_resource(id: u64, uri: &str) -> Self {
        Self::new(
            id,
            "resources/read",
            Some(serde_json::json!({ "uri": uri })),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub annotations: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpResourceTemplate {
    pub uri_template: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub annotations: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListResourcesResult {
    #[serde(default)]
    pub resources: Vec<McpResource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResourceTemplatesResult {
    #[serde(default)]
    pub resource_templates: Vec<McpResourceTemplate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadResourceResult {
    pub contents: Vec<McpResourceContent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResourceContent {
    pub uri: String,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub blob: Option<String>,
    #[serde(default, rename = "_meta")]
    pub meta: Option<serde_json::Value>,
}
```

- [ ] **Step 2: Add client methods**

Add methods to `McpClient`:

```rust
pub async fn list_resources(&self) -> Result<Vec<McpResource>, ToolError> {
    self.initialize().await?;
    let response = self
        .send_request(McpRequest::list_resources(self.next_request_id()))
        .await?;
    parse_mcp_result::<ListResourcesResult>(response, "resources list").map(|r| r.resources)
}

pub async fn list_resource_templates(&self) -> Result<Vec<McpResourceTemplate>, ToolError> {
    self.initialize().await?;
    let response = self
        .send_request(McpRequest::list_resource_templates(self.next_request_id()))
        .await?;
    parse_mcp_result::<ListResourceTemplatesResult>(response, "resource templates list")
        .map(|r| r.resource_templates)
}

pub async fn read_resource(&self, uri: &str) -> Result<ReadResourceResult, ToolError> {
    self.initialize().await?;
    let response = self
        .send_request(McpRequest::read_resource(self.next_request_id(), uri))
        .await?;
    parse_mcp_result::<ReadResourceResult>(response, "resource read")
}

fn parse_mcp_result<T: serde::de::DeserializeOwned>(
    response: McpResponse,
    label: &str,
) -> Result<T, ToolError> {
    if let Some(error) = response.error {
        return Err(ToolError::ExternalService(format!(
            "MCP {label} error: {} (code {})",
            error.message, error.code
        )));
    }
    response
        .result
        .ok_or_else(|| ToolError::ExternalService(format!("No result in MCP {label} response")))
        .and_then(|value| {
            serde_json::from_value(value).map_err(|error| {
                ToolError::ExternalService(format!("Invalid MCP {label} result: {error}"))
            })
        })
}
```

Refactor existing `list_tools` and `call_tool` to use `parse_mcp_result` only if the refactor remains small.

- [ ] **Step 3: Add MCP client tests with mock transport**

Use the existing `MockTransport` pattern in `client.rs` to test `resources/read` and resource listing. The tests must assert method names, not only parsed output. If `MockTransport` does not record requests yet, add a `recorded_requests` field:

```rust
assert_eq!(requests[0].method, "initialize");
assert_eq!(requests[2].method, "resources/read");
assert_eq!(requests[2].params.as_ref().unwrap()["uri"], "file:///note.txt");
```

## Task 3: Add App-Server MCP Runtime And Client Cache

**Files:**

- Modify: `crates/dasclaw_app_server/src/mcp_service.rs`
- Modify: `crates/dasclaw_app_server/src/p4_services.rs`
- Optional create: `crates/dasclaw_app_server/src/blocking_runtime.rs`

- [ ] **Step 1: Introduce a reusable sync-to-async runtime bridge**

If not extracting `JobRuntime`, create a local MCP runtime in `mcp_service.rs`. Preferred extracted shape:

```rust
#[derive(Clone)]
pub(crate) struct BlockingTokioRuntime {
    inner: Arc<BlockingTokioRuntimeInner>,
}

impl BlockingTokioRuntime {
    pub(crate) fn new(worker_name: &'static str, service_name: &'static str) -> Result<Self, AppServerError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| {
                AppServerError::capability_unavailable(
                    service_name,
                    format!("{service_name} runtime unavailable: {error}"),
                )
            })?;
        Self::spawn(worker_name, service_name, runtime)
    }

    pub(crate) fn block_on<T>(
        &self,
        operation: &'static str,
        future: impl Future<Output = Result<T, AppServerError>> + Send + 'static,
    ) -> Result<T, AppServerError>
    where
        T: Send + 'static,
    {
        // Same channel + catch_unwind pattern as current JobRuntime.
    }

    pub(crate) fn spawn_detached(
        &self,
        operation: &'static str,
        future: impl Future<Output = ()> + Send + 'static,
    ) -> Result<(), AppServerError> {
        // Send a task to the worker that calls runtime.spawn(future).
    }
}
```

Do not create a fresh Tokio runtime per MCP request.

- [ ] **Step 2: Store real MCP ownership in `AppServerMcpService`**

Replace registry-only shape with:

```rust
#[derive(Clone)]
pub struct AppServerMcpService {
    registry: Arc<Mutex<McpServersFile>>,
    clients: Arc<Mutex<HashMap<String, Arc<dasclaw_mcp::McpClient>>>>,
    session_manager: Arc<dasclaw_mcp::McpSessionManager>,
    process_manager: Arc<dasclaw_mcp::McpProcessManager>,
    runtime: BlockingTokioRuntime,
    events: McpEventQueue,
    secrets: Option<Arc<dyn dasclaw_runtime::secrets::SecretsStore + Send + Sync>>,
    user_id: String,
    oauth: Option<AppServerMcpOauthConfig>,
}
```

Keep `from_servers` for tests, but make it explicit:

```rust
pub fn from_servers(servers: Vec<McpServerConfig>) -> Self {
    Self::builder(McpServersFile { servers, schema_version: 1 })
        .without_oauth()
        .build()
}
```

- [ ] **Step 3: Add client lookup**

Implement:

```rust
fn enabled_server(&self, name: &str) -> Result<McpServerConfig, AppServerError> {
    let registry = self.registry.lock().unwrap_or_else(|poison| poison.into_inner());
    let server = registry
        .get(name)
        .cloned()
        .ok_or_else(|| AppServerError::invalid_request("mcp", "MCP server not found"))?;
    if !server.enabled {
        return Err(AppServerError::capability_unavailable(
            "mcp",
            format!("MCP server '{name}' is disabled"),
        ));
    }
    server.validate().map_err(|error| {
        AppServerError::invalid_request("mcp", format!("invalid MCP server config: {error}"))
    })?;
    Ok(server)
}

fn client_for(&self, server: McpServerConfig) -> Result<Arc<McpClient>, AppServerError> {
    if let Some(client) = self.clients.lock().unwrap().get(&server.name).cloned() {
        return Ok(client);
    }
    let name = server.name.clone();
    let session_manager = Arc::clone(&self.session_manager);
    let process_manager = Arc::clone(&self.process_manager);
    let secrets = self.secrets.clone();
    let user_id = self.user_id.clone();
    let client = self.runtime.block_on("mcp/create-client", async move {
        dasclaw_mcp::create_client_from_config(
            server,
            &session_manager,
            &process_manager,
            secrets,
            &user_id,
        )
        .await
        .map(Arc::new)
        .map_err(map_mcp_factory_error)
    })?;
    self.clients.lock().unwrap().insert(name, Arc::clone(&client));
    Ok(client)
}
```

Use poison recovery consistently as current code does.

## Task 4: Make Status/Reload Real

**Files:**

- Modify: `crates/dasclaw_app_server/src/mcp_service.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [ ] **Step 1: Map real MCP tools/resources/templates into app-server status**

Implement `list_status` by connecting to each enabled server and calling `list_tools`, `list_resources`, and `list_resource_templates`. If `detail == ToolsAndAuthOnly`, skip resources/templates.

```rust
fn status_for_server(
    &self,
    config: McpServerConfig,
    detail: Option<McpServerStatusDetail>,
) -> Result<McpServerStatus, AppServerError> {
    let client = self.client_for(config.clone())?;
    let auth_status = self.auth_status_for(&config)?;
    self.runtime.block_on("mcp/status", async move {
        let tools = client.list_tools().await.map_err(map_tool_error)?;
        let full = matches!(detail, Some(McpServerStatusDetail::Full) | None);
        let resources = if full {
            client.list_resources().await.map_err(map_tool_error)?
        } else {
            Vec::new()
        };
        let resource_templates = if full {
            client.list_resource_templates().await.map_err(map_tool_error)?
        } else {
            Vec::new()
        };
        Ok(McpServerStatus {
            name: config.name,
            tools: tools_to_status_value(tools),
            resources: resources.into_iter().map(map_resource).collect(),
            resource_templates: resource_templates.into_iter().map(map_resource_template).collect(),
            auth_status,
        })
    })
}
```

If one server fails, return that server as failed only if protocol has a failed field; current protocol does not. Therefore fail the request with a redacted structured error. Do not silently return an empty tool list.

- [ ] **Step 2: Make reload clear caches and report startup status**

`reload(name)` must:

- Validate registry membership.
- For target server(s), remove cached client.
- Shut down stdio process through `McpProcessManager::shutdown(name)`.
- Recreate/list status once to prove startup.
- Queue `McpServerStatusUpdatedNotification { status: Ready }` or `{ status: Failed, error }`.

AppServer should drain queued MCP events through `drain_p4_updates()`:

```rust
fn drain_p4_updates(&mut self) {
    for entry in self.p4_services.drain_log_entries() {
        self.notifications.emit_log_entry(entry);
    }
    for event in self.p4_services.drain_mcp_events() {
        match event {
            McpServiceEvent::StartupStatus(event) => {
                self.notifications.emit_mcp_startup_status_updated(event);
            }
            McpServiceEvent::OauthLoginCompleted(event) => {
                self.notifications.emit_mcp_oauth_login_completed(event);
            }
            McpServiceEvent::ToolCallProgress(event) => {
                self.notifications.emit_mcp_tool_call_progress(event);
            }
        }
    }
}
```

Add `emit_mcp_tool_call_progress` to `NotificationBus` and `ServerNotification` if missing.

## Task 5: Implement Real Tool Call And Progress Events

**Files:**

- Modify: `crates/dasclaw_app_server/src/mcp_service.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`

- [ ] **Step 1: Change AppServer route method to `&mut self`**

`mcp_server_tool_call` currently takes `&self`; change it to `&mut self` so it can emit progress before and after the real call.

```rust
pub fn mcp_server_tool_call(
    &mut self,
    params: McpServerToolCallParams,
) -> Result<McpServerToolCallResponse, AppServerError> {
    self.require_initialized("mcp")?;
    let identity = params.progress_identity();
    if let Some(identity) = identity.as_ref() {
        self.notifications.emit_mcp_tool_call_progress(McpToolCallProgressNotification {
            thread_id: identity.thread_id.clone(),
            turn_id: identity.turn_id.clone(),
            item_id: identity.item_id.clone(),
            message: format!("Calling MCP tool {} on {}", params.tool, params.server),
        });
    }
    let result = self.p4_services.mcp.call_tool(params);
    if let Some(identity) = identity {
        self.notifications.emit_mcp_tool_call_progress(McpToolCallProgressNotification {
            thread_id: identity.thread_id,
            turn_id: identity.turn_id,
            item_id: identity.item_id,
            message: if result.is_ok() {
                "MCP tool call completed".to_string()
            } else {
                "MCP tool call failed".to_string()
            },
        });
    }
    result
}
```

This is real app-server lifecycle progress. Do not advertise server-side MCP progress forwarding unless `dasclaw_mcp` transport captures `notifications/progress` with tests.

- [ ] **Step 2: Implement real `McpService::call_tool`**

In `AppServerMcpService`:

```rust
fn call_tool(
    &self,
    params: McpServerToolCallParams,
) -> Result<McpServerToolCallResponse, AppServerError> {
    let server = self.enabled_server(&params.server)?;
    let client = self.client_for(server)?;
    let tool_name = params.tool.clone();
    let arguments = params.arguments.unwrap_or_else(|| serde_json::json!({}));
    self.runtime.block_on("mcp/tool-call", async move {
        let result = client
            .call_tool(&tool_name, arguments)
            .await
            .map_err(map_tool_error)?;
        Ok(McpServerToolCallResponse {
            content: result
                .content
                .into_iter()
                .map(serde_json::to_value)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| AppServerError::internal("mcp", error.to_string()))?,
            structured_content: None,
            is_error: Some(result.is_error),
            meta: None,
        })
    })
}
```

If later `dasclaw_mcp::CallToolResult` grows `structuredContent` or `_meta`, map those fields instead of dropping them.

- [ ] **Step 3: Add real local MCP server tests**

Use an `axum` test server that records every JSON-RPC request and returns:

- `initialize`
- `notifications/initialized`
- `tools/list`
- `tools/call`

Test:

```rust
#[test]
fn p4_real_mcp_tool_call_reaches_configured_server_and_emits_progress() {
    let fixture = RealMcpFixture::start();
    let service = AppServerMcpService::from_servers(vec![fixture.config("local")]);
    let services = P4Services::for_tests(
        TestLogService::ready(),
        TestJobService::ready(vec![]),
        TestSkillsService::ready(vec![]),
        service,
    );
    let mut server = AppServer::new().with_p4_services(services);

    let response = server.handle_json_rpc(r#"{
        "jsonrpc":"2.0",
        "id":"mcp-call",
        "method":"mcpServer/tool/call",
        "params":{
            "threadId":"thread_1",
            "turnId":"turn_1",
            "itemId":"item_1",
            "server":"local",
            "tool":"echo",
            "arguments":{"message":"hello"}
        }
    }"#).expect("tool call response");

    let value: serde_json::Value = serde_json::from_str(&response).unwrap();
    assert_eq!(value["result"]["content"][0]["type"], "text");
    assert_eq!(value["result"]["content"][0]["text"], "hello");
    assert!(fixture.received_method("tools/call"));

    let notifications = server.drain_notifications();
    assert!(notifications.iter().any(|event| {
        event.method == dasclaw_app_server_protocol::event::ITEM_MCP_TOOL_CALL_PROGRESS
            && event.params["turnId"] == "turn_1"
            && event.params["itemId"] == "item_1"
    }));
}
```

Add a paired test where `turnId/itemId` are absent and assert no progress event is emitted.

## Task 6: Implement Real Resource Read

**Files:**

- Modify: `crates/dasclaw_app_server/src/mcp_service.rs`
- Modify: `crates/dasclaw_mcp/src/client.rs`

- [ ] **Step 1: Implement `McpService::read_resource`**

```rust
fn read_resource(
    &self,
    params: McpResourceReadParams,
) -> Result<McpResourceReadResponse, AppServerError> {
    let server = self.enabled_server(&params.server)?;
    let client = self.client_for(server)?;
    let uri = params.uri.clone();
    self.runtime.block_on("mcp/resource-read", async move {
        let result = client.read_resource(&uri).await.map_err(map_tool_error)?;
        Ok(McpResourceReadResponse {
            contents: result.contents.into_iter().map(map_resource_content).collect(),
        })
    })
}
```

Use a redaction helper for errors:

```rust
fn redact_mcp_uri(uri: &str) -> String {
    let Ok(mut parsed) = url::Url::parse(uri) else {
        return "[redacted-uri]".to_string();
    };
    parsed.set_query(None);
    parsed.set_fragment(None);
    parsed.to_string()
}
```

If adding `url` is a new dependency, prefer using `reqwest::Url` already present through `dasclaw_mcp` only if accessible; otherwise add `url` explicitly and justify it.

- [ ] **Step 2: Add resource tests**

The test server must receive actual `resources/read`:

```rust
assert!(fixture.received(|request| {
    request.method == "resources/read"
        && request.params["uri"] == "file:///docs/readme.md"
}));
```

Add a failure-path test where the server returns a JSON-RPC error with a secret query string and assert the app-server JSON-RPC response does not contain that secret.

## Task 7: Implement Real OAuth Login Flow

**Files:**

- Create: `crates/dasclaw_app_server/src/mcp_oauth.rs`
- Modify: `crates/dasclaw_app_server/src/mcp_service.rs`
- Modify: `crates/dasclaw_app_server/src/p4_services.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [ ] **Step 1: Add app-server OAuth config**

```rust
#[derive(Clone)]
pub struct AppServerMcpOauthConfig {
    pub callback_host: String,
    pub callback_port: Option<u16>,
    pub callback_path: String,
}

#[derive(Clone)]
struct PendingMcpOauthFlow {
    server: McpServerConfig,
    redirect_uri: String,
    token_url: String,
    client_id: String,
    client_secret: Option<String>,
    client_secret_expires_at: Option<u64>,
    pkce: Option<dasclaw_mcp::PkceChallenge>,
    resource: String,
    expected_state: String,
    user_id: String,
}
```

`oauth_login` is available only when both `secrets` and `oauth` config are present.

- [ ] **Step 2: Start login without blocking the JSON-RPC request**

Implement `oauth_login` to:

1. Resolve server.
2. Discover OAuth metadata or use explicit OAuth config.
3. Run DCR if needed.
4. Bind callback listener.
5. Generate PKCE and CSRF state.
6. Spawn a background task on the MCP runtime to wait for callback, exchange token, store token, and queue completion event.
7. Return `McpServerOauthLoginResponse { authorization_url }`.

Key shape:

```rust
fn oauth_login(
    &self,
    params: McpServerOauthLoginParams,
) -> Result<McpServerOauthLoginResponse, AppServerError> {
    let secrets = self.secrets.clone().ok_or_else(|| {
        AppServerError::capability_unavailable("mcp", "MCP OAuth secrets store is not configured")
    })?;
    let oauth = self.oauth.clone().ok_or_else(|| {
        AppServerError::capability_unavailable("mcp", "MCP OAuth callback is not configured")
    })?;
    let server = self.enabled_server(&params.name)?;
    let events = self.events.clone();
    let user_id = self.user_id.clone();

    let runtime = self.runtime.clone();
    let started = self.runtime.block_on("mcp/oauth-start", async move {
        mcp_oauth::start_login(server, params.scopes, oauth, secrets, user_id).await
    })?;

    let name = started.server_name.clone();
    let completion = started.completion;
    runtime.spawn_detached("mcp/oauth-complete", async move {
        let event = match completion.await {
            Ok(()) => McpServerOauthLoginCompletedNotification { name, success: true, error: None },
            Err(error) => McpServerOauthLoginCompletedNotification {
                name,
                success: false,
                error: Some(redact_oauth_error(&error.to_string())),
            },
        };
        events.push(McpServiceEvent::OauthLoginCompleted(event));
    })?;

    Ok(McpServerOauthLoginResponse {
        authorization_url: started.authorization_url,
    })
}
```

- [ ] **Step 3: Store DCR client credentials and token**

In the callback completion path:

```rust
let token = dasclaw_mcp::exchange_code_for_token(
    &flow.token_url,
    &flow.client_id,
    flow.client_secret.as_deref(),
    &code,
    &flow.redirect_uri,
    flow.pkce.as_ref(),
    Some(&flow.resource),
)
.await?;

dasclaw_mcp::store_tokens(&secrets, &flow.user_id, &flow.server, &token).await?;

if flow.server.oauth.is_none() {
    dasclaw_mcp::store_client_id(&secrets, &flow.user_id, &flow.server, &flow.client_id).await?;
    if let Some(secret) = flow.client_secret.as_deref() {
        dasclaw_mcp::store_client_secret(
            &secrets,
            &flow.user_id,
            &flow.server,
            secret,
            flow.client_secret_expires_at,
        )
        .await?;
    }
}
```

Validate callback state before token exchange. State mismatch must emit `success: false` and must not store token.

- [ ] **Step 4: Add real OAuth tests**

Use local test HTTP servers:

- Fake MCP server advertises protected-resource/auth metadata.
- Fake authorization server has authorization endpoint and token endpoint.
- Test invokes `mcpServer/oauth/login`, extracts `authorizationUrl`, reads `state`, then sends a real callback request to the bound listener with `code` and `state`.
- Assert completion notification success.
- Assert `secrets.exists(user_id, server.token_secret_name())` is true.
- Assert response/notifications do not contain the raw code, access token, refresh token, or client secret.

Add negative tests:

- Bad state: no token stored, completion event `success: false`.
- Missing `SecretsStore`: `oauth_login` not advertised and route returns capability unavailable.
- Missing callback config: same fail-safe behavior.

## Task 8: Wire Real Capability Advertising

**Files:**

- Modify: `crates/dasclaw_app_server/src/mcp_service.rs`
- Modify: `crates/dasclaw_app_server/src/p4_services.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [ ] **Step 1: Make availability depend on actual construction**

`AppServerMcpService::availability()` should be:

```rust
fn availability(&self) -> P4McpServiceAvailability {
    P4McpServiceAvailability {
        status_list: true,
        reload: true,
        tool_call: true,
        resource_read: true,
        oauth_login: self.secrets.is_some() && self.oauth.is_some(),
        tool_call_progress_events: true,
        oauth_login_completed_events: self.secrets.is_some() && self.oauth.is_some(),
        startup_status_events: true,
    }
}
```

If a server registry is empty, `status_list` and `reload` can stay true because the methods are functional and return an empty list. If the runtime cannot be constructed, service health must be disabled/degraded and `P4Services::availability()` must return default for MCP.

- [ ] **Step 2: Add capability tests**

Tests:

- Registry-only service advertises tool/resource/status/reload/progress, but not OAuth.
- Service with secrets + callback advertises OAuth method and OAuth completed event.
- Noop/disabled MCP advertises nothing.
- `capabilities/list` method list equals actual route support.

## Task 9: Wire `dasclaw-app-server` Binary Configuration

**Files:**

- Modify: `crates/dasclaw_app_server/src/main.rs`
- Modify: `crates/dasclaw_app_server/Cargo.toml`

- [ ] **Step 1: Add explicit env configuration**

Implement an env loader with no hidden defaults:

- `DASCLAW_MCP_SERVERS_FILE`: path to MCP server JSON.
- `DASCLAW_MCP_USER_ID`: defaults to `default` only for local single-user mode.
- `DASCLAW_MCP_OAUTH_CALLBACK_HOST`: defaults to `127.0.0.1`.
- `DASCLAW_MCP_OAUTH_CALLBACK_PORT`: optional fixed port.
- `DASCLAW_MCP_SECRETS_DB`: libSQL file path for OAuth tokens.
- `DASCLAW_SECRETS_MASTER_KEY`: required when `DASCLAW_MCP_SECRETS_DB` is set.

If only `DASCLAW_MCP_SERVERS_FILE` is present, construct real non-OAuth MCP service and keep OAuth unavailable. If secrets DB and master key are present, enable OAuth.

- [ ] **Step 2: Add minimal secrets table initialization**

Either reuse an existing migration helper if available, or create a tiny app-server local initializer:

```rust
async fn ensure_secrets_schema(db: &libsql::Database) -> Result<(), String> {
    let conn = db.connect().map_err(|error| error.to_string())?;
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS secrets (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            name TEXT NOT NULL,
            encrypted_value BLOB NOT NULL,
            key_salt BLOB NOT NULL,
            provider TEXT,
            expires_at TEXT,
            last_used_at TEXT,
            usage_count INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE (user_id, name)
        )
        "#,
        (),
    )
    .await
    .map_err(|error| error.to_string())?;
    Ok(())
}
```

Keep this helper app-server local. Do not depend on old desktop-client migrations.

- [ ] **Step 3: Add binary smoke tests where feasible**

At minimum add unit tests for env parsing. If spawning the binary is already supported in tests, add a smoke test with:

```bash
DASCLAW_MCP_SERVERS_FILE=<fixture> \
DASCLAW_MCP_SECRETS_DB=<tmp>/mcp-secrets.db \
DASCLAW_SECRETS_MASTER_KEY=01234567890123456789012345678901 \
cargo run -p dasclaw_app_server -- --capabilities-once
```

Assert the JSON has MCP implemented methods matching configured capabilities.

## Task 10: Documentation And Honest Wording Updates

**Files:**

- Modify: `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`
- Modify: `docs/superpowers/plans/2026-06-19-dasclaw-app-server-p4-mcp-skills-logs-jobs.md`

- [ ] **Step 1: Update status language**

Replace “P4 honest subset complete” wording only after implementation and verification pass:

- Before implementation: “P4 honest subset complete; real MCP effectful paths remain planned.”
- After implementation: “P4 MCP effectful paths complete when app-server is constructed with MCP registry/runtime; OAuth additionally requires secrets + callback config.”

Do not write “完整 MCP/OAuth/product integration complete” because marketplace/account/plugin/future P6 domains are still outside P4.

- [ ] **Step 2: Add operation notes**

Document environment variables and what each one unlocks. State clearly:

- Tool/resource routes need MCP server config.
- OAuth additionally needs secrets DB and master key.
- Progress events require client-supplied `turnId` and `itemId`.

## Task 11: Verification

Run in this order:

```bash
cargo check -p dasclaw_mcp --tests
cargo nextest run -p dasclaw_mcp
cargo check -p dasclaw_app_server_protocol --tests
cargo nextest run -p dasclaw_app_server_protocol
cargo check -p dasclaw_app_server --tests
cargo nextest run -p dasclaw_app_server -E 'test(p4_real_mcp)'
cargo nextest run -p dasclaw_app_server -E 'test(p4_mcp)'
cargo fmt --all
python3.12 scripts/check_no_panics.py --base origin/xClaw
cargo clippy --no-deps -p dasclaw_mcp --all-targets -- -D warnings
cargo clippy --no-deps -p dasclaw_app_server_protocol --all-targets -- -D warnings
cargo clippy --no-deps -p dasclaw_app_server --all-targets -- -D warnings
```

If `cargo nextest run -p dasclaw_mcp` is too broad, run the new MCP resource/client/OAuth tests plus existing client/factory/session tests, and state the gap explicitly.

## Task 12: Review Checklist

- [ ] Real MCP tests use `AppServerMcpService`, not `TestMcpService`.
- [ ] At least one test proves `tools/call` was received by a local MCP server.
- [ ] At least one test proves `resources/read` was received by a local MCP server.
- [ ] OAuth test performs callback + token exchange + secrets write.
- [ ] Capability tests fail if OAuth is advertised without secrets/callback config.
- [ ] Progress event test uses explicit `turnId/itemId`.
- [ ] Missing `turnId/itemId` test proves no fake progress event is emitted.
- [ ] Error redaction tests cover URI query, Authorization header, auth code, access token, refresh token, client secret.
- [ ] `code-simplifier` pass removes duplicated runtime/queue logic if implementation copied the current job runtime pattern.
- [ ] Code review explicitly checks for test-side adaptation or client bug masking.

## Commit Message Requirement

Every implementation commit touching this plan must include a second body paragraph:

```text
已检查 P4 real MCP owner 是否已有，结论：dasclaw_mcp 已有 session/transport/client/OAuth protocol 底座；app-server 只有 registry/status/reload honest subset，真实 tool/resource/OAuth/progress owner 需要本提交接线。
```

If a commit is narrower, adjust the conclusion to that slice while keeping the `已检查 ... 结论：...` line.
