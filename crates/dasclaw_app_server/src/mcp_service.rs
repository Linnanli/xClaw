use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use dasclaw_app_server_protocol::{
    ListMcpServerStatusParams, ListMcpServerStatusResponse, McpAuthStatus, McpResourceContent,
    McpResourceReadParams, McpResourceReadResponse, McpServerOauthLoginParams,
    McpServerOauthLoginResponse, McpServerReloadParams, McpServerReloadResponse, McpServerStatus,
    McpServerStatusDetail, McpServerToolCallParams, McpServerToolCallResponse,
    McpServiceAvailability, McpToolCallProgressNotification, Resource, ResourceTemplate,
    ServiceHealth, ServiceName,
};
use dasclaw_mcp::{
    CallToolResult, ContentBlock, McpClient, McpFactoryError, McpProcessManager,
    McpResource as WireResource, McpResourceTemplate as WireResourceTemplate, McpServerConfig,
    McpServersFile, McpSessionManager, McpTool, ResourceContent, create_client_from_config,
};
use dasclaw_tool::ToolError;

use crate::AppServerError;
use crate::app_services::McpService;
use crate::blocking_runtime::BlockingTokioRuntime;

#[derive(Clone)]
pub struct AppServerMcpService {
    registry: Arc<Mutex<McpServersFile>>,
    clients: Arc<Mutex<HashMap<String, Arc<McpClient>>>>,
    session_manager: Arc<McpSessionManager>,
    process_manager: Arc<McpProcessManager>,
    runtime: Arc<Mutex<Option<BlockingTokioRuntime>>>,
    events: McpEventQueue,
}

#[derive(Clone, Default)]
struct McpEventQueue {
    tool_call_progress: Arc<Mutex<Vec<McpToolCallProgressNotification>>>,
}

impl Default for AppServerMcpService {
    fn default() -> Self {
        Self::new(McpServersFile::default())
    }
}

impl AppServerMcpService {
    pub fn new(registry: McpServersFile) -> Self {
        Self {
            registry: Arc::new(Mutex::new(registry)),
            clients: Arc::new(Mutex::new(HashMap::new())),
            session_manager: Arc::new(McpSessionManager::new()),
            process_manager: Arc::new(McpProcessManager::new()),
            runtime: Arc::new(Mutex::new(None)),
            events: McpEventQueue::default(),
        }
    }

    pub fn from_servers(servers: Vec<McpServerConfig>) -> Self {
        Self::new(McpServersFile {
            servers,
            schema_version: 1,
        })
    }
}

impl McpService for AppServerMcpService {
    fn health(&self) -> ServiceHealth {
        ServiceHealth::ready(ServiceName::Mcp)
    }

    fn list_status(
        &self,
        params: ListMcpServerStatusParams,
    ) -> Result<ListMcpServerStatusResponse, AppServerError> {
        let mut servers = self
            .registry
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .servers
            .clone();
        servers.sort_by(|left, right| left.name.cmp(&right.name));

        let start = parse_cursor(params.cursor.as_deref())?;
        let limit = parse_limit(params.limit, servers.len())?;
        let end = start.saturating_add(limit).min(servers.len());
        let data = servers
            .get(start..end)
            .unwrap_or(&[])
            .iter()
            .cloned()
            .map(|server| self.status_for_server(server, params.detail))
            .collect::<Result<Vec<_>, _>>()?;
        let next_cursor = (end < servers.len()).then(|| end.to_string());

        Ok(ListMcpServerStatusResponse { data, next_cursor })
    }

    fn reload(
        &self,
        params: McpServerReloadParams,
    ) -> Result<McpServerReloadResponse, AppServerError> {
        let servers = if let Some(name) = params.name {
            vec![self.configured_server(&name)?]
        } else {
            self.registry
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .servers
                .iter()
                .filter(|server| server.enabled)
                .cloned()
                .collect()
        };

        let mut reloaded = Vec::with_capacity(servers.len());
        for server in servers {
            self.clients
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .remove(&server.name);
            let name = server.name.clone();
            let session_manager = Arc::clone(&self.session_manager);
            let process_manager = Arc::clone(&self.process_manager);
            self.runtime()?.block_on("mcp/reload", async move {
                session_manager.terminate(&name).await;
                process_manager
                    .shutdown(&name)
                    .await
                    .map_err(map_tool_error)?;
                Ok(())
            })?;
            if server.enabled {
                self.status_for_server(
                    server.clone(),
                    Some(McpServerStatusDetail::ToolsAndAuthOnly),
                )?;
            }
            reloaded.push(server.name);
        }

        Ok(McpServerReloadResponse { reloaded })
    }

    fn call_tool(
        &self,
        params: McpServerToolCallParams,
    ) -> Result<McpServerToolCallResponse, AppServerError> {
        let server = self.enabled_server(&params.server)?;
        let client = self.client_for(server)?;
        let identity = params.progress_identity();
        if let Some(identity) = identity.as_ref() {
            self.events.push_tool_call_progress(
                identity,
                format!(
                    "calling MCP tool '{}' on server '{}'",
                    params.tool, params.server
                ),
            );
        }

        let tool_name = params.tool.clone();
        let arguments = params.arguments.unwrap_or_else(|| serde_json::json!({}));
        let result = self.runtime()?.block_on("mcp/tool-call", async move {
            client
                .call_tool(&tool_name, arguments)
                .await
                .map_err(map_tool_error)
        });

        match result {
            Ok(result) => {
                if let Some(identity) = identity.as_ref() {
                    self.events
                        .push_tool_call_progress(identity, "MCP tool call completed");
                }
                Ok(map_tool_call_result(result)?)
            }
            Err(error) => {
                if let Some(identity) = identity.as_ref() {
                    self.events
                        .push_tool_call_progress(identity, "MCP tool call failed");
                }
                Err(error)
            }
        }
    }

    fn read_resource(
        &self,
        params: McpResourceReadParams,
    ) -> Result<McpResourceReadResponse, AppServerError> {
        let server = self.enabled_server(&params.server)?;
        let client = self.client_for(server)?;
        let uri = params.uri;
        let result = self.runtime()?.block_on("mcp/resource-read", async move {
            client.read_resource(&uri).await.map_err(map_tool_error)
        })?;

        Ok(McpResourceReadResponse {
            contents: result
                .contents
                .into_iter()
                .map(map_resource_content)
                .collect(),
        })
    }

    fn oauth_login(
        &self,
        params: McpServerOauthLoginParams,
    ) -> Result<McpServerOauthLoginResponse, AppServerError> {
        self.enabled_server(&params.name)?;
        Err(AppServerError::capability_unavailable(
            "mcp",
            format!(
                "MCP OAuth login orchestration is not wired for server '{}'",
                params.name
            ),
        ))
    }

    fn drain_tool_call_progress_events(&self) -> Vec<McpToolCallProgressNotification> {
        self.events.drain_tool_call_progress()
    }

    fn availability(&self) -> McpServiceAvailability {
        McpServiceAvailability {
            status_list: true,
            reload: true,
            tool_call: true,
            resource_read: true,
            tool_call_progress_events: true,
            startup_status_events: true,
            ..McpServiceAvailability::default()
        }
    }
}

impl AppServerMcpService {
    fn runtime(&self) -> Result<BlockingTokioRuntime, AppServerError> {
        let mut runtime = self
            .runtime
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some(runtime) = runtime.as_ref() {
            return Ok(runtime.clone());
        }
        let created = BlockingTokioRuntime::new("dasclaw-app-server-mcp", "mcp")?;
        *runtime = Some(created.clone());
        Ok(created)
    }

    fn configured_server(&self, name: &str) -> Result<McpServerConfig, AppServerError> {
        self.registry
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .get(name)
            .cloned()
            .ok_or_else(|| AppServerError::invalid_request("mcp", "MCP server not found"))
    }

    fn enabled_server(&self, name: &str) -> Result<McpServerConfig, AppServerError> {
        let server = self.configured_server(name)?;
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
        let mut clients = self
            .clients
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some(client) = clients.get(&server.name).cloned() {
            return Ok(client);
        }

        let name = server.name.clone();
        let session_manager = Arc::clone(&self.session_manager);
        let process_manager = Arc::clone(&self.process_manager);
        let client = self.runtime()?.block_on("mcp/create-client", async move {
            create_client_from_config(
                server,
                &session_manager,
                &process_manager,
                None,
                "app-server",
            )
            .await
            .map(Arc::new)
            .map_err(map_factory_error)
        })?;

        clients.insert(name, Arc::clone(&client));
        Ok(client)
    }

    fn status_for_server(
        &self,
        config: McpServerConfig,
        detail: Option<McpServerStatusDetail>,
    ) -> Result<McpServerStatus, AppServerError> {
        if !config.enabled {
            return Ok(status_from_config(&config));
        }

        let auth_status = auth_status(&config);
        let client = self.client_for(config.clone())?;
        self.runtime()?.block_on("mcp/status", async move {
            let tools = client.list_tools().await.map_err(map_tool_error)?;
            let full = !matches!(detail, Some(McpServerStatusDetail::ToolsAndAuthOnly));
            let resources_supported = client.supports_resources().await.map_err(map_tool_error)?;
            let resources = if full && resources_supported {
                client.list_resources().await.map_err(map_tool_error)?
            } else {
                Vec::new()
            };
            let resource_templates = if full && resources_supported {
                client
                    .list_resource_templates()
                    .await
                    .map_err(map_tool_error)?
            } else {
                Vec::new()
            };

            Ok(McpServerStatus {
                name: config.name,
                tools: tools_to_status_value(tools)?,
                resources: resources.into_iter().map(map_resource).collect(),
                resource_templates: resource_templates
                    .into_iter()
                    .map(map_resource_template)
                    .collect(),
                auth_status,
            })
        })
    }
}

impl McpEventQueue {
    fn push_tool_call_progress(
        &self,
        identity: &dasclaw_app_server_protocol::McpToolProgressIdentity,
        message: impl Into<String>,
    ) {
        self.tool_call_progress
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .push(McpToolCallProgressNotification {
                thread_id: identity.thread_id.clone(),
                turn_id: identity.turn_id.clone(),
                item_id: identity.item_id.clone(),
                message: message.into(),
            });
    }

    fn drain_tool_call_progress(&self) -> Vec<McpToolCallProgressNotification> {
        self.tool_call_progress
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .drain(..)
            .collect()
    }
}

fn status_from_config(config: &McpServerConfig) -> McpServerStatus {
    McpServerStatus {
        name: config.name.clone(),
        tools: serde_json::json!({}),
        resources: Vec::new(),
        resource_templates: Vec::new(),
        auth_status: auth_status(config),
    }
}

fn auth_status(config: &McpServerConfig) -> McpAuthStatus {
    if config.oauth.is_some() {
        McpAuthStatus::OAuth
    } else if config
        .headers
        .keys()
        .any(|key| key.eq_ignore_ascii_case("authorization"))
    {
        McpAuthStatus::BearerToken
    } else {
        McpAuthStatus::Unsupported
    }
}

fn tools_to_status_value(tools: Vec<McpTool>) -> Result<serde_json::Value, AppServerError> {
    let mut value = serde_json::Map::new();
    for tool in tools {
        let name = tool.name.clone();
        let tool_value = serde_json::to_value(tool).map_err(|error| {
            AppServerError::service_degraded(
                "mcp",
                format!("failed to serialize MCP tool: {error}"),
            )
        })?;
        value.insert(name, tool_value);
    }
    Ok(serde_json::Value::Object(value))
}

fn map_resource(resource: WireResource) -> Resource {
    Resource {
        annotations: resource.annotations,
        description: resource.description,
        mime_type: resource.mime_type,
        name: resource.name,
        size: resource.size,
        title: None,
        uri: resource.uri,
        icons: Vec::new(),
        meta: resource.meta,
    }
}

fn map_resource_template(template: WireResourceTemplate) -> ResourceTemplate {
    ResourceTemplate {
        annotations: template.annotations,
        uri_template: template.uri_template,
        name: template.name,
        title: None,
        description: template.description,
        mime_type: template.mime_type,
    }
}

fn map_resource_content(content: ResourceContent) -> McpResourceContent {
    McpResourceContent {
        uri: content.uri,
        mime_type: content.mime_type,
        text: content.text,
        blob: content.blob,
        meta: content.meta,
    }
}

fn map_tool_call_result(
    result: CallToolResult,
) -> Result<McpServerToolCallResponse, AppServerError> {
    let content = result
        .content
        .into_iter()
        .map(content_block_to_value)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(McpServerToolCallResponse {
        content,
        structured_content: result.structured_content,
        is_error: Some(result.is_error),
        meta: result.meta,
    })
}

fn content_block_to_value(block: ContentBlock) -> Result<serde_json::Value, AppServerError> {
    serde_json::to_value(block).map_err(|error| {
        AppServerError::service_degraded(
            "mcp",
            format!("failed to serialize MCP tool content: {error}"),
        )
    })
}

fn map_factory_error(error: McpFactoryError) -> AppServerError {
    AppServerError::capability_unavailable("mcp", redact_sensitive_message(error.to_string()))
}

fn map_tool_error(error: ToolError) -> AppServerError {
    AppServerError::service_degraded("mcp", redact_sensitive_message(error.to_string()))
}

fn redact_sensitive_message(message: String) -> String {
    let lower = message.to_ascii_lowercase();
    let sensitive_markers = [
        "authorization",
        "bearer ",
        "access_token",
        "refresh_token",
        "client_secret",
        "api_key",
        "apikey",
        "password",
        "x-api-key",
        "token=",
        "key=",
        "secret",
    ];
    if sensitive_markers
        .iter()
        .any(|marker| lower.contains(marker))
    {
        "MCP request failed; sensitive details redacted".to_string()
    } else {
        message
    }
}

fn parse_cursor(cursor: Option<&str>) -> Result<usize, AppServerError> {
    match cursor {
        Some(cursor) => cursor.parse::<usize>().map_err(|_| {
            AppServerError::invalid_request("mcp", "cursor must be a non-negative integer")
        }),
        None => Ok(0),
    }
}

fn parse_limit(limit: Option<u32>, total_servers: usize) -> Result<usize, AppServerError> {
    match limit {
        Some(0) => Err(AppServerError::invalid_request(
            "mcp",
            "limit must be greater than 0",
        )),
        Some(limit) => Ok(limit as usize),
        None => Ok(total_servers.max(1)),
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::thread;

    pub(crate) struct TestMcpHttpServer {
        pub(crate) url: String,
        requests: Arc<Mutex<Vec<serde_json::Value>>>,
    }

    impl TestMcpHttpServer {
        pub(crate) fn start(expected_requests: usize) -> Self {
            Self::start_with_resources(expected_requests, true)
        }

        pub(crate) fn start_without_resources(expected_requests: usize) -> Self {
            Self::start_with_resources(expected_requests, false)
        }

        fn start_with_resources(expected_requests: usize, supports_resources: bool) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind test MCP server");
            let url = format!("http://{}", listener.local_addr().unwrap());
            let requests = Arc::new(Mutex::new(Vec::new()));
            let captured = Arc::clone(&requests);
            thread::spawn(move || {
                for _ in 0..expected_requests {
                    let (mut stream, _) = listener.accept().expect("accept MCP request");
                    let request = read_http_json(&mut stream);
                    let response = mcp_response_for(&request, supports_resources);
                    captured
                        .lock()
                        .unwrap_or_else(|poison| poison.into_inner())
                        .push(request);
                    write_http_json(&mut stream, response);
                }
            });

            Self { url, requests }
        }

        pub(crate) fn methods(&self) -> Vec<String> {
            self.requests
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .iter()
                .map(|request| request["method"].as_str().unwrap().to_string())
                .collect()
        }

        pub(crate) fn params_for(&self, method: &str) -> serde_json::Value {
            self.requests
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .iter()
                .find(|request| request["method"] == method)
                .and_then(|request| request.get("params"))
                .cloned()
                .unwrap_or(serde_json::Value::Null)
        }
    }

    fn read_http_json(stream: &mut std::net::TcpStream) -> serde_json::Value {
        let mut buffer = Vec::new();
        let mut chunk = [0_u8; 1024];
        let header_end = loop {
            let read = stream.read(&mut chunk).expect("read request");
            assert!(read > 0, "client closed before headers");
            buffer.extend_from_slice(&chunk[..read]);
            if let Some(index) = find_header_end(&buffer) {
                break index;
            }
        };

        let headers = String::from_utf8_lossy(&buffer[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                line.strip_prefix("content-length:")
                    .or_else(|| line.strip_prefix("Content-Length:"))
                    .and_then(|value| value.trim().parse::<usize>().ok())
            })
            .expect("content-length header");
        let body_start = header_end + 4;
        while buffer.len() < body_start + content_length {
            let read = stream.read(&mut chunk).expect("read body");
            assert!(read > 0, "client closed before body");
            buffer.extend_from_slice(&chunk[..read]);
        }
        serde_json::from_slice(&buffer[body_start..body_start + content_length])
            .expect("request body is JSON")
    }

    fn write_http_json(stream: &mut std::net::TcpStream, body: serde_json::Value) {
        let body = body.to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write response");
    }

    fn find_header_end(buffer: &[u8]) -> Option<usize> {
        buffer.windows(4).position(|window| window == b"\r\n\r\n")
    }

    fn mcp_response_for(
        request: &serde_json::Value,
        supports_resources: bool,
    ) -> serde_json::Value {
        let id = request
            .get("id")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        match request["method"].as_str().unwrap() {
            "initialize" => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": if supports_resources {
                        serde_json::json!({"tools": {}, "resources": {"listChanged": false}})
                    } else {
                        serde_json::json!({"tools": {}})
                    },
                    "serverInfo": {"name": "test", "version": "1.0"}
                }
            }),
            "notifications/initialized" => {
                serde_json::json!({"jsonrpc": "2.0", "id": null, "result": {}})
            }
            "tools/list" => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "tools": [{
                        "name": "echo",
                        "description": "Echo input",
                        "inputSchema": {"type": "object"}
                    }]
                }
            }),
            "resources/list" if supports_resources => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "resources": [{
                        "uri": "file:///note.txt",
                        "name": "note",
                        "description": "A note",
                        "mimeType": "text/plain"
                    }]
                }
            }),
            "resources/templates/list" if supports_resources => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "resourceTemplates": [{
                        "uriTemplate": "file:///{name}.txt",
                        "name": "text-file",
                        "mimeType": "text/plain"
                    }]
                }
            }),
            "tools/call" => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [{"type": "text", "text": "ok"}],
                    "structuredContent": {"echoed": true},
                    "isError": false,
                    "_meta": {"trace": "abc"}
                }
            }),
            "resources/read" => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "contents": [{
                        "uri": "file:///note.txt",
                        "mimeType": "text/plain",
                        "text": "note"
                    }]
                }
            }),
            other => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": -32601,
                    "message": format!("unexpected MCP method {other}")
                }
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use dasclaw_app_server_protocol::{
        ListMcpServerStatusParams, McpResourceReadParams, McpServerOauthLoginParams,
        McpServerReloadParams, McpServerStatusDetail, McpServerToolCallParams,
    };
    use dasclaw_mcp::{McpServerConfig, OAuthConfig};

    use super::test_support::TestMcpHttpServer;
    use super::{AppServerMcpService, redact_sensitive_message};
    use crate::app_services::McpService;

    #[test]
    fn app_server_mcp_service_lists_disabled_configured_servers_without_token_leakage() {
        let mut bearer = McpServerConfig::new("bearer", "https://bearer.example/mcp");
        bearer.enabled = false;
        bearer.headers.insert(
            "Authorization".to_string(),
            "Bearer secret-token".to_string(),
        );
        let mut github = McpServerConfig::new("github", "https://github.example/mcp")
            .with_oauth(OAuthConfig::new("client-id"));
        github.enabled = false;
        let service = AppServerMcpService::from_servers(vec![bearer, github]);

        let response = service
            .list_status(ListMcpServerStatusParams::default())
            .unwrap();
        let value = serde_json::to_value(response).unwrap();

        assert_eq!(value["data"][0]["name"], "bearer");
        assert_eq!(value["data"][0]["authStatus"], "bearerToken");
        assert_eq!(value["data"][1]["name"], "github");
        assert_eq!(value["data"][1]["authStatus"], "oAuth");
        assert!(
            !value.to_string().contains("secret-token"),
            "MCP status must not leak authorization headers"
        );
    }

    #[test]
    fn app_server_mcp_service_status_uses_real_mcp_methods() {
        let server = TestMcpHttpServer::start(5);
        let service =
            AppServerMcpService::from_servers(vec![McpServerConfig::new("local", &server.url)]);

        let response = service
            .list_status(ListMcpServerStatusParams::default())
            .unwrap();

        assert_eq!(response.data[0].name, "local");
        assert_eq!(response.data[0].tools["echo"]["name"], "echo");
        assert_eq!(response.data[0].resources[0].uri, "file:///note.txt");
        assert_eq!(
            response.data[0].resource_templates[0].uri_template,
            "file:///{name}.txt"
        );
        assert_eq!(
            server.methods(),
            vec![
                "initialize",
                "notifications/initialized",
                "tools/list",
                "resources/list",
                "resources/templates/list",
            ]
        );
    }

    #[test]
    fn app_server_mcp_service_tool_call_uses_real_mcp_client_and_emits_located_progress() {
        let server = TestMcpHttpServer::start(3);
        let service =
            AppServerMcpService::from_servers(vec![McpServerConfig::new("local", &server.url)]);

        let response = service
            .call_tool(McpServerToolCallParams {
                thread_id: "thread_1".to_string(),
                turn_id: Some("turn_1".to_string()),
                item_id: Some("item_1".to_string()),
                server: "local".to_string(),
                tool: "echo".to_string(),
                arguments: Some(serde_json::json!({"message": "hi"})),
                meta: None,
            })
            .unwrap();
        let events = service.drain_tool_call_progress_events();

        assert_eq!(response.content[0]["text"], "ok");
        assert_eq!(
            response.structured_content,
            Some(serde_json::json!({"echoed": true}))
        );
        assert_eq!(response.meta, Some(serde_json::json!({"trace": "abc"})));
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].thread_id, "thread_1");
        assert_eq!(events[0].turn_id, "turn_1");
        assert_eq!(events[0].item_id, "item_1");
        assert_eq!(
            server.methods(),
            vec!["initialize", "notifications/initialized", "tools/call"]
        );
        assert_eq!(
            server.params_for("tools/call")["arguments"],
            serde_json::json!({"message": "hi"})
        );
    }

    #[test]
    fn app_server_mcp_service_tool_call_without_turn_item_does_not_emit_unlocated_progress() {
        let server = TestMcpHttpServer::start(3);
        let service =
            AppServerMcpService::from_servers(vec![McpServerConfig::new("local", &server.url)]);

        service
            .call_tool(McpServerToolCallParams {
                thread_id: "thread_1".to_string(),
                turn_id: None,
                item_id: None,
                server: "local".to_string(),
                tool: "echo".to_string(),
                arguments: Some(serde_json::json!({})),
                meta: None,
            })
            .unwrap();

        assert!(service.drain_tool_call_progress_events().is_empty());
    }

    #[test]
    fn app_server_mcp_service_resource_read_uses_real_mcp_client() {
        let server = TestMcpHttpServer::start(3);
        let service =
            AppServerMcpService::from_servers(vec![McpServerConfig::new("local", &server.url)]);

        let response = service
            .read_resource(McpResourceReadParams {
                thread_id: Some("thread_1".to_string()),
                server: "local".to_string(),
                uri: "file:///note.txt".to_string(),
            })
            .unwrap();

        assert_eq!(response.contents[0].text.as_deref(), Some("note"));
        assert_eq!(
            server.methods(),
            vec!["initialize", "notifications/initialized", "resources/read"]
        );
        assert_eq!(
            server.params_for("resources/read")["uri"],
            serde_json::json!("file:///note.txt")
        );
    }

    #[test]
    fn app_server_mcp_service_reload_validates_registry_membership() {
        let service = AppServerMcpService::from_servers(vec![disabled_server(
            "github",
            "https://github.example/mcp",
        )]);

        let response = service
            .reload(McpServerReloadParams {
                name: Some("github".to_string()),
            })
            .unwrap();
        let missing = service.reload(McpServerReloadParams {
            name: Some("missing".to_string()),
        });

        assert_eq!(response.reloaded, vec!["github"]);
        assert!(missing.is_err());
    }

    #[test]
    fn app_server_mcp_service_rejects_invalid_pagination() {
        let service = AppServerMcpService::from_servers(vec![disabled_server(
            "github",
            "https://github.example/mcp",
        )]);

        let bad_cursor = service.list_status(ListMcpServerStatusParams {
            cursor: Some("not-a-number".to_string()),
            limit: None,
            detail: None,
        });
        let zero_limit = service.list_status(ListMcpServerStatusParams {
            cursor: None,
            limit: Some(0),
            detail: None,
        });

        assert!(bad_cursor.is_err());
        assert!(zero_limit.is_err());
    }

    #[test]
    fn app_server_mcp_effectful_methods_fail_safe_for_disabled_server_and_unwired_oauth() {
        let service = AppServerMcpService::from_servers(vec![disabled_server(
            "github",
            "https://github.example/mcp",
        )]);

        assert!(
            service
                .call_tool(McpServerToolCallParams {
                    thread_id: "thread_1".to_string(),
                    turn_id: None,
                    item_id: None,
                    server: "github".to_string(),
                    tool: "list_issues".to_string(),
                    arguments: Some(serde_json::json!({})),
                    meta: None,
                })
                .is_err()
        );
        assert!(
            service
                .read_resource(McpResourceReadParams {
                    thread_id: Some("thread_1".to_string()),
                    server: "github".to_string(),
                    uri: "repo://issues".to_string(),
                })
                .is_err()
        );
        let enabled = AppServerMcpService::from_servers(vec![McpServerConfig::new(
            "github",
            "http://127.0.0.1:9/mcp",
        )]);
        assert!(
            enabled
                .oauth_login(McpServerOauthLoginParams {
                    name: "github".to_string(),
                    scopes: None,
                    timeout_secs: None,
                })
                .is_err()
        );
    }

    #[test]
    fn app_server_mcp_status_tools_and_auth_only_skips_resource_methods() {
        let server = TestMcpHttpServer::start(3);
        let service =
            AppServerMcpService::from_servers(vec![McpServerConfig::new("local", &server.url)]);

        let response = service
            .list_status(ListMcpServerStatusParams {
                cursor: None,
                limit: None,
                detail: Some(McpServerStatusDetail::ToolsAndAuthOnly),
            })
            .unwrap();

        assert!(response.data[0].resources.is_empty());
        assert_eq!(
            server.methods(),
            vec!["initialize", "notifications/initialized", "tools/list"]
        );
    }

    #[test]
    fn app_server_mcp_status_skips_resource_methods_when_capability_is_absent() {
        let server = TestMcpHttpServer::start_without_resources(3);
        let service =
            AppServerMcpService::from_servers(vec![McpServerConfig::new("local", &server.url)]);

        let response = service
            .list_status(ListMcpServerStatusParams::default())
            .unwrap();

        assert!(response.data[0].resources.is_empty());
        assert!(response.data[0].resource_templates.is_empty());
        assert_eq!(
            server.methods(),
            vec!["initialize", "notifications/initialized", "tools/list"]
        );
    }

    #[test]
    fn app_server_mcp_error_redaction_covers_common_secret_markers() {
        for message in [
            "request failed with api_key=secret",
            "request failed with password=hunter2",
            "request failed with key=secret",
            "request failed with x-api-key: secret",
        ] {
            assert_eq!(
                redact_sensitive_message(message.to_string()),
                "MCP request failed; sensitive details redacted"
            );
        }
    }

    fn disabled_server(name: &str, url: &str) -> McpServerConfig {
        let mut config = McpServerConfig::new(name, url);
        config.enabled = false;
        config
    }
}
