//! Minimal dasclaw app-server host skeleton.
//!
//! Phase 1 deliberately stops at lifecycle, initialize, health, and
//! capability reporting. It does not wire `dasclaw_runtime::Agent`, DLP,
//! jobs, skills, MCP, or sandbox execution yet.

use std::time::{SystemTime, UNIX_EPOCH};

use dasclaw_app_server_protocol::{
    CapabilitiesChangedEvent, CapabilitiesChangedReason, CapabilitiesListResponse,
    CapabilityMatrix, ClientInfo, ErrorCode, ErrorData, HealthCheckParams, HealthCheckResponse,
    InitializeParams, InitializeResponse, JsonRpcError, JsonRpcRequest, JsonRpcResponse,
    LifecycleChangedEvent, LifecycleReason, LifecycleSnapshot, LifecycleState,
    LifecycleStatusResponse, ProtocolSchemaResponse, ProtocolVersion, ServerInfo,
    ServerNotification, ServiceHealth, ServiceName, ShutdownParams, ShutdownReason,
    ShutdownResponse, ThreadCreateParams, ThreadCreateResponse, ThreadCreatedEvent,
    ThreadListResponse, ThreadReadParams, ThreadReadResponse, ThreadSummary, TurnCancelParams,
    TurnCancelResponse, TurnCancelledEvent, TurnListParams, TurnListResponse, TurnReadParams,
    TurnReadResponse, TurnStartParams, TurnStartResponse, TurnStartedEvent, TurnStatus,
    TurnSummary,
};
use dasclaw_app_server_protocol::{JSON_RPC_VERSION, method};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const SERVER_NAME: &str = "dasclaw_app_server";
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone)]
pub struct AppServer {
    server: ServerInfo,
    lifecycle: LifecycleSnapshot,
    capabilities: CapabilityMatrix,
    client: Option<ClientInfo>,
    notifications: NotificationBus,
    threads: SessionThreadHost,
}

impl Default for AppServer {
    fn default() -> Self {
        Self::new()
    }
}

impl AppServer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            server: ServerInfo {
                name: SERVER_NAME.to_string(),
                version: SERVER_VERSION.to_string(),
                protocol_version: ProtocolVersion::current(),
            },
            lifecycle: lifecycle_snapshot(
                LifecycleState::Starting,
                LifecycleReason::ProcessStarted,
                None,
                Vec::new(),
            ),
            capabilities: CapabilityMatrix::phase_one(),
            client: None,
            notifications: NotificationBus::new(),
            threads: SessionThreadHost::new(),
        }
    }

    pub fn initialize(
        &mut self,
        params: InitializeParams,
    ) -> Result<InitializeResponse, AppServerError> {
        if !self
            .server
            .protocol_version
            .is_compatible_with(params.protocol_version)
        {
            let previous_state = self.lifecycle.state;
            self.lifecycle = lifecycle_snapshot(
                LifecycleState::Failed,
                LifecycleReason::VersionMismatch,
                Some("client protocol major version is incompatible".to_string()),
                Vec::new(),
            );
            self.emit_lifecycle_changed(previous_state);
            return Err(AppServerError::version_mismatch(self.lifecycle.clone()));
        }

        if self.lifecycle.state == LifecycleState::Stopped {
            return Err(AppServerError::server_stopped(self.lifecycle.clone()));
        }

        let unavailable_requested_capabilities =
            self.unavailable_requested_capabilities(&params.requested_capabilities);

        if matches!(
            self.lifecycle.state,
            LifecycleState::Ready | LifecycleState::Running | LifecycleState::Degraded
        ) {
            self.client = Some(params.client);
            return Ok(InitializeResponse {
                server: self.server.clone(),
                lifecycle: self.lifecycle.clone(),
                capabilities: self.capabilities.clone(),
                unavailable_requested_capabilities,
            });
        }

        let previous_state = self.lifecycle.state;
        self.lifecycle = lifecycle_snapshot(
            LifecycleState::Initializing,
            LifecycleReason::InitializeRequested,
            Some("initialize request accepted".to_string()),
            Vec::new(),
        );
        self.emit_lifecycle_changed(previous_state);

        self.client = Some(params.client);
        let previous_state = self.lifecycle.state;
        self.lifecycle = lifecycle_snapshot(
            LifecycleState::Ready,
            LifecycleReason::RuntimeReady,
            Some("minimal app-server skeleton initialized".to_string()),
            Vec::new(),
        );
        self.emit_lifecycle_changed(previous_state);
        self.emit_capabilities_changed(CapabilitiesChangedReason::Initialize);

        Ok(InitializeResponse {
            server: self.server.clone(),
            lifecycle: self.lifecycle.clone(),
            capabilities: self.capabilities.clone(),
            unavailable_requested_capabilities,
        })
    }

    #[must_use]
    pub fn health_check(&self, params: HealthCheckParams) -> HealthCheckResponse {
        let services = if params.include_details {
            self.service_health()
        } else {
            Vec::new()
        };
        let ok = matches!(
            self.lifecycle.state,
            LifecycleState::Ready | LifecycleState::Running | LifecycleState::Degraded
        );

        HealthCheckResponse {
            ok,
            lifecycle: self.lifecycle.clone(),
            services,
        }
    }

    #[must_use]
    pub fn capabilities(&self) -> CapabilitiesListResponse {
        CapabilitiesListResponse {
            capabilities: self.capabilities.clone(),
        }
    }

    #[must_use]
    pub fn protocol_schema(&self) -> ProtocolSchemaResponse {
        ProtocolSchemaResponse::phase_one(self.capabilities.clone())
    }

    pub fn thread_create(
        &mut self,
        params: ThreadCreateParams,
    ) -> Result<ThreadCreateResponse, AppServerError> {
        if self.lifecycle.state == LifecycleState::Stopped {
            return Err(AppServerError::server_stopped(self.lifecycle.clone()));
        }
        self.require_initialized("session")?;

        let thread_id = self.threads.create(params);
        self.notifications.emit_thread_created(ThreadCreatedEvent {
            thread_id: thread_id.clone(),
        });

        Ok(ThreadCreateResponse {
            thread_id,
            lifecycle: self.lifecycle.clone(),
        })
    }

    pub fn thread_list(&self) -> Result<ThreadListResponse, AppServerError> {
        self.require_initialized("session")?;
        Ok(ThreadListResponse {
            threads: self.threads.list(),
        })
    }

    pub fn thread_read(
        &self,
        params: ThreadReadParams,
    ) -> Result<ThreadReadResponse, AppServerError> {
        self.require_initialized("session")?;
        Ok(ThreadReadResponse {
            thread: self.thread_summary_or_error(&params.thread_id)?,
        })
    }

    pub fn turn_start(
        &mut self,
        params: TurnStartParams,
    ) -> Result<TurnStartResponse, AppServerError> {
        self.require_initialized("session")?;
        self.require_thread_exists(&params.thread_id)?;
        let turn_id = self.threads.start_turn(&params.thread_id);
        self.notifications.emit_turn_started(TurnStartedEvent {
            thread_id: params.thread_id,
            turn_id: turn_id.clone(),
            status: TurnStatus::Pending,
        });

        Ok(TurnStartResponse {
            turn_id,
            status: TurnStatus::Pending,
            lifecycle: self.lifecycle.clone(),
        })
    }

    pub fn turn_cancel(
        &mut self,
        params: TurnCancelParams,
    ) -> Result<TurnCancelResponse, AppServerError> {
        self.require_initialized("session")?;
        self.require_thread_exists(&params.thread_id)?;
        let changed = self
            .threads
            .cancel_turn(&params.thread_id, &params.turn_id)
            .ok_or_else(|| {
                AppServerError::invalid_request(
                    "session",
                    format!("unknown turn id: {}", params.turn_id),
                )
            })?;
        if changed {
            self.notifications.emit_turn_cancelled(TurnCancelledEvent {
                thread_id: params.thread_id,
                turn_id: params.turn_id,
                status: TurnStatus::Cancelled,
            });
        }

        Ok(TurnCancelResponse {
            accepted: true,
            status: TurnStatus::Cancelled,
            lifecycle: self.lifecycle.clone(),
        })
    }

    pub fn turn_list(&self, params: TurnListParams) -> Result<TurnListResponse, AppServerError> {
        self.require_initialized("session")?;
        self.require_thread_exists(&params.thread_id)?;
        Ok(TurnListResponse {
            turns: self.threads.list_turns(&params.thread_id),
        })
    }

    pub fn turn_read(&self, params: TurnReadParams) -> Result<TurnReadResponse, AppServerError> {
        self.require_initialized("session")?;
        self.require_thread_exists(&params.thread_id)?;
        Ok(TurnReadResponse {
            turn: self.turn_summary_or_error(&params.thread_id, &params.turn_id)?,
        })
    }

    #[must_use]
    pub fn lifecycle_status(&self) -> LifecycleStatusResponse {
        LifecycleStatusResponse {
            lifecycle: self.lifecycle.clone(),
        }
    }

    #[must_use]
    pub fn server_info(&self) -> ServerInfo {
        self.server.clone()
    }

    #[must_use]
    pub fn shutdown(&mut self, params: ShutdownParams) -> ShutdownResponse {
        if self.lifecycle.state == LifecycleState::Stopped {
            return ShutdownResponse {
                accepted: true,
                lifecycle: self.lifecycle.clone(),
            };
        }

        let reason = params.reason.unwrap_or(ShutdownReason::ClientExit);
        let previous_state = self.lifecycle.state;
        self.lifecycle = lifecycle_snapshot(
            LifecycleState::Stopped,
            LifecycleReason::ShutdownRequested,
            Some(format!("shutdown accepted: {reason:?}")),
            Vec::new(),
        );
        self.emit_lifecycle_changed(previous_state);

        ShutdownResponse {
            accepted: true,
            lifecycle: self.lifecycle.clone(),
        }
    }

    pub fn drain_notifications(&mut self) -> Vec<ServerNotification> {
        self.notifications.drain()
    }

    pub fn drain_json_rpc_notifications(&mut self) -> Vec<String> {
        self.notifications.drain_json_rpc()
    }

    #[must_use]
    pub fn is_stopped(&self) -> bool {
        self.lifecycle.state == LifecycleState::Stopped
    }

    pub fn handle_json_rpc(&mut self, input: &str) -> Option<String> {
        let response = match serde_json::from_str::<Value>(input) {
            Ok(Value::Array(_)) => Some(invalid_request_response(
                None,
                "JSON-RPC batch requests are not supported in Phase 1".to_string(),
            )),
            Ok(value) => {
                let id = value.get("id").cloned();
                let is_notification = id.is_none();
                match serde_json::from_value::<JsonRpcRequest>(value) {
                    Ok(request) => self.route_json_rpc(request),
                    Err(error) => response_for_request(
                        invalid_request_response(id, error.to_string()),
                        is_notification,
                    ),
                }
            }
            Err(error) => Some(parse_error_response(error.to_string())),
        };

        response.map(|response| serialize_response(&response))
    }

    fn route_json_rpc(&mut self, request: JsonRpcRequest) -> Option<JsonRpcResponse> {
        let is_notification = request.id.is_none();
        if request.jsonrpc != JSON_RPC_VERSION {
            return response_for_request(
                invalid_request_response(request.id, "jsonrpc must be \"2.0\"".to_string()),
                is_notification,
            );
        }

        let response = match request.method.as_str() {
            method::INITIALIZE => {
                route_with_params(request.id, request.params, |params| self.initialize(params))
            }
            method::PROTOCOL_SCHEMA => {
                route_with_no_params(request.id, request.params, || self.protocol_schema())
            }
            method::HEALTH_CHECK => {
                route_with_optional_params(request.id, request.params, |params| {
                    Ok(self.health_check(params))
                })
            }
            method::CAPABILITIES_LIST => {
                route_with_no_params(request.id, request.params, || self.capabilities())
            }
            method::LIFECYCLE_STATUS => {
                route_with_no_params(request.id, request.params, || self.lifecycle_status())
            }
            method::SHUTDOWN => route_with_optional_params(request.id, request.params, |params| {
                Ok(self.shutdown(params))
            }),
            method::THREAD_CREATE => {
                route_with_params(request.id, request.params, |params: ThreadCreateParams| {
                    self.thread_create(params)
                })
            }
            method::THREAD_LIST => {
                route_with_no_params_result(request.id, request.params, || self.thread_list())
            }
            method::THREAD_READ => {
                route_with_params(request.id, request.params, |params: ThreadReadParams| {
                    self.thread_read(params)
                })
            }
            method::TURN_START => {
                route_with_params(request.id, request.params, |params: TurnStartParams| {
                    self.turn_start(params)
                })
            }
            method::TURN_CANCEL => {
                route_with_params(request.id, request.params, |params: TurnCancelParams| {
                    self.turn_cancel(params)
                })
            }
            method::TURN_LIST => {
                route_with_params(request.id, request.params, |params: TurnListParams| {
                    self.turn_list(params)
                })
            }
            method::TURN_READ => {
                route_with_params(request.id, request.params, |params: TurnReadParams| {
                    self.turn_read(params)
                })
            }
            _ => method_not_found_response(request.id, request.method),
        };

        response_for_request(response, is_notification)
    }

    fn service_health(&self) -> Vec<ServiceHealth> {
        vec![
            ServiceHealth::ready(ServiceName::Protocol),
            ServiceHealth::ready(ServiceName::Lifecycle),
            ServiceHealth::degraded(
                ServiceName::Session,
                "in-memory session host is available; runtime turn execution is not wired",
            ),
            ServiceHealth::disabled(ServiceName::Logs, "log source is not wired in Phase 1"),
            ServiceHealth::disabled(
                ServiceName::Runtime,
                "runtime bridge is not wired in Phase 1",
            ),
            ServiceHealth::unavailable_fail_safe(
                ServiceName::DlpPolicy,
                "DLP/policy service is declared but not migrated in Phase 1",
            ),
            ServiceHealth::disabled(
                ServiceName::ModelProvider,
                "model provider service is not wired in Phase 1",
            ),
            ServiceHealth::disabled(ServiceName::Tools, "tool registry is not wired in Phase 1"),
            ServiceHealth::disabled(
                ServiceName::Sandbox,
                "sandbox adapter is not wired in Phase 1",
            ),
            ServiceHealth::disabled(ServiceName::Jobs, "job host is not wired in Phase 1"),
            ServiceHealth::disabled(
                ServiceName::Skills,
                "skills registry is not wired in Phase 1",
            ),
            ServiceHealth::disabled(ServiceName::Mcp, "MCP registry is not wired in Phase 1"),
        ]
    }

    fn emit_lifecycle_changed(&mut self, previous_state: LifecycleState) {
        let event = LifecycleChangedEvent {
            lifecycle: self.lifecycle.clone(),
            previous_state: Some(previous_state),
        };
        self.notifications.emit_lifecycle_changed(event);
    }

    fn emit_capabilities_changed(&mut self, reason: CapabilitiesChangedReason) {
        let event = CapabilitiesChangedEvent {
            capabilities: self.capabilities.clone(),
            reason,
        };
        self.notifications.emit_capabilities_changed(event);
    }

    fn unavailable_requested_capabilities(&self, requested: &[String]) -> Vec<String> {
        let mut unavailable = Vec::new();
        for capability in requested {
            if !self.is_capability_implemented(capability) && !unavailable.contains(capability) {
                unavailable.push(capability.clone());
            }
        }
        unavailable
    }

    fn is_capability_implemented(&self, capability: &str) -> bool {
        let status = match capability {
            "protocol" => self.capabilities.protocol.status,
            "lifecycle" => self.capabilities.lifecycle.status,
            "health" => self.capabilities.health.status,
            "session" => self.capabilities.session.status,
            "approval" => self.capabilities.approval.status,
            "dlp_policy" => self.capabilities.dlp_policy.status,
            "model_provider" => self.capabilities.model_provider.status,
            "tools" => self.capabilities.tools.status,
            "jobs" => self.capabilities.jobs.status,
            "skills" => self.capabilities.skills.status,
            "mcp" => self.capabilities.mcp.status,
            "sandbox" => self.capabilities.sandbox.status,
            "logs" => self.capabilities.logs.status,
            _ => return false,
        };
        status == dasclaw_app_server_protocol::CapabilityStatus::Implemented
    }

    fn require_initialized(&self, capability: impl Into<String>) -> Result<(), AppServerError> {
        if self.lifecycle.state == LifecycleState::Stopped {
            return Err(AppServerError::server_stopped(self.lifecycle.clone()));
        }

        if matches!(
            self.lifecycle.state,
            LifecycleState::Ready | LifecycleState::Running | LifecycleState::Degraded
        ) {
            Ok(())
        } else {
            Err(AppServerError::not_initialized(
                capability,
                self.lifecycle.clone(),
            ))
        }
    }

    fn require_thread_exists(&self, thread_id: &str) -> Result<(), AppServerError> {
        if self.threads.contains(thread_id) {
            Ok(())
        } else {
            Err(AppServerError::invalid_request(
                "session",
                format!("unknown thread id: {thread_id}"),
            ))
        }
    }

    fn thread_summary_or_error(&self, thread_id: &str) -> Result<ThreadSummary, AppServerError> {
        self.threads.summary(thread_id).ok_or_else(|| {
            AppServerError::invalid_request("session", format!("unknown thread id: {thread_id}"))
        })
    }

    fn turn_summary_or_error(
        &self,
        thread_id: &str,
        turn_id: &str,
    ) -> Result<TurnSummary, AppServerError> {
        self.threads
            .turn_summary(thread_id, turn_id)
            .ok_or_else(|| {
                AppServerError::invalid_request("session", format!("unknown turn id: {turn_id}"))
            })
    }
}

#[derive(Debug, Clone, Default)]
pub struct SessionThreadHost {
    next_thread_id: u64,
    next_turn_id: u64,
    threads: Vec<ThreadRecord>,
    turns: Vec<TurnRecord>,
}

impl SessionThreadHost {
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_thread_id: 1,
            next_turn_id: 1,
            threads: Vec::new(),
            turns: Vec::new(),
        }
    }

    fn create(&mut self, params: ThreadCreateParams) -> String {
        let thread_id = format!("thread_{}", self.next_thread_id);
        self.next_thread_id += 1;
        self.threads.push(ThreadRecord {
            thread_id: thread_id.clone(),
            title: params.title,
            workspace_root: params.workspace_root,
        });
        thread_id
    }

    fn list(&self) -> Vec<ThreadSummary> {
        self.threads.iter().map(ThreadRecord::to_summary).collect()
    }

    fn summary(&self, thread_id: &str) -> Option<ThreadSummary> {
        self.threads
            .iter()
            .find(|thread| thread.thread_id == thread_id)
            .map(ThreadRecord::to_summary)
    }

    fn start_turn(&mut self, thread_id: &str) -> String {
        let turn_id = format!("turn_{}", self.next_turn_id);
        self.next_turn_id += 1;
        self.turns.push(TurnRecord {
            thread_id: thread_id.to_string(),
            turn_id: turn_id.clone(),
            status: TurnStatus::Pending,
        });
        turn_id
    }

    fn cancel_turn(&mut self, thread_id: &str, turn_id: &str) -> Option<bool> {
        let turn = self
            .turns
            .iter_mut()
            .find(|turn| turn.thread_id == thread_id && turn.turn_id == turn_id)?;
        if turn.status == TurnStatus::Cancelled {
            return Some(false);
        }

        turn.status = TurnStatus::Cancelled;
        Some(true)
    }

    fn list_turns(&self, thread_id: &str) -> Vec<TurnSummary> {
        self.turns
            .iter()
            .filter(|turn| turn.thread_id == thread_id)
            .map(TurnRecord::to_summary)
            .collect()
    }

    fn turn_summary(&self, thread_id: &str, turn_id: &str) -> Option<TurnSummary> {
        self.turns
            .iter()
            .find(|turn| turn.thread_id == thread_id && turn.turn_id == turn_id)
            .map(TurnRecord::to_summary)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.threads.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.threads.is_empty()
    }

    #[must_use]
    pub fn contains(&self, thread_id: &str) -> bool {
        self.threads
            .iter()
            .any(|thread| thread.thread_id == thread_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadRecord {
    pub thread_id: String,
    pub title: Option<String>,
    pub workspace_root: Option<String>,
}

impl ThreadRecord {
    fn to_summary(&self) -> ThreadSummary {
        ThreadSummary {
            thread_id: self.thread_id.clone(),
            title: self.title.clone(),
            workspace_root: self.workspace_root.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnRecord {
    pub thread_id: String,
    pub turn_id: String,
    pub status: TurnStatus,
}

impl TurnRecord {
    fn to_summary(&self) -> TurnSummary {
        TurnSummary {
            thread_id: self.thread_id.clone(),
            turn_id: self.turn_id.clone(),
            status: self.status,
        }
    }
}

fn response_for_request(
    response: JsonRpcResponse,
    is_notification: bool,
) -> Option<JsonRpcResponse> {
    if is_notification {
        None
    } else {
        Some(response)
    }
}

#[derive(Debug, Clone, Default)]
pub struct NotificationBus {
    pending: Vec<ServerNotification>,
}

impl NotificationBus {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn emit_lifecycle_changed(&mut self, event: LifecycleChangedEvent) {
        self.push(ServerNotification::lifecycle_changed(event));
    }

    pub fn emit_capabilities_changed(&mut self, event: CapabilitiesChangedEvent) {
        self.push(ServerNotification::capabilities_changed(event));
    }

    pub fn emit_thread_created(&mut self, event: ThreadCreatedEvent) {
        self.push(ServerNotification::thread_created(event));
    }

    pub fn emit_turn_started(&mut self, event: TurnStartedEvent) {
        self.push(ServerNotification::turn_started(event));
    }

    pub fn emit_turn_cancelled(&mut self, event: TurnCancelledEvent) {
        self.push(ServerNotification::turn_cancelled(event));
    }

    pub fn drain(&mut self) -> Vec<ServerNotification> {
        std::mem::take(&mut self.pending)
    }

    pub fn drain_json_rpc(&mut self) -> Vec<String> {
        self.drain()
            .into_iter()
            .filter_map(|notification| serde_json::to_string(&notification).ok())
            .collect()
    }

    fn push(&mut self, notification: Result<ServerNotification, serde_json::Error>) {
        if let Ok(notification) = notification {
            self.pending.push(notification);
        }
    }
}

fn json_rpc_ok(id: Option<Value>, result: impl Serialize) -> JsonRpcResponse {
    JsonRpcResponse::ok(id.clone(), result)
        .unwrap_or_else(|error| internal_error_response(id, error.to_string()))
}

fn parse_error_response(message: String) -> JsonRpcResponse {
    json_rpc_error_response(
        None,
        -32700,
        "Parse error",
        message,
        ErrorCode::InvalidParams,
    )
}

fn invalid_request_response(id: Option<Value>, message: String) -> JsonRpcResponse {
    json_rpc_error_response(
        id,
        -32600,
        "Invalid Request",
        message,
        ErrorCode::InvalidParams,
    )
}

fn method_not_found_response(id: Option<Value>, method: String) -> JsonRpcResponse {
    json_rpc_error_response(
        id,
        -32601,
        "Method not found",
        format!("unknown method: {method}"),
        ErrorCode::UnknownMethod,
    )
}

fn invalid_params_response(id: Option<Value>, message: String) -> JsonRpcResponse {
    json_rpc_error_response(
        id,
        -32602,
        "Invalid params",
        message,
        ErrorCode::InvalidParams,
    )
}

fn app_error_response(id: Option<Value>, error: AppServerError) -> JsonRpcResponse {
    match error {
        AppServerError::Protocol { data } => JsonRpcResponse::error(
            id,
            JsonRpcError::new(app_error_code(data.code), data.message.clone(), Some(data)),
        ),
    }
}

fn internal_error_response(id: Option<Value>, message: String) -> JsonRpcResponse {
    json_rpc_error_response(
        id,
        -32603,
        "Internal error",
        message,
        ErrorCode::InternalError,
    )
}

fn json_rpc_error_response(
    id: Option<Value>,
    json_rpc_code: i64,
    json_rpc_message: &str,
    message: String,
    code: ErrorCode,
) -> JsonRpcResponse {
    JsonRpcResponse::error(
        id,
        JsonRpcError::new(
            json_rpc_code,
            json_rpc_message,
            Some(ErrorData {
                code,
                message,
                lifecycle: None,
                capability: None,
                retryable: false,
            }),
        ),
    )
}

fn route_with_params<T, R, F>(
    id: Option<Value>,
    params: Option<Value>,
    handler: F,
) -> JsonRpcResponse
where
    T: for<'de> Deserialize<'de>,
    R: Serialize,
    F: FnOnce(T) -> Result<R, AppServerError>,
{
    let Some(params) = params else {
        return invalid_params_response(id, "missing params".to_string());
    };

    match serde_json::from_value::<T>(params) {
        Ok(params) => match handler(params) {
            Ok(result) => json_rpc_ok(id, result),
            Err(error) => app_error_response(id, error),
        },
        Err(error) => invalid_params_response(id, error.to_string()),
    }
}

fn route_with_optional_params<T, R, F>(
    id: Option<Value>,
    params: Option<Value>,
    handler: F,
) -> JsonRpcResponse
where
    T: for<'de> Deserialize<'de> + Default,
    R: Serialize,
    F: FnOnce(T) -> Result<R, AppServerError>,
{
    let params = params.unwrap_or(Value::Object(Default::default()));
    route_with_params(id, Some(params), handler)
}

fn route_with_no_params<R, F>(
    id: Option<Value>,
    params: Option<Value>,
    handler: F,
) -> JsonRpcResponse
where
    R: Serialize,
    F: FnOnce() -> R,
{
    if has_non_empty_params(params) {
        return invalid_params_response(id, "method does not accept params".to_string());
    }

    json_rpc_ok(id, handler())
}

fn route_with_no_params_result<R, F>(
    id: Option<Value>,
    params: Option<Value>,
    handler: F,
) -> JsonRpcResponse
where
    R: Serialize,
    F: FnOnce() -> Result<R, AppServerError>,
{
    if has_non_empty_params(params) {
        return invalid_params_response(id, "method does not accept params".to_string());
    }

    match handler() {
        Ok(result) => json_rpc_ok(id, result),
        Err(error) => app_error_response(id, error),
    }
}

fn has_non_empty_params(params: Option<Value>) -> bool {
    match params {
        None | Some(Value::Null) => false,
        Some(Value::Object(map)) => !map.is_empty(),
        Some(Value::Array(values)) => !values.is_empty(),
        Some(_) => true,
    }
}

fn app_error_code(code: ErrorCode) -> i64 {
    match code {
        ErrorCode::VersionMismatch => -32001,
        ErrorCode::CapabilityUnavailable => -32002,
        ErrorCode::NotInitialized => -32003,
        ErrorCode::OperationInProgress => -32004,
        ErrorCode::ServiceDegraded => -32005,
        ErrorCode::UnknownMethod => -32601,
        ErrorCode::InvalidParams => -32602,
        ErrorCode::InternalError => -32603,
    }
}

fn serialize_response(response: &JsonRpcResponse) -> String {
    serde_json::to_string(response).unwrap_or_else(|error| {
        format!(
            r#"{{"jsonrpc":"2.0","error":{{"code":-32603,"message":"Internal error","data":{{"code":"INTERNAL_ERROR","message":"failed to serialize response: {error}","retryable":false}}}}}}"#
        )
    })
}
#[derive(Debug, Clone, thiserror::Error)]
pub enum AppServerError {
    #[error("{data:?}")]
    Protocol { data: ErrorData },
}

impl AppServerError {
    fn version_mismatch(lifecycle: LifecycleSnapshot) -> Self {
        Self::Protocol {
            data: ErrorData {
                code: ErrorCode::VersionMismatch,
                message: "client protocol major version is incompatible".to_string(),
                lifecycle: Some(lifecycle),
                capability: Some("protocol".to_string()),
                retryable: false,
            },
        }
    }

    fn server_stopped(lifecycle: LifecycleSnapshot) -> Self {
        Self::Protocol {
            data: ErrorData {
                code: ErrorCode::ServiceDegraded,
                message:
                    "app-server is stopped and cannot be initialized again; restart the process"
                        .to_string(),
                lifecycle: Some(lifecycle),
                capability: Some("lifecycle".to_string()),
                retryable: false,
            },
        }
    }

    fn not_initialized(capability: impl Into<String>, lifecycle: LifecycleSnapshot) -> Self {
        Self::Protocol {
            data: ErrorData {
                code: ErrorCode::NotInitialized,
                message: "initialize must complete before this method can be called".to_string(),
                lifecycle: Some(lifecycle),
                capability: Some(capability.into()),
                retryable: true,
            },
        }
    }

    fn invalid_request(capability: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Protocol {
            data: ErrorData {
                code: ErrorCode::InvalidParams,
                message: message.into(),
                lifecycle: None,
                capability: Some(capability.into()),
                retryable: false,
            },
        }
    }
}

pub fn supported_methods() -> &'static [&'static str] {
    &[
        method::INITIALIZE,
        method::PROTOCOL_SCHEMA,
        method::HEALTH_CHECK,
        method::CAPABILITIES_LIST,
        method::LIFECYCLE_STATUS,
        method::SHUTDOWN,
        method::THREAD_CREATE,
        method::THREAD_LIST,
        method::THREAD_READ,
        method::TURN_START,
        method::TURN_CANCEL,
        method::TURN_LIST,
        method::TURN_READ,
    ]
}

fn lifecycle_snapshot(
    state: LifecycleState,
    reason: LifecycleReason,
    message: Option<String>,
    degraded_services: Vec<ServiceHealth>,
) -> LifecycleSnapshot {
    LifecycleSnapshot {
        state,
        reason,
        message,
        since: unix_timestamp_string(),
        degraded_services,
    }
}

fn unix_timestamp_string() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    seconds.to_string()
}

#[cfg(test)]
mod tests {
    use dasclaw_app_server_protocol::{
        CapabilityStatus, ServiceStatus, TransportKind, WorkspaceInfo, WorkspaceTrust,
    };

    use super::*;

    #[test]
    fn initialize_accepts_compatible_client_and_returns_minimal_capabilities() {
        let mut server = AppServer::new();
        let response = server
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "open-cowork".to_string(),
                    version: "0.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: Some(WorkspaceInfo {
                    root: Some("/tmp/workspace".to_string()),
                    trust: WorkspaceTrust::Unknown,
                }),
                requested_capabilities: Vec::new(),
            })
            .expect("compatible v0 client should initialize");

        assert_eq!(response.lifecycle.state, LifecycleState::Ready);
        assert_eq!(
            response.capabilities.protocol.status,
            CapabilityStatus::Implemented
        );
        assert_eq!(
            response.capabilities.dlp_policy.status,
            CapabilityStatus::Declared
        );
    }

    #[test]
    fn initialize_rejects_incompatible_major_version() {
        let mut server = AppServer::new();
        let error = server
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "future-client".to_string(),
                    version: "1.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion {
                    major: 1,
                    minor: 0,
                    patch: 0,
                },
                workspace: None,
                requested_capabilities: Vec::new(),
            })
            .expect_err("incompatible major version should fail");

        match error {
            AppServerError::Protocol { data } => {
                assert_eq!(data.code, ErrorCode::VersionMismatch);
                assert!(!data.retryable);
            }
        }
    }

    #[test]
    fn initialize_reports_requested_capabilities_that_are_not_implemented() {
        let mut server = AppServer::new();
        let response = server
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "open-cowork".to_string(),
                    version: "0.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: None,
                requested_capabilities: vec![
                    "protocol".to_string(),
                    "dlp_policy".to_string(),
                    "unknown_future".to_string(),
                    "dlp_policy".to_string(),
                ],
            })
            .expect("initialize should tolerate unavailable requested capabilities");

        assert_eq!(
            response.unavailable_requested_capabilities,
            vec!["dlp_policy".to_string(), "unknown_future".to_string()]
        );
    }

    #[test]
    fn initialize_is_idempotent_after_ready_without_reemitting_notifications() {
        let mut server = AppServer::new();
        let params = InitializeParams {
            client: ClientInfo {
                name: "open-cowork".to_string(),
                version: "0.0.0".to_string(),
                transport: TransportKind::Stdio,
            },
            protocol_version: ProtocolVersion::current(),
            workspace: None,
            requested_capabilities: Vec::new(),
        };

        let first = server
            .initialize(params.clone())
            .expect("first initialize should succeed");
        assert_eq!(first.lifecycle.state, LifecycleState::Ready);
        assert_eq!(server.drain_notifications().len(), 3);

        let second = server
            .initialize(params)
            .expect("second initialize should be idempotent");
        assert_eq!(second.lifecycle.state, LifecycleState::Ready);
        assert!(server.drain_notifications().is_empty());
    }

    #[test]
    fn initialize_after_shutdown_is_rejected_without_leaving_stopped() {
        let mut server = AppServer::new();
        let _ = server.shutdown(ShutdownParams {
            reason: Some(ShutdownReason::Test),
            timeout_ms: None,
        });

        let error = server
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "open-cowork".to_string(),
                    version: "0.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: None,
                requested_capabilities: Vec::new(),
            })
            .expect_err("stopped process should require a restart");

        match error {
            AppServerError::Protocol { data } => {
                assert_eq!(data.code, ErrorCode::ServiceDegraded);
                assert_eq!(
                    data.lifecycle.expect("lifecycle should be attached").state,
                    LifecycleState::Stopped
                );
                assert!(!data.retryable);
            }
        }
    }

    #[test]
    fn health_reports_disabled_future_services_without_claiming_handlers() {
        let mut server = AppServer::new();
        server
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "open-cowork".to_string(),
                    version: "0.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: None,
                requested_capabilities: Vec::new(),
            })
            .expect("server should initialize");

        let health = server.health_check(HealthCheckParams {
            include_details: true,
        });

        assert!(health.ok);
        assert!(
            health
                .services
                .iter()
                .any(|service| service.service == ServiceName::DlpPolicy && service.fail_safe)
        );
        assert!(
            health
                .services
                .iter()
                .any(|service| service.service == ServiceName::Runtime)
        );
        assert!(health.services.iter().any(|service| {
            service.service == ServiceName::Session && service.status == ServiceStatus::Degraded
        }));
    }

    #[test]
    fn health_can_omit_service_details_for_lightweight_probes() {
        let server = AppServer::new();
        let health = server.health_check(HealthCheckParams {
            include_details: false,
        });

        assert!(!health.ok);
        assert_eq!(health.lifecycle.state, LifecycleState::Starting);
        assert!(health.services.is_empty());
    }

    #[test]
    fn shutdown_moves_lifecycle_to_stopped() {
        let mut server = AppServer::new();
        let response = server.shutdown(ShutdownParams {
            reason: Some(ShutdownReason::Test),
            timeout_ms: None,
        });

        assert!(response.accepted);
        assert_eq!(response.lifecycle.state, LifecycleState::Stopped);
    }

    #[test]
    fn shutdown_is_idempotent_after_stopped_without_reemitting_notifications() {
        let mut server = AppServer::new();
        let first = server.shutdown(ShutdownParams {
            reason: Some(ShutdownReason::Test),
            timeout_ms: None,
        });
        assert_eq!(first.lifecycle.state, LifecycleState::Stopped);
        assert_eq!(server.drain_notifications().len(), 1);

        let second = server.shutdown(ShutdownParams {
            reason: Some(ShutdownReason::Test),
            timeout_ms: None,
        });
        assert_eq!(second.lifecycle.state, LifecycleState::Stopped);
        assert!(server.drain_notifications().is_empty());
    }

    #[test]
    fn server_info_exposes_protocol_version() {
        let server = AppServer::new();
        let info = server.server_info();

        assert_eq!(info.name, SERVER_NAME);
        assert_eq!(info.protocol_version, ProtocolVersion::current());
    }

    #[test]
    fn protocol_schema_exposes_current_methods_events_and_capabilities() {
        let server = AppServer::new();
        let schema = server.protocol_schema();
        let method_names = schema
            .methods
            .iter()
            .map(|method| method.method.as_str())
            .collect::<Vec<_>>();

        assert_eq!(schema.protocol_version, ProtocolVersion::current());
        assert!(method_names.contains(&method::INITIALIZE));
        assert!(method_names.contains(&method::PROTOCOL_SCHEMA));
        assert_eq!(
            schema.capabilities.protocol.status,
            CapabilityStatus::Implemented
        );
        assert_eq!(schema.capabilities.logs.status, CapabilityStatus::Declared);
    }

    #[test]
    fn protocol_schema_methods_are_all_routable() {
        let server = AppServer::new();
        let schema = server.protocol_schema();

        for method in schema.methods {
            assert!(
                supported_methods().contains(&method.method.as_str()),
                "protocol/schema advertised an unroutable method: {}",
                method.method
            );
        }
    }

    #[test]
    fn initialize_queues_lifecycle_and_capability_notifications() {
        let mut server = AppServer::new();
        server
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "open-cowork".to_string(),
                    version: "0.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: None,
                requested_capabilities: Vec::new(),
            })
            .expect("server should initialize");

        let notifications = server.drain_notifications();
        assert_eq!(notifications.len(), 3);
        assert_eq!(notifications[0].method, "lifecycle/changed");
        assert_eq!(notifications[1].method, "lifecycle/changed");
        assert_eq!(notifications[2].method, "capabilities/changed");
    }

    #[test]
    fn draining_notifications_is_one_shot() {
        let mut server = AppServer::new();
        let _response = server.shutdown(ShutdownParams {
            reason: Some(ShutdownReason::Test),
            timeout_ms: None,
        });

        assert_eq!(server.drain_notifications().len(), 1);
        assert!(server.drain_notifications().is_empty());
    }

    #[test]
    fn notification_bus_drains_json_rpc_notifications() {
        let mut bus = NotificationBus::new();
        bus.emit_lifecycle_changed(LifecycleChangedEvent {
            lifecycle: lifecycle_snapshot(
                LifecycleState::Ready,
                LifecycleReason::RuntimeReady,
                None,
                Vec::new(),
            ),
            previous_state: Some(LifecycleState::Initializing),
        });

        let lines = bus.drain_json_rpc();
        assert_eq!(lines.len(), 1);

        let value: Value = serde_json::from_str(&lines[0]).expect("notification should be JSON");
        assert_eq!(value["jsonrpc"], "2.0");
        assert_eq!(value["method"], "lifecycle/changed");
        assert!(bus.drain_json_rpc().is_empty());
    }

    #[test]
    fn implemented_capability_methods_are_routable() {
        let matrix = CapabilityMatrix::phase_one();
        let implemented_methods = [
            matrix.protocol.methods,
            matrix.lifecycle.methods,
            matrix.health.methods,
            matrix.logs.methods,
        ]
        .concat();

        for method in implemented_methods {
            assert!(
                supported_methods().contains(&method.as_str()),
                "implemented method must be routable: {method}"
            );
        }
    }

    #[test]
    fn json_rpc_initialize_routes_to_server_method() {
        let mut server = AppServer::new();
        let request = format!(
            r#"{{
                "jsonrpc":"2.0",
                "id":"req_1",
                "method":"{}",
                "params":{{
                    "client":{{"name":"open-cowork","version":"0.0.0","transport":"stdio"}},
                    "protocolVersion":{{"major":0,"minor":1,"patch":0}},
                    "requestedCapabilities":[]
                }}
            }}"#,
            method::INITIALIZE
        );
        let response = server
            .handle_json_rpc(&request)
            .expect("request should return a response");
        let value: Value = serde_json::from_str(&response).expect("response should be JSON");

        assert_eq!(value["jsonrpc"], "2.0");
        assert_eq!(value["id"], "req_1");
        assert_eq!(value["result"]["lifecycle"]["state"], "ready");
        assert_eq!(
            value["result"]["capabilities"]["protocol"]["status"],
            "implemented"
        );
    }

    #[test]
    fn json_rpc_health_reports_fail_safe_dlp_policy() {
        let mut server = AppServer::new();
        let request = format!(
            r#"{{"jsonrpc":"2.0","id":2,"method":"{}","params":{{"includeDetails":true}}}}"#,
            method::HEALTH_CHECK
        );
        let response = server
            .handle_json_rpc(&request)
            .expect("request should return a response");
        let health: Value = serde_json::from_str(&response).expect("response should be JSON");
        let services = health["result"]["services"]
            .as_array()
            .expect("health services should be an array");

        assert!(services.iter().any(|service| {
            service["service"] == "dlp_policy"
                && service["status"] == serde_json::json!(ServiceStatus::Unavailable)
                && service["failSafe"] == true
        }));
    }

    #[test]
    fn json_rpc_protocol_schema_routes_without_initialize() {
        let mut server = AppServer::new();
        let request = format!(
            r#"{{"jsonrpc":"2.0","id":"schema","method":"{}"}}"#,
            method::PROTOCOL_SCHEMA
        );
        let response = server
            .handle_json_rpc(&request)
            .expect("request should return a response");
        let value: Value = serde_json::from_str(&response).expect("response should be JSON");

        assert_eq!(value["id"], "schema");
        assert_eq!(value["result"]["protocolVersion"]["major"], 0);
        assert_eq!(
            value["result"]["capabilities"]["protocol"]["status"],
            "implemented"
        );
        assert!(
            value["result"]["methods"]
                .as_array()
                .expect("methods should be an array")
                .iter()
                .any(|method| method["method"] == "protocol/schema")
        );
    }

    #[test]
    fn json_rpc_thread_create_requires_initialize() {
        let mut server = AppServer::new();
        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"thread","method":"thread/create","params":{"title":"Draft"}}"#,
            )
            .expect("thread/create should return a structured response");
        let value: Value = serde_json::from_str(&response).expect("response should be JSON");

        assert_eq!(value["id"], "thread");
        assert_eq!(value["error"]["code"], -32003);
        assert_eq!(value["error"]["data"]["code"], "NOT_INITIALIZED");
        assert_eq!(value["error"]["data"]["capability"], "session");
        assert_eq!(value["error"]["data"]["retryable"], true);
    }

    #[test]
    fn json_rpc_thread_create_returns_thread_id_after_initialize() {
        let mut server = AppServer::new();
        server
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "open-cowork".to_string(),
                    version: "0.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: None,
                requested_capabilities: Vec::new(),
            })
            .expect("initialize should succeed");
        let _ = server.drain_notifications();

        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"thread","method":"thread/create","params":{"title":"Draft"}}"#,
            )
            .expect("thread/create should return a structured response");
        let value: Value = serde_json::from_str(&response).expect("response should be JSON");

        assert_eq!(value["id"], "thread");
        assert_eq!(value["result"]["threadId"], "thread_1");
        assert_eq!(value["result"]["lifecycle"]["state"], "ready");
        let notifications = server.drain_notifications();
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].method, "thread/created");
    }

    #[test]
    fn json_rpc_thread_list_requires_initialize() {
        let mut server = AppServer::new();
        let response = server
            .handle_json_rpc(r#"{"jsonrpc":"2.0","id":"threads","method":"thread/list"}"#)
            .expect("thread/list should return a structured response");
        let value: Value = serde_json::from_str(&response).expect("response should be JSON");

        assert_eq!(value["error"]["data"]["code"], "NOT_INITIALIZED");
        assert_eq!(value["error"]["data"]["retryable"], true);
    }

    #[test]
    fn json_rpc_thread_list_and_read_return_in_memory_threads() {
        let mut server = initialized_server();
        let first = server
            .thread_create(ThreadCreateParams {
                title: Some("First".to_string()),
                workspace_root: Some("/tmp/workspace".to_string()),
            })
            .expect("first thread should be created");
        let _second = server
            .thread_create(ThreadCreateParams {
                title: Some("Second".to_string()),
                workspace_root: None,
            })
            .expect("second thread should be created");

        let list = server
            .handle_json_rpc(r#"{"jsonrpc":"2.0","id":"threads","method":"thread/list"}"#)
            .expect("thread/list should return a response");
        let read = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"thread","method":"thread/read","params":{{"threadId":"{}"}}}}"#,
                first.thread_id
            ))
            .expect("thread/read should return a response");
        let list_value: Value = serde_json::from_str(&list).expect("list response JSON");
        let read_value: Value = serde_json::from_str(&read).expect("read response JSON");

        assert_eq!(
            list_value["result"]["threads"]
                .as_array()
                .expect("threads should be an array")
                .len(),
            2
        );
        assert_eq!(read_value["result"]["thread"]["threadId"], "thread_1");
        assert_eq!(read_value["result"]["thread"]["title"], "First");
        assert_eq!(
            read_value["result"]["thread"]["workspaceRoot"],
            "/tmp/workspace"
        );
    }

    #[test]
    fn json_rpc_thread_read_rejects_unknown_thread() {
        let mut server = initialized_server();
        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"thread","method":"thread/read","params":{"threadId":"missing"}}"#,
            )
            .expect("thread/read should return a structured response");
        let value: Value = serde_json::from_str(&response).expect("response should be JSON");

        assert_eq!(value["error"]["code"], -32602);
        assert_eq!(value["error"]["data"]["code"], "INVALID_PARAMS");
        assert_eq!(value["error"]["data"]["capability"], "session");
    }

    #[test]
    fn json_rpc_turn_start_requires_initialize() {
        let mut server = AppServer::new();
        let start = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"turn-start","method":"turn/start","params":{"threadId":"thread_1","prompt":"hello"}}"#,
            )
            .expect("turn/start skeleton should return a structured response");
        let start_value: Value = serde_json::from_str(&start).expect("start response JSON");

        assert_eq!(start_value["error"]["data"]["code"], "NOT_INITIALIZED");
        assert_eq!(start_value["error"]["data"]["retryable"], true);
    }

    #[test]
    fn json_rpc_turn_start_rejects_unknown_thread_after_initialize() {
        let mut server = initialized_server();
        let start = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"turn-start","method":"turn/start","params":{"threadId":"missing","prompt":"hello"}}"#,
            )
            .expect("turn/start skeleton should return a structured response");
        let start_value: Value = serde_json::from_str(&start).expect("start response JSON");

        assert_eq!(start_value["error"]["code"], -32602);
        assert_eq!(start_value["error"]["data"]["code"], "INVALID_PARAMS");
        assert_eq!(start_value["error"]["data"]["capability"], "session");
    }

    #[test]
    fn json_rpc_turn_start_and_cancel_manage_pending_in_memory_turns() {
        let mut server = initialized_server();
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created");
        let _ = server.drain_notifications();

        let start = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"turn-start","method":"turn/start","params":{{"threadId":"{}","prompt":"hello"}}}}"#,
                thread.thread_id
            ))
            .expect("turn/start skeleton should return a structured response");
        let start_value: Value = serde_json::from_str(&start).expect("start response JSON");

        assert_eq!(start_value["result"]["turnId"], "turn_1");
        assert_eq!(start_value["result"]["status"], "pending");
        let started = server.drain_notifications();
        assert_eq!(started.len(), 1);
        assert_eq!(started[0].method, "turn/started");

        let cancel = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"turn-cancel","method":"turn/cancel","params":{{"threadId":"{}","turnId":"turn_1"}}}}"#,
                thread.thread_id
            ))
            .expect("turn/cancel skeleton should return a structured response");
        let cancel_value: Value = serde_json::from_str(&cancel).expect("cancel response JSON");

        assert_eq!(cancel_value["result"]["accepted"], true);
        assert_eq!(cancel_value["result"]["status"], "cancelled");
        let cancelled = server.drain_notifications();
        assert_eq!(cancelled.len(), 1);
        assert_eq!(cancelled[0].method, "turn/cancelled");
    }

    #[test]
    fn json_rpc_turn_list_and_read_return_in_memory_turn_status() {
        let mut server = initialized_server();
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created");
        let start = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                prompt: "hello".to_string(),
            })
            .expect("turn should be started");
        let _ = server.turn_cancel(TurnCancelParams {
            thread_id: thread.thread_id.clone(),
            turn_id: start.turn_id.clone(),
        });

        let list = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"turns","method":"turn/list","params":{{"threadId":"{}"}}}}"#,
                thread.thread_id
            ))
            .expect("turn/list should return a response");
        let read = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"turn","method":"turn/read","params":{{"threadId":"{}","turnId":"{}"}}}}"#,
                thread.thread_id, start.turn_id
            ))
            .expect("turn/read should return a response");
        let list_value: Value = serde_json::from_str(&list).expect("list response JSON");
        let read_value: Value = serde_json::from_str(&read).expect("read response JSON");

        assert_eq!(
            list_value["result"]["turns"]
                .as_array()
                .expect("turns should be an array")
                .len(),
            1
        );
        assert_eq!(read_value["result"]["turn"]["turnId"], "turn_1");
        assert_eq!(read_value["result"]["turn"]["status"], "cancelled");
    }

    #[test]
    fn json_rpc_turn_read_rejects_unknown_turn() {
        let mut server = initialized_server();
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created");
        let response = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"turn","method":"turn/read","params":{{"threadId":"{}","turnId":"missing"}}}}"#,
                thread.thread_id
            ))
            .expect("turn/read should return a structured response");
        let value: Value = serde_json::from_str(&response).expect("read response JSON");

        assert_eq!(value["error"]["code"], -32602);
        assert_eq!(value["error"]["data"]["code"], "INVALID_PARAMS");
        assert_eq!(value["error"]["data"]["capability"], "session");
    }

    #[test]
    fn session_methods_after_shutdown_return_stopped_lifecycle_error() {
        let mut server = initialized_server();
        let _ = server.shutdown(ShutdownParams {
            reason: Some(ShutdownReason::Test),
            timeout_ms: None,
        });

        let response = server
            .handle_json_rpc(r#"{"jsonrpc":"2.0","id":"threads","method":"thread/list"}"#)
            .expect("thread/list after shutdown should return a structured response");
        let value: Value = serde_json::from_str(&response).expect("response should be JSON");

        assert_eq!(value["error"]["code"], -32005);
        assert_eq!(value["error"]["data"]["code"], "SERVICE_DEGRADED");
        assert_eq!(value["error"]["data"]["lifecycle"]["state"], "stopped");
        assert_eq!(value["error"]["data"]["retryable"], false);
    }

    #[test]
    fn json_rpc_turn_cancel_rejects_unknown_turn_after_thread_exists() {
        let mut server = initialized_server();
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created");
        let response = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"turn-cancel","method":"turn/cancel","params":{{"threadId":"{}","turnId":"missing"}}}}"#,
                thread.thread_id
            ))
            .expect("turn/cancel should return a structured response");
        let value: Value = serde_json::from_str(&response).expect("cancel response JSON");

        assert_eq!(value["error"]["code"], -32602);
        assert_eq!(value["error"]["data"]["code"], "INVALID_PARAMS");
        assert_eq!(value["error"]["data"]["capability"], "session");
    }

    fn initialized_server() -> AppServer {
        let mut server = AppServer::new();
        server
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "open-cowork".to_string(),
                    version: "0.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: None,
                requested_capabilities: Vec::new(),
            })
            .expect("initialize should succeed");
        let _ = server.drain_notifications();
        server
    }

    #[test]
    fn json_rpc_rejects_unknown_method() {
        let mut server = AppServer::new();
        let response = server
            .handle_json_rpc(r#"{"jsonrpc":"2.0","id":"bad","method":"thread/send"}"#)
            .expect("request should return a response");
        let value: Value = serde_json::from_str(&response).expect("response should be JSON");

        assert_eq!(value["error"]["code"], -32601);
        assert_eq!(value["error"]["data"]["code"], "UNKNOWN_METHOD");
    }

    #[test]
    fn json_rpc_notification_without_id_does_not_return_response() {
        let mut server = AppServer::new();
        let response = server.handle_json_rpc(&format!(
            r#"{{"jsonrpc":"2.0","method":"{}","params":{{"reason":"test"}}}}"#,
            method::SHUTDOWN
        ));

        assert!(response.is_none());
        assert!(server.is_stopped());
        assert_eq!(server.drain_notifications().len(), 1);
    }

    #[test]
    fn invalid_json_rpc_notification_without_id_does_not_return_response() {
        let mut server = AppServer::new();
        let response = server.handle_json_rpc(r#"{"jsonrpc":"1.0","method":"health/check"}"#);

        assert!(response.is_none());
    }

    #[test]
    fn invalid_json_rpc_version_request_returns_invalid_request_error() {
        let mut server = AppServer::new();
        let response = server
            .handle_json_rpc(r#"{"jsonrpc":"1.0","id":"bad-version","method":"health/check"}"#)
            .expect("request with id should return an error response");
        let value: Value = serde_json::from_str(&response).expect("response should be JSON");

        assert_eq!(value["id"], "bad-version");
        assert_eq!(value["error"]["code"], -32600);
        assert_eq!(value["error"]["data"]["code"], "INVALID_PARAMS");
        assert!(value["result"].is_null());
    }

    #[test]
    fn json_rpc_parse_error_returns_error_without_id() {
        let mut server = AppServer::new();
        let response = server
            .handle_json_rpc("{not-json")
            .expect("parse errors should return an error response");
        let value: Value = serde_json::from_str(&response).expect("response should be JSON");

        assert_eq!(value["id"], "missing-method");
        assert_eq!(value["error"]["code"], -32700);
        assert_eq!(value["error"]["data"]["code"], "INVALID_PARAMS");
    }

    #[test]
    fn json_rpc_invalid_request_shape_returns_invalid_request_error() {
        let mut server = AppServer::new();
        let response = server
            .handle_json_rpc(r#"{"jsonrpc":"2.0","id":"missing-method"}"#)
            .expect("invalid request shapes should return an error response");
        let value: Value = serde_json::from_str(&response).expect("response should be JSON");

        assert!(value["id"].is_null());
        assert_eq!(value["error"]["code"], -32600);
        assert_eq!(value["error"]["data"]["code"], "INVALID_PARAMS");
    }

    #[test]
    fn json_rpc_invalid_notification_shape_without_id_does_not_return_response() {
        let mut server = AppServer::new();
        let response = server.handle_json_rpc(r#"{"jsonrpc":"2.0"}"#);

        assert!(response.is_none());
    }

    #[test]
    fn json_rpc_batch_request_is_rejected_explicitly() {
        let mut server = AppServer::new();
        let response = server
            .handle_json_rpc(r#"[{"jsonrpc":"2.0","id":"one","method":"health/check"}]"#)
            .expect("batch requests should return an error response");
        let value: Value = serde_json::from_str(&response).expect("response should be JSON");

        assert!(value["id"].is_null());
        assert_eq!(value["error"]["code"], -32600);
        assert_eq!(value["error"]["data"]["code"], "INVALID_PARAMS");
    }

    #[test]
    fn no_params_method_rejects_non_empty_params() {
        let mut server = AppServer::new();
        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"bad-params","method":"lifecycle/status","params":{"unexpected":true}}"#,
            )
            .expect("request with id should return an error response");
        let value: Value = serde_json::from_str(&response).expect("response should be JSON");

        assert_eq!(value["id"], "bad-params");
        assert_eq!(value["error"]["code"], -32602);
        assert_eq!(value["error"]["data"]["code"], "INVALID_PARAMS");
    }

    #[test]
    fn unknown_method_notification_without_id_does_not_return_response() {
        let mut server = AppServer::new();
        let response = server.handle_json_rpc(r#"{"jsonrpc":"2.0","method":"thread/send"}"#);

        assert!(response.is_none());
    }
}
