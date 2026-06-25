use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use dasclaw_app_server_protocol::{
    ListMcpServerStatusParams, ListMcpServerStatusResponse, McpAuthStatus, McpResourceContent,
    McpResourceReadParams, McpResourceReadResponse, McpServerOauthLoginCompletedNotification,
    McpServerOauthLoginParams, McpServerOauthLoginResponse, McpServerReloadParams,
    McpServerReloadResponse, McpServerStatus, McpServerStatusDetail, McpServerToolCallParams,
    McpServerToolCallResponse, McpServiceAvailability, McpToolCallProgressNotification, Resource,
    ResourceTemplate, ServiceHealth, ServiceName,
};
use dasclaw_mcp::{
    AuthError, CallToolResult, ContentBlock, McpClient, McpFactoryError, McpProcessManager,
    McpResource as WireResource, McpResourceTemplate as WireResourceTemplate, McpServerConfig,
    McpServersFile, McpSessionManager, McpTool, PkceChallenge, ResourceContent,
    build_authorization_url, canonical_resource_uri, create_client_from_config,
    exchange_code_for_token, store_tokens,
};
use dasclaw_runtime::secrets::{InMemorySecretsStore, SecretsCrypto, SecretsStore};
use dasclaw_tool::ToolError;
use secrecy::SecretString;

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
    oauth_flows: Arc<Mutex<HashMap<String, PendingOAuthFlow>>>,
    secrets: Arc<dyn SecretsStore + Send + Sync>,
}

#[derive(Clone, Default)]
struct McpEventQueue {
    tool_call_progress: Arc<Mutex<Vec<McpToolCallProgressNotification>>>,
    oauth_login_completed: Arc<Mutex<Vec<McpServerOauthLoginCompletedNotification>>>,
}

#[derive(Clone)]
struct PendingOAuthFlow {
    server: McpServerConfig,
    state: String,
    redirect_uri: String,
    pkce: Option<PkceChallenge>,
    expires_at: Instant,
}

const OAUTH_USER_ID: &str = "app-server";
const OAUTH_SECRET_KEY: &str = "dasclaw-app-server-oauth-secret-key";

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
            oauth_flows: Arc::new(Mutex::new(HashMap::new())),
            secrets: default_oauth_secrets(),
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
        let server = self.enabled_server(&params.name)?;
        let oauth = server.oauth.as_ref().ok_or_else(|| {
            AppServerError::capability_unavailable(
                "mcp",
                format!(
                    "MCP server '{}' does not have OAuth configured",
                    params.name
                ),
            )
        })?;
        let authorization_url = oauth.authorization_url.as_deref().ok_or_else(|| {
            AppServerError::capability_unavailable(
                "mcp",
                format!(
                    "MCP OAuth authorization endpoint is not configured for server '{}'",
                    params.name
                ),
            )
        })?;
        if oauth.token_url.is_none() {
            return Err(AppServerError::capability_unavailable(
                "mcp",
                format!(
                    "MCP OAuth token endpoint is not configured for server '{}'",
                    params.name
                ),
            ));
        }

        let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| {
            AppServerError::service_degraded(
                "mcp",
                format!("failed to bind MCP OAuth callback listener: {error}"),
            )
        })?;
        let callback_url = format!(
            "http://{}/oauth/callback",
            listener.local_addr().map_err(|error| {
                AppServerError::service_degraded(
                    "mcp",
                    format!("failed to read MCP OAuth callback listener address: {error}"),
                )
            },)?
        );

        let state = PkceChallenge::generate().verifier;
        let pkce = oauth.use_pkce.then(PkceChallenge::generate);
        let mut scopes = params.scopes.unwrap_or_else(|| oauth.scopes.clone());
        scopes.sort();
        scopes.dedup();
        let mut extra_params = oauth.extra_params.clone();
        extra_params.insert("state".to_string(), state.clone());
        let resource = canonical_resource_uri(&server.url);
        let authorization_url = build_authorization_url(
            authorization_url,
            &oauth.client_id,
            &callback_url,
            &scopes,
            pkce.as_ref(),
            &extra_params,
            Some(&resource),
        );
        let timeout = Duration::from_secs(params.timeout_secs.unwrap_or(300));
        let name = params.name.clone();
        self.oauth_flows
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .insert(
                name.clone(),
                PendingOAuthFlow {
                    server,
                    state: state.clone(),
                    redirect_uri: callback_url.clone(),
                    pkce,
                    expires_at: Instant::now() + timeout,
                },
            );
        self.spawn_oauth_callback_listener(name, listener, Instant::now() + timeout);

        Ok(McpServerOauthLoginResponse {
            authorization_url,
            callback_url,
            state,
        })
    }

    fn drain_tool_call_progress_events(&self) -> Vec<McpToolCallProgressNotification> {
        self.events.drain_tool_call_progress()
    }

    fn drain_oauth_login_completed_events(&self) -> Vec<McpServerOauthLoginCompletedNotification> {
        self.events.drain_oauth_login_completed()
    }

    fn availability(&self) -> McpServiceAvailability {
        let oauth_ready = self.has_oauth_ready_server();
        McpServiceAvailability {
            status_list: true,
            reload: true,
            tool_call: true,
            resource_read: true,
            tool_call_progress_events: true,
            oauth_login: oauth_ready,
            oauth_login_completed_events: oauth_ready,
            startup_status_events: true,
        }
    }
}

impl AppServerMcpService {
    pub fn complete_oauth_login(
        &self,
        name: &str,
        state: &str,
        code: &str,
    ) -> Result<(), AppServerError> {
        let flow = self.take_oauth_flow(name)?;
        if flow.expires_at <= Instant::now() {
            let error = AppServerError::capability_unavailable("mcp", "MCP OAuth login expired");
            self.events
                .push_oauth_login_completed(name, false, Some(error.public_message()));
            return Err(error);
        }
        if flow.state != state {
            let error = AppServerError::invalid_request("mcp", "MCP OAuth state mismatch");
            self.events
                .push_oauth_login_completed(name, false, Some(error.public_message()));
            return Err(error);
        }

        let token_url = flow
            .server
            .oauth
            .as_ref()
            .and_then(|oauth| oauth.token_url.clone())
            .ok_or_else(|| {
                AppServerError::capability_unavailable(
                    "mcp",
                    "MCP OAuth token endpoint is not configured",
                )
            })?;
        let client_id = flow
            .server
            .oauth
            .as_ref()
            .map(|oauth| oauth.client_id.clone())
            .unwrap_or_default();
        let resource = canonical_resource_uri(&flow.server.url);
        let server = flow.server.clone();
        let redirect_uri = flow.redirect_uri.clone();
        let pkce = flow.pkce.clone();
        let code = code.to_string();
        let secrets = Arc::clone(&self.secrets);
        let result = self.runtime()?.block_on("mcp/oauth-callback", async move {
            let token = exchange_code_for_token(
                &token_url,
                &client_id,
                None,
                &code,
                &redirect_uri,
                pkce.as_ref(),
                Some(&resource),
            )
            .await
            .map_err(map_auth_error)?;
            store_tokens(&secrets, OAUTH_USER_ID, &server, &token)
                .await
                .map_err(map_auth_error)?;
            Ok(())
        });

        match result {
            Ok(()) => {
                self.events.push_oauth_login_completed(name, true, None);
                Ok(())
            }
            Err(error) => {
                self.events
                    .push_oauth_login_completed(name, false, Some(error.public_message()));
                Err(error)
            }
        }
    }

    pub fn cancel_oauth_login(&self, name: &str) -> Result<(), AppServerError> {
        self.take_oauth_flow(name)?;
        self.events.push_oauth_login_completed(
            name,
            false,
            Some("MCP OAuth login cancelled or timed out"),
        );
        Ok(())
    }

    pub fn drain_oauth_login_completed_events(
        &self,
    ) -> Vec<McpServerOauthLoginCompletedNotification> {
        self.events.drain_oauth_login_completed()
    }

    fn spawn_oauth_callback_listener(
        &self,
        name: String,
        listener: TcpListener,
        expires_at: Instant,
    ) {
        let service = self.clone();
        thread::spawn(move || {
            let _ = listener.set_nonblocking(true);
            loop {
                if Instant::now() >= expires_at {
                    let _ = service.cancel_oauth_login(&name);
                    return;
                }
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let response = service.handle_oauth_callback_stream(&name, &mut stream);
                        let _ = write_oauth_callback_response(&mut stream, response);
                        return;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(25));
                    }
                    Err(error) => {
                        service.events.push_oauth_login_completed(
                            &name,
                            false,
                            Some(&format!("MCP OAuth callback listener failed: {error}")),
                        );
                        return;
                    }
                }
            }
        });
    }

    fn handle_oauth_callback_stream(
        &self,
        name: &str,
        stream: &mut TcpStream,
    ) -> OAuthCallbackResponse {
        match read_oauth_callback_request(stream).and_then(parse_oauth_callback_request) {
            Ok(OAuthCallbackRequest { code, state }) => {
                match self.complete_oauth_login(name, &state, &code) {
                    Ok(()) => OAuthCallbackResponse::ok("MCP OAuth login completed"),
                    Err(error) => OAuthCallbackResponse::bad_request(error.public_message()),
                }
            }
            Err(message) => {
                self.events
                    .push_oauth_login_completed(name, false, Some(&message));
                OAuthCallbackResponse::bad_request(&message)
            }
        }
    }

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

    fn take_oauth_flow(&self, name: &str) -> Result<PendingOAuthFlow, AppServerError> {
        self.oauth_flows
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .remove(name)
            .ok_or_else(|| AppServerError::invalid_request("mcp", "MCP OAuth login not pending"))
    }

    fn has_oauth_ready_server(&self) -> bool {
        self.registry
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .servers
            .iter()
            .any(|server| {
                server.enabled
                    && server.oauth.as_ref().is_some_and(|oauth| {
                        oauth.authorization_url.is_some() && oauth.token_url.is_some()
                    })
            })
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

    fn push_oauth_login_completed(&self, name: &str, success: bool, error: Option<&str>) {
        self.oauth_login_completed
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .push(McpServerOauthLoginCompletedNotification {
                name: name.to_string(),
                success,
                error: error.map(redact_sensitive_message_str),
            });
    }

    fn drain_oauth_login_completed(&self) -> Vec<McpServerOauthLoginCompletedNotification> {
        self.oauth_login_completed
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .drain(..)
            .collect()
    }
}

fn default_oauth_secrets() -> Arc<dyn SecretsStore + Send + Sync> {
    let crypto = SecretsCrypto::new(SecretString::from(OAUTH_SECRET_KEY.to_string()))
        .expect("app-server OAuth secret key is at least 32 bytes");
    Arc::new(InMemorySecretsStore::new(Arc::new(crypto)))
}

struct OAuthCallbackRequest {
    code: String,
    state: String,
}

struct OAuthCallbackResponse {
    status: &'static str,
    body: &'static str,
}

impl OAuthCallbackResponse {
    fn ok(body: &'static str) -> Self {
        Self {
            status: "200 OK",
            body,
        }
    }

    fn bad_request(_message: &str) -> Self {
        Self {
            status: "400 Bad Request",
            body: "MCP OAuth login failed",
        }
    }
}

fn read_oauth_callback_request(stream: &mut TcpStream) -> Result<String, String> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        let read = stream
            .read(&mut chunk)
            .map_err(|error| format!("failed to read MCP OAuth callback: {error}"))?;
        if read == 0 {
            return Err("MCP OAuth callback closed before headers".to_string());
        }
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if buffer.len() > 8192 {
            return Err("MCP OAuth callback request is too large".to_string());
        }
    }
    String::from_utf8(buffer).map_err(|_| "MCP OAuth callback request is not UTF-8".to_string())
}

fn parse_oauth_callback_request(request: String) -> Result<OAuthCallbackRequest, String> {
    let request_line = request
        .lines()
        .next()
        .ok_or_else(|| "MCP OAuth callback request is empty".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or_default();
    if method != "GET" {
        return Err("MCP OAuth callback must use GET".to_string());
    }
    let query = target
        .strip_prefix("/oauth/callback?")
        .ok_or_else(|| "MCP OAuth callback path is invalid".to_string())?;
    let params = parse_query_params(query);
    let code = params
        .get("code")
        .filter(|value| !value.is_empty())
        .cloned()
        .ok_or_else(|| "MCP OAuth callback is missing code".to_string())?;
    let state = params
        .get("state")
        .filter(|value| !value.is_empty())
        .cloned()
        .ok_or_else(|| "MCP OAuth callback is missing state".to_string())?;
    Ok(OAuthCallbackRequest { code, state })
}

fn parse_query_params(query: &str) -> HashMap<String, String> {
    query
        .split('&')
        .filter_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            Some((key.to_string(), value.to_string()))
        })
        .collect()
}

fn write_oauth_callback_response(
    stream: &mut TcpStream,
    response: OAuthCallbackResponse,
) -> std::io::Result<()> {
    let body = response.body;
    let response = format!(
        "HTTP/1.1 {}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        response.status,
        body.len(),
        body
    );
    stream.write_all(response.as_bytes())
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

fn map_auth_error(error: AuthError) -> AppServerError {
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

fn redact_sensitive_message_str(message: &str) -> String {
    redact_sensitive_message(message.to_string())
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

    pub(crate) struct TestOAuthTokenServer {
        url: String,
        bodies: Arc<Mutex<Vec<String>>>,
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

    impl TestOAuthTokenServer {
        pub(crate) fn start_success() -> Self {
            Self::start(
                200,
                r#"{"access_token":"stored-access-token","token_type":"Bearer","refresh_token":"stored-refresh-token","expires_in":3600}"#,
            )
        }

        pub(crate) fn start_failure_with_secret() -> Self {
            Self::start(
                400,
                r#"{"error":"invalid_grant","error_description":"access_token=leaked"}"#,
            )
        }

        fn start(status: u16, body: &'static str) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind test OAuth token server");
            let url = format!("http://{}", listener.local_addr().unwrap());
            let bodies = Arc::new(Mutex::new(Vec::new()));
            let captured = Arc::clone(&bodies);
            thread::spawn(move || {
                if let Ok((mut stream, _)) = listener.accept() {
                    let body_text = read_http_body(&mut stream);
                    captured
                        .lock()
                        .unwrap_or_else(|poison| poison.into_inner())
                        .push(body_text);
                    write_http_response(&mut stream, status, body);
                }
            });

            Self { url, bodies }
        }

        pub(crate) fn token_url(&self) -> &str {
            &self.url
        }

        pub(crate) fn form_body(&self) -> String {
            self.bodies
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .first()
                .cloned()
                .unwrap_or_default()
        }

        pub(crate) fn request_count(&self) -> usize {
            self.bodies
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .len()
        }
    }

    pub(crate) fn perform_oauth_callback(callback_url: &str, state: &str, code: &str) -> String {
        let without_scheme = callback_url
            .strip_prefix("http://")
            .expect("test callback URL should be HTTP");
        let (host, path) = without_scheme
            .split_once('/')
            .expect("test callback URL should include path");
        let mut stream = std::net::TcpStream::connect(host).expect("connect callback listener");
        let path = format!("/{path}?code={code}&state={state}");
        let request = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
        stream
            .write_all(request.as_bytes())
            .expect("write callback request");
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .expect("read callback response");
        response
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

    fn read_http_body(stream: &mut std::net::TcpStream) -> String {
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
        String::from_utf8(buffer[body_start..body_start + content_length].to_vec())
            .expect("request body is utf8")
    }

    fn write_http_json(stream: &mut std::net::TcpStream, body: serde_json::Value) {
        let body = body.to_string();
        write_http_response(stream, 200, &body);
    }

    fn write_http_response(stream: &mut std::net::TcpStream, status: u16, body: &str) {
        let reason = if status == 200 { "OK" } else { "Bad Request" };
        let response = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
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

    use super::test_support::{TestMcpHttpServer, TestOAuthTokenServer, perform_oauth_callback};
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
    fn app_server_mcp_effectful_methods_fail_safe_for_disabled_server_and_missing_oauth_config() {
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
    fn app_server_mcp_service_oauth_login_starts_authorization_flow() {
        let github = McpServerConfig::new("github", "http://127.0.0.1:9/mcp").with_oauth(
            OAuthConfig::new("client-id").with_endpoints(
                "https://auth.example.test/oauth",
                "https://auth.example.test/token",
            ),
        );
        let service = AppServerMcpService::from_servers(vec![github]);

        let response = service
            .oauth_login(McpServerOauthLoginParams {
                name: "github".to_string(),
                scopes: Some(vec!["repo".to_string()]),
                timeout_secs: Some(30),
            })
            .expect("oauth login should start an authorization flow");

        assert!(
            response
                .authorization_url
                .starts_with("https://auth.example.test/oauth?")
        );
        assert!(response.authorization_url.contains("client_id=client-id"));
        assert!(response.authorization_url.contains("scope=repo"));
        assert!(response.authorization_url.contains("state="));
        assert!(response.callback_url.starts_with("http://127.0.0.1:"));
        assert!(response.callback_url.ends_with("/oauth/callback"));
        assert!(!response.state.is_empty());
    }

    #[test]
    fn app_server_mcp_service_oauth_callback_stores_tokens_and_emits_success() {
        let token_server = TestOAuthTokenServer::start_success();
        let github = McpServerConfig::new("github", "http://127.0.0.1:9/mcp").with_oauth(
            OAuthConfig::new("client-id")
                .with_endpoints("https://auth.example.test/oauth", token_server.token_url()),
        );
        let service = AppServerMcpService::from_servers(vec![github]);

        let response = service
            .oauth_login(McpServerOauthLoginParams {
                name: "github".to_string(),
                scopes: None,
                timeout_secs: Some(30),
            })
            .expect("oauth login should start");

        let callback_response =
            perform_oauth_callback(&response.callback_url, &response.state, "auth-code-secret");
        let completed = service.drain_oauth_login_completed_events();

        assert!(callback_response.starts_with("HTTP/1.1 200 OK"));
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].name, "github");
        assert!(completed[0].success);
        assert_eq!(completed[0].error, None);
        let request = token_server.form_body();
        assert!(request.contains("code=auth-code-secret"));
        assert!(request.contains("client_id=client-id"));
    }

    #[test]
    fn app_server_mcp_service_oauth_callback_rejects_state_mismatch_without_leaking_state_or_code()
    {
        let token_server = TestOAuthTokenServer::start_success();
        let github = McpServerConfig::new("github", "http://127.0.0.1:9/mcp").with_oauth(
            OAuthConfig::new("client-id")
                .with_endpoints("https://auth.example.test/oauth", token_server.token_url()),
        );
        let service = AppServerMcpService::from_servers(vec![github]);
        let response = service
            .oauth_login(McpServerOauthLoginParams {
                name: "github".to_string(),
                scopes: None,
                timeout_secs: Some(30),
            })
            .expect("oauth login should start");

        let error = service
            .complete_oauth_login("github", "wrong-state-secret", "auth-code-secret")
            .expect_err("state mismatch must fail safe");
        let error_text = error.public_message().to_string();
        let completed = service.drain_oauth_login_completed_events();

        assert!(error_text.contains("state mismatch"));
        assert!(!error_text.contains("wrong-state-secret"));
        assert!(!error_text.contains(&response.state));
        assert!(!error_text.contains("auth-code-secret"));
        assert_eq!(completed.len(), 1);
        assert!(!completed[0].success);
        assert!(
            !serde_json::to_string(&completed)
                .unwrap()
                .contains("auth-code-secret"),
            "completion notification must redact authorization codes"
        );
        assert_eq!(token_server.request_count(), 0);
    }

    #[test]
    fn app_server_mcp_service_oauth_cancel_completes_failure_without_leaking_state() {
        let github = McpServerConfig::new("github", "http://127.0.0.1:9/mcp").with_oauth(
            OAuthConfig::new("client-id").with_endpoints(
                "https://auth.example.test/oauth",
                "https://auth.example.test/token",
            ),
        );
        let service = AppServerMcpService::from_servers(vec![github]);
        let response = service
            .oauth_login(McpServerOauthLoginParams {
                name: "github".to_string(),
                scopes: None,
                timeout_secs: Some(30),
            })
            .expect("oauth login should start");

        service
            .cancel_oauth_login("github")
            .expect("cancel should complete pending flow");
        let completed = service.drain_oauth_login_completed_events();

        assert_eq!(completed.len(), 1);
        assert!(!completed[0].success);
        let serialized = serde_json::to_string(&completed).unwrap();
        assert!(!serialized.contains(&response.state));
    }

    #[test]
    fn app_server_mcp_service_oauth_timeout_completes_failure_without_leaking_code() {
        let github = McpServerConfig::new("github", "http://127.0.0.1:9/mcp").with_oauth(
            OAuthConfig::new("client-id").with_endpoints(
                "https://auth.example.test/oauth",
                "https://auth.example.test/token",
            ),
        );
        let service = AppServerMcpService::from_servers(vec![github]);
        let response = service
            .oauth_login(McpServerOauthLoginParams {
                name: "github".to_string(),
                scopes: None,
                timeout_secs: Some(0),
            })
            .expect("oauth login should start");

        let error = service
            .complete_oauth_login("github", &response.state, "auth-code-secret")
            .expect_err("expired OAuth flow must fail safe");
        let completed = service.drain_oauth_login_completed_events();
        let serialized = format!(
            "{} {}",
            error.public_message(),
            serde_json::to_string(&completed).unwrap()
        );

        assert!(serialized.contains("expired"));
        assert!(!serialized.contains("auth-code-secret"));
        assert!(!serialized.contains(&response.state));
    }

    #[test]
    fn app_server_mcp_service_oauth_token_exchange_error_is_redacted() {
        let token_server = TestOAuthTokenServer::start_failure_with_secret();
        let github = McpServerConfig::new("github", "http://127.0.0.1:9/mcp").with_oauth(
            OAuthConfig::new("client-id")
                .with_endpoints("https://auth.example.test/oauth", token_server.token_url()),
        );
        let service = AppServerMcpService::from_servers(vec![github]);
        let response = service
            .oauth_login(McpServerOauthLoginParams {
                name: "github".to_string(),
                scopes: None,
                timeout_secs: Some(30),
            })
            .expect("oauth login should start");

        let error = service
            .complete_oauth_login("github", &response.state, "auth-code-secret")
            .expect_err("token exchange failure should be surfaced safely");
        let completed = service.drain_oauth_login_completed_events();
        let serialized = format!(
            "{} {}",
            error.public_message(),
            serde_json::to_string(&completed).unwrap()
        );

        assert!(!serialized.contains("access_token=leaked"));
        assert!(!serialized.contains("auth-code-secret"));
        assert!(!serialized.contains(&response.state));
        assert!(serialized.contains("sensitive details redacted"));
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
